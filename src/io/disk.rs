//! Disk-file [`Driver`](super::Driver).

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use super::{Driver, map_create_err, map_open_err, map_read_err, map_write_err};
use crate::error::Result;

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
