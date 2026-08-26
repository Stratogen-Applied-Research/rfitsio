//! Rice coding used by tiled image compression (`RICE_1`).
//!
//! Port of `fits_rcomp` / `fits_rdecomp` and the byte/short variants in
//! CFITSIO `ricecomp.c`. Adjacent-pixel differences are mapped to unsigned
//! values, then Rice-coded in blocks of `nblock` (typically 32) pixels.

use crate::error::{FitsError, Result};
use crate::status::DATA_DECOMPRESSION_ERR;

/// Default Rice block size written as `ZVAL1`.
pub const DEFAULT_BLOCKSIZE: i32 = 32;

/// Bit width of a byte (`floor(log2(i))+1` for `i > 0`). CFITSIO `nonzero_count`.
const NONZERO_COUNT: [i32; 256] = {
    let mut t = [0i32; 256];
    let mut i = 1u32;
    while i < 256 {
        t[i as usize] = 32 - i.leading_zeros() as i32;
        i += 1;
    }
    t
};

const MASK: [u32; 33] = [
    0,
    0x1,
    0x3,
    0x7,
    0xf,
    0x1f,
    0x3f,
    0x7f,
    0xff,
    0x1ff,
    0x3ff,
    0x7ff,
    0xfff,
    0x1fff,
    0x3fff,
    0x7fff,
    0xffff,
    0x1ffff,
    0x3ffff,
    0x7ffff,
    0xfffff,
    0x1f_ffff,
    0x3f_ffff,
    0x7f_ffff,
    0xff_ffff,
    0x1ff_ffff,
    0x3ff_ffff,
    0x7ff_ffff,
    0xfff_ffff,
    0x1fff_ffff,
    0x3fff_ffff,
    0x7fff_ffff,
    0xffff_ffff,
];

fn mask_nbits(nbits: i32) -> u32 {
    if (0..=32).contains(&nbits) {
        MASK[nbits as usize]
    } else if nbits > 32 {
        u32::MAX
    } else {
        0
    }
}

fn fs_params(bytepix: i32) -> Result<(i32, i32)> {
    match bytepix {
        1 => Ok((3, 6)),
        2 => Ok((4, 14)),
        4 => Ok((5, 25)),
        _ => Err(FitsError::with_message(
            crate::status::DATA_COMPRESSION_ERR,
            "rcomp: bsize must be 1, 2, or 4 bytes",
        )),
    }
}

struct BitWriter {
    buf: Vec<u8>,
    bitbuffer: i32,
    bits_to_go: i32,
}

impl BitWriter {
    fn new(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
            bitbuffer: 0,
            bits_to_go: 8,
        }
    }

    fn output_nbits(&mut self, bits: i32, mut n: i32) {
        let mut lbitbuffer = self.bitbuffer;
        let mut lbits_to_go = self.bits_to_go;
        if lbits_to_go + n > 32 {
            lbitbuffer <<= lbits_to_go;
            lbitbuffer |= (bits >> (n - lbits_to_go)) & MASK[lbits_to_go as usize] as i32;
            self.buf.push((lbitbuffer & 0xff) as u8);
            n -= lbits_to_go;
            lbits_to_go = 8;
        }
        lbitbuffer <<= n;
        lbitbuffer |= bits & MASK[n as usize] as i32;
        lbits_to_go -= n;
        while lbits_to_go <= 0 {
            self.buf.push(((lbitbuffer >> (-lbits_to_go)) & 0xff) as u8);
            lbits_to_go += 8;
        }
        self.bitbuffer = lbitbuffer;
        self.bits_to_go = lbits_to_go;
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits_to_go < 8 {
            self.buf.push((self.bitbuffer << self.bits_to_go) as u8);
        }
        self.buf
    }
}

fn map_diff(pdiff: i32) -> u32 {
    let shifted = pdiff.wrapping_shl(1);
    if pdiff < 0 {
        (!shifted) as u32
    } else {
        shifted as u32
    }
}

/// Compress `pixels` with Rice coding. `bytepix` is 1, 2, or 4.
pub fn compress(pixels: &[i32], bytepix: i32, nblock: i32) -> Result<Vec<u8>> {
    if pixels.is_empty() {
        return Ok(Vec::new());
    }
    let nblock = if nblock < 1 {
        DEFAULT_BLOCKSIZE
    } else {
        nblock
    };
    let (fsbits, fsmax) = fs_params(bytepix)?;
    let bbits = 1 << fsbits;
    let first_bits = bytepix * 8;
    let mut w =
        BitWriter::new(pixels.len() * bytepix as usize + pixels.len() / nblock as usize + 8);
    w.output_nbits(pixels[0], first_bits);
    let mut lastpix = match bytepix {
        1 => pixels[0] as i8 as i32,
        2 => pixels[0] as i16 as i32,
        _ => pixels[0],
    };
    let mut i = 0usize;
    while i < pixels.len() {
        let thisblock = (pixels.len() - i).min(nblock as usize);
        let mut diff = vec![0u32; thisblock];
        let mut pixelsum = 0.0f64;
        for j in 0..thisblock {
            let nextpix = pixels[i + j];
            let pdiff = match bytepix {
                1 => (nextpix as i8).wrapping_sub(lastpix as i8) as i32,
                2 => (nextpix as i16).wrapping_sub(lastpix as i16) as i32,
                _ => nextpix.wrapping_sub(lastpix),
            };
            diff[j] = map_diff(pdiff);
            pixelsum += f64::from(diff[j]);
            lastpix = nextpix;
        }
        let mut dpsum = (pixelsum - (thisblock as f64 / 2.0) - 1.0) / thisblock as f64;
        if dpsum < 0.0 {
            dpsum = 0.0;
        }
        let mut psum = if bytepix == 2 {
            (dpsum as u16 as u32) >> 1
        } else if bytepix == 1 {
            (dpsum as u8 as u32) >> 1
        } else {
            (dpsum as u32) >> 1
        };
        let mut fs = 0i32;
        while psum > 0 {
            fs += 1;
            psum >>= 1;
        }
        if fs >= fsmax {
            w.output_nbits(fsmax + 1, fsbits);
            for &d in &diff {
                w.output_nbits(d as i32, bbits);
            }
        } else if fs == 0 && pixelsum == 0.0 {
            w.output_nbits(0, fsbits);
        } else {
            w.output_nbits(fs + 1, fsbits);
            let fsmask = (1u32 << fs) - 1;
            let mut lbitbuffer = w.bitbuffer;
            let mut lbits_to_go = w.bits_to_go;
            for &v in &diff {
                let mut top = (v >> fs) as i32;
                if lbits_to_go > top {
                    lbitbuffer <<= top + 1;
                    lbitbuffer |= 1;
                    lbits_to_go -= top + 1;
                } else {
                    lbitbuffer <<= lbits_to_go;
                    w.buf.push((lbitbuffer & 0xff) as u8);
                    top -= lbits_to_go;
                    while top >= 8 {
                        w.buf.push(0);
                        top -= 8;
                    }
                    lbitbuffer = 1;
                    lbits_to_go = 7 - top;
                }
                if fs > 0 {
                    lbitbuffer <<= fs;
                    lbitbuffer |= (v & fsmask) as i32;
                    lbits_to_go -= fs;
                    while lbits_to_go <= 0 {
                        w.buf.push(((lbitbuffer >> (-lbits_to_go)) & 0xff) as u8);
                        lbits_to_go += 8;
                    }
                }
            }
            w.bitbuffer = lbitbuffer;
            w.bits_to_go = lbits_to_go;
        }
        i += thisblock;
    }
    Ok(w.finish())
}

/// Decompress a Rice stream into `nx` signed pixels.
pub fn decompress(c: &[u8], nx: usize, bytepix: i32, nblock: i32) -> Result<Vec<i32>> {
    if nx == 0 {
        return Ok(Vec::new());
    }
    let nblock = nblock.max(1) as usize;
    let (fsbits, fsmax) = fs_params(bytepix)?;
    let bbits = 1 << fsbits;
    let first_bytes = bytepix as usize;
    if c.len() < first_bytes {
        return Err(FitsError::with_message(
            DATA_DECOMPRESSION_ERR,
            "decompression error: input buffer not properly allocated",
        ));
    }
    let mut lastpix = match bytepix {
        1 => c[0] as u32,
        2 => u32::from(c[0]) << 8 | u32::from(c[1]),
        _ => u32::from(c[0]) << 24 | u32::from(c[1]) << 16 | u32::from(c[2]) << 8 | u32::from(c[3]),
    };
    let mut pos = first_bytes;
    let mut b = if pos < c.len() {
        let v = u32::from(c[pos]);
        pos += 1;
        v
    } else {
        0
    };
    let mut nbits = 8i32;
    let mut out = vec![0i32; nx];
    let mut i = 0usize;
    while i < nx {
        nbits -= fsbits;
        while nbits < 0 {
            if pos >= c.len() {
                return Err(FitsError::with_message(
                    DATA_DECOMPRESSION_ERR,
                    "decompression error: hit end of compressed byte stream",
                ));
            }
            b = (b << 8) | u32::from(c[pos]);
            pos += 1;
            nbits += 8;
        }
        let fs = (b >> nbits) as i32 - 1;
        b &= mask_nbits(nbits);
        let imax = (i + nblock).min(nx);
        if fs < 0 {
            let v = sign_extend(lastpix, bytepix);
            for slot in out.iter_mut().take(imax).skip(i) {
                *slot = v;
            }
            i = imax;
        } else if fs == fsmax {
            while i < imax {
                let mut k = bbits - nbits;
                let mut diff = b << k;
                k -= 8;
                while k >= 0 {
                    if pos >= c.len() {
                        return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
                    }
                    b = u32::from(c[pos]);
                    pos += 1;
                    diff |= b << k;
                    k -= 8;
                }
                if nbits > 0 {
                    if pos >= c.len() {
                        return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
                    }
                    b = u32::from(c[pos]);
                    pos += 1;
                    diff |= b >> (-k);
                    b &= mask_nbits(nbits);
                } else {
                    b = 0;
                }
                lastpix = trunc_pix(unmap_add(diff, lastpix), bytepix);
                out[i] = sign_extend(lastpix, bytepix);
                i += 1;
            }
        } else {
            while i < imax {
                while b == 0 {
                    nbits += 8;
                    if pos >= c.len() {
                        return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
                    }
                    b = u32::from(c[pos]);
                    pos += 1;
                }
                let idx = b as usize;
                if idx >= NONZERO_COUNT.len() {
                    return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
                }
                let nzero = nbits - NONZERO_COUNT[idx];
                nbits -= nzero + 1;
                if !(0..32).contains(&nbits) {
                    return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
                }
                b ^= 1u32 << nbits;
                nbits -= fs;
                while nbits < 0 {
                    if pos >= c.len() {
                        return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
                    }
                    b = (b << 8) | u32::from(c[pos]);
                    pos += 1;
                    nbits += 8;
                }
                let diff = ((nzero as u32) << fs) | (b >> nbits);
                b &= mask_nbits(nbits);
                lastpix = trunc_pix(unmap_add(diff, lastpix), bytepix);
                out[i] = sign_extend(lastpix, bytepix);
                i += 1;
            }
        }
    }
    Ok(out)
}

fn trunc_pix(v: u32, bytepix: i32) -> u32 {
    match bytepix {
        1 => v & 0xff,
        2 => v & 0xffff,
        _ => v,
    }
}

fn sign_extend(v: u32, bytepix: i32) -> i32 {
    match bytepix {
        1 => v as u8 as i8 as i32,
        2 => v as u16 as i16 as i32,
        _ => v as i32,
    }
}

fn unmap_add(diff: u32, lastpix: u32) -> u32 {
    let mapped = if diff & 1 == 0 {
        diff >> 1
    } else {
        !(diff >> 1)
    };
    mapped.wrapping_add(lastpix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(data: &[i32], bytepix: i32) {
        let c = compress(data, bytepix, DEFAULT_BLOCKSIZE).unwrap();
        let out = decompress(&c, data.len(), bytepix, DEFAULT_BLOCKSIZE).unwrap();
        assert_eq!(out, data, "rice roundtrip failed bytepix={bytepix}");
    }

    #[test]
    fn rice_constant_and_ramp() {
        roundtrip(&[7i32; 64], 4);
        roundtrip(&(0..128).collect::<Vec<_>>(), 4);
        roundtrip(&(0..64).map(|i| i - 20).collect::<Vec<_>>(), 2);
        roundtrip(&(0..32).map(|i| i % 17).collect::<Vec<_>>(), 1);
    }

    #[test]
    fn rice_high_entropy() {
        let data: Vec<i32> = (0..64).map(|i| (i * 7919) % 100_000 - 50_000).collect();
        roundtrip(&data, 4);
    }

    #[test]
    fn nonzero_count_is_byte_bit_width() {
        assert_eq!(NONZERO_COUNT[0], 0);
        assert_eq!(NONZERO_COUNT[126], 7);
        assert_eq!(NONZERO_COUNT[127], 7);
        assert_eq!(NONZERO_COUNT[128], 8);
        for i in 1..256u32 {
            assert_eq!(NONZERO_COUNT[i as usize], 32 - i.leading_zeros() as i32);
        }
    }

    #[test]
    fn rice_row_wrap_jumps_u16() {
        // Row-wrap jumps encode as long unary; stop bits can land on 0x7e/0x7f.
        for nx in [32usize, 64, 128, 256] {
            for ny in [1usize, 4, 16, 64] {
                let data: Vec<i32> = (0..ny)
                    .flat_map(|y| (0..nx).map(move |x| 93 + ((x * 17 + y * 31) % 4003) as i32))
                    .collect();
                roundtrip(&data, 2);
            }
        }
        let mut jumps = vec![100i32; 256];
        jumps[31] = 4000;
        jumps[32] = 93;
        jumps[63] = 4095;
        jumps[64] = 100;
        roundtrip(&jumps, 2);
    }
}
