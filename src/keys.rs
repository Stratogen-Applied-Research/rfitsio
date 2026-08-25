//! Header keyword CRUD on [`FitsFile`] (`fits_write_key_*` / read / update / …).

use crate::card::{
    Card, format_exp_double, format_fixed_double, format_string_value, make_card_string,
};
use crate::datetime::{is_fits_date, now_fits_datetime};
use crate::error::Result;
use crate::file::FitsFile;
use crate::header::PrimaryInfo;

impl FitsFile {
    /// `fits_write_key_str` / `ffpkys`.
    pub fn write_key_str(&mut self, name: &str, value: &str, comm: Option<&str>) -> Result<()> {
        self.header_mut()?.write_string(name, value, comm)
    }

    /// `fits_write_key_log` / `ffpkyl`.
    pub fn write_key_log(&mut self, name: &str, value: bool, comm: Option<&str>) -> Result<()> {
        self.header_mut()?.write_logical(name, value, comm)
    }

    /// `fits_write_key_lng` / `ffpkyj`.
    pub fn write_key_lng(&mut self, name: &str, value: i64, comm: Option<&str>) -> Result<()> {
        self.header_mut()?.write_long(name, value, comm)
    }

    /// `fits_write_key_ulng` / `ffpkyuj`.
    pub fn write_key_ulng(&mut self, name: &str, value: u64, comm: Option<&str>) -> Result<()> {
        self.header_mut()?.write_ulong(name, value, comm)
    }

    /// `fits_write_key_flt` / `ffpkye`.
    pub fn write_key_flt(
        &mut self,
        name: &str,
        value: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        self.header_mut()?.write_float_exp(name, value, decim, comm)
    }

    /// `fits_write_key_fixflt` / `ffpkyf`.
    pub fn write_key_fixflt(
        &mut self,
        name: &str,
        value: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        self.header_mut()?
            .write_fixed_double(name, value, decim, comm)
    }

    /// `fits_write_key_dbl` / `ffpkyd`.
    pub fn write_key_dbl(
        &mut self,
        name: &str,
        value: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        self.write_key_flt(name, value, decim, comm)
    }

    /// `fits_write_key_fixdbl` / `ffpkyg`.
    pub fn write_key_fixdbl(
        &mut self,
        name: &str,
        value: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        self.write_key_fixflt(name, value, decim, comm)
    }

    /// `fits_write_key_null` / `ffpkyu`.
    pub fn write_key_null(&mut self, name: &str, comm: Option<&str>) -> Result<()> {
        self.header_mut()?.write_null(name, comm)
    }

    /// `fits_write_comment` / `ffpcom`.
    pub fn write_comment(&mut self, comm: &str) -> Result<()> {
        self.header_mut()?.write_comment(comm);
        Ok(())
    }

    /// `fits_write_history` / `ffphis`.
    pub fn write_history(&mut self, hist: &str) -> Result<()> {
        self.header_mut()?.write_history(hist);
        Ok(())
    }

    /// `fits_write_date` / `ffpdat`. Uses current UTC.
    pub fn write_date(&mut self) -> Result<()> {
        let dt = now_fits_datetime();
        self.header_mut()?.write_date_value(&dt)
    }

    /// Write DATE with an explicit timestamp (for tests / frozen clocks).
    pub fn write_date_value(&mut self, datetime: &str) -> Result<()> {
        self.header_mut()?.write_date_value(datetime)
    }

    /// `fits_write_key_unit` / `ffpunt`.
    pub fn write_key_unit(&mut self, name: &str, unit: &str) -> Result<()> {
        self.header_mut()?.set_unit(name, unit)
    }

    /// `fits_write_key_cmp` / `ffpkyc`.
    pub fn write_key_cmp(
        &mut self,
        name: &str,
        real: f64,
        imag: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        self.header_mut()?
            .write_complex_exp(name, real, imag, decim, comm)
    }

    /// `fits_write_key_fixcmp` / `ffpkfc`.
    pub fn write_key_fixcmp(
        &mut self,
        name: &str,
        real: f64,
        imag: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        self.header_mut()?
            .write_complex_fixed(name, real, imag, decim, comm)
    }

    /// `fits_read_key_str` / `ffgkys`.
    pub fn read_key_str(&self, name: &str) -> Result<(String, String)> {
        self.header()?.get_string(name)
    }

    /// `fits_read_key_log` / `ffgkyl`.
    pub fn read_key_log(&self, name: &str) -> Result<(bool, String)> {
        self.header()?.get_logical(name)
    }

    /// `fits_read_key_lng` / `ffgkyj`.
    pub fn read_key_lng(&self, name: &str) -> Result<i64> {
        self.header()?.get_i64(name)
    }

    /// `fits_read_key_dbl` / `ffgkyd`.
    pub fn read_key_dbl(&self, name: &str) -> Result<f64> {
        self.header()?.get_f64(name)
    }

    /// `fits_read_card` / `ffgcrd`.
    pub fn read_card(&self, name: &str) -> Result<Card> {
        self.header()?
            .card_by_name(name)
            .copied()
            .ok_or_else(|| crate::FitsError::new(crate::status::KEY_NO_EXIST))
    }

    /// `fits_read_record` / `ffgrec` (1-based). `n == 0` reads the next card.
    pub fn read_record(&mut self, n: usize) -> Result<Card> {
        let idx = if n == 0 { self.inner()?.nextkey } else { n };
        let card = self.header()?.record(idx)?;
        if let Ok(inner) = self.inner_mut() {
            inner.nextkey = idx + 1;
        }
        Ok(card)
    }

    /// `fits_read_key_unit` / `ffgunt`.
    pub fn read_key_unit(&self, name: &str) -> Result<String> {
        self.header()?.get_unit(name)
    }

    /// `fits_get_hdrspace` / `ffghsp`: (existing keys, remaining in current record).
    pub fn hdrspace(&self) -> Result<(usize, usize)> {
        let n = self.header()?.len();
        let used_in_rec = (n + 1) % 36; // +1 for END
        let more = if used_in_rec == 0 {
            0
        } else {
            36 - used_in_rec
        };
        Ok((n, more))
    }

    /// `fits_update_key_str` / `ffukys`.
    pub fn update_key_str(&mut self, name: &str, value: &str, comm: Option<&str>) -> Result<()> {
        let v = format_string_value(value);
        self.header_mut()?.update_with(name, &v, comm)
    }

    /// `fits_update_key_log` / `ffukyl`.
    pub fn update_key_log(&mut self, name: &str, value: bool, comm: Option<&str>) -> Result<()> {
        let v = if value { "T" } else { "F" };
        self.header_mut()?.update_with(name, v, comm)
    }

    /// `fits_update_key_lng` / `ffukyj`.
    pub fn update_key_lng(&mut self, name: &str, value: i64, comm: Option<&str>) -> Result<()> {
        self.header_mut()?
            .update_with(name, &value.to_string(), comm)
    }

    /// `fits_update_key_dbl` / `ffukyd`.
    pub fn update_key_dbl(
        &mut self,
        name: &str,
        value: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        let v = format_exp_double(value, decim);
        self.header_mut()?.update_with(name, &v, comm)
    }

    /// `fits_update_key_fixdbl` / `ffukyg`.
    pub fn update_key_fixdbl(
        &mut self,
        name: &str,
        value: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        let v = format_fixed_double(value, decim);
        self.header_mut()?.update_with(name, &v, comm)
    }

    /// `fits_update_key_null` / `ffukyu`.
    pub fn update_key_null(&mut self, name: &str, comm: Option<&str>) -> Result<()> {
        self.header_mut()?.update_with(name, " ", comm)
    }

    /// `fits_modify_key_str` / `ffmkys`.
    pub fn modify_key_str(&mut self, name: &str, value: &str, comm: Option<&str>) -> Result<()> {
        let v = format_string_value(value);
        self.header_mut()?.modify_with(name, &v, comm)
    }

    /// `fits_modify_key_lng` / `ffmkyj`.
    pub fn modify_key_lng(&mut self, name: &str, value: i64, comm: Option<&str>) -> Result<()> {
        self.header_mut()?
            .modify_with(name, &value.to_string(), comm)
    }

    /// `fits_insert_key_str` / `ffikys`. Inserts before 1-based `pos`.
    pub fn insert_key_str(
        &mut self,
        pos: usize,
        name: &str,
        value: &str,
        comm: Option<&str>,
    ) -> Result<()> {
        let v = format_string_value(value);
        let s = make_card_string(name, &v, comm)?;
        self.header_mut()?
            .insert_record_at(pos, Card::from_text(&s))
    }

    /// `fits_insert_record` / `ffirec`.
    pub fn insert_record(&mut self, pos: usize, card: &str) -> Result<()> {
        self.header_mut()?
            .insert_record_at(pos, Card::from_text(card))
    }

    /// `fits_delete_key` / `ffdkey`.
    pub fn delete_key(&mut self, name: &str) -> Result<()> {
        self.header_mut()?.delete_key(name)
    }

    /// `fits_delete_record` / `ffdrec`.
    pub fn delete_record(&mut self, n: usize) -> Result<()> {
        self.header_mut()?.delete_record(n)
    }

    /// `fits_read_imghdr` / `ffghpr`.
    pub fn read_imghdr(&self) -> Result<PrimaryInfo> {
        self.header()?.primary_info()
    }

    /// True if `name` is a DATE keyword whose value looks like a timestamp.
    pub fn is_date_keyword(&self, name: &str) -> bool {
        self.read_key_str(name)
            .map(|(v, _)| is_fits_date(&v))
            .unwrap_or(false)
    }

    /// `fits_read_keyword` / `ffgkey`: raw value string + comment.
    pub fn read_keyword(&mut self, name: &str) -> Result<(String, String)> {
        let (val, comm) = self.header()?.raw_value_comment(name)?;
        if let Some(n) = self.header()?.cards().iter().position(|c| {
            c.keyword_name()
                .map(|(nm, _)| nm.eq_ignore_ascii_case(name))
                .unwrap_or(false)
        }) {
            if let Ok(inner) = self.inner_mut() {
                inner.nextkey = n + 2;
            }
        }
        Ok((val, comm))
    }

    /// `fits_read_keyn` / `ffgkyn`.
    pub fn read_keyn(&mut self, n: usize) -> Result<(String, String, String)> {
        let card = self.header()?.record(n)?;
        let (name, _) = card.keyword_name()?;
        let (val, comm) = card.parse_value_comment()?;
        if let Ok(inner) = self.inner_mut() {
            inner.nextkey = n + 1;
        }
        Ok((name, val, comm))
    }

    /// `fits_modify_name` / `ffmnam`.
    pub fn modify_name(&mut self, old: &str, new: &str) -> Result<()> {
        let card = self.read_card(old)?;
        let mut c = card;
        c.set_keyword_name(new);
        self.header_mut()?.replace_name(old, c)
    }

    /// `fits_modify_comment` / `ffmcom`.
    pub fn modify_comment(&mut self, name: &str, comm: &str) -> Result<()> {
        let (val, _) = self.header()?.raw_value_comment(name)?;
        self.header_mut()?.modify_with(name, &val, Some(comm))
    }

    /// `fits_modify_record` / `ffmrec`.
    pub fn modify_record(&mut self, n: usize, card: &str) -> Result<()> {
        let idx = n.wrapping_sub(1);
        self.header_mut()?
            .replace_record_idx(idx, crate::card::Card::from_text(card))
    }

    /// `fits_update_card` / `ffucrd`.
    pub fn update_card(&mut self, name: &str, card: &str) -> Result<()> {
        if self.header()?.card_by_name(name).is_some() {
            let c = crate::card::Card::from_text(card);
            self.header_mut()?.replace_name(name, c)
        } else {
            self.write_record(card)
        }
    }

    /// `fits_read_keys_lng` / `ffgknj`.
    pub fn read_keys_lng(&self, root: &str, start: i32, nmax: usize) -> Result<(Vec<i64>, usize)> {
        Ok(self.header()?.get_keys_long(root, start, nmax))
    }

    /// `fits_read_keys_str` / `ffgkns`.
    pub fn read_keys_str(
        &self,
        root: &str,
        start: i32,
        nmax: usize,
    ) -> Result<(Vec<String>, usize)> {
        let mut out = Vec::new();
        for i in 0..nmax {
            let name = format!("{root}{}", start as usize + i);
            match self.header()?.get_string(&name) {
                Ok((v, _)) => out.push(v),
                Err(_) => break,
            }
        }
        let n = out.len();
        Ok((out, n))
    }

    /// `fits_write_keys_str` / `ffpkns`.
    pub fn write_keys_str(
        &mut self,
        root: &str,
        start: i32,
        values: &[&str],
        comm: Option<&str>,
    ) -> Result<()> {
        for (i, val) in values.iter().enumerate() {
            let name = format!("{root}{}", start as usize + i);
            self.write_key_str(&name, val, comm)?;
        }
        Ok(())
    }

    /// `fits_write_keys_lng` / `ffpknj`.
    pub fn write_keys_lng(
        &mut self,
        root: &str,
        start: i32,
        values: &[i64],
        comm: Option<&str>,
    ) -> Result<()> {
        for (i, val) in values.iter().enumerate() {
            let name = format!("{root}{}", start as usize + i);
            self.write_key_lng(&name, *val, comm)?;
        }
        Ok(())
    }

    /// `fits_write_key_lng` of `int.frac` (`fits_write_key_dblcomp` analogue / `ffpkyt`).
    pub fn write_key_time(
        &mut self,
        name: &str,
        intg: i64,
        frac: f64,
        comm: Option<&str>,
    ) -> Result<()> {
        let v = format!("{}.{}", intg, format_frac(frac));
        self.header_mut()?.update_with(name, &v, comm)
    }

    /// `fits_find_nextkey` / `ffgnxk`.
    pub fn find_next_key(
        &mut self,
        inclist: &[&str],
        exclist: &[&str],
    ) -> Result<crate::card::Card> {
        let start = self.inner()?.nextkey.max(1);
        let cards = self.inner()?.hdus[self.inner()?.current]
            .header
            .cards()
            .to_vec();
        for (i, card) in cards.iter().enumerate().skip(start.saturating_sub(1)) {
            let Ok((name, _)) = card.keyword_name() else {
                continue;
            };
            if matches_any(&name, inclist) && !matches_any(&name, exclist) {
                self.inner_mut()?.nextkey = i + 2;
                return Ok(*card);
            }
        }
        Err(crate::FitsError::new(crate::status::KEY_NO_EXIST))
    }

    /// `fits_write_keys_template` / `ffpktp`.
    pub fn write_keys_template(&mut self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let text = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            crate::FitsError::with_message(crate::status::FILE_NOT_OPENED, e.to_string())
        })?;
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let lower = line.to_ascii_lowercase();
            if lower == "end" {
                break;
            }
            if let Some(rest) = line.strip_prefix('-') {
                let mut parts = rest.split_whitespace();
                let old = parts.next().unwrap_or("");
                if let Some(new) = parts.next() {
                    let _ = self.modify_name(old, new);
                } else {
                    let _ = self.delete_key(old.trim());
                }
                continue;
            }
            if lower.starts_with("comment") {
                let comm = line[7..].trim_start();
                self.write_comment(comm)?;
                continue;
            }
            if lower.starts_with("history") {
                let hist = line[7..].trim_start();
                self.write_history(hist)?;
                continue;
            }
            self.write_record(line)?;
        }
        Ok(())
    }

    /// `fits_write_key_longstr` / `ffpkls`.
    pub fn write_key_longstr(&mut self, name: &str, value: &str, comm: Option<&str>) -> Result<()> {
        // Fits in one card: ordinary string.
        if value.len() <= 68 {
            return self.write_key_str(name, value, comm);
        }
        let mut rest = value;
        let first_n = 67usize.min(rest.len());
        let mut first = rest[..first_n].to_string();
        first.push('&');
        self.write_key_str(name, &first, comm)?;
        rest = &rest[first_n..];
        while !rest.is_empty() {
            let n = 67usize.min(rest.len());
            let mut chunk = rest[..n].to_string();
            rest = &rest[n..];
            if !rest.is_empty() {
                chunk.push('&');
            }
            let card = format!("CONTINUE  '{}'", chunk.replace('\'', "''"));
            self.write_record(&card)?;
        }
        Ok(())
    }

    /// `fits_write_key_longwarn` / `ffplsw`.
    pub fn write_key_longwarn(&mut self) -> Result<()> {
        self.write_key_str(
            "LONGSTRN",
            "OGIP 1.0",
            Some("The OGIP long string convention may be used."),
        )
    }

    /// `fits_read_key_longstr` / `ffgkls`.
    pub fn read_key_longstr(&mut self, name: &str) -> Result<(String, String)> {
        let (val, comm) = self.read_key_str(name)?;
        let mut out = val;
        if out.ends_with('&') {
            out.pop();
            let cards = self.header()?.cards().to_vec();
            let start = cards.iter().position(|c| {
                c.keyword_name()
                    .map(|(n, _)| n.eq_ignore_ascii_case(name))
                    .unwrap_or(false)
            });
            if let Some(i) = start {
                for c in cards.iter().skip(i + 1) {
                    let text = c.as_str().unwrap_or("");
                    if !text.starts_with("CONTINUE") {
                        break;
                    }
                    if let Ok((v, _)) = c.parse_value_comment() {
                        let s = crate::card::unquote_fits_string(&v);
                        let amp = s.ends_with('&');
                        out.push_str(s.trim_end_matches('&'));
                        if !amp {
                            break;
                        }
                    }
                }
            }
        }
        Ok((out, comm))
    }
}

fn matches_any(name: &str, pats: &[&str]) -> bool {
    if pats.is_empty() {
        return false;
    }
    pats.iter().any(|p| glob_match(p, name))
}

fn glob_match(pat: &str, name: &str) -> bool {
    let p = pat.to_ascii_uppercase();
    let n = name.to_ascii_uppercase();
    let mut pi = 0;
    let pb = p.as_bytes();
    let nb = n.as_bytes();
    let mut ni = 0;
    let mut star = None;
    while ni < nb.len() {
        if pi < pb.len() && (pb[pi] == b'?' || pb[pi] == nb[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < pb.len() && pb[pi] == b'*' {
            star = Some((pi, ni));
            pi += 1;
        } else if let Some((sp, sn)) = star {
            pi = sp + 1;
            ni = sn + 1;
            star = Some((sp, ni));
        } else {
            return false;
        }
    }
    while pi < pb.len() && pb[pi] == b'*' {
        pi += 1;
    }
    pi == pb.len()
}

fn format_frac(frac: f64) -> String {
    let s = format!("{frac:.16}");
    s.trim_start_matches("0.").trim_end_matches('0').to_string()
}

/// `fits_write_key_str`.
pub fn fits_write_key_str(
    f: &mut FitsFile,
    name: &str,
    value: &str,
    comm: Option<&str>,
) -> Result<()> {
    f.write_key_str(name, value, comm)
}

/// `fits_read_key_str`.
pub fn fits_read_key_str(f: &FitsFile, name: &str) -> Result<(String, String)> {
    f.read_key_str(name)
}

/// `fits_write_date`.
pub fn fits_write_date(f: &mut FitsFile) -> Result<()> {
    f.write_date()
}
