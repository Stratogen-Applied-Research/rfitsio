//! gzip encode/decode for `.gz` FITS files (`flate2` rust backend).

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use super::{Driver, map_open_err, map_read_err, map_write_err};
use crate::error::Result;

const STREAM_CHUNK: usize = 64 * 1024;

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

/// Stream-decompress a gzip file into `dest` (no full-file RAM buffer).
pub fn decompress_file_to_driver(src: &Path, dest: &mut dyn Driver) -> Result<()> {
    let file = File::open(src).map_err(map_open_err)?;
    let mut decoder = GzDecoder::new(file);
    let mut buf = vec![0u8; STREAM_CHUNK];
    let mut pos = 0u64;
    loop {
        let n = decoder.read(&mut buf).map_err(map_read_err)?;
        if n == 0 {
            break;
        }
        dest.write_at(pos, &buf[..n])?;
        pos += n as u64;
    }
    dest.truncate(pos)?;
    dest.flush()
}

/// Stream-compress `src` to a gzip file at `path` (no full-file RAM buffer).
pub fn compress_driver_to_file(src: &mut dyn Driver, path: &Path) -> Result<()> {
    let file = File::create(path).map_err(map_write_err)?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    let len = src.len()?;
    let mut buf = vec![0u8; STREAM_CHUNK];
    let mut pos = 0u64;
    while pos < len {
        let n = ((len - pos) as usize).min(buf.len());
        let got = src.read_at(pos, &mut buf[..n])?;
        if got == 0 {
            break;
        }
        encoder.write_all(&buf[..got]).map_err(map_write_err)?;
        pos += got as u64;
    }
    encoder.finish().map_err(map_write_err)?;
    Ok(())
}
