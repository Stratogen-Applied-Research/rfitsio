//! Implicit datatype conversion and BSCALE/BZERO scaling.

use crate::error::{FitsError, Result};
use crate::status::{NUM_OVERFLOW, ZERO_SCALE};
use crate::types::ImageType;

/// Round the way CFITSIO does for float→int (nearest, ties away from 0
/// via `round`).
fn iround(x: f64) -> f64 {
    x.round()
}

fn check_scale(bscale: f64) -> Result<()> {
    if bscale == 0.0 {
        Err(FitsError::new(ZERO_SCALE))
    } else {
        Ok(())
    }
}

/// Encode a physical value into `ty`'s on-disk storage (big-endian).
pub fn encode_physical(physical: f64, ty: ImageType, bscale: f64, bzero: f64) -> Result<Vec<u8>> {
    check_scale(bscale)?;
    match ty {
        ImageType::U8 => {
            let raw = iround((physical - bzero) / bscale);
            if !(0.0..=255.0).contains(&raw) {
                return Err(FitsError::new(NUM_OVERFLOW));
            }
            Ok(vec![raw as u8])
        }
        ImageType::I8 => {
            // stored_u8 = physical - BZERO, BZERO = -128
            let raw = iround((physical - bzero) / bscale);
            if !(0.0..=255.0).contains(&raw) {
                return Err(FitsError::new(NUM_OVERFLOW));
            }
            Ok(vec![raw as u8])
        }
        ImageType::I16 | ImageType::U16 => {
            let raw = iround((physical - bzero) / bscale);
            if !((i16::MIN as f64)..=(i16::MAX as f64)).contains(&raw) {
                return Err(FitsError::new(NUM_OVERFLOW));
            }
            Ok((raw as i16).to_be_bytes().to_vec())
        }
        ImageType::I32 | ImageType::U32 => {
            let raw = iround((physical - bzero) / bscale);
            if !((i32::MIN as f64)..=(i32::MAX as f64)).contains(&raw) {
                return Err(FitsError::new(NUM_OVERFLOW));
            }
            Ok((raw as i32).to_be_bytes().to_vec())
        }
        ImageType::I64 | ImageType::U64 => {
            let raw = iround((physical - bzero) / bscale);
            if !((i64::MIN as f64)..=(i64::MAX as f64)).contains(&raw) {
                return Err(FitsError::new(NUM_OVERFLOW));
            }
            Ok((raw as i64).to_be_bytes().to_vec())
        }
        ImageType::F32 => {
            let raw = ((physical - bzero) / bscale) as f32;
            Ok(raw.to_be_bytes().to_vec())
        }
        ImageType::F64 => {
            let raw = (physical - bzero) / bscale;
            Ok(raw.to_be_bytes().to_vec())
        }
    }
}

/// Decode on-disk storage (big-endian) to a physical value.
pub fn decode_physical(bytes: &[u8], ty: ImageType, bscale: f64, bzero: f64) -> Result<f64> {
    check_scale(bscale)?;
    let raw = match ty {
        ImageType::U8 | ImageType::I8 => {
            if bytes.is_empty() {
                return Err(FitsError::new(crate::status::READ_ERROR));
            }
            f64::from(bytes[0])
        }
        ImageType::I16 | ImageType::U16 => f64::from(i16::from_be_bytes(copy_arr(bytes)?)),
        ImageType::I32 | ImageType::U32 => f64::from(i32::from_be_bytes(copy_arr(bytes)?)),
        ImageType::I64 | ImageType::U64 => i64::from_be_bytes(copy_arr(bytes)?) as f64,
        ImageType::F32 => f64::from(f32::from_be_bytes(copy_arr(bytes)?)),
        ImageType::F64 => f64::from_be_bytes(copy_arr(bytes)?),
    };
    Ok(raw * bscale + bzero)
}

/// Encode native unsigned 16-bit pixels into BITPIX=16 + BZERO=32768.
#[must_use]
pub fn encode_u16_ushort(v: u16) -> [u8; 2] {
    (v.wrapping_sub(32768) as i16).to_be_bytes()
}

/// Decode BITPIX=16 + BZERO=32768 into u16.
#[must_use]
pub fn decode_ushort_u16(bytes: [u8; 2]) -> u16 {
    (i16::from_be_bytes(bytes) as u16).wrapping_add(32768)
}

/// Encode native i8 into BITPIX=8 + BZERO=-128.
#[must_use]
pub fn encode_i8_sbyte(v: i8) -> u8 {
    (v as u8).wrapping_add(128)
}

/// Decode BITPIX=8 + BZERO=-128 into i8.
#[must_use]
pub fn decode_sbyte_i8(b: u8) -> i8 {
    b.wrapping_sub(128) as i8
}

/// Encode native u32 into BITPIX=32 + BZERO=2^31.
#[must_use]
pub fn encode_u32_ulong(v: u32) -> [u8; 4] {
    (v.wrapping_sub(1u32 << 31) as i32).to_be_bytes()
}

/// Decode BITPIX=32 + BZERO=2^31 into u32.
#[must_use]
pub fn decode_ulong_u32(bytes: [u8; 4]) -> u32 {
    (i32::from_be_bytes(bytes) as u32).wrapping_add(1u32 << 31)
}

/// Encode native u64 into BITPIX=64 + BZERO=2^63.
#[must_use]
pub fn encode_u64_ulonglong(v: u64) -> [u8; 8] {
    (v.wrapping_sub(1u64 << 63) as i64).to_be_bytes()
}

/// Decode BITPIX=64 + BZERO=2^63 into u64.
#[must_use]
pub fn decode_ulonglong_u64(bytes: [u8; 8]) -> u64 {
    (i64::from_be_bytes(bytes) as u64).wrapping_add(1u64 << 63)
}

fn copy_arr<const N: usize>(bytes: &[u8]) -> Result<[u8; N]> {
    bytes
        .get(..N)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| FitsError::new(crate::status::READ_ERROR))
}

/// Pad a data-unit length to a multiple of 2880. A zero-length unit stays 0.
#[must_use]
pub fn pad_data_len(n: u64) -> u64 {
    if n == 0 {
        0
    } else {
        n.div_ceil(crate::types::RECORD_LEN as u64) * crate::types::RECORD_LEN as u64
    }
}
