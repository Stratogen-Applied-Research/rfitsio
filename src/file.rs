//! [`FitsFile`]: create, open, close.

use std::path::{Path, PathBuf};

use crate::error::{FitsError, Result};
use crate::hdu::Hdu;
use crate::header::Header;
use crate::io::{self, DiskDriver, Driver, MemoryDriver};
use crate::status::{BAD_FILEPTR, BAD_HDU_NUM, FILE_NOT_CREATED, FILE_NOT_OPENED, READONLY_FILE};
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
        if clobber && path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| FitsError::with_message(FILE_NOT_CREATED, e.to_string()))?;
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
            }),
        })
    }

    /// Open an existing disk file.
    pub fn open(path: impl AsRef<Path>, mode: AccessMode) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let writable = mode == AccessMode::ReadWrite;
        let mut io = Backend::Disk(DiskDriver::open(&path, writable)?);
        let bytes = io::read_all(&mut io)?;
        if bytes.is_empty() {
            return Err(FitsError::with_message(FILE_NOT_OPENED, "file is empty"));
        }
        let hdus = parse_hdus(&bytes)?;
        Ok(Self {
            inner: Some(Inner {
                io,
                writable,
                path: Some(path),
                hdus,
                current: 0,
                dirty: false,
            }),
        })
    }

    /// Flush and close. Consumes the file.
    pub fn close(mut self) -> Result<()> {
        self.flush_inner()?;
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
    pub fn hdu_type(&self) -> Result<HduType> {
        let inner = self.inner()?;
        Ok(inner.hdus[inner.current].hdu_type)
    }

    /// 1-based current HDU number (`fits_get_hdu_num` / `ffghdn`).
    pub fn hdunum(&self) -> Result<usize> {
        Ok(self.inner()?.current + 1)
    }

    /// Move to 1-based HDU `hdunum` (`fits_movabs_hdu` / `ffmahd`).
    pub fn movabs_hdu(&mut self, hdunum: usize) -> Result<HduType> {
        self.flush_inner()?;
        let inner = self.inner_mut()?;
        if hdunum < 1 || hdunum > inner.hdus.len() {
            return Err(FitsError::new(BAD_HDU_NUM));
        }
        inner.current = hdunum - 1;
        Ok(inner.hdus[inner.current].hdu_type)
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
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| FitsError::new(BAD_FILEPTR))?;
        if !inner.writable {
            return if inner.dirty {
                Err(FitsError::new(READONLY_FILE))
            } else {
                inner.io.flush()
            };
        }
        if inner.dirty {
            flush_header_and_maybe_shift(inner)?;
            inner.dirty = false;
        }
        inner.io.flush()?;
        Ok(())
    }
}

impl Drop for FitsFile {
    fn drop(&mut self) {
        let _ = self.flush_inner();
        self.inner.take();
    }
}

fn flush_header_and_maybe_shift(inner: &mut Inner) -> Result<()> {
    let idx = inner.current;
    let header_start = inner.hdus[idx].header_start;
    let old_data_start = inner.hdus[idx].data_start;
    let mut hb = inner.hdus[idx].header.to_record_bytes();
    let data_bytes = inner.hdus[idx].data_bytes().unwrap_or(0);
    let old_data_len = inner.io.len()?.saturating_sub(old_data_start);
    if data_bytes > 0 && hb.len() < (old_data_start - header_start) as usize {
        hb.resize((old_data_start - header_start) as usize, b' ');
    }
    let new_data_start = header_start + hb.len() as u64;
    if new_data_start != old_data_start && old_data_len > 0 && data_bytes > 0 {
        let mut buf = vec![0u8; old_data_len as usize];
        let n = inner.io.read_at(old_data_start, &mut buf)?;
        buf.truncate(n);
        inner.io.write_at(new_data_start, &buf)?;
    }
    inner.io.write_at(header_start, &hb)?;
    let tail = if data_bytes > 0 {
        crate::convert::pad_data_len(data_bytes)
    } else {
        0
    };
    inner.io.truncate(new_data_start + tail)?;
    if inner.hdus[idx].hdu_type == HduType::AsciiTable && tail > data_bytes {
        io::write_fill(
            &mut inner.io,
            new_data_start + data_bytes,
            tail - data_bytes,
            b' ',
        )?;
    }
    inner.hdus[idx].data_start = new_data_start;
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
