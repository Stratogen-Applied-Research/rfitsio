//! Pluggable I/O backends. FITS is a sequence of 2880-byte records.

mod disk;
mod memory;

pub use disk::DiskDriver;
pub use memory::MemoryDriver;

use crate::error::{FitsError, Result};
use crate::status::{FILE_NOT_CLOSED, FILE_NOT_CREATED, FILE_NOT_OPENED, READ_ERROR, WRITE_ERROR};
use crate::types::RECORD_LEN;

// FILE_NOT_CLOSED is used by map_close_err (flush/close mapping, PR 2+).

/// Byte-oriented storage used by [`crate::file::FitsFile`].
pub trait Driver: Send {
    /// Read up to `buf.len()` bytes at `pos`. Returns bytes actually read
    /// (0 at EOF).
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> Result<usize>;

    /// Write `buf` at `pos`, extending the store if needed.
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> Result<()>;

    /// Current length in bytes.
    fn len(&self) -> Result<u64>;

    /// Truncate or extend to `len` bytes.
    fn truncate(&mut self, len: u64) -> Result<()>;

    /// Flush buffered data to the underlying device.
    fn flush(&mut self) -> Result<()>;

    /// True when the store contains no bytes.
    fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
}

pub(crate) fn map_create_err(err: std::io::Error) -> FitsError {
    FitsError::with_message(FILE_NOT_CREATED, err.to_string())
}

pub(crate) fn map_open_err(err: std::io::Error) -> FitsError {
    FitsError::with_message(FILE_NOT_OPENED, err.to_string())
}

pub(crate) fn map_read_err(err: std::io::Error) -> FitsError {
    FitsError::with_message(READ_ERROR, err.to_string())
}

pub(crate) fn map_write_err(err: std::io::Error) -> FitsError {
    FitsError::with_message(WRITE_ERROR, err.to_string())
}

#[allow(dead_code)]
pub(crate) fn map_close_err(err: std::io::Error) -> FitsError {
    FitsError::with_message(FILE_NOT_CLOSED, err.to_string())
}

/// Read the entire contents of `driver`.
pub fn read_all(driver: &mut dyn Driver) -> Result<Vec<u8>> {
    let len = driver.len()? as usize;
    let mut buf = vec![0u8; len];
    let mut got = 0usize;
    while got < len {
        let n = driver.read_at(got as u64, &mut buf[got..])?;
        if n == 0 {
            buf.truncate(got);
            break;
        }
        got += n;
    }
    Ok(buf)
}

/// Replace the store with `bytes`.
pub fn write_all(driver: &mut dyn Driver, bytes: &[u8]) -> Result<()> {
    driver.write_at(0, bytes)?;
    driver.truncate(bytes.len() as u64)?;
    driver.flush()
}

/// Write `len` copies of `byte` starting at `pos`.
pub(crate) fn write_fill(io: &mut dyn Driver, pos: u64, len: u64, byte: u8) -> Result<()> {
    const CHUNK: usize = RECORD_LEN;
    let buf = [byte; CHUNK];
    let mut remaining = len;
    let mut at = pos;
    while remaining > 0 {
        let n = remaining.min(CHUNK as u64) as usize;
        io.write_at(at, &buf[..n])?;
        at += n as u64;
        remaining -= n as u64;
    }
    Ok(())
}
