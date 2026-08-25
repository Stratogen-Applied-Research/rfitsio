//! PSRFITS convention helpers on top of binary-table vector I/O.
//!
//! PSRFITS is a FITS *convention* (empty primary + named BINTABLE HDUs), not a
//! separate file format. This module reconstructs physical samples from a
//! `SUBINT` `DATA` cell:
//!
//! `physical = (raw - ZERO_OFF) * DAT_SCL + DAT_OFFS`
//!
//! `DAT_SCL` / `DAT_OFFS` have length `NCHAN * NPOL` and index as
//! `ichan + NCHAN * ipol`. Fold-mode `DATA` is TDIM `(NBIN, NCHAN, NPOL)`
//! (first axis fastest). Search-mode samples after NBIT unpack are TDIM
//! `(NCHAN, NPOL, NSBLK)`. Packed 1/2/4-bit search data is **not** FITS `X`;
//! see [`crate::nbit`].
//!
//! Layer C row filters and a CFITSIO C ABI are out of scope for pulsar I/O.

use crate::error::{FitsError, Result};
use crate::file::FitsFile;
use crate::nbit::{pack_samples, samples_per_byte, unpack_samples};
use crate::status::{BAD_COL_NUM, BAD_DIMEN, COL_NOT_FOUND, NOT_BTABLE, ZERO_SCALE};
use crate::tform::{BinaryKind, parse_binary_tform};
use crate::types::HduType;

/// Layout of a flattened `DATA` cell relative to `DAT_SCL` / `DAT_OFFS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CubeLayout {
    /// Fold / PSR / CAL: TDIM `(nbin, nchan, npol)`.
    Fold {
        /// Phase bins (fastest axis).
        nbin: usize,
        /// Frequency channels.
        nchan: usize,
        /// Polarisation products.
        npol: usize,
    },
    /// Search-mode samples: TDIM `(nchan, npol, nsblk)`.
    Search {
        /// Frequency channels (fastest axis).
        nchan: usize,
        /// Polarisation products.
        npol: usize,
        /// Time samples per row.
        nsblk: usize,
    },
}

impl CubeLayout {
    /// Number of samples in one cell.
    #[must_use]
    pub fn len(self) -> usize {
        match self {
            Self::Fold { nbin, nchan, npol } => nbin.saturating_mul(nchan).saturating_mul(npol),
            Self::Search { nchan, npol, nsblk } => nchan.saturating_mul(npol).saturating_mul(nsblk),
        }
    }

    /// True when [`Self::len`] is zero.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// `NCHAN * NPOL` — length of `DAT_SCL` / `DAT_OFFS`.
    #[must_use]
    pub fn nplane(self) -> usize {
        match self {
            Self::Fold { nchan, npol, .. } | Self::Search { nchan, npol, .. } => {
                nchan.saturating_mul(npol)
            }
        }
    }

    /// Channel count.
    #[must_use]
    pub fn nchan(self) -> usize {
        match self {
            Self::Fold { nchan, .. } | Self::Search { nchan, .. } => nchan,
        }
    }

    /// 0-based `DAT_SCL` index `ichan + nchan * ipol` for flattened sample `i`.
    #[must_use]
    pub fn plane_index(self, i: usize) -> usize {
        match self {
            Self::Fold { nbin, .. } => i.checked_div(nbin).unwrap_or(0),
            Self::Search { nchan, npol, .. } => {
                let ichan = i.checked_rem(nchan).unwrap_or(0);
                let ipol = i
                    .checked_div(nchan)
                    .and_then(|v| v.checked_rem(npol))
                    .unwrap_or(0);
                ichan + nchan.saturating_mul(ipol)
            }
        }
    }

    /// 0-based channel for flattened sample `i` (for `DAT_WTS`).
    #[must_use]
    pub fn chan_index(self, i: usize) -> usize {
        match self {
            Self::Fold { nbin, nchan, .. } => i
                .checked_div(nbin)
                .and_then(|v| v.checked_rem(nchan))
                .unwrap_or(0),
            Self::Search { nchan, .. } => i.checked_rem(nchan).unwrap_or(0),
        }
    }
}

/// SUBINT header dimensions used to interpret `DATA`.
#[derive(Clone, Debug, PartialEq)]
pub struct SubintInfo {
    /// `NPOL` (default 1).
    pub npol: i64,
    /// `NBIN` (0 in search mode).
    pub nbin: i64,
    /// `NCHAN`.
    pub nchan: i64,
    /// `NBITS` (search packing; 16 for fold `I` data).
    pub nbits: i64,
    /// `NSBLK` (search samples per row; 1 in fold mode).
    pub nsblk: i64,
    /// `ZERO_OFF` / `ZERO_OFFS` subtracted from unsigned search samples.
    pub zero_off: f64,
    /// `SIGNINT != 0`: packed samples are two's complement.
    pub signint: bool,
}

impl SubintInfo {
    /// Fold if `NBIN > 1`, otherwise search.
    #[must_use]
    pub fn layout(&self) -> CubeLayout {
        let nchan = self.nchan.max(1) as usize;
        let npol = self.npol.max(1) as usize;
        if self.nbin > 1 {
            CubeLayout::Fold {
                nbin: self.nbin as usize,
                nchan,
                npol,
            }
        } else {
            CubeLayout::Search {
                nchan,
                npol,
                nsblk: self.nsblk.max(1) as usize,
            }
        }
    }
}

/// `physical[i] = (raw[i] - zero_off) * scl[plane] + offs[plane]`.
pub fn apply_scale(
    raw: &[f64],
    scl: &[f32],
    offs: &[f32],
    layout: CubeLayout,
    zero_off: f64,
) -> Result<Vec<f64>> {
    let nplane = layout.nplane();
    check_plane_len(scl, offs, nplane)?;
    if raw.len() != layout.len() {
        return Err(FitsError::with_message(
            BAD_DIMEN,
            format!(
                "DATA length {} does not match cube layout {}",
                raw.len(),
                layout.len()
            ),
        ));
    }
    let mut out = vec![0.0f64; raw.len()];
    for (i, &x) in raw.iter().enumerate() {
        let p = layout.plane_index(i);
        out[i] = (x - zero_off) * f64::from(scl[p]) + f64::from(offs[p]);
    }
    Ok(out)
}

/// Inverse of [`apply_scale`]: `raw = (physical - offs) / scl + zero_off`.
pub fn invert_scale(
    physical: &[f64],
    scl: &[f32],
    offs: &[f32],
    layout: CubeLayout,
    zero_off: f64,
) -> Result<Vec<f64>> {
    let nplane = layout.nplane();
    check_plane_len(scl, offs, nplane)?;
    if physical.len() != layout.len() {
        return Err(FitsError::with_message(
            BAD_DIMEN,
            format!(
                "sample length {} does not match cube layout {}",
                physical.len(),
                layout.len()
            ),
        ));
    }
    let mut out = vec![0.0f64; physical.len()];
    for (i, &y) in physical.iter().enumerate() {
        let p = layout.plane_index(i);
        let s = f64::from(scl[p]);
        if s == 0.0 {
            return Err(FitsError::new(ZERO_SCALE));
        }
        out[i] = (y - f64::from(offs[p])) / s + zero_off;
    }
    Ok(out)
}

/// Multiply samples by per-channel `DAT_WTS` (length `NCHAN`).
pub fn apply_channel_weights(data: &mut [f64], wts: &[f32], layout: CubeLayout) -> Result<()> {
    if wts.len() != layout.nchan() {
        return Err(FitsError::with_message(
            BAD_DIMEN,
            format!(
                "DAT_WTS length {} does not equal NCHAN {}",
                wts.len(),
                layout.nchan()
            ),
        ));
    }
    if data.len() != layout.len() {
        return Err(FitsError::with_message(
            BAD_DIMEN,
            format!(
                "DATA length {} does not match cube layout {}",
                data.len(),
                layout.len()
            ),
        ));
    }
    for (i, x) in data.iter_mut().enumerate() {
        *x *= f64::from(wts[layout.chan_index(i)]);
    }
    Ok(())
}

/// Unpack NBIT samples and interpret as signed/unsigned integers.
pub fn decode_packed_samples(
    bytes: &[u8],
    nbit: u32,
    signint: bool,
    nsamp: usize,
) -> Result<Vec<f64>> {
    let unpacked = unpack_samples(bytes, nbit, nsamp)?;
    Ok(unpacked
        .into_iter()
        .map(|s| f64::from(decode_sample(s, nbit, signint)))
        .collect())
}

/// Quantize samples into packed unsigned NBIT bytes (two's complement if `signint`).
pub fn encode_packed_samples(samples: &[f64], nbit: u32, signint: bool) -> Result<Vec<u8>> {
    if samples_per_byte(nbit).is_none() {
        return Err(FitsError::with_message(
            crate::status::BAD_DATATYPE,
            format!("unsupported NBIT={nbit} (want 1, 2, 4, or 8)"),
        ));
    }
    let mut raw = Vec::with_capacity(samples.len());
    for &x in samples {
        raw.push(encode_sample(x, nbit, signint));
    }
    pack_samples(&raw, nbit)
}

fn decode_sample(raw: u8, nbit: u32, signint: bool) -> i32 {
    let bits = nbit.min(8);
    let mask = if bits >= 8 { 0xff } else { (1u32 << bits) - 1 };
    let u = i32::from(raw) & mask as i32;
    if signint && bits > 0 {
        let sign_bit = 1i32 << (bits - 1);
        if u & sign_bit != 0 {
            u - (1i32 << bits)
        } else {
            u
        }
    } else {
        u
    }
}

fn encode_sample(x: f64, nbit: u32, signint: bool) -> u8 {
    let bits = nbit.min(8);
    let rounded = x.round();
    let v = if signint {
        let min = -(1i32 << (bits - 1));
        let max = (1i32 << (bits - 1)) - 1;
        let c = rounded.clamp(f64::from(min), f64::from(max)) as i32;
        (c as u32) & ((1u32 << bits) - 1)
    } else {
        let max = (1u32 << bits) - 1;
        rounded.clamp(0.0, f64::from(max)) as u32
    };
    v as u8
}

fn check_plane_len(scl: &[f32], offs: &[f32], nplane: usize) -> Result<()> {
    if scl.len() != nplane || offs.len() != nplane {
        return Err(FitsError::with_message(
            BAD_DIMEN,
            format!(
                "DAT_SCL/DAT_OFFS lengths {}/{} do not equal NCHAN*NPOL {nplane}",
                scl.len(),
                offs.len()
            ),
        ));
    }
    Ok(())
}

fn header_i64_or(f: &FitsFile, name: &str, default: i64) -> i64 {
    f.header()
        .ok()
        .and_then(|h| h.get_i64(name).ok())
        .unwrap_or(default)
}

fn header_f64_or(f: &FitsFile, names: &[&str], default: f64) -> f64 {
    let Ok(h) = f.header() else {
        return default;
    };
    for name in names {
        if let Ok(v) = h.get_f64(name) {
            return v;
        }
    }
    default
}

impl FitsFile {
    /// Read `NBIN` / `NCHAN` / `NPOL` / `NBITS` / `NSBLK` / `ZERO_OFF` / `SIGNINT`
    /// from the current HDU.
    pub fn read_subint_info(&self) -> Result<SubintInfo> {
        self.require_table()?;
        if self.inner()?.hdus[self.inner()?.current].hdu_type != HduType::BinaryTable {
            return Err(FitsError::new(NOT_BTABLE));
        }
        let signint = match self.header()?.get_i64("SIGNINT") {
            Ok(v) => v != 0,
            Err(_) => self
                .header()?
                .get_logical("SIGNINT")
                .map(|(v, _)| v)
                .unwrap_or(false),
        };
        Ok(SubintInfo {
            npol: header_i64_or(self, "NPOL", 1).max(1),
            nbin: header_i64_or(self, "NBIN", 0),
            nchan: header_i64_or(self, "NCHAN", 1).max(1),
            nbits: header_i64_or(self, "NBITS", 8).max(1),
            nsblk: header_i64_or(self, "NSBLK", 1).max(1),
            zero_off: header_f64_or(self, &["ZERO_OFF", "ZERO_OFFS"], 0.0),
            signint,
        })
    }

    /// Read one `SUBINT` `DATA` row, unpack NBIT if needed, and apply
    /// `DAT_SCL` / `DAT_OFFS` (`physical = (raw - ZERO_OFF) * scl + offs`).
    pub fn read_subint_data(&mut self, row: i64) -> Result<Vec<f64>> {
        let info = self.read_subint_info()?;
        let layout = info.layout();
        let col = self.get_colnum(false, "DATA")?;
        let tform = self
            .header()?
            .get_string(&format!("TFORM{col}"))
            .map(|(v, _)| v)
            .map_err(|_| FitsError::new(BAD_COL_NUM))?;
        let parsed = parse_binary_tform(&tform)?;
        let nbit = info.nbits as u32;
        let packed = matches!(parsed.kind, BinaryKind::B | BinaryKind::S) && nbit < 8;
        let raw_len = if packed {
            parsed.repeat.max(0) as usize
        } else {
            layout.len()
        };
        let (ints, _) = self.read_col_i64(col, row, raw_len.max(1), None)?;
        let samples = if packed {
            let bytes: Vec<u8> = ints.iter().map(|&v| v as u8).collect();
            decode_packed_samples(&bytes, nbit, info.signint, layout.len())?
        } else if matches!(parsed.kind, BinaryKind::I | BinaryKind::J | BinaryKind::K) || nbit > 8 {
            ints.iter().map(|&v| v as f64).collect()
        } else {
            ints.iter()
                .map(|&v| f64::from(decode_sample(v as u8, nbit.min(8), info.signint)))
                .collect()
        };
        if samples.len() != layout.len() {
            return Err(FitsError::with_message(
                BAD_DIMEN,
                format!(
                    "DATA length {} does not match cube layout {}",
                    samples.len(),
                    layout.len()
                ),
            ));
        }
        let nplane = layout.nplane();
        let scl = read_optional_f32_col(self, "DAT_SCL", row, nplane, 1.0)?;
        let offs = read_optional_f32_col(self, "DAT_OFFS", row, nplane, 0.0)?;
        apply_scale(&samples, &scl, &offs, layout, info.zero_off)
    }
}

fn read_optional_f32_col(
    f: &mut FitsFile,
    name: &str,
    row: i64,
    nelem: usize,
    fill: f32,
) -> Result<Vec<f32>> {
    match f.get_colnum(false, name) {
        Err(e) if e.status == COL_NOT_FOUND => Ok(vec![fill; nelem]),
        Err(e) => Err(e),
        Ok(col) => {
            let (vals, _) = f.read_col_f32(col, row, nelem, None)?;
            if vals.len() != nelem {
                return Err(FitsError::with_message(
                    BAD_DIMEN,
                    format!("{name} length {} does not equal {nelem}", vals.len()),
                ));
            }
            Ok(vals)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_plane_index_nbin_fastest() {
        let lay = CubeLayout::Fold {
            nbin: 4,
            nchan: 2,
            npol: 2,
        };
        assert_eq!(lay.len(), 16);
        assert_eq!(lay.nplane(), 4);
        // i = ibin + 4*(ichan + 2*ipol)
        assert_eq!(lay.plane_index(0), 0); // bin0, ch0, pol0
        assert_eq!(lay.plane_index(3), 0);
        assert_eq!(lay.plane_index(4), 1); // ch1, pol0
        assert_eq!(lay.plane_index(8), 2); // ch0, pol1
        assert_eq!(lay.chan_index(4), 1);
        assert_eq!(lay.chan_index(8), 0);
    }

    #[test]
    fn search_plane_index_nchan_fastest() {
        let lay = CubeLayout::Search {
            nchan: 4,
            npol: 2,
            nsblk: 3,
        };
        assert_eq!(lay.len(), 24);
        // i = ichan + 4*(ipol + 2*isamp)
        assert_eq!(lay.plane_index(0), 0);
        assert_eq!(lay.plane_index(3), 3);
        assert_eq!(lay.plane_index(4), 4); // ch0, pol1
        assert_eq!(lay.chan_index(5), 1);
    }

    #[test]
    fn scale_roundtrip_fold() {
        let lay = CubeLayout::Fold {
            nbin: 2,
            nchan: 2,
            npol: 1,
        };
        let raw = [1.0, 2.0, 3.0, 4.0];
        let scl = [0.5f32, 2.0];
        let offs = [10.0f32, -1.0];
        let phys = apply_scale(&raw, &scl, &offs, lay, 0.0).unwrap();
        assert_eq!(phys, vec![10.5, 11.0, 5.0, 7.0]);
        let back = invert_scale(&phys, &scl, &offs, lay, 0.0).unwrap();
        for (a, b) in raw.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn packed_signed_two_bit() {
        // 0,1,-2,-1 in two's complement 2-bit, MSB first → 0b00_01_10_11
        let packed = pack_samples(&[0, 1, 2, 3], 2).unwrap();
        let s = decode_packed_samples(&packed, 2, true, 4).unwrap();
        assert_eq!(s, vec![0.0, 1.0, -2.0, -1.0]);
        let back = encode_packed_samples(&s, 2, true).unwrap();
        assert_eq!(back, packed);
    }

    #[test]
    fn weights_zap_channel() {
        let lay = CubeLayout::Fold {
            nbin: 2,
            nchan: 2,
            npol: 1,
        };
        let mut data = [1.0, 1.0, 1.0, 1.0];
        apply_channel_weights(&mut data, &[1.0, 0.0], lay).unwrap();
        assert_eq!(data, [1.0, 1.0, 0.0, 0.0]);
    }
}
