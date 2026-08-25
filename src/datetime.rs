//! FITS DATE keyword utilities (`fits_write_date` / `ffpdat`).

use std::time::{SystemTime, UNIX_EPOCH};

/// Format a Unix timestamp as `YYYY-MM-DDThh:mm:ss` (UTC).
#[must_use]
pub fn format_fits_datetime(unix_secs: i64) -> String {
    let (y, m, d, hh, mm, ss) = civil_from_unix(unix_secs);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}")
}

/// Current UTC time as a FITS datetime string.
#[must_use]
pub fn now_fits_datetime() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_fits_datetime(secs)
}

/// True if `s` looks like a CFITSIO DATE value (`YYYY-MM-DD` or with time).
#[must_use]
pub fn is_fits_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 10 {
        return false;
    }
    b[0..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
}

fn civil_from_unix(unix_secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let mut days = unix_secs.div_euclid(86400);
    let mut tod = unix_secs.rem_euclid(86400);
    if tod < 0 {
        tod += 86400;
        days -= 1;
    }
    let hh = (tod / 3600) as u32;
    let mm = ((tod % 3600) / 60) as u32;
    let ss = (tod % 60) as u32;
    // Howard Hinnant civil_from_days, unix epoch day 0 = 1970-01-01
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, hh, mm, ss)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch() {
        assert_eq!(format_fits_datetime(0), "1970-01-01T00:00:00");
    }

    #[test]
    fn known_date() {
        // 2026-08-25 00:00:00 UTC
        assert_eq!(format_fits_datetime(1_787_616_000), "2026-08-25T00:00:00");
    }
}
