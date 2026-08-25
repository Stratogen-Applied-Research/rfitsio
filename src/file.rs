//! [`FitsFile`]: create, open, close.

use std::path::{Path, PathBuf};

use crate::error::{FitsError, Result};
use crate::hdu::Hdu;
use crate::header::Header;
use crate::io::{self, DiskDriver, Driver, MemoryDriver};
use crate::status::{BAD_FILEPTR, FILE_NOT_CREATED, FILE_NOT_OPENED, READONLY_FILE};
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

enum Backend {
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

struct Inner {
    io: Backend,
    writable: bool,
    path: Option<PathBuf>,
    hdus: Vec<Hdu>,
    current: usize,
    dirty: bool,
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
        let (header, data_start) = Header::parse(&bytes)?;
        let hdu = Hdu {
            hdu_type: HduType::Image,
            header,
            data_start,
        };
        Ok(Self {
            inner: Some(Inner {
                io,
                writable,
                path: Some(path),
                hdus: vec![hdu],
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

    /// Current HDU header.
    pub fn header(&self) -> Result<&Header> {
        let inner = self.inner()?;
        Ok(&inner.hdus[inner.current].header)
    }

    /// Copy on-disk / in-memory bytes after flushing.
    pub fn to_bytes(&mut self) -> Result<Vec<u8>> {
        self.flush_inner()?;
        io::read_all(&mut self.inner_mut()?.io)
    }

    fn inner(&self) -> Result<&Inner> {
        self.inner
            .as_ref()
            .ok_or_else(|| FitsError::new(BAD_FILEPTR))
    }

    fn inner_mut(&mut self) -> Result<&mut Inner> {
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
            let bytes = inner.hdus[inner.current].header.to_record_bytes();
            io::write_all(&mut inner.io, &bytes)?;
            inner.dirty = false;
        } else {
            inner.io.flush()?;
        }
        Ok(())
    }
}

impl Drop for FitsFile {
    fn drop(&mut self) {
        let _ = self.flush_inner();
        self.inner.take();
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
