//! Floating-point quantization and subtractive dithering.
//!
//! Random sequence matches `fits_init_randoms` (Park–Miller, seed 1, N=10000;
//! the 10000th seed must be 1043618065). Quantized integers are stored by
//! Rice/GZIP/Hcompress; `ZSCALE`/`ZZERO` recover an approximation of the
//! original floats.

use crate::types::{NO_DITHER, SUBTRACTIVE_DITHER_2};

/// Length of the dither table (`N_RANDOM`). Do not change.
pub const N_RANDOM: usize = 10_000;
/// Reserved quantized value for IEEE NaN / FITS nulls.
pub const NULL_VALUE: i32 = -2_147_483_647;
/// Reserved quantized value for exact zeros under `SUBTRACTIVE_DITHER_2`.
pub const ZERO_VALUE: i32 = -2_147_483_646;
const N_RESERVED: f64 = 10.0;

/// Park–Miller sequence used by CFITSIO dithering.
#[must_use]
pub fn dither_table() -> Vec<f32> {
    let a = 16807.0;
    let m = 2_147_483_647.0;
    let mut seed = 1.0;
    let mut out = Vec::with_capacity(N_RANDOM);
    for _ in 0..N_RANDOM {
        let temp = a * seed;
        seed = temp - m * ((temp / m) as i32 as f64);
        out.push((seed / m) as f32);
    }
    debug_assert_eq!(seed as i32, 1_043_618_065);
    out
}

fn nint(x: f64) -> i32 {
    if x >= 0.0 {
        (x + 0.5) as i32
    } else {
        (x - 0.5) as i32
    }
}

/// Median of a copy of `v`.
fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// MAD-style noise estimate plus min/max, used when `qlevel > 0`.
fn noise_minmax(data: &[f64]) -> (f64, f64, f64) {
    if data.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut minv = data[0];
    let mut maxv = data[0];
    for &x in data {
        if x < minv {
            minv = x;
        }
        if x > maxv {
            maxv = x;
        }
    }
    if data.len() < 3 {
        return ((maxv - minv).max(f64::EPSILON), minv, maxv);
    }
    let mut diffs = Vec::with_capacity(data.len());
    for w in data.windows(2) {
        diffs.push((w[1] - w[0]).abs());
    }
    let mad = median(diffs);
    let noise = (mad * 1.482_602).max(f64::EPSILON);
    (noise, minv, maxv)
}

/// Result of quantizing a float tile.
#[derive(Debug, Clone)]
pub struct Quantized {
    /// Scaled integers.
    pub idata: Vec<i32>,
    /// `ZSCALE`.
    pub bscale: f64,
    /// `ZZERO`.
    pub bzero: f64,
}

/// Quantize `fdata` (`nx` fastest). `qlevel > 0` is RMS / q; `qlevel < 0`
/// is an absolute step; `row` is the 1-based tile number for dither.
pub fn quantize(
    fdata: &[f64],
    nx: usize,
    _ny: usize,
    qlevel: f32,
    dither_method: i32,
    row: i64,
    rand: &[f32],
) -> Option<Quantized> {
    let n = fdata.len();
    if n <= 1 {
        return None;
    }
    let (stdev, minval, maxval) = noise_minmax(fdata);
    let delta = if qlevel >= 0.0 {
        let q = if qlevel == 0.0 {
            4.0
        } else {
            f64::from(qlevel)
        };
        stdev / q
    } else {
        -f64::from(qlevel)
    };
    if delta == 0.0 {
        return None;
    }
    if (maxval - minval) / delta > 2.0 * 2_147_483_647.0 - N_RESERVED {
        return None;
    }
    let mut nextrand = 0i32;
    let mut iseed = 0i32;
    let dither = row > 0 && dither_method != NO_DITHER;
    if dither {
        iseed = ((row - 1) as i32).rem_euclid(N_RANDOM as i32);
        nextrand = (rand[iseed as usize] * 500.0) as i32;
    }
    let zeropt = if dither_method == SUBTRACTIVE_DITHER_2 {
        minval - delta * (f64::from(NULL_VALUE) + N_RESERVED)
    } else if (maxval - minval) / delta < 2_147_483_647.0 - N_RESERVED {
        let z = minval;
        let iq = nint(z / delta);
        f64::from(iq) * delta
    } else {
        (minval + maxval) / 2.0
    };
    let mut idata = vec![0i32; n];
    for i in 0..n {
        if dither && dither_method == SUBTRACTIVE_DITHER_2 && fdata[i] == 0.0 {
            idata[i] = ZERO_VALUE;
        } else if dither {
            idata[i] = nint((fdata[i] - zeropt) / delta + f64::from(rand[nextrand as usize]) - 0.5);
        } else {
            idata[i] = nint((fdata[i] - zeropt) / delta);
        }
        if dither {
            nextrand += 1;
            if nextrand == N_RANDOM as i32 {
                iseed += 1;
                if iseed == N_RANDOM as i32 {
                    iseed = 0;
                }
                nextrand = (rand[iseed as usize] * 500.0) as i32;
            }
        }
    }
    let _ = nx;
    Some(Quantized {
        idata,
        bscale: delta,
        bzero: zeropt,
    })
}

/// Inverse of [`quantize`], including subtractive dither.
pub fn dequantize(
    idata: &[i32],
    bscale: f64,
    bzero: f64,
    dither_method: i32,
    row: i64,
    rand: &[f32],
) -> Vec<f64> {
    let dither = row > 0 && dither_method != NO_DITHER;
    let mut iseed = 0i32;
    let mut nextrand = 0i32;
    if dither {
        iseed = ((row - 1) as i32).rem_euclid(N_RANDOM as i32);
        nextrand = (rand[iseed as usize] * 500.0) as i32;
    }
    let mut out = vec![0.0f64; idata.len()];
    for (i, &q) in idata.iter().enumerate() {
        if q == NULL_VALUE {
            out[i] = f64::NAN;
        } else if dither_method == SUBTRACTIVE_DITHER_2 && q == ZERO_VALUE {
            out[i] = 0.0;
        } else if dither {
            out[i] = (f64::from(q) - f64::from(rand[nextrand as usize]) + 0.5) * bscale + bzero;
        } else {
            out[i] = f64::from(q) * bscale + bzero;
        }
        if dither {
            nextrand += 1;
            if nextrand == N_RANDOM as i32 {
                iseed += 1;
                if iseed == N_RANDOM as i32 {
                    iseed = 0;
                }
                nextrand = (rand[iseed as usize] * 500.0) as i32;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn park_miller_seed() {
        let t = dither_table();
        assert_eq!(t.len(), N_RANDOM);
        assert!((t[0] - 7.826_369e-6).abs() < 1e-10);
    }
}
