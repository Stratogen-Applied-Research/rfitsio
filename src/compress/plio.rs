//! IRAF pixel-list (PLIO) codec used by `PLIO_1`.
#![allow(clippy::needless_range_loop)]
//!
//! Port of `pl_p2li` / `pl_l2pi` from CFITSIO `pliocomp.c`. Values must be
//! in `0..=16_777_215` (unsigned 24-bit). The compressed form is a stream
//! of big-endian `i16` words stored in a `1PI`/`1QI` VLA column.

use crate::error::{FitsError, Result};
use crate::status::{DATA_COMPRESSION_ERR, DATA_DECOMPRESSION_ERR};

/// Convert a pixel array to a PLIO line list. Returns the number of `i16`
/// words written (FORTRAN 1-based length, matching `pl_p2li`).
pub fn compress(pixels: &[i32]) -> Result<Vec<i16>> {
    for &p in pixels {
        if !(0..=16_777_215).contains(&p) {
            return Err(FitsError::with_message(
                DATA_COMPRESSION_ERR,
                "data out of range for PLIO compression (0 - 2**24)",
            ));
        }
    }
    Ok(pl_p2li(pixels))
}

/// Translate a PLIO line list into `npix` integer pixels.
pub fn decompress(words: &[i16], npix: usize) -> Result<Vec<i32>> {
    pl_l2pi(words, npix)
}

fn pl_p2li(pxsrc0: &[i32]) -> Vec<i16> {
    let npix = pxsrc0.len() as i32;
    if npix <= 0 {
        return Vec::new();
    }
    let mut pxsrc = vec![0i32; npix as usize + 1];
    pxsrc[1..].copy_from_slice(pxsrc0);
    let mut lldst = vec![0i16; (npix as usize).saturating_mul(2).max(16) + 8];
    lldst[3] = -100;
    lldst[2] = 7;
    lldst[1] = 0;
    lldst[6] = 0;
    lldst[7] = 0;
    let xs = 1i32;
    let xe = xs + npix - 1;
    let mut op = 8i32;
    let mut pv = pxsrc[xs as usize].max(0);
    let mut x1 = xs;
    let mut iz = xs;
    let mut hi = 1i32;
    let mut nv = 0i32;
    for ip in xs..=xe {
        if ip < xe {
            nv = pxsrc[(ip + 1) as usize].max(0);
            if nv == pv {
                continue;
            }
            if pv == 0 {
                pv = nv;
                x1 = ip + 1;
                continue;
            }
        } else if pv == 0 {
            x1 = xe + 1;
        }
        let mut np = ip - x1 + 1;
        let mut nz = x1 - iz;
        if pv > 0 {
            let dv = pv - hi;
            if dv != 0 {
                hi = pv;
                if dv.abs() > 4095 {
                    lldst[op as usize] = ((pv & 4095) + 4096) as i16;
                    op += 1;
                    lldst[op as usize] = (pv / 4096) as i16;
                    op += 1;
                } else {
                    if dv < 0 {
                        lldst[op as usize] = (-dv + 12288) as i16;
                    } else {
                        lldst[op as usize] = (dv + 8192) as i16;
                    }
                    op += 1;
                    if np == 1 && nz == 0 {
                        let v = lldst[(op - 1) as usize];
                        lldst[(op - 1) as usize] = v | 16384;
                        x1 = ip + 1;
                        iz = x1;
                        pv = nv;
                        continue;
                    }
                }
            }
        }
        if nz > 0 {
            while nz > 0 {
                lldst[op as usize] = nz.min(4095) as i16;
                op += 1;
                nz -= 4095;
            }
            if np == 1 && pv > 0 {
                lldst[(op - 1) as usize] = (i32::from(lldst[(op - 1) as usize]) + 20481) as i16;
                x1 = ip + 1;
                iz = x1;
                pv = nv;
                continue;
            }
        }
        while np > 0 {
            lldst[op as usize] = (np.min(4095) + 16384) as i16;
            op += 1;
            np -= 4095;
        }
        x1 = ip + 1;
        iz = x1;
        pv = nv;
    }
    lldst[4] = ((op - 1) % 32768) as i16;
    lldst[5] = ((op - 1) / 32768) as i16;
    let n = (op - 1) as usize;
    lldst[1..=n].to_vec()
}

fn pl_l2pi(ll_src0: &[i16], npix: usize) -> Result<Vec<i32>> {
    if npix == 0 || ll_src0.is_empty() {
        return Ok(vec![0; npix]);
    }
    let mut ll_src = vec![0i16; ll_src0.len() + 1];
    ll_src[1..].copy_from_slice(ll_src0);
    let srclen = ll_src0.len();
    let (lllen, llfirt) = if ll_src[3] > 0 {
        (i32::from(ll_src[3]), 4i32)
    } else {
        (
            (i32::from(ll_src[5]) << 15) + i32::from(ll_src[4]),
            i32::from(ll_src[2]) + 1,
        )
    };
    if lllen <= 0 {
        return Ok(vec![0; npix]);
    }
    let xs = 1i32;
    let xe = xs + npix as i32 - 1;
    let mut skipwd = false;
    let mut op = 1i32;
    let mut x1 = 1i32;
    let mut pv = 1i32;
    let mut px_dst = vec![0i32; npix + 1];
    for ip in llfirt..=lllen {
        if skipwd {
            skipwd = false;
            continue;
        }
        if ip < 1 || ip as usize > srclen {
            return Err(FitsError::with_message(
                DATA_DECOMPRESSION_ERR,
                "error: out-of-bounds memory access attempt (imcomp_decompress_tile)",
            ));
        }
        let opcode = i32::from(ll_src[ip as usize]) / 4096;
        let data = i32::from(ll_src[ip as usize]) & 4095;
        match opcode {
            0 | 4 | 5 => {
                let x2 = x1 + data - 1;
                let i1 = x1.max(xs);
                let i2 = x2.min(xe);
                let np = i2 - i1 + 1;
                if np > 0 {
                    let otop = op + np - 1;
                    if opcode == 4 {
                        for i in op..=otop {
                            px_dst[i as usize] = pv;
                        }
                    } else {
                        for i in op..=otop {
                            px_dst[i as usize] = 0;
                        }
                        if opcode == 5 && i2 == x2 {
                            px_dst[otop as usize] = pv;
                        }
                    }
                    op = otop + 1;
                }
                x1 = x2 + 1;
            }
            1 => {
                if (ip as usize + 1) > srclen {
                    return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
                }
                pv = (i32::from(ll_src[ip as usize + 1]) << 12) + data;
                skipwd = true;
            }
            2 => {
                pv += data;
            }
            3 => {
                pv -= data;
            }
            6 => {
                pv += data;
                if x1 >= xs && x1 <= xe {
                    px_dst[op as usize] = pv;
                    op += 1;
                }
                x1 += 1;
            }
            7 => {
                pv -= data;
                if x1 >= xs && x1 <= xe {
                    px_dst[op as usize] = pv;
                    op += 1;
                }
                x1 += 1;
            }
            _ => {}
        }
        if x1 > xe {
            break;
        }
    }
    for i in op as usize..=npix {
        px_dst[i] = 0;
    }
    Ok(px_dst[1..=npix].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plio_roundtrip_ramp() {
        let data: Vec<i32> = (0..64).map(|i| i + 1).collect();
        let words = compress(&data).unwrap();
        let out = decompress(&words, data.len()).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn plio_roundtrip_runs() {
        let mut data = vec![0i32; 40];
        for i in 10..20 {
            data[i] = 7;
        }
        for i in 25..30 {
            data[i] = 12;
        }
        let words = compress(&data).unwrap();
        let out = decompress(&words, data.len()).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn plio_rejects_negative() {
        assert!(compress(&[-1, 0, 1]).is_err());
    }
}
