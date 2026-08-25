//! FITS `CHECKSUM` / `DATASUM` (Seaman–Pence 32-bit 1's complement).

use crate::datetime::now_fits_datetime;
use crate::error::{FitsError, Result};
use crate::file::FitsFile;
use crate::io::Driver;
use crate::status::KEY_NO_EXIST;
use crate::types::RECORD_LEN;

const ZEROS_16: &str = "0000000000000000";
const DATASUM_ZERO: &str = "         0";

/// Encode a 32-bit checksum as 16 ASCII letters/digits (`fits_encode_chksum` / `ffesum`).
///
/// If `complement` is true, the bit-complement of `sum` is encoded (the form
/// stored in the `CHECKSUM` keyword).
#[must_use]
pub fn encode_checksum(sum: u32, complement: bool) -> String {
    const EXCLUDE: [u8; 13] = [
        0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f, 0x40, 0x5b, 0x5c, 0x5d, 0x5e, 0x5f, 0x60,
    ];
    let value = if complement { !sum } else { sum };
    let mut asc = [0u8; 16];
    for ii in 0..4 {
        let byte = ((value >> (24 - 8 * ii)) & 0xff) as i32;
        let quotient = byte / 4 + 0x30;
        let remainder = byte % 4;
        let mut ch = [quotient; 4];
        ch[0] += remainder;
        let mut check = 1;
        while check != 0 {
            check = 0;
            for &ex in &EXCLUDE {
                for jj in (0..4).step_by(2) {
                    if ch[jj] as u8 == ex || ch[jj + 1] as u8 == ex {
                        ch[jj] += 1;
                        ch[jj + 1] -= 1;
                        check += 1;
                    }
                }
            }
        }
        for jj in 0..4 {
            asc[4 * jj + ii] = ch[jj] as u8;
        }
    }
    let mut out = [0u8; 16];
    for ii in 0..16 {
        out[ii] = asc[(ii + 15) % 16];
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Decode a 16-character ASCII checksum (`fits_decode_chksum` / `ffdsum`).
#[must_use]
pub fn decode_checksum(ascii: &str, complement: bool) -> u32 {
    let bytes = ascii.as_bytes();
    let mut cbuf = [0i32; 16];
    for (ii, slot) in cbuf.iter_mut().enumerate() {
        let b = bytes.get((ii + 1) % 16).copied().unwrap_or(b'0');
        *slot = i32::from(b) - 0x30;
    }
    let mut hi = 0u32;
    let mut lo = 0u32;
    for ii in (0..16).step_by(4) {
        hi += ((cbuf[ii] as u32) << 8) + cbuf[ii + 1] as u32;
        lo += ((cbuf[ii + 2] as u32) << 8) + cbuf[ii + 3] as u32;
    }
    let mut hicarry = hi >> 16;
    let mut locarry = lo >> 16;
    while hicarry != 0 || locarry != 0 {
        hi = (hi & 0xffff) + locarry;
        lo = (lo & 0xffff) + hicarry;
        hicarry = hi >> 16;
        locarry = lo >> 16;
    }
    let mut sum = (hi << 16) + lo;
    if complement {
        sum = !sum;
    }
    sum
}

/// Accumulate one 2880-byte FITS record into a running 1's complement checksum.
#[must_use]
pub fn add_checksum_record(sum: u32, rec: &[u8]) -> u32 {
    let rec = if rec.len() >= RECORD_LEN {
        &rec[..RECORD_LEN]
    } else {
        return sum;
    };
    let mut hi = sum >> 16;
    let mut lo = sum & 0xffff;
    let mut i = 0;
    while i + 3 < RECORD_LEN {
        let s0 = u16::from_be_bytes([rec[i], rec[i + 1]]) as u32;
        let s1 = u16::from_be_bytes([rec[i + 2], rec[i + 3]]) as u32;
        hi = hi.wrapping_add(s0);
        lo = lo.wrapping_add(s1);
        i += 4;
    }
    let mut hicarry = hi >> 16;
    let mut locarry = lo >> 16;
    while hicarry != 0 || locarry != 0 {
        hi = (hi & 0xffff) + locarry;
        lo = (lo & 0xffff) + hicarry;
        hicarry = hi >> 16;
        locarry = lo >> 16;
    }
    (hi << 16) + lo
}

/// Checksum of `nrec` consecutive 2880-byte records at `bytes`.
#[must_use]
pub fn checksum_records(bytes: &[u8], nrec: usize) -> u32 {
    let mut sum = 0u32;
    for i in 0..nrec {
        let off = i * RECORD_LEN;
        if off + RECORD_LEN > bytes.len() {
            break;
        }
        sum = add_checksum_record(sum, &bytes[off..off + RECORD_LEN]);
    }
    sum
}

fn parse_datasum(s: &str) -> u32 {
    s.trim().parse::<f64>().unwrap_or(0.0) as u32
}

fn checksum_is_undefined(s: &str) -> bool {
    s.is_empty() || s.bytes().all(|b| b == b' ')
}

impl FitsFile {
    /// `fits_get_chksum` / `ffgcks`: `(datasum, hdusum)`.
    ///
    /// `hdusum` is the data checksum folded with the header records (data
    /// first, then header), matching CFITSIO.
    pub fn get_chksum(&mut self) -> Result<(u32, u32)> {
        self.flush()?;
        let inner = self.inner_mut()?;
        let idx = inner.current;
        let hdu = &inner.hdus[idx];
        let head = hdu.header_start;
        let data = hdu.data_start;
        let end = hdu.end()?;
        let datasum = sum_range(&mut inner.io, data, end, 0)?;
        let hdusum = sum_range(&mut inner.io, head, data, datasum)?;
        Ok((datasum, hdusum))
    }

    /// `fits_verify_chksum` / `ffvcks`.
    ///
    /// Returns `(datastatus, hdustatus)`:
    /// `1` correct, `0` keyword absent/blank, `-1` incorrect.
    pub fn verify_chksum(&mut self) -> Result<(i32, i32)> {
        self.flush()?;
        let (has_check, has_data, data_val) = {
            let h = self.header()?;
            let has_check = match h.get_string("CHECKSUM") {
                Ok((v, _)) => !checksum_is_undefined(&v),
                Err(e) if e.status == KEY_NO_EXIST => false,
                Err(e) => return Err(e),
            };
            let (has_data, data_val) = match h.get_string("DATASUM") {
                Ok((v, _)) => (!v.trim().is_empty(), v),
                Err(e) if e.status == KEY_NO_EXIST => (false, String::new()),
                Err(e) => return Err(e),
            };
            (has_check, has_data, data_val)
        };
        let mut datastatus = if has_data { -1 } else { 0 };
        let mut hdustatus = if has_check { -1 } else { 0 };
        if datastatus == 0 && hdustatus == 0 {
            return Ok((0, 0));
        }
        let (datasum, hdusum) = self.get_chksum()?;
        if has_data && datasum == parse_datasum(&data_val) {
            datastatus = 1;
        }
        if has_check && (hdusum == 0 || hdusum == 0xffff_ffff) {
            hdustatus = 1;
        }
        Ok((datastatus, hdustatus))
    }

    /// `fits_write_chksum` / `ffpcks` using the current UTC timestamp in comments.
    pub fn write_chksum(&mut self) -> Result<()> {
        let dt = now_fits_datetime();
        self.write_chksum_at(&dt)
    }

    /// `ffpcks` with a frozen `YYYY-MM-DDThh:mm:ss` for the keyword comments.
    pub fn write_chksum_at(&mut self, datetime: &str) -> Result<()> {
        self.require_write()?;
        self.flush()?;
        let chkcomm = format!("HDU checksum updated {datetime}");
        let datacomm = format!("data unit checksum updated {datetime}");
        let existing_check = match self.header()?.get_string("CHECKSUM") {
            Ok((v, _)) => Some(v),
            Err(e) if e.status == KEY_NO_EXIST => None,
            Err(e) => return Err(e),
        };
        if existing_check.is_none() {
            self.write_key_str("CHECKSUM", ZEROS_16, Some(&chkcomm))?;
        }
        let mut checksum = existing_check.unwrap_or_else(|| ZEROS_16.to_string());
        let existing_data = match self.header()?.get_string("DATASUM") {
            Ok((v, _)) => Some(v),
            Err(e) if e.status == KEY_NO_EXIST => None,
            Err(e) => return Err(e),
        };
        let olddsum = if let Some(ds) = existing_data {
            parse_datasum(&ds)
        } else {
            self.write_key_str("DATASUM", DATASUM_ZERO, Some(&datacomm))?;
            if checksum != ZEROS_16 {
                checksum = ZEROS_16.to_string();
                self.modify_key_str("CHECKSUM", ZEROS_16, Some(&chkcomm))?;
            }
            0
        };
        self.flush()?;
        let (datasum, _) = self.get_chksum()?;
        if datasum != olddsum {
            self.modify_key_str("DATASUM", &datasum.to_string(), Some(&datacomm))?;
            if checksum != ZEROS_16 {
                checksum = ZEROS_16.to_string();
                self.modify_key_str("CHECKSUM", ZEROS_16, Some(&chkcomm))?;
            }
            self.flush()?;
        }
        if checksum != ZEROS_16 {
            let (_, hdusum) = self.get_chksum()?;
            if hdusum == 0 || hdusum == 0xffff_ffff {
                return Ok(());
            }
            self.modify_key_str("CHECKSUM", ZEROS_16, Some(&chkcomm))?;
            self.flush()?;
        }
        let (_, hdusum) = self.get_chksum()?;
        let encoded = encode_checksum(hdusum, true);
        let old_comm = self
            .header()?
            .get_string("CHECKSUM")
            .map(|(_, c)| c)
            .unwrap_or(chkcomm);
        self.modify_key_str("CHECKSUM", &encoded, Some(&old_comm))?;
        self.flush()?;
        Ok(())
    }

    /// `fits_update_chksum` / `ffupck`. Requires an existing `DATASUM`.
    pub fn update_chksum(&mut self) -> Result<()> {
        self.require_write()?;
        self.flush()?;
        let dt = now_fits_datetime();
        self.update_chksum_at(&dt)
    }

    /// `ffupck` with a frozen comment timestamp.
    pub fn update_chksum_at(&mut self, datetime: &str) -> Result<()> {
        self.require_write()?;
        self.flush()?;
        let chkcomm = format!("HDU checksum updated {datetime}");
        let datasum_s = self
            .header()?
            .get_string("DATASUM")
            .map_err(|e| {
                if e.status == KEY_NO_EXIST {
                    FitsError::with_message(KEY_NO_EXIST, "DATASUM keyword not found (ffupck")
                } else {
                    e
                }
            })?
            .0;
        let dsum = parse_datasum(&datasum_s);
        let existing = match self.header()?.get_string("CHECKSUM") {
            Ok((v, _)) => Some(v),
            Err(e) if e.status == KEY_NO_EXIST => None,
            Err(e) => return Err(e),
        };
        if existing.is_none() {
            self.write_key_str("CHECKSUM", ZEROS_16, Some(&chkcomm))?;
            self.flush()?;
        } else {
            let (_, hdusum) = self.get_chksum()?;
            if hdusum == 0 || hdusum == 0xffff_ffff {
                return Ok(());
            }
            self.modify_key_str("CHECKSUM", ZEROS_16, Some(&chkcomm))?;
            self.flush()?;
        }
        let _ = dsum;
        let (_, hdusum) = self.get_chksum()?;
        let encoded = encode_checksum(hdusum, true);
        let old_comm = self
            .header()?
            .get_string("CHECKSUM")
            .map(|(_, c)| c)
            .unwrap_or(chkcomm);
        self.modify_key_str("CHECKSUM", &encoded, Some(&old_comm))?;
        self.flush()?;
        Ok(())
    }
}

fn sum_range(io: &mut dyn Driver, start: u64, end: u64, mut sum: u32) -> Result<u32> {
    let nbytes = end.saturating_sub(start);
    let nrec = nbytes / RECORD_LEN as u64;
    let mut buf = [0u8; RECORD_LEN];
    for i in 0..nrec {
        let n = io.read_at(start + i * RECORD_LEN as u64, &mut buf)?;
        if n < RECORD_LEN {
            buf[n..].fill(0);
        }
        sum = add_checksum_record(sum, &buf);
    }
    Ok(sum)
}

/// `fits_write_chksum` / `ffpcks`.
pub fn fits_write_chksum(f: &mut FitsFile) -> Result<()> {
    f.write_chksum()
}

/// `fits_update_chksum` / `ffupck`.
pub fn fits_update_chksum(f: &mut FitsFile) -> Result<()> {
    f.update_chksum()
}

/// `fits_verify_chksum` / `ffvcks`.
pub fn fits_verify_chksum(f: &mut FitsFile) -> Result<(i32, i32)> {
    f.verify_chksum()
}

/// `fits_get_chksum` / `ffgcks`.
pub fn fits_get_chksum(f: &mut FitsFile) -> Result<(u32, u32)> {
    f.get_chksum()
}

/// `fits_encode_chksum` / `ffesum`.
#[must_use]
pub fn fits_encode_chksum(sum: u32, complement: bool) -> String {
    encode_checksum(sum, complement)
}

/// `fits_decode_chksum` / `ffdsum`.
#[must_use]
pub fn fits_decode_chksum(ascii: &str, complement: bool) -> u32 {
    decode_checksum(ascii, complement)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        for &s in &[
            0u32,
            1,
            0x1234_5678,
            0xabcd_ef00,
            0xffff_ffff,
            42,
            0x8000_0000,
        ] {
            let a = encode_checksum(s, false);
            assert_eq!(a.len(), 16);
            assert_eq!(decode_checksum(&a, false), s);
            let c = encode_checksum(s, true);
            assert_eq!(decode_checksum(&c, true), s);
        }
    }
}
