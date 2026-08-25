//! [`FitsFile`]: create, open, close.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::{FitsError, Result};
use crate::hdu::Hdu;
use crate::header::Header;
use crate::io::{self, DiskDriver, Driver, MemoryDriver};
use crate::status::{
    BAD_FILEPTR, BAD_HDU_NUM, END_OF_FILE, FILE_NOT_CREATED, FILE_NOT_OPENED, READONLY_FILE,
};
use crate::types::{HduType, READONLY, READWRITE};

/// Open mode matching CFITSIO `READONLY` / `READWRITE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    /// `READONLY` (0).
    ReadOnly,
    /// `READWRITE` (1).
    ReadWrite,
}

impl AccessMode {
    /// CFITSIO integer code.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::ReadOnly => READONLY,
            Self::ReadWrite => READWRITE,
        }
    }
}

pub(crate) enum Backend {
    Disk(DiskDriver),
    Memory(MemoryDriver),
}

impl Driver for Backend {
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> Result<usize> {
        match self {
            Self::Disk(d) => d.read_at(pos, buf),
            Self::Memory(d) => d.read_at(pos, buf),
        }
    }
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> Result<()> {
        match self {
            Self::Disk(d) => d.write_at(pos, buf),
            Self::Memory(d) => d.write_at(pos, buf),
        }
    }
    fn len(&self) -> Result<u64> {
        match self {
            Self::Disk(d) => d.len(),
            Self::Memory(d) => d.len(),
        }
    }
    fn truncate(&mut self, len: u64) -> Result<()> {
        match self {
            Self::Disk(d) => d.truncate(len),
            Self::Memory(d) => d.truncate(len),
        }
    }
    fn flush(&mut self) -> Result<()> {
        match self {
            Self::Disk(d) => d.flush(),
            Self::Memory(d) => d.flush(),
        }
    }
}

pub(crate) struct Inner {
    pub(crate) io: Backend,
    pub(crate) writable: bool,
    path: Option<PathBuf>,
    pub(crate) hdus: Vec<Hdu>,
    pub(crate) current: usize,
    pub(crate) dirty: bool,
    /// If set, the in-memory image is gzipped to this path on close.
    gzip_out: Option<PathBuf>,
    /// If set, the in-memory image is written to stdout on [`FitsFile::close`].
    emit_stdout: bool,
    /// 1-based next header record to read (`ffghps` position).
    pub(crate) nextkey: usize,
    /// Requested tiled-image compression for the next `create_image`.
    pub(crate) compression: crate::compress::CompressionRequest,
}

impl Inner {
    pub(crate) fn last_end(&self) -> Result<u64> {
        let last = self
            .hdus
            .last()
            .ok_or_else(|| FitsError::new(BAD_HDU_NUM))?;
        last.end()
    }

    pub(crate) fn bump_offsets_from(&mut self, from_idx: usize, delta: i64) {
        for hdu in self.hdus.iter_mut().skip(from_idx) {
            if delta >= 0 {
                hdu.header_start = hdu.header_start.saturating_add(delta as u64);
                hdu.data_start = hdu.data_start.saturating_add(delta as u64);
            } else {
                let d = (-delta) as u64;
                hdu.header_start = hdu.header_start.saturating_sub(d);
                hdu.data_start = hdu.data_start.saturating_sub(d);
            }
        }
    }
}

/// An open FITS file.
pub struct FitsFile {
    inner: Option<Inner>,
}

impl std::fmt::Debug for FitsFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            None => f.debug_struct("FitsFile").field("closed", &true).finish(),
            Some(inner) => f
                .debug_struct("FitsFile")
                .field("path", &inner.path)
                .field("writable", &inner.writable)
                .field("hdu_count", &inner.hdus.len())
                .finish(),
        }
    }
}

impl FitsFile {
    /// Create a new disk file containing an empty primary HDU.
    ///
    /// Fails if `path` already exists. A leading `!` on a UTF-8 path
    /// clobbers an existing file (CFITSIO convention).
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let (path, clobber) = split_clobber(path);
        if is_stdout_name(&path) {
            let mut f = Self::create_memory()?;
            if let Some(inner) = f.inner.as_mut() {
                inner.emit_stdout = true;
            }
            return Ok(f);
        }
        if clobber && path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| FitsError::with_message(FILE_NOT_CREATED, e.to_string()))?;
        }
        if ends_with_gz(&path) {
            if path.exists() {
                return Err(FitsError::new(FILE_NOT_CREATED));
            }
            let mut io = Backend::Memory(MemoryDriver::new());
            let hdu = Hdu::empty_primary()?;
            let bytes = hdu.header.to_record_bytes();
            io::write_all(&mut io, &bytes)?;
            return Ok(Self {
                inner: Some(Inner {
                    io,
                    writable: true,
                    path: Some(path.clone()),
                    hdus: vec![hdu],
                    current: 0,
                    dirty: false,
                    gzip_out: Some(path),
                    emit_stdout: false,
                    nextkey: 1,
                    compression: crate::compress::CompressionRequest::default(),
                }),
            });
        }
        let mut io = Backend::Disk(DiskDriver::create(&path)?);
        let hdu = Hdu::empty_primary()?;
        let bytes = hdu.header.to_record_bytes();
        io::write_all(&mut io, &bytes)?;
        Ok(Self {
            inner: Some(Inner {
                io,
                writable: true,
                path: Some(path),
                hdus: vec![hdu],
                current: 0,
                dirty: false,
                gzip_out: None,
                emit_stdout: false,
                nextkey: 1,
                compression: crate::compress::CompressionRequest::default(),
            }),
        })
    }

    /// Create an in-memory FITS file with an empty primary HDU.
    pub fn create_memory() -> Result<Self> {
        let mut io = Backend::Memory(MemoryDriver::new());
        let hdu = Hdu::empty_primary()?;
        let bytes = hdu.header.to_record_bytes();
        io::write_all(&mut io, &bytes)?;
        Ok(Self {
            inner: Some(Inner {
                io,
                writable: true,
                path: None,
                hdus: vec![hdu],
                current: 0,
                dirty: false,
                gzip_out: None,
                emit_stdout: false,
                nextkey: 1,
                compression: crate::compress::CompressionRequest::default(),
            }),
        })
    }

    /// Open an in-memory copy of `bytes` (stdin analogue).
    ///
    /// gzip-compressed buffers are decompressed when the `gzip` feature is on.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let data = maybe_gunzip(bytes)?;
        if data.is_empty() {
            return Err(FitsError::with_message(FILE_NOT_OPENED, "file is empty"));
        }
        let hdus = parse_hdus(&data)?;
        Ok(Self {
            inner: Some(Inner {
                io: Backend::Memory(MemoryDriver::from_vec(data)),
                writable: true,
                path: None,
                hdus,
                current: 0,
                dirty: false,
                gzip_out: None,
                emit_stdout: false,
                nextkey: 1,
                compression: crate::compress::CompressionRequest::default(),
            }),
        })
    }

    /// Read a FITS file (optionally gzip-compressed) from any [`Read`] stream.
    pub fn open_reader<R: Read>(mut reader: R) -> Result<Self> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map_err(io::map_read_err)?;
        Self::from_bytes(&bytes)
    }

    /// Open an existing disk file.
    ///
    /// gzip files (`*.gz`, or gzip magic) are decompressed into memory.
    /// If `path` is missing, `path` with a `.gz` suffix is tried (CFITSIO).
    /// `"-"` / `"stdin"` reads the process stdin.
    pub fn open(path: impl AsRef<Path>, mode: AccessMode) -> Result<Self> {
        let path = path.as_ref();
        if is_stdin_name(path) {
            return Self::open_reader(std::io::stdin());
        }
        let writable = mode == AccessMode::ReadWrite;
        let (actual, raw) = read_open_bytes(path)?;
        if is_gzip_magic(&raw) {
            let data = maybe_gunzip(&raw)?;
            if data.is_empty() {
                return Err(FitsError::with_message(FILE_NOT_OPENED, "file is empty"));
            }
            let hdus = parse_hdus(&data)?;
            let gzip_out = if writable { Some(actual.clone()) } else { None };
            return Ok(Self {
                inner: Some(Inner {
                    io: Backend::Memory(MemoryDriver::from_vec(data)),
                    writable,
                    path: Some(actual),
                    hdus,
                    current: 0,
                    dirty: false,
                    gzip_out,
                    emit_stdout: false,
                    nextkey: 1,
                    compression: crate::compress::CompressionRequest::default(),
                }),
            });
        }
        if raw.is_empty() {
            return Err(FitsError::with_message(FILE_NOT_OPENED, "file is empty"));
        }
        let hdus = parse_hdus(&raw)?;
        let io = Backend::Disk(DiskDriver::open(&actual, writable)?);
        Ok(Self {
            inner: Some(Inner {
                io,
                writable,
                path: Some(actual),
                hdus,
                current: 0,
                dirty: false,
                gzip_out: None,
                emit_stdout: false,
                nextkey: 1,
                compression: crate::compress::CompressionRequest::default(),
            }),
        })
    }

    /// Flush and close. Consumes the file.
    pub fn close(mut self) -> Result<()> {
        self.flush_inner()?;
        self.persist_special(true)?;
        self.inner.take();
        Ok(())
    }

    /// Write buffered headers to the store.
    pub fn flush(&mut self) -> Result<()> {
        self.flush_inner()
    }

    /// Number of HDUs currently known.
    pub fn hdu_count(&self) -> Result<usize> {
        Ok(self.inner()?.hdus.len())
    }

    /// Type of the current HDU.
    ///
    /// A tiled compressed image (`ZIMAGE = T`) is reported as [`HduType::Image`]
    /// even though it is stored as a binary table.
    pub fn hdu_type(&self) -> Result<HduType> {
        let inner = self.inner()?;
        Ok(logical_hdu_type(&inner.hdus[inner.current]))
    }

    /// 1-based current HDU number (`fits_get_hdu_num` / `ffghdn`).
    pub fn hdunum(&self) -> Result<usize> {
        Ok(self.inner()?.current + 1)
    }

    /// Move to 1-based HDU `hdunum` (`fits_movabs_hdu` / `ffmahd`).
    ///
    /// Moving past the last HDU returns [`END_OF_FILE`] (CFITSIO). `hdunum < 1`
    /// is [`BAD_HDU_NUM`].
    pub fn movabs_hdu(&mut self, hdunum: usize) -> Result<HduType> {
        self.flush_inner()?;
        let inner = self.inner_mut()?;
        if hdunum < 1 {
            return Err(FitsError::new(BAD_HDU_NUM));
        }
        if hdunum > inner.hdus.len() {
            return Err(FitsError::new(END_OF_FILE));
        }
        inner.current = hdunum - 1;
        inner.nextkey = 1;
        Ok(logical_hdu_type(&inner.hdus[inner.current]))
    }

    /// `fits_movrel_hdu` / `ffmrhd`.
    pub fn movrel_hdu(&mut self, nmove: i32) -> Result<HduType> {
        let here = self.hdunum()? as i32;
        let dest = here + nmove;
        if dest < 1 {
            return Err(FitsError::new(BAD_HDU_NUM));
        }
        self.movabs_hdu(dest as usize)
    }

    /// `fits_movnam_hdu` / `ffmnhd`. `extver == 0` means any version.
    pub fn movnam_hdu(&mut self, hdutype: i32, extname: &str, extver: i32) -> Result<HduType> {
        self.flush_inner()?;
        let inner = self.inner()?;
        let mut found = None;
        for (i, hdu) in inner.hdus.iter().enumerate() {
            if hdutype >= 0 && logical_hdu_type(hdu).code() != hdutype {
                continue;
            }
            let name = hdu
                .header
                .get_string("EXTNAME")
                .map(|(v, _)| v)
                .unwrap_or_default();
            if !name.eq_ignore_ascii_case(extname) {
                continue;
            }
            let ver = hdu.header.get_i64("EXTVER").unwrap_or(1) as i32;
            if extver != 0 && ver != extver {
                continue;
            }
            found = Some(i);
            break;
        }
        let idx = found.ok_or_else(|| FitsError::new(BAD_HDU_NUM))?;
        let inner = self.inner_mut()?;
        inner.current = idx;
        inner.nextkey = 1;
        Ok(logical_hdu_type(&inner.hdus[idx]))
    }

    /// `fits_file_name` / `ffflnm`.
    pub fn filename(&self) -> Result<String> {
        Ok(self
            .inner()?
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string())
    }

    /// `fits_file_mode` / `ffflmd`.
    pub fn filemode(&self) -> Result<i32> {
        Ok(if self.inner()?.writable {
            READWRITE
        } else {
            READONLY
        })
    }

    /// `fits_delete_file` / `ffdelt`.
    pub fn delete_file(mut self) -> Result<()> {
        let path = self.inner()?.path.clone();
        let gzip = self.inner()?.gzip_out.clone();
        self.inner.take();
        if let Some(p) = gzip.or(path) {
            let _ = std::fs::remove_file(p);
        }
        Ok(())
    }

    /// `fits_get_hdrpos` / `ffghps`: (existing keywords, next keyword number).
    pub fn hdrpos(&self) -> Result<(usize, usize)> {
        let inner = self.inner()?;
        Ok((inner.hdus[inner.current].header.len(), inner.nextkey.max(1)))
    }

    /// `fits_write_record` / `ffprec`.
    pub fn write_record(&mut self, card: &str) -> Result<()> {
        self.header_mut()?.write_record(card);
        Ok(())
    }

    /// `fits_get_num_hdus` / `ffthdu`.
    pub fn get_num_hdus(&self) -> Result<i32> {
        Ok(self.hdu_count()? as i32)
    }

    /// Current HDU header.
    pub fn header(&self) -> Result<&Header> {
        let inner = self.inner()?;
        Ok(&inner.hdus[inner.current].header)
    }

    /// Mutable header; marks the HDU dirty so the next flush rewrites it.
    pub fn header_mut(&mut self) -> Result<&mut Header> {
        self.require_write()?;
        let inner = self.inner_mut()?;
        inner.dirty = true;
        let idx = inner.current;
        Ok(&mut inner.hdus[idx].header)
    }

    pub(crate) fn require_write(&self) -> Result<()> {
        if self.inner()?.writable {
            Ok(())
        } else {
            Err(FitsError::new(READONLY_FILE))
        }
    }

    /// Copy on-disk / in-memory bytes after flushing.
    pub fn to_bytes(&mut self) -> Result<Vec<u8>> {
        self.flush_inner()?;
        io::read_all(&mut self.inner_mut()?.io)
    }

    pub(crate) fn inner(&self) -> Result<&Inner> {
        self.inner
            .as_ref()
            .ok_or_else(|| FitsError::new(BAD_FILEPTR))
    }

    pub(crate) fn inner_mut(&mut self) -> Result<&mut Inner> {
        self.inner
            .as_mut()
            .ok_or_else(|| FitsError::new(BAD_FILEPTR))
    }

    fn flush_inner(&mut self) -> Result<()> {
        let writable = self.inner()?.writable;
        let dirty = self.inner()?.dirty;
        let is_bin = self.inner()?.hdus[self.inner()?.current].hdu_type == HduType::BinaryTable;
        if !writable {
            return if dirty {
                Err(FitsError::new(READONLY_FILE))
            } else {
                self.inner_mut()?.io.flush()
            };
        }
        if is_bin {
            self.sync_binary_heap_keywords()?;
        }
        let inner = self.inner_mut()?;
        if inner.dirty {
            flush_header_and_maybe_shift(inner)?;
            inner.dirty = false;
        }
        inner.io.flush()?;
        Ok(())
    }

    fn persist_special(&mut self, emit_stdout: bool) -> Result<()> {
        let gzip_out = self.inner.as_ref().and_then(|i| i.gzip_out.clone());
        let do_stdout = emit_stdout && self.inner.as_ref().is_some_and(|i| i.emit_stdout);
        if gzip_out.is_none() && !do_stdout {
            return Ok(());
        }
        let bytes = io::read_all(&mut self.inner_mut()?.io)?;
        if let Some(path) = gzip_out {
            write_gzip_file(&path, &bytes)?;
        }
        if do_stdout {
            std::io::stdout()
                .write_all(&bytes)
                .map_err(io::map_write_err)?;
            std::io::stdout().flush().map_err(io::map_write_err)?;
        }
        Ok(())
    }
}

impl Drop for FitsFile {
    fn drop(&mut self) {
        let _ = self.flush_inner();
        let _ = self.persist_special(false);
        self.inner.take();
    }
}

fn flush_header_and_maybe_shift(inner: &mut Inner) -> Result<()> {
    let idx = inner.current;
    let header_start = inner.hdus[idx].header_start;
    let old_data_start = inner.hdus[idx].data_start;
    let mut hb = inner.hdus[idx].header.to_record_bytes();
    let data_bytes = inner.hdus[idx].data_bytes().unwrap_or(0);
    let old_header_len = old_data_start.saturating_sub(header_start);
    if data_bytes > 0 && (hb.len() as u64) < old_header_len {
        hb.resize(old_header_len as usize, b' ');
    }
    let new_data_start = header_start + hb.len() as u64;
    let delta = new_data_start as i64 - old_data_start as i64;
    if delta > 0 {
        io::insert_bytes(&mut inner.io, old_data_start, delta as u64, b' ')?;
        inner.bump_offsets_from(idx + 1, delta);
    } else if delta < 0 {
        io::delete_bytes(&mut inner.io, new_data_start, (-delta) as u64)?;
        inner.bump_offsets_from(idx + 1, delta);
    }
    inner.io.write_at(header_start, &hb)?;
    inner.hdus[idx].hdu_type = hdu_type_from_header(&inner.hdus[idx].header);
    inner.hdus[idx].data_start = new_data_start;
    let tail = if data_bytes > 0 {
        crate::convert::pad_data_len(data_bytes)
    } else {
        0
    };
    if inner.hdus[idx].hdu_type == HduType::AsciiTable && tail > data_bytes {
        io::write_fill(
            &mut inner.io,
            new_data_start + data_bytes,
            tail - data_bytes,
            b' ',
        )?;
    }
    let file_end = inner.last_end()?;
    if inner.io.len()? > file_end {
        inner.io.truncate(file_end)?;
    }
    Ok(())
}

fn parse_hdus(bytes: &[u8]) -> Result<Vec<Hdu>> {
    let mut hdus = Vec::new();
    let mut offset = 0u64;
    while (offset as usize) < bytes.len() {
        let slice = &bytes[offset as usize..];
        if slice.iter().all(|&b| b == 0) {
            break;
        }
        let (header, header_len) = Header::parse(slice)?;
        let hdu_type = hdu_type_from_header(&header);
        let mut hdu = Hdu {
            hdu_type,
            header,
            header_start: offset,
            data_start: offset + header_len,
        };
        let data_unit = hdu.data_unit_len()?;
        // `Header::parse` returns the padded header length as data_start.
        hdu.data_start = offset + header_len;
        hdus.push(hdu);
        offset += header_len + data_unit;
        if offset == 0 {
            break;
        }
    }
    if hdus.is_empty() {
        return Err(FitsError::with_message(FILE_NOT_OPENED, "no HDUs found"));
    }
    Ok(hdus)
}

pub(crate) fn logical_hdu_type(hdu: &Hdu) -> HduType {
    if hdu.header.is_compressed_image() {
        HduType::Image
    } else {
        hdu.hdu_type
    }
}

fn hdu_type_from_header(header: &Header) -> HduType {
    if header.card_by_name("SIMPLE").is_some() {
        return HduType::Image;
    }
    if let Ok(card) = header.get_i64("BITPIX") {
        let _ = card;
    }
    if let Some(c) = header.card_by_name("XTENSION") {
        let text = c.as_str().unwrap_or("");
        if text.contains("BINTABLE") {
            return HduType::BinaryTable;
        }
        if text.contains("TABLE") {
            return HduType::AsciiTable;
        }
    }
    HduType::Image
}

fn is_stdout_name(path: &Path) -> bool {
    path == Path::new("-") || path == Path::new("stdout") || path == Path::new("STDOUT")
}

fn is_stdin_name(path: &Path) -> bool {
    path == Path::new("-") || path == Path::new("stdin") || path == Path::new("STDIN")
}

fn ends_with_gz(path: &Path) -> bool {
    path.as_os_str()
        .to_str()
        .is_some_and(|s| s.ends_with(".gz"))
}

fn is_gzip_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

fn read_open_bytes(path: &Path) -> Result<(PathBuf, Vec<u8>)> {
    match std::fs::read(path) {
        Ok(b) => Ok((path.to_path_buf(), b)),
        Err(_) => {
            let mut gz = path.as_os_str().to_os_string();
            gz.push(".gz");
            let gz_path = PathBuf::from(gz);
            let b = std::fs::read(&gz_path).map_err(io::map_open_err)?;
            Ok((gz_path, b))
        }
    }
}

fn maybe_gunzip(bytes: &[u8]) -> Result<Vec<u8>> {
    if !is_gzip_magic(bytes) {
        return Ok(bytes.to_vec());
    }
    #[cfg(feature = "gzip")]
    {
        crate::io::gzip::decompress(bytes)
    }
    #[cfg(not(feature = "gzip"))]
    {
        Err(FitsError::with_message(
            FILE_NOT_OPENED,
            "gzip support is disabled (rebuild with the gzip feature)",
        ))
    }
}

fn write_gzip_file(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(feature = "gzip")]
    {
        let gz = crate::io::gzip::compress(bytes)?;
        std::fs::write(path, gz).map_err(io::map_write_err)
    }
    #[cfg(not(feature = "gzip"))]
    {
        let _ = (path, bytes);
        Err(FitsError::with_message(
            FILE_NOT_CREATED,
            "gzip support is disabled (rebuild with the gzip feature)",
        ))
    }
}

fn split_clobber(path: &Path) -> (PathBuf, bool) {
    if let Some(s) = path.to_str() {
        let trimmed = s.trim_start();
        if let Some(rest) = trimmed.strip_prefix('!') {
            return (PathBuf::from(rest), true);
        }
    }
    (path.to_path_buf(), false)
}

/// `fits_create_file` / `ffinit` then a standard empty primary.
pub fn fits_create_file(name: &str) -> Result<FitsFile> {
    FitsFile::create(name)
}

/// `fits_open_file` / `ffopen`.
pub fn fits_open_file(name: &str, mode: AccessMode) -> Result<FitsFile> {
    FitsFile::open(name, mode)
}

/// `fits_close_file` / `ffclos`.
pub fn fits_close_file(f: FitsFile) -> Result<()> {
    f.close()
}

/// `fits_create_memfile` analogue: empty primary in RAM.
pub fn fits_create_memfile() -> Result<FitsFile> {
    FitsFile::create_memory()
}
