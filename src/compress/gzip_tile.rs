//! GZIP_1 / GZIP_2 tile codecs.
//!
//! GZIP_1 is gzip of the big-endian pixel bytes. GZIP_2 byte-shuffles each
//! pixel (all byte-0s, then all byte-1s, …) and then gzips, matching
//! `fits_shuffle_*bytes` in CFITSIO `imcompress.c`.
//!
//! Compression uses zlib gzip wrapping at level 1 (`Z_BEST_SPEED`), the
//! same settings as `compress2mem_from_mem`. Pixel identity is the gate;
//! bitstream identity with CFITSIO is not required.

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use crate::error::{FitsError, Result};
use crate::status::{DATA_COMPRESSION_ERR, DATA_DECOMPRESSION_ERR};

/// Shuffle `width`-byte pixels in place (GZIP_2).
pub fn shuffle(bytes: &mut [u8], width: usize) {
    if width <= 1 || bytes.is_empty() {
        return;
    }
    let n = bytes.len() / width;
    if n == 0 {
        return;
    }
    let mut tmp = vec![0u8; n * width];
    for i in 0..n {
        for b in 0..width {
            tmp[b * n + i] = bytes[i * width + b];
        }
    }
    bytes[..n * width].copy_from_slice(&tmp);
}

/// Inverse of [`shuffle`].
pub fn unshuffle(bytes: &mut [u8], width: usize) {
    if width <= 1 || bytes.is_empty() {
        return;
    }
    let n = bytes.len() / width;
    if n == 0 {
        return;
    }
    let mut tmp = vec![0u8; n * width];
    for i in 0..n {
        for b in 0..width {
            tmp[i * width + b] = bytes[b * n + i];
        }
    }
    bytes[..n * width].copy_from_slice(&tmp);
}

/// Gzip-compress `src` at CFITSIO's `Z_BEST_SPEED` (level 1).
pub fn gzip_bytes(src: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::new(1));
    encoder
        .write_all(src)
        .map_err(|e| FitsError::with_message(DATA_COMPRESSION_ERR, e.to_string()))?;
    encoder
        .finish()
        .map_err(|e| FitsError::with_message(DATA_COMPRESSION_ERR, e.to_string()))
}

/// Gunzip a GZIP_1 / GZIP_2 tile payload.
pub fn gunzip_bytes(src: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(src);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| FitsError::with_message(DATA_DECOMPRESSION_ERR, e.to_string()))?;
    Ok(out)
}

/// Compress a tile of `width`-byte big-endian pixels.
pub fn compress_tile(pixels: &[u8], width: usize, shuffle_bytes: bool) -> Result<Vec<u8>> {
    let mut buf = pixels.to_vec();
    if shuffle_bytes {
        shuffle(&mut buf, width);
    }
    gzip_bytes(&buf)
}

/// Decompress a GZIP_1 / GZIP_2 tile into `npix * width` bytes.
pub fn decompress_tile(
    compressed: &[u8],
    npix: usize,
    width: usize,
    shuffle_bytes: bool,
) -> Result<Vec<u8>> {
    let mut raw = gunzip_bytes(compressed)?;
    let expect = npix.saturating_mul(width);
    if raw.len() != expect {
        return Err(FitsError::with_message(
            DATA_DECOMPRESSION_ERR,
            "error: uncompressed tile has wrong size",
        ));
    }
    if shuffle_bytes {
        unshuffle(&mut raw, width);
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_roundtrip() {
        let mut b: Vec<u8> = (0u8..16).collect();
        let orig = b.clone();
        shuffle(&mut b, 2);
        assert_ne!(b, orig);
        unshuffle(&mut b, 2);
        assert_eq!(b, orig);
        let mut b4 = orig.clone();
        shuffle(&mut b4, 4);
        unshuffle(&mut b4, 4);
        assert_eq!(b4, orig);
    }

    #[test]
    fn gzip_roundtrip() {
        let src: Vec<u8> = (0..64).map(|i| (i * 3) as u8).collect();
        let c = gzip_bytes(&src).unwrap();
        let out = gunzip_bytes(&c).unwrap();
        assert_eq!(out, src);
    }
}
