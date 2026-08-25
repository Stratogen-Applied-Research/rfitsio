//! Binary-table `TDIMn` parse / write (`ffgtdm` / `ffptdm`) and Fortran-order
//! indexing so a vector cell can be addressed as a multidimensional array.

use crate::error::{FitsError, Result};
use crate::file::FitsFile;
use crate::status::{BAD_COL_NUM, BAD_DIMEN, BAD_ELEM_NUM, BAD_TDIM, NOT_BTABLE};
use crate::tform::parse_binary_tform;
use crate::types::{FLEN_VALUE, HduType};

const TDIM_COMMENT: &str = "size of the multidimensional array";

/// Decode a `TDIMn` value such as `(10,20)` or `(1, 128, 1, 2048)`.
///
/// First axis is fastest (FITS / Fortran order). Empty / missing keyword is
/// not handled here — [`FitsFile::read_tdim`] defaults to `[repeat]`.
pub fn decode_tdim(tdimstr: &str) -> Result<Vec<i64>> {
    let s = tdimstr.trim();
    let rest = s.strip_prefix('(').ok_or_else(|| {
        FitsError::with_message(BAD_TDIM, format!("Illegal dimensions format: {tdimstr}"))
    })?;
    let inner = rest.rsplit_once(')').map(|(a, _)| a).ok_or_else(|| {
        FitsError::with_message(BAD_TDIM, format!("Illegal dimensions format: {tdimstr}"))
    })?;
    let mut dims = Vec::new();
    if inner.trim().is_empty() {
        return Err(FitsError::with_message(
            BAD_TDIM,
            format!("Illegal dimensions format: {tdimstr}"),
        ));
    }
    for part in inner.split(',') {
        let t = part.trim();
        let dim: i64 = t.parse().map_err(|_| {
            FitsError::with_message(BAD_TDIM, format!("Illegal dimensions format: {tdimstr}"))
        })?;
        if dim < 0 {
            return Err(FitsError::with_message(
                BAD_TDIM,
                "one or more dimension are less than 0 (ffdtdm)",
            ));
        }
        dims.push(dim);
    }
    if dims.is_empty() {
        return Err(FitsError::new(BAD_TDIM));
    }
    Ok(dims)
}

/// Format `TDIMn` as `(n1,n2,...)` (no spaces, matching `ffptdm`).
pub fn format_tdim(dims: &[i64]) -> Result<String> {
    if dims.is_empty() {
        return Err(FitsError::new(BAD_DIMEN));
    }
    let mut s = String::from("(");
    for (i, &d) in dims.iter().enumerate() {
        if d < 0 {
            return Err(FitsError::new(BAD_TDIM));
        }
        if i > 0 {
            s.push(',');
        }
        s.push_str(&d.to_string());
    }
    s.push(')');
    if s.len() > FLEN_VALUE - 1 {
        return Err(FitsError::with_message(
            BAD_TDIM,
            "TDIM string too long (ffptdm)",
        ));
    }
    Ok(s)
}

/// Product of TDIM axes.
pub fn tdim_product(dims: &[i64]) -> Result<i64> {
    let mut n = 1i64;
    for &d in dims {
        if d < 0 {
            return Err(FitsError::new(BAD_TDIM));
        }
        n = n
            .checked_mul(d)
            .ok_or_else(|| FitsError::new(crate::status::ARRAY_TOO_BIG))?;
    }
    Ok(n)
}

/// 1-based Fortran-order element from 1-based coordinates.
///
/// `elem = 1 + (c0-1) + n0*(c1-1) + n0*n1*(c2-1) + …`
pub fn tdim_elem(dims: &[i64], coords: &[i64]) -> Result<i64> {
    if dims.is_empty() || coords.len() != dims.len() {
        return Err(FitsError::new(BAD_DIMEN));
    }
    let mut elem = 1i64;
    let mut stride = 1i64;
    for (&n, &c) in dims.iter().zip(coords.iter()) {
        if c < 1 || c > n {
            return Err(FitsError::new(BAD_ELEM_NUM));
        }
        elem += (c - 1) * stride;
        stride = stride
            .checked_mul(n)
            .ok_or_else(|| FitsError::new(crate::status::ARRAY_TOO_BIG))?;
    }
    Ok(elem)
}

/// Inverse of [`tdim_elem`]: 1-based coordinates for a 1-based element.
pub fn tdim_coords(dims: &[i64], elem: i64) -> Result<Vec<i64>> {
    let prod = tdim_product(dims)?;
    if elem < 1 || (prod > 0 && elem > prod) {
        return Err(FitsError::new(BAD_ELEM_NUM));
    }
    let mut rest = elem - 1;
    let mut coords = vec![1i64; dims.len()];
    for (i, &n) in dims.iter().enumerate() {
        if n == 0 {
            coords[i] = 1;
            continue;
        }
        coords[i] = rest % n + 1;
        rest /= n;
    }
    Ok(coords)
}

impl FitsFile {
    /// `fits_read_tdim` / `ffgtdm`. Missing `TDIMn` → `[repeat]`.
    pub fn read_tdim(&self, colnum: i32) -> Result<Vec<i64>> {
        self.require_table()?;
        let inner = self.inner()?;
        let hdu = &inner.hdus[inner.current];
        if hdu.hdu_type != HduType::BinaryTable {
            return Err(FitsError::new(NOT_BTABLE));
        }
        let tfields = hdu.header.get_i64("TFIELDS").unwrap_or(0) as i32;
        if colnum < 1 || colnum > tfields {
            return Err(FitsError::new(BAD_COL_NUM));
        }
        let tform = hdu
            .header
            .get_string(&format!("TFORM{colnum}"))
            .map(|(v, _)| v)
            .map_err(|_| FitsError::new(crate::status::NO_TFORM))?;
        let parsed = parse_binary_tform(&tform)?;
        match hdu.header.get_string(&format!("TDIM{colnum}")) {
            Err(_) => Ok(vec![parsed.repeat.max(1)]),
            Ok((val, _)) => {
                let dims = decode_tdim(&val)?;
                if !parsed.is_variable() {
                    let prod = tdim_product(&dims)?;
                    if prod != parsed.repeat {
                        return Err(FitsError::with_message(
                            BAD_TDIM,
                            format!(
                                "column vector length, {}, does not equal TDIMn array size, {prod}",
                                parsed.repeat
                            ),
                        ));
                    }
                }
                Ok(dims)
            }
        }
    }

    /// `fits_write_tdim` / `ffptdm`.
    pub fn write_tdim(&mut self, colnum: i32, dims: &[i64]) -> Result<()> {
        self.require_write()?;
        self.require_table()?;
        if self.inner()?.hdus[self.inner()?.current].hdu_type != HduType::BinaryTable {
            return Err(FitsError::with_message(
                NOT_BTABLE,
                "Error: The TDIMn keyword is only allowed in BINTABLE extensions (ffptdm)",
            ));
        }
        if !(1..=999).contains(&colnum) {
            return Err(FitsError::with_message(
                BAD_COL_NUM,
                "column number is out of range 1 - 999 (ffptdm)",
            ));
        }
        if dims.is_empty() {
            return Err(FitsError::with_message(
                BAD_DIMEN,
                "naxis is less than 1 (ffptdm)",
            ));
        }
        let tfields = self.ncols()?;
        if colnum > tfields {
            return Err(FitsError::new(BAD_COL_NUM));
        }
        let tform = self
            .header()?
            .get_string(&format!("TFORM{colnum}"))
            .map(|(v, _)| v)
            .map_err(|_| FitsError::new(crate::status::NO_TFORM))?;
        let parsed = parse_binary_tform(&tform)?;
        let prod = tdim_product(dims)?;
        if !parsed.is_variable() && prod != parsed.repeat {
            return Err(FitsError::with_message(
                BAD_TDIM,
                format!(
                    "column vector length, {}, does not equal TDIMn array size, {prod}",
                    parsed.repeat
                ),
            ));
        }
        let val = format_tdim(dims)?;
        self.header_mut()?.update_with(
            &format!("TDIM{colnum}"),
            &crate::card::format_string_value(&val),
            Some(TDIM_COMMENT),
        )?;
        Ok(())
    }
}

/// `fits_read_tdim`.
pub fn fits_read_tdim(f: &FitsFile, colnum: i32) -> Result<Vec<i64>> {
    f.read_tdim(colnum)
}

/// `fits_write_tdim`.
pub fn fits_write_tdim(f: &mut FitsFile, colnum: i32, dims: &[i64]) -> Result<()> {
    f.write_tdim(colnum, dims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::BAD_TDIM;

    #[test]
    fn decode_with_and_without_spaces() {
        assert_eq!(decode_tdim("(10,20)").unwrap(), vec![10, 20]);
        assert_eq!(
            decode_tdim("(1, 128, 1, 2048)").unwrap(),
            vec![1, 128, 1, 2048]
        );
        assert_eq!(decode_tdim("  (4) ").unwrap(), vec![4]);
    }

    #[test]
    fn format_matches_ffptdm() {
        assert_eq!(format_tdim(&[1, 128, 4, 4096]).unwrap(), "(1,128,4,4096)");
    }

    #[test]
    fn fortran_index_roundtrip() {
        let dims = [2i64, 3, 4];
        let n = tdim_product(&dims).unwrap();
        assert_eq!(n, 24);
        for e in 1..=n {
            let c = tdim_coords(&dims, e).unwrap();
            assert_eq!(tdim_elem(&dims, &c).unwrap(), e);
        }
        assert_eq!(tdim_elem(&dims, &[1, 1, 1]).unwrap(), 1);
        assert_eq!(tdim_elem(&dims, &[2, 1, 1]).unwrap(), 2);
        assert_eq!(tdim_elem(&dims, &[1, 2, 1]).unwrap(), 3);
    }

    #[test]
    fn bad_format() {
        assert_eq!(decode_tdim("10,20").unwrap_err().status, BAD_TDIM);
        assert_eq!(decode_tdim("(-1)").unwrap_err().status, BAD_TDIM);
    }
}
