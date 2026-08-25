//! Disk-file [`Driver`](super::Driver).

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{Driver, map_create_err, map_open_err, map_read_err, map_write_err};
use crate::error::{FitsError, Result};
use crate::status::FILE_NOT_CREATED;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

/// Filesystem-backed store.
pub struct DiskDriver {
    file: File,
}

impl DiskDriver {
    /// Create a new file; fails if it already exists.
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path.as_ref())
            .map_err(map_create_err)?;
        Ok(Self { file })
    }

    /// Open an existing file.
    pub fn open(path: impl AsRef<Path>, write: bool) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(write)
            .open(path.as_ref())
            .map_err(map_open_err)?;
        Ok(Self { file })
    }

    /// Unique scratch file in the process temp directory (deleted by the caller).
    pub fn create_temp() -> Result<(Self, PathBuf)> {
        let pid = std::process::id();
        for _ in 0..1024 {
            let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("srfits-{pid}-{seq}.fits"));
            let mut opts = OpenOptions::new();
            opts.read(true).write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            match opts.open(&path) {
                Ok(file) => return Ok((Self { file }, path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(map_create_err(e)),
            }
        }
        Err(FitsError::with_message(
            FILE_NOT_CREATED,
            "could not create a unique scratch FITS file",
        ))
    }
}

impl Driver for DiskDriver {
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> Result<usize> {
        self.file.seek(SeekFrom::Start(pos)).map_err(map_read_err)?;
        self.file.read(buf).map_err(map_read_err)
    }

    fn write_at(&mut self, pos: u64, buf: &[u8]) -> Result<()> {
        self.file
            .seek(SeekFrom::Start(pos))
            .map_err(map_write_err)?;
        self.file.write_all(buf).map_err(map_write_err)
    }

    fn len(&self) -> Result<u64> {
        self.file.metadata().map(|m| m.len()).map_err(map_read_err)
    }

    fn truncate(&mut self, len: u64) -> Result<()> {
        self.file.set_len(len).map_err(map_write_err)
    }

    fn flush(&mut self) -> Result<()> {
        self.file.flush().map_err(map_write_err)
    }
}
