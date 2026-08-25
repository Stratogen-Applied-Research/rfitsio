//! ASCII-table TFORM parsing (`ffasfm`), column spacing (`ffgabc`),
//! and C `printf`-compatible field formatting (`ffcfmt`).

use crate::card::format_exp_double;
use crate::error::{FitsError, Result};
use crate::status::{BAD_TFORM, BAD_TFORM_DTYPE, NUM_OVERFLOW};
use crate::types::{FLEN_VALUE, TDOUBLE, TFLOAT, TLONG, TSHORT, TSTRING};

/// ASCII-table TFORM letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiKind {
    /// `Aw` character field.
    A,
    /// `Iw` integer.
    I,
    /// `Fw.d` fixed-point.
    F,
    /// `Ew.d` exponential.
    E,
    /// `Dw.d` exponential (written with `E`, matching CFITSIO `ffcfmt`).
    D,
}

/// Parsed ASCII `TFORMn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsciiTform {
    /// Format letter.
    pub kind: AsciiKind,
    /// Field width in characters.
    pub width: usize,
    /// Decimal places (`F`/`E`/`D`); 0 for `A`/`I`.
    pub decimals: usize,
    /// CFITSIO datatype code (`TSTRING` / `TSHORT` / `TLONG` / `TFLOAT` / `TDOUBLE`).
    pub datacode: i32,
}

impl AsciiTform {
    /// `fits_ascii_tform` / `ffasfm`.
    pub fn parse(tform: &str) -> Result<Self> {
        parse_ascii_tform(tform)
    }

    /// C `printf` conversion matching `ffcfmt` (values printed as `double`).
    #[must_use]
    pub fn c_format(self) -> String {
        match self.kind {
            AsciiKind::A => format!("%{}s", self.width),
            AsciiKind::I => format!("%{}.0f", self.width),
            AsciiKind::F => format!("%{}.{}f", self.width, self.decimals),
            AsciiKind::E | AsciiKind::D => format!("%{}.{}E", self.width, self.decimals),
        }
    }
}

/// Parse an ASCII-table TFORM (`ffasfm`).
pub fn parse_ascii_tform(tform: &str) -> Result<AsciiTform> {
    let trimmed = tform.trim_start();
    if trimmed.len() > FLEN_VALUE - 1 {
        return Err(FitsError::new(BAD_TFORM));
    }
    let form: String = trimmed.chars().map(|c| c.to_ascii_uppercase()).collect();
    if form.is_empty() {
        return Err(FitsError::new(BAD_TFORM));
    }
    let first = form.as_bytes()[0];
    let mut datacode = match first {
        b'A' => TSTRING,
        b'I' => TLONG,
        b'F' | b'E' => TFLOAT,
        b'D' => TDOUBLE,
        _ => {
            return Err(FitsError::with_message(
                BAD_TFORM_DTYPE,
                format!("Illegal ASCII table TFORMn datatype: '{tform}'"),
            ));
        }
    };
    let rest = &form[1..];
    let kind = match first {
        b'A' => AsciiKind::A,
        b'I' => AsciiKind::I,
        b'F' => AsciiKind::F,
        b'E' => AsciiKind::E,
        _ => AsciiKind::D,
    };

    let (width, decimals) = if kind == AsciiKind::A || kind == AsciiKind::I {
        let width = parse_c_long(rest)?;
        if width <= 0 {
            return Err(FitsError::new(BAD_TFORM));
        }
        if width <= 4 && datacode == TLONG {
            datacode = TSHORT;
        }
        (width as usize, 0usize)
    } else {
        let fwidth = parse_c_double(rest)?;
        if fwidth <= 0.0 {
            return Err(FitsError::new(BAD_TFORM));
        }
        let width = fwidth as i64;
        if width > 7 && first == b'F' {
            datacode = TDOUBLE;
        }
        // CFITSIO advances `form` by 1 or 2 characters from the start of the
        // width field, then looks for a '.' — not by digit count of `width`.
        let skip = if width < 10 { 1usize } else { 2usize };
        let mut decimals = 0i64;
        if rest.len() > skip && rest.as_bytes()[skip] == b'.' {
            decimals = parse_c_long(&rest[skip + 1..])?;
            if decimals >= width {
                return Err(FitsError::new(BAD_TFORM));
            }
            if decimals > 6 && first == b'E' {
                datacode = TDOUBLE;
            }
        }
        (width as usize, decimals.max(0) as usize)
    };

    Ok(AsciiTform {
        kind,
        width,
        decimals,
        datacode,
    })
}

/// `fits_get_tbcol` / `ffgabc`: 1-based TBCOL and total row width.
///
/// `space` is the number of blanks between columns (CFITSIO recommends 1).
pub fn ascii_column_starts(tforms: &[&str], space: i64) -> Result<(i64, Vec<i64>)> {
    if tforms.is_empty() {
        return Ok((0, Vec::new()));
    }
    let mut rowlen = 0i64;
    let mut tbcol = Vec::with_capacity(tforms.len());
    for tform in tforms {
        tbcol.push(rowlen + 1);
        let parsed = parse_ascii_tform(tform)?;
        rowlen += parsed.width as i64 + space;
    }
    rowlen -= space;
    Ok((rowlen, tbcol))
}

/// Format `value` into a `width`-character ASCII field (`snprintf` + `ffcfmt`).
pub fn format_ascii_number(value: f64, tform: &AsciiTform) -> Result<Vec<u8>> {
    if !value.is_finite() {
        return Err(FitsError::new(NUM_OVERFLOW));
    }
    let body = match tform.kind {
        AsciiKind::A => {
            return Err(FitsError::new(crate::status::BAD_ATABLE_FORMAT));
        }
        AsciiKind::I => format!("{value:.0}").replace(',', "."),
        AsciiKind::F => format!("{value:.prec$}", prec = tform.decimals).replace(',', "."),
        AsciiKind::E | AsciiKind::D => format_exp_double(value, tform.decimals),
    };
    if body.len() > tform.width {
        return Err(FitsError::new(NUM_OVERFLOW));
    }
    let mut out = vec![b' '; tform.width];
    let start = tform.width - body.len();
    out[start..].copy_from_slice(body.as_bytes());
    Ok(out)
}

/// Left-justify `s` in a `width`-character field, space-padded (or truncated).
#[must_use]
pub fn format_ascii_string(s: &str, width: usize) -> Vec<u8> {
    let mut out = vec![b' '; width];
    let bytes = s.as_bytes();
    let n = bytes.len().min(width);
    out[..n].copy_from_slice(&bytes[..n]);
    out
}

/// Decode an ASCII numeric field (`fffstr*`).
///
/// Implied decimal places from TFORM apply only when the field has no `.`.
pub fn parse_ascii_number(field: &[u8], implied_decimals: usize) -> Result<f64> {
    let mut i = 0usize;
    let n = field.len();
    while i < n && field[i] == b' ' {
        i += 1;
    }
    if i == n {
        return Ok(0.0);
    }
    let mut sign = 1.0f64;
    if field[i] == b'-' || field[i] == b'+' {
        if field[i] == b'-' {
            sign = -1.0;
        }
        i += 1;
        while i < n && field[i] == b' ' {
            i += 1;
        }
    }
    let mut val = 0.0f64;
    let mut power = 1.0f64;
    let mut decpt = false;
    while i < n && field[i].is_ascii_digit() {
        val = val * 10.0 + f64::from(field[i] - b'0');
        i += 1;
        while i < n && field[i] == b' ' {
            i += 1;
        }
    }
    if i < n && (field[i] == b'.' || field[i] == b',') {
        decpt = true;
        i += 1;
        while i < n && field[i] == b' ' {
            i += 1;
        }
        while i < n && field[i].is_ascii_digit() {
            val = val * 10.0 + f64::from(field[i] - b'0');
            power *= 10.0;
            i += 1;
            while i < n && field[i] == b' ' {
                i += 1;
            }
        }
    }
    let mut exponent = 0i32;
    let mut esign = 1i32;
    if i < n && (field[i] == b'E' || field[i] == b'D' || field[i] == b'e' || field[i] == b'd') {
        i += 1;
        while i < n && field[i] == b' ' {
            i += 1;
        }
        if i < n && (field[i] == b'-' || field[i] == b'+') {
            if field[i] == b'-' {
                esign = -1;
            }
            i += 1;
            while i < n && field[i] == b' ' {
                i += 1;
            }
        }
        while i < n && field[i].is_ascii_digit() {
            exponent = exponent * 10 + i32::from(field[i] - b'0');
            i += 1;
            while i < n && field[i] == b' ' {
                i += 1;
            }
        }
    }
    let mut dvalue = sign * val / power;
    if exponent != 0 {
        dvalue *= 10f64.powi(esign * exponent);
    } else if !decpt && implied_decimals > 0 {
        dvalue /= 10f64.powi(implied_decimals as i32);
    }
    Ok(dvalue)
}

/// True if `field` matches the ASCII TNULLn string (`strncmp` of `tnull.len()`).
#[must_use]
pub fn field_is_null(field: &[u8], tnull: &str) -> bool {
    if tnull.is_empty() {
        return false;
    }
    let nb = tnull.as_bytes();
    if nb.len() > field.len() {
        return false;
    }
    field.get(..nb.len()) == Some(nb)
}

/// Trim trailing blanks from an ASCII string field (`ffgcls2`).
#[must_use]
pub fn trim_ascii_field(field: &[u8]) -> String {
    let mut end = field.len();
    while end > 0 && field[end - 1] == b' ' {
        end -= 1;
    }
    String::from_utf8_lossy(&field[..end]).into_owned()
}

fn parse_c_long(s: &str) -> Result<i64> {
    let t = s.trim_start();
    if t.is_empty() {
        return Err(FitsError::new(BAD_TFORM));
    }
    let mut end = 0usize;
    if t.as_bytes().first() == Some(&b'+') || t.as_bytes().first() == Some(&b'-') {
        end = 1;
    }
    while end < t.len() && t.as_bytes()[end].is_ascii_digit() {
        end += 1;
    }
    if end == 0 || (end == 1 && !t.as_bytes()[0].is_ascii_digit()) {
        return Err(FitsError::new(BAD_TFORM));
    }
    t[..end]
        .parse::<i64>()
        .map_err(|_| FitsError::new(BAD_TFORM))
}

fn parse_c_double(s: &str) -> Result<f64> {
    let t = s.trim_start();
    if t.is_empty() {
        return Err(FitsError::new(BAD_TFORM));
    }
    let mut end = 0usize;
    let b = t.as_bytes();
    if b.first() == Some(&b'+') || b.first() == Some(&b'-') {
        end = 1;
    }
    let start_digits = end;
    while end < b.len() && b[end].is_ascii_digit() {
        end += 1;
    }
    if end < b.len() && b[end] == b'.' {
        end += 1;
        while end < b.len() && b[end].is_ascii_digit() {
            end += 1;
        }
    }
    if end == start_digits {
        return Err(FitsError::new(BAD_TFORM));
    }
    t[..end]
        .parse::<f64>()
        .map_err(|_| FitsError::new(BAD_TFORM))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_standard_tforms() {
        let a = AsciiTform::parse("A15").unwrap();
        assert_eq!(a.kind, AsciiKind::A);
        assert_eq!(a.width, 15);
        assert_eq!(a.datacode, TSTRING);

        let i = AsciiTform::parse("I11").unwrap();
        assert_eq!(i.kind, AsciiKind::I);
        assert_eq!(i.width, 11);
        assert_eq!(i.datacode, TLONG);

        let i4 = AsciiTform::parse("I4").unwrap();
        assert_eq!(i4.datacode, TSHORT);

        let f = AsciiTform::parse("F15.6").unwrap();
        assert_eq!(f.kind, AsciiKind::F);
        assert_eq!(f.width, 15);
        assert_eq!(f.decimals, 6);
        assert_eq!(f.datacode, TDOUBLE); // width > 7

        let e = AsciiTform::parse("E13.5").unwrap();
        assert_eq!(e.kind, AsciiKind::E);
        assert_eq!(e.width, 13);
        assert_eq!(e.decimals, 5);

        let d = AsciiTform::parse("D22.14").unwrap();
        assert_eq!(d.kind, AsciiKind::D);
        assert_eq!(d.width, 22);
        assert_eq!(d.decimals, 14);
        assert_eq!(d.datacode, TDOUBLE);
    }

    #[test]
    fn ffgabc_spacing() {
        let tforms = ["A15", "I11", "F15.6", "E13.5", "D22.14"];
        let (rowlen, tbcol) = ascii_column_starts(&tforms, 1).unwrap();
        assert_eq!(rowlen, 80);
        assert_eq!(tbcol, vec![1, 17, 29, 45, 59]);
    }

    #[test]
    fn bad_tform_codes() {
        assert_eq!(
            AsciiTform::parse("X10").unwrap_err().status,
            BAD_TFORM_DTYPE
        );
        assert_eq!(AsciiTform::parse("").unwrap_err().status, BAD_TFORM);
        assert_eq!(AsciiTform::parse("I0").unwrap_err().status, BAD_TFORM);
        assert_eq!(AsciiTform::parse("   ").unwrap_err().status, BAD_TFORM);
    }

    #[test]
    fn number_format_matches_c_width() {
        let i = AsciiTform::parse("I11").unwrap();
        assert_eq!(format_ascii_number(1.0, &i).unwrap(), b"          1");
        assert_eq!(format_ascii_number(333.0, &i).unwrap(), b"        333");

        let f = AsciiTform::parse("F15.6").unwrap();
        assert_eq!(format_ascii_number(1.25, &f).unwrap(), b"       1.250000");

        let e = AsciiTform::parse("E13.5").unwrap();
        assert_eq!(format_ascii_number(1.25, &e).unwrap(), b"  1.25000E+00");
        assert_eq!(format_ascii_number(2.5e10, &e).unwrap(), b"  2.50000E+10");
        assert_eq!(format_ascii_number(-3.75e-4, &e).unwrap(), b" -3.75000E-04");
    }
}
