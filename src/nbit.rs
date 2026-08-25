//! PSRFITS search-mode sample packing (`NBIT` = 1, 2, 4, or 8).
//!
//! Earlier samples occupy the high-order bits of each byte (Parkes / PSRFITS /
//! SKA PST). This is not FITS `X` bit-column coding.

use crate::error::{FitsError, Result};
use crate::status::BAD_DATATYPE;

/// Samples that fit in one byte for a given `nbit`.
#[must_use]
pub fn samples_per_byte(nbit: u32) -> Option<usize> {
    match nbit {
        1 => Some(8),
        2 => Some(4),
        4 => Some(2),
        8 => Some(1),
        _ => None,
    }
}

/// Pack unsigned samples (`0 .. 2^nbit - 1`) into bytes, MSB first.
pub fn pack_samples(samples: &[u8], nbit: u32) -> Result<Vec<u8>> {
    let spb = samples_per_byte(nbit).ok_or_else(|| {
        FitsError::with_message(
            BAD_DATATYPE,
            format!("unsupported NBIT={nbit} (want 1, 2, 4, or 8)"),
        )
    })?;
    let max = (1u32 << nbit) - 1;
    for &s in samples {
        if u32::from(s) > max {
            return Err(FitsError::with_message(
                BAD_DATATYPE,
                format!("sample {s} does not fit in {nbit} bits"),
            ));
        }
    }
    let nbytes = samples.len().div_ceil(spb);
    let mut out = vec![0u8; nbytes];
    let shift0 = 8 - nbit;
    for (i, &s) in samples.iter().enumerate() {
        let byte = i / spb;
        let slot = i % spb;
        let shift = shift0 - nbit * slot as u32;
        out[byte] |= s << shift;
    }
    Ok(out)
}

/// Unpack `nsamp` samples from packed bytes. `nsamp == 0` means “as many as
/// the buffer holds” (whole bytes only).
pub fn unpack_samples(bytes: &[u8], nbit: u32, nsamp: usize) -> Result<Vec<u8>> {
    let spb = samples_per_byte(nbit).ok_or_else(|| {
        FitsError::with_message(
            BAD_DATATYPE,
            format!("unsupported NBIT={nbit} (want 1, 2, 4, or 8)"),
        )
    })?;
    let max_from_buf = bytes.len() * spb;
    let n = if nsamp == 0 { max_from_buf } else { nsamp };
    if n > max_from_buf {
        return Err(FitsError::with_message(
            crate::status::NEG_BYTES,
            "not enough packed bytes for requested samples",
        ));
    }
    let mask = ((1u32 << nbit) - 1) as u8;
    let shift0 = 8 - nbit;
    let mut out = vec![0u8; n];
    for i in 0..n {
        let byte = bytes[i / spb];
        let slot = i % spb;
        let shift = shift0 - nbit * slot as u32;
        out[i] = (byte >> shift) & mask;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_bit_msb_first() {
        // val1 in bit7 … val8 in bit0
        let s = [1u8, 0, 1, 0, 1, 0, 1, 0];
        let b = pack_samples(&s, 1).unwrap();
        assert_eq!(b, vec![0b1010_1010]);
        assert_eq!(unpack_samples(&b, 1, 8).unwrap(), s);
    }

    #[test]
    fn two_bit_msb_first() {
        let s = [0u8, 1, 2, 3];
        let b = pack_samples(&s, 2).unwrap();
        assert_eq!(b, vec![0b0001_1011]);
        assert_eq!(unpack_samples(&b, 2, 4).unwrap(), s);
    }

    #[test]
    fn four_bit_high_nibble_first() {
        let s = [0xAu8, 0xB];
        let b = pack_samples(&s, 4).unwrap();
        assert_eq!(b, vec![0xAB]);
        assert_eq!(unpack_samples(&b, 4, 2).unwrap(), s);
    }

    #[test]
    fn eight_bit_identity() {
        let s = [0u8, 7, 255, 16];
        let b = pack_samples(&s, 8).unwrap();
        assert_eq!(b, s);
        assert_eq!(unpack_samples(&b, 8, 0).unwrap(), s);
    }

    #[test]
    fn pad_partial_last_byte() {
        let s = [1u8, 1, 1];
        let b = pack_samples(&s, 1).unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(b[0] & 0b1110_0000, 0b1110_0000);
        assert_eq!(unpack_samples(&b, 1, 3).unwrap(), s);
    }

    #[test]
    fn rejects_bad_nbit_and_range() {
        assert!(pack_samples(&[1], 3).is_err());
        assert!(pack_samples(&[4], 2).is_err());
    }
}
