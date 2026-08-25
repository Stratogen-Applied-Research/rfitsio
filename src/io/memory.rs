//! In-memory [`Driver`](super::Driver).

use super::Driver;
use crate::error::{FitsError, Result};
use crate::status::WRITE_ERROR;

/// Growable byte buffer acting as a FITS file.
#[derive(Debug, Default)]
pub struct MemoryDriver {
    data: Vec<u8>,
}

impl MemoryDriver {
    /// Empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Take ownership of the bytes.
    #[must_use]
    pub fn into_inner(self) -> Vec<u8> {
        self.data
    }

    /// Borrow the bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

impl Driver for MemoryDriver {
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> Result<usize> {
        let pos = pos as usize;
        if pos >= self.data.len() {
            return Ok(0);
        }
        let n = buf.len().min(self.data.len() - pos);
        buf[..n].copy_from_slice(&self.data[pos..pos + n]);
        Ok(n)
    }

    fn write_at(&mut self, pos: u64, buf: &[u8]) -> Result<()> {
        let pos = usize::try_from(pos).map_err(|_| FitsError::new(WRITE_ERROR))?;
        let end = pos
            .checked_add(buf.len())
            .ok_or_else(|| FitsError::new(WRITE_ERROR))?;
        if end > self.data.len() {
            self.data.resize(end, 0);
        }
        self.data[pos..end].copy_from_slice(buf);
        Ok(())
    }

    fn len(&self) -> Result<u64> {
        Ok(self.data.len() as u64)
    }

    fn truncate(&mut self, len: u64) -> Result<()> {
        let len = usize::try_from(len).map_err(|_| FitsError::new(WRITE_ERROR))?;
        if len > self.data.len() {
            self.data.resize(len, 0);
        } else {
            self.data.truncate(len);
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}
