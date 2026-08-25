//! World coordinate conversion (`ffwldp` / `ffxypx` / `ffgics`).

#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use crate::error::{FitsError, Result};
use crate::file::FitsFile;
use crate::status::{BAD_WCS_PROJ, NO_WCS_KEY};

const D2R: f64 = 0.01745329252;

/// `fits_read_img_coord` / `ffgics`.
pub fn read_img_coord(f: &FitsFile) -> Result<(f64, f64, f64, f64, f64, f64, f64, String)> {
    let h = f.header()?;
    let xrval = h
        .get_f64("CRVAL1")
        .map_err(|_| FitsError::new(NO_WCS_KEY))?;
    let yrval = h.get_f64("CRVAL2").unwrap_or(0.0);
    let xrpix = h.get_f64("CRPIX1").unwrap_or(0.0);
    let yrpix = h.get_f64("CRPIX2").unwrap_or(0.0);
    let xinc = h.get_f64("CDELT1").unwrap_or(1.0);
    let yinc = h.get_f64("CDELT2").unwrap_or(1.0);
    let rot = h
        .get_f64("CROTA2")
        .or_else(|_| h.get_f64("CROTA1"))
        .unwrap_or(0.0);
    let ctype = h.get_string("CTYPE1").map(|(v, _)| v).unwrap_or_default();
    let proj = if ctype.len() >= 4 {
        ctype[ctype.len().saturating_sub(4)..].to_string()
    } else {
        ctype
    };
    Ok((xrval, yrval, xrpix, yrpix, xinc, yinc, rot, proj))
}

/// Pixel → world (`fits_pix_to_world` / `ffwldp`). `ctype` is e.g. `"-TAN"`.
pub fn pix_to_world(
    xpix: f64,
    ypix: f64,
    xref: f64,
    yref: f64,
    xrefpix: f64,
    yrefpix: f64,
    xinc: f64,
    yinc: f64,
    rot: f64,
    ctype: &str,
) -> Result<(f64, f64)> {
    let dx = (xpix - xrefpix) * xinc;
    let dy0 = (ypix - yrefpix) * yinc;
    let (dx, dy) = if rot != 0.0 {
        let cosr = (rot * D2R).cos();
        let sinr = (rot * D2R).sin();
        (dx * cosr - dy0 * sinr, dy0 * cosr + dx * sinr)
    } else {
        (dx, dy0)
    };
    let ra0 = xref * D2R;
    let dec0 = yref * D2R;
    let l = dx * D2R;
    let m = dy * D2R;
    let cos0 = dec0.cos();
    let sin0 = dec0.sin();
    let code = proj_code(ctype)?;
    let (rat, dect) = match &code {
        b"CAR" => (ra0 + l, dec0 + m),
        b"TAN" => {
            let x = cos0 * ra0.cos() - l * ra0.sin() - m * ra0.cos() * sin0;
            let y = cos0 * ra0.sin() + l * ra0.cos() - m * ra0.sin() * sin0;
            let z = sin0 + m * cos0;
            (y.atan2(x), (z / (x * x + y * y).sqrt()).atan())
        }
        _ => return Err(FitsError::new(BAD_WCS_PROJ)),
    };
    let mut xpos = rat / D2R;
    let ypos = dect / D2R;
    if xpos < 0.0 {
        xpos += 360.0;
    }
    if xpos >= 360.0 {
        xpos -= 360.0;
    }
    Ok((xpos, ypos))
}

/// World → pixel (`fits_world_to_pix` / `ffxypx`).
pub fn world_to_pix(
    xpos: f64,
    ypos: f64,
    xref: f64,
    yref: f64,
    xrefpix: f64,
    yrefpix: f64,
    xinc: f64,
    yinc: f64,
    rot: f64,
    ctype: &str,
) -> Result<(f64, f64)> {
    let code = proj_code(ctype)?;
    let ra0 = xref * D2R;
    let dec0 = yref * D2R;
    let ra = xpos * D2R;
    let dec = ypos * D2R;
    let coss = dec.cos();
    let sins = dec.sin();
    let cos0 = dec0.cos();
    let sin0 = dec0.sin();
    let sint = sins * sin0 + coss * cos0 * (ra - ra0).cos();
    let mut l;
    let m = match &code {
        b"CAR" => {
            l = (xpos - xref) * D2R;
            (ypos - yref) * D2R
        }
        b"TAN" => {
            if sint <= 0.0 {
                return Err(FitsError::new(crate::status::ANGLE_TOO_BIG));
            }
            let m = if cos0 < 0.001 {
                let mm = (coss * (ra - ra0).cos()) / (sins * sin0);
                (-mm + cos0 * (1.0 + mm * mm)) / sin0
            } else {
                (sins / sint - sin0) / cos0
            };
            if ra0.sin().abs() < 0.3 {
                l = coss * ra.sin() / sint - cos0 * ra0.sin() + m * ra0.sin() * sin0;
                l /= ra0.cos();
            } else {
                l = coss * ra.cos() / sint - cos0 * ra0.cos() + m * ra0.cos() * sin0;
                l /= -ra0.sin();
            }
            m
        }
        _ => return Err(FitsError::new(BAD_WCS_PROJ)),
    };
    let mut dx = l / D2R;
    let mut dy = m / D2R;
    if rot != 0.0 {
        let cosr = (rot * D2R).cos();
        let sinr = (rot * D2R).sin();
        let temp = dx * cosr + dy * sinr;
        dy = dy * cosr - dx * sinr;
        dx = temp;
    }
    Ok((dx / xinc + xrefpix, dy / yinc + yrefpix))
}

fn proj_code(ctype: &str) -> Result<[u8; 3]> {
    let t = ctype.trim();
    let bytes = t.as_bytes();
    let start = if bytes.first() == Some(&b'-') {
        1
    } else {
        bytes.len().saturating_sub(3)
    };
    let slice = &bytes[start.min(bytes.len())..];
    if slice.len() < 3 {
        return Err(FitsError::new(BAD_WCS_PROJ));
    }
    Ok([
        slice[0].to_ascii_uppercase(),
        slice[1].to_ascii_uppercase(),
        slice[2].to_ascii_uppercase(),
    ])
}
