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

    /// `fits_read_record` / `ffgrec` (1-based).
    pub fn read_record(&self, n: usize) -> Result<Card> {
        self.header()?.record(n)
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
