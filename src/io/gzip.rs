//! gzip encode/decode for `.gz` FITS files (`flate2` rust backend).

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use super::{map_read_err, map_write_err};
use crate::error::Result;

/// gzip magic `1F 8B`.
#[must_use]
pub fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

/// Decompress a gzip bitstream into the raw FITS bytes.
pub fn decompress(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).map_err(map_read_err)?;
    Ok(out)
}

/// Compress raw FITS bytes as gzip (default zlib level 6).
pub fn compress(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).map_err(map_write_err)?;
    encoder.finish().map_err(map_write_err)
}
