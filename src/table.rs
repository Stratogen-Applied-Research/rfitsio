//! ASCII table HDU create / column I/O / row-column edit.

use crate::convert::pad_data_len;
use crate::error::{FitsError, Result};
use crate::file::FitsFile;
use crate::hdu::Hdu;
use crate::header::{COMM_TBCOL_INSERTED, COMM_TFORM_INSERTED, COMM_TTYPE_INSERTED, Header};
use crate::io::{self, Driver};
use crate::status::{
    BAD_ATABLE_FORMAT, BAD_COL_NUM, BAD_ELEM_NUM, BAD_HDU_NUM, BAD_ROW_NUM, BAD_TFIELDS,
    COL_NOT_FOUND, COL_NOT_UNIQUE, HEADER_NOT_EMPTY, NEG_BYTES, NOT_ASCII_COL, NOT_TABLE,
    NUM_OVERFLOW, ZERO_SCALE,
};
use crate::tform::{
    AsciiKind, AsciiTform, field_is_null, format_ascii_number, format_ascii_string,
    parse_ascii_number, parse_ascii_tform, trim_ascii_field,
};
use crate::types::HduType;

/// One ASCII-table column, derived from the current header.
#[derive(Debug, Clone)]
struct AsciiCol {
    tbcol: i64,
    tform: AsciiTform,
    tscal: f64,
    tzero: f64,
    tnull: String,
}

impl FitsFile {
    /// `fits_create_tbl` / `ffcrtb` for [`HduType::AsciiTable`].
    ///
    /// Always appends an extension (a TABLE cannot replace the primary array).
    pub fn create_tbl(
        &mut self,
        kind: HduType,
        nrows: i64,
        ttype: &[&str],
        tform: &[&str],
        tunit: &[Option<&str>],
        extname: Option<&str>,
    ) -> Result<()> {
        match kind {
            HduType::AsciiTable => self.create_ascii_table(nrows, ttype, tform, tunit, extname),
            HduType::BinaryTable => self.create_binary_table(nrows, ttype, tform, tunit, extname),
            HduType::Image => Err(FitsError::new(NOT_TABLE)),
        }
    }

    /// `fits_create_tbl` with `ASCII_TBL`.
    pub fn create_ascii_table(
        &mut self,
        nrows: i64,
        ttype: &[&str],
        tform: &[&str],
        tunit: &[Option<&str>],
        extname: Option<&str>,
    ) -> Result<()> {
        self.require_write()?;
        self.flush()?;
        if tform.len() > 999 {
            return Err(FitsError::new(BAD_TFIELDS));
        }
        let inner = self.inner()?;
        let last = inner
            .hdus
            .last()
            .ok_or_else(|| FitsError::new(BAD_HDU_NUM))?;
        let end = last.data_start + last.data_unit_len()?;
        let header = Header::ascii_table(nrows, ttype, tform, tunit, extname)?;
        let header_bytes = header.to_record_bytes();
        let data_start = end + header_bytes.len() as u64;
        let data_len = header.get_i64("NAXIS1")?.max(0) as u64 * nrows.max(0) as u64;
        let padded = pad_data_len(data_len);
        let inner = self.inner_mut()?;
        inner.io.write_at(end, &header_bytes)?;
        if padded > 0 {
            io::write_fill(&mut inner.io, data_start, padded, b' ')?;
        }
        inner.io.truncate(data_start + padded)?;
        inner.io.flush()?;
        inner.hdus.push(Hdu {
            hdu_type: HduType::AsciiTable,
            header,
            header_start: end,
            data_start,
        });
        inner.current = inner.hdus.len() - 1;
        inner.dirty = false;
        Ok(())
    }

    /// `fits_read_atblhdr` / `ffghtb`.
    pub fn ascii_table_info(&self) -> Result<crate::header::AsciiTableInfo> {
        let inner = self.inner()?;
        inner.hdus[inner.current].header.ascii_table_info()
    }

    /// NAXIS2 of the current table.
    pub fn nrows(&self) -> Result<i64> {
        self.require_table()?;
        Ok(self.inner()?.hdus[self.inner()?.current]
            .header
            .get_i64("NAXIS2")
            .unwrap_or(0))
    }

    /// TFIELDS of the current table.
    pub fn ncols(&self) -> Result<i32> {
        self.require_table()?;
        Ok(self.inner()?.hdus[self.inner()?.current]
            .header
            .get_i64("TFIELDS")
            .unwrap_or(0) as i32)
    }

    /// `fits_get_colnum` / `ffgcno`. `casesen` matches CFITSIO (`1` = case sensitive).
    pub fn get_colnum(&self, casesen: bool, templt: &str) -> Result<i32> {
        self.require_table()?;
        let n = self.ncols()?;
        let mut matches = Vec::new();
        for i in 1..=n {
            let name = self
                .header()?
                .get_string(&format!("TTYPE{i}"))
                .map(|(v, _)| v)
                .unwrap_or_default();
            let ok = if templt.contains('*') || templt.contains('?') {
                col_glob(templt, &name, casesen)
            } else if casesen {
                name == templt
            } else {
                name.eq_ignore_ascii_case(templt)
            };
            if ok {
                matches.push(i);
            }
        }
        match matches.as_slice() {
            [one] => Ok(*one),
            [] => Err(FitsError::new(COL_NOT_FOUND)),
            _ => Err(FitsError::new(COL_NOT_UNIQUE)),
        }
    }

    /// `fits_read_tblbytes` / `ffgtbb`.
    pub fn read_tblbytes(
        &mut self,
        firstrow: i64,
        firstchar: i64,
        nchars: usize,
    ) -> Result<Vec<u8>> {
        self.require_table()?;
        let (data_start, rowlen) = self.table_geom()?;
        let pos = data_start + (firstrow - 1) as u64 * rowlen + (firstchar - 1) as u64;
        let mut buf = vec![0u8; nchars];
        let n = self.inner_mut()?.io.read_at(pos, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    /// `fits_write_tblbytes` / `ffptbb`.
    pub fn write_tblbytes(&mut self, firstrow: i64, firstchar: i64, bytes: &[u8]) -> Result<()> {
        self.require_write()?;
        self.require_table()?;
        let (data_start, rowlen) = self.table_geom()?;
        let pos = data_start + (firstrow - 1) as u64 * rowlen + (firstchar - 1) as u64;
        self.inner_mut()?.io.write_at(pos, bytes)?;
        Ok(())
    }

    /// Write string values to column `colnum` starting at 1-based `firstrow`.
    pub fn write_col_str(&mut self, colnum: i32, firstrow: i64, values: &[&str]) -> Result<()> {
        if self.hdu_type()? == HduType::BinaryTable {
            return self.write_bin_col_str(colnum, firstrow, values);
        }
        self.require_write()?;
        if values.is_empty() {
            return Ok(());
        }
        let col = self.ascii_col(colnum)?;
        if col.tform.kind != AsciiKind::A {
            return Err(FitsError::new(NOT_ASCII_COL));
        }
        self.ensure_rows(firstrow, values.len() as i64)?;
        let (data_start, rowlen) = self.table_geom()?;
        let width = col.tform.width;
        let off = (col.tbcol - 1) as u64;
        for (i, s) in values.iter().enumerate() {
            let pos = data_start + ((firstrow - 1 + i as i64) as u64) * rowlen + off;
            let buf = format_ascii_string(s, width);
            self.inner_mut()?.io.write_at(pos, &buf)?;
        }
        Ok(())
    }

    /// Write integer values; formatted with the column TFORM (`I`/`F`/`E`/`D`).
    ///
    /// Binary vector columns use CFITSIO flattened-element order (`firstelem = 1`).
    pub fn write_col_i64(&mut self, colnum: i32, firstrow: i64, values: &[i64]) -> Result<()> {
        self.write_col_i64_elem(colnum, firstrow, 1, values)
    }

    /// `fits_write_col` with explicit 1-based `firstelem` in a vector cell.
    pub fn write_col_i64_elem(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[i64],
    ) -> Result<()> {
        if self.inner()?.hdus[self.inner()?.current].hdu_type == HduType::BinaryTable {
            return self.write_bin_col_i64_at(colnum, firstrow, firstelem, values);
        }
        if firstelem != 1 {
            return Err(FitsError::new(BAD_ELEM_NUM));
        }
        self.write_col_numbers(colnum, firstrow, values.iter().map(|&v| v as f64))
    }

    /// Write unsigned 64-bit values (binary `K`/`W` columns).
    pub fn write_col_u64(&mut self, colnum: i32, firstrow: i64, values: &[u64]) -> Result<()> {
        self.write_col_u64_elem(colnum, firstrow, 1, values)
    }

    /// Write unsigned 64-bit values starting at 1-based `firstelem`.
    pub fn write_col_u64_elem(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[u64],
    ) -> Result<()> {
        if self.inner()?.hdus[self.inner()?.current].hdu_type == HduType::BinaryTable {
            return self.write_bin_col_u64_at(colnum, firstrow, firstelem, values);
        }
        self.write_col_i64_elem(
            colnum,
            firstrow,
            firstelem,
            &values.iter().map(|&v| v as i64).collect::<Vec<_>>(),
        )
    }

    /// Write `f32` values.
    pub fn write_col_f32(&mut self, colnum: i32, firstrow: i64, values: &[f32]) -> Result<()> {
        self.write_col_f32_elem(colnum, firstrow, 1, values)
    }

    /// Write `f32` values starting at 1-based `firstelem`.
    pub fn write_col_f32_elem(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[f32],
    ) -> Result<()> {
        if self.inner()?.hdus[self.inner()?.current].hdu_type == HduType::BinaryTable {
            return self.write_bin_col_f32_at(colnum, firstrow, firstelem, values);
        }
        if firstelem != 1 {
            return Err(FitsError::new(BAD_ELEM_NUM));
        }
        self.write_col_numbers(colnum, firstrow, values.iter().map(|&v| f64::from(v)))
    }

    /// Write `f64` values.
    pub fn write_col_f64(&mut self, colnum: i32, firstrow: i64, values: &[f64]) -> Result<()> {
        self.write_col_f64_elem(colnum, firstrow, 1, values)
    }

    /// Write `f64` values starting at 1-based `firstelem`.
    pub fn write_col_f64_elem(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[f64],
    ) -> Result<()> {
        if self.inner()?.hdus[self.inner()?.current].hdu_type == HduType::BinaryTable {
            return self.write_bin_col_f64_at(colnum, firstrow, firstelem, values);
        }
        if firstelem != 1 {
            return Err(FitsError::new(BAD_ELEM_NUM));
        }
        self.write_col_numbers(colnum, firstrow, values.iter().copied())
    }

    /// Read strings, trimming trailing blanks. `nulval = None` skips null checks.
    pub fn read_col_str(
        &mut self,
        colnum: i32,
        firstrow: i64,
        nelem: usize,
        nulval: Option<&str>,
    ) -> Result<(Vec<String>, bool)> {
        if self.hdu_type()? == HduType::BinaryTable {
            return self.read_bin_col_str(colnum, firstrow, nelem, nulval);
        }
        if nelem == 0 {
            return Ok((Vec::new(), false));
        }
        let col = self.ascii_col(colnum)?;
        if col.tform.kind != AsciiKind::A {
            return Err(FitsError::new(NOT_ASCII_COL));
        }
        self.check_read_rows(firstrow, nelem as i64)?;
        let (data_start, rowlen) = self.table_geom()?;
        let width = col.tform.width;
        let off = (col.tbcol - 1) as u64;
        let check = nulval.is_some();
        let mut out = Vec::with_capacity(nelem);
        let mut anynul = false;
        for i in 0..nelem {
            let pos = data_start + ((firstrow - 1 + i as i64) as u64) * rowlen + off;
            let mut buf = vec![0u8; width];
            let n = self.inner_mut()?.io.read_at(pos, &mut buf)?;
            if n < width {
                return Err(FitsError::new(crate::status::END_OF_FILE));
            }
            if check && field_is_null(&buf, &col.tnull) {
                anynul = true;
                out.push(nulval.unwrap_or("").to_string());
            } else {
                out.push(trim_ascii_field(&buf));
            }
        }
        Ok((out, anynul))
    }

    /// Read integers. `nulval = None` skips null substitution (CFITSIO `nulval == 0`).
    pub fn read_col_i64(
        &mut self,
        colnum: i32,
        firstrow: i64,
        nelem: usize,
        nulval: Option<i64>,
    ) -> Result<(Vec<i64>, bool)> {
        self.read_col_i64_elem(colnum, firstrow, 1, nelem, nulval)
    }

    /// Read integers starting at 1-based `firstelem` in a vector cell.
    pub fn read_col_i64_elem(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        nelem: usize,
        nulval: Option<i64>,
    ) -> Result<(Vec<i64>, bool)> {
        if self.inner()?.hdus[self.inner()?.current].hdu_type == HduType::BinaryTable {
            return self.read_bin_col_i64_at(colnum, firstrow, firstelem, nelem, nulval);
        }
        if firstelem != 1 {
            return Err(FitsError::new(BAD_ELEM_NUM));
        }
        let (vals, anynul) =
            self.read_col_f64(colnum, firstrow, nelem, nulval.map(|v| v as f64))?;
        let mut out = Vec::with_capacity(vals.len());
        for v in vals {
            let r = v.round();
            if r < i64::MIN as f64 || r > i64::MAX as f64 {
                return Err(FitsError::new(NUM_OVERFLOW));
            }
            out.push(r as i64);
        }
        Ok((out, anynul))
    }

    /// Read `f32` values.
    pub fn read_col_f32(
        &mut self,
        colnum: i32,
        firstrow: i64,
        nelem: usize,
        nulval: Option<f32>,
    ) -> Result<(Vec<f32>, bool)> {
        self.read_col_f32_elem(colnum, firstrow, 1, nelem, nulval)
    }

    /// Read `f32` values starting at 1-based `firstelem`.
    pub fn read_col_f32_elem(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        nelem: usize,
        nulval: Option<f32>,
    ) -> Result<(Vec<f32>, bool)> {
        let (vals, anynul) =
            self.read_col_f64_elem(colnum, firstrow, firstelem, nelem, nulval.map(f64::from))?;
        Ok((vals.into_iter().map(|v| v as f32).collect(), anynul))
    }

    /// Read `f64` values, applying TSCAL/TZERO.
    pub fn read_col_f64(
        &mut self,
        colnum: i32,
        firstrow: i64,
        nelem: usize,
        nulval: Option<f64>,
    ) -> Result<(Vec<f64>, bool)> {
        self.read_col_f64_elem(colnum, firstrow, 1, nelem, nulval)
    }

    /// Read `f64` values starting at 1-based `firstelem` in a vector cell.
    pub fn read_col_f64_elem(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        nelem: usize,
        nulval: Option<f64>,
    ) -> Result<(Vec<f64>, bool)> {
        if self.inner()?.hdus[self.inner()?.current].hdu_type == HduType::BinaryTable {
            return self.read_bin_col_f64_at(colnum, firstrow, firstelem, nelem, nulval);
        }
        if firstelem != 1 {
            return Err(FitsError::new(BAD_ELEM_NUM));
        }
        if nelem == 0 {
            return Ok((Vec::new(), false));
        }
        let col = self.ascii_col(colnum)?;
        if col.tform.kind == AsciiKind::A {
            return Err(FitsError::new(BAD_ATABLE_FORMAT));
        }
        if col.tscal == 0.0 {
            return Err(FitsError::new(ZERO_SCALE));
        }
        self.check_read_rows(firstrow, nelem as i64)?;
        let (data_start, rowlen) = self.table_geom()?;
        let width = col.tform.width;
        let off = (col.tbcol - 1) as u64;
        let implied = col.tform.decimals;
        let check = nulval.is_some();
        let mut out = Vec::with_capacity(nelem);
        let mut anynul = false;
        for i in 0..nelem {
            let pos = data_start + ((firstrow - 1 + i as i64) as u64) * rowlen + off;
            let mut buf = vec![0u8; width];
            let n = self.inner_mut()?.io.read_at(pos, &mut buf)?;
            if n < width {
                return Err(FitsError::new(crate::status::END_OF_FILE));
            }
            if check && field_is_null(&buf, &col.tnull) {
                anynul = true;
                out.push(nulval.unwrap_or(0.0));
                continue;
            }
            let raw = parse_ascii_number(&buf, implied)?;
            out.push(raw * col.tscal + col.tzero);
        }
        Ok((out, anynul))
    }

    /// Write the column's null string into `nelem` rows (`fits_write_col_null` / `ffpclu`).
    pub fn write_col_null(&mut self, colnum: i32, firstrow: i64, nelem: i64) -> Result<()> {
        if self.hdu_type()? == HduType::BinaryTable {
            return self.write_bin_col_null(colnum, firstrow, nelem);
        }
        self.require_write()?;
        if nelem < 0 {
            return Err(FitsError::new(NEG_BYTES));
        }
        if nelem == 0 {
            return Ok(());
        }
        let col = self.ascii_col(colnum)?;
        self.ensure_rows(firstrow, nelem)?;
        let (data_start, rowlen) = self.table_geom()?;
        let width = col.tform.width;
        let mut buf = vec![b' '; width];
        let nb = col.tnull.as_bytes();
        let n = nb.len().min(width);
        buf[..n].copy_from_slice(&nb[..n]);
        let off = (col.tbcol - 1) as u64;
        for i in 0..nelem {
            let pos = data_start + ((firstrow - 1 + i) as u64) * rowlen + off;
            self.inner_mut()?.io.write_at(pos, &buf)?;
        }
        Ok(())
    }

    /// Insert `nrows` blank rows after 1-based `firstrow` (`firstrow = 0` inserts at the start).
    pub fn insert_rows(&mut self, firstrow: i64, nrows: i64) -> Result<()> {
        self.require_write()?;
        self.require_table()?;
        self.require_last_hdu()?;
        if nrows < 0 {
            return Err(FitsError::new(NEG_BYTES));
        }
        if nrows == 0 {
            return Ok(());
        }
        if firstrow < 0 {
            return Err(FitsError::new(BAD_ROW_NUM));
        }
        let naxis2 = self.nrows()?;
        if firstrow > naxis2 {
            return Err(FitsError::new(BAD_ROW_NUM));
        }
        let (data_start, rowlen) = self.table_geom()?;
        let nshift = rowlen * nrows as u64;
        let from = data_start + firstrow as u64 * rowlen;
        let fill = if self.hdu_type()? == HduType::AsciiTable {
            b' '
        } else {
            0
        };
        shift_tail(&mut self.inner_mut()?.io, from, nshift as i64, fill)?;
        {
            let inner = self.inner_mut()?;
            inner.hdus[inner.current]
                .header
                .update_long_keep_comment("NAXIS2", naxis2 + nrows)?;
            inner.dirty = true;
        }
        self.flush()?;
        Ok(())
    }

    /// Delete `nrows` rows starting at 1-based `firstrow`.
    pub fn delete_rows(&mut self, firstrow: i64, nrows: i64) -> Result<()> {
        self.require_write()?;
        self.require_table()?;
        self.require_last_hdu()?;
        if nrows < 0 {
            return Err(FitsError::new(NEG_BYTES));
        }
        if nrows == 0 {
            return Ok(());
        }
        let naxis2 = self.nrows()?;
        if firstrow < 1 || firstrow > naxis2 || firstrow + nrows - 1 > naxis2 {
            return Err(FitsError::new(BAD_ROW_NUM));
        }
        let (data_start, rowlen) = self.table_geom()?;
        let from = data_start + (firstrow - 1 + nrows) as u64 * rowlen;
        let nshift = -(rowlen as i64 * nrows);
        shift_tail(&mut self.inner_mut()?.io, from, nshift, b' ')?;
        {
            let inner = self.inner_mut()?;
            inner.hdus[inner.current]
                .header
                .update_long_keep_comment("NAXIS2", naxis2 - nrows)?;
            inner.dirty = true;
        }
        self.flush()?;
        Ok(())
    }

    /// Insert a column at 1-based `numcol` (`fits_insert_col` / `fficol`).
    pub fn insert_col(&mut self, numcol: i32, ttype: &str, tform: &str) -> Result<()> {
        self.require_write()?;
        self.require_table()?;
        if self.hdu_type()? == HduType::BinaryTable {
            return self.insert_bin_col(numcol, ttype, tform);
        }
        self.require_last_hdu()?;
        let tfields = self.ncols()?;
        if numcol < 1 {
            return Err(FitsError::new(BAD_COL_NUM));
        }
        let colnum = if numcol > tfields {
            tfields + 1
        } else {
            numcol
        };
        let parsed = parse_ascii_tform(tform)?;
        let delbyte = parsed.width as i64 + 1;
        let info = self.ascii_table_info()?;
        let naxis1 = info.rowlen;
        let naxis2 = info.nrows;
        let firstcol_0 = if colnum > tfields {
            naxis1
        } else {
            info.tbcol[(colnum - 1) as usize] - 1
        };
        let (data_start, _) = self.table_geom()?;
        if naxis2 > 0 && naxis1 >= 0 {
            insert_in_rows(
                &mut self.inner_mut()?.io,
                data_start,
                naxis1 as usize,
                naxis2 as usize,
                firstcol_0 as usize,
                delbyte as usize,
                b' ',
            )?;
        }
        {
            let inner = self.inner_mut()?;
            let header = &mut inner.hdus[inner.current].header;
            for ii in 0..tfields {
                let name = format!("TBCOL{}", ii + 1);
                if let Ok(tb) = header.get_i64(&name) {
                    if tb > firstcol_0 {
                        header.update_long_keep_comment(&name, tb + delbyte)?;
                    }
                }
            }
            header.update_long_keep_comment("TFIELDS", i64::from(tfields) + 1)?;
            header.update_long_keep_comment("NAXIS1", naxis1 + delbyte)?;
            if colnum <= tfields {
                header.shift_table_col_keys(colnum, tfields, 1)?;
            }
            let tfm = tform.to_ascii_uppercase();
            header.write_string(&format!("TTYPE{colnum}"), ttype, Some(COMM_TTYPE_INSERTED))?;
            header.write_string(&format!("TFORM{colnum}"), &tfm, Some(COMM_TFORM_INSERTED))?;
            let tbcol = if colnum == tfields + 1 {
                firstcol_0 + 2
            } else {
                firstcol_0 + 1
            };
            header.write_long(&format!("TBCOL{colnum}"), tbcol, Some(COMM_TBCOL_INSERTED))?;
            inner.dirty = true;
        }
        self.flush()?;
        Ok(())
    }

    /// Delete 1-based column `colnum` (`fits_delete_col` / `ffdcol`).
    pub fn delete_col(&mut self, colnum: i32) -> Result<()> {
        self.require_write()?;
        self.require_table()?;
        self.require_last_hdu()?;
        let info = self.ascii_table_info()?;
        if colnum < 1 || colnum > info.tfields {
            return Err(FitsError::new(BAD_COL_NUM));
        }
        let idx = (colnum - 1) as usize;
        let parsed = parse_ascii_tform(&info.tform[idx])?;
        let mut delbyte = parsed.width as i64;
        let mut firstcol_0 = info.tbcol[idx] - 1;
        if colnum < info.tfields {
            let nspace = info.tbcol[idx + 1] - info.tbcol[idx] - parsed.width as i64;
            if nspace > 0 {
                delbyte += 1;
            }
        } else if colnum > 1 {
            let prev_w = parse_ascii_tform(&info.tform[idx - 1])?.width as i64;
            let nspace = info.tbcol[idx] - info.tbcol[idx - 1] - prev_w;
            if nspace > 0 {
                delbyte += 1;
                firstcol_0 -= 1;
            }
        }
        let (data_start, _) = self.table_geom()?;
        if info.nrows > 0 && info.rowlen > 0 {
            delete_in_rows(
                &mut self.inner_mut()?.io,
                data_start,
                info.rowlen as usize,
                info.nrows as usize,
                firstcol_0 as usize,
                delbyte as usize,
            )?;
        }
        {
            let inner = self.inner_mut()?;
            let header = &mut inner.hdus[inner.current].header;
            for ii in 1..=info.tfields {
                let name = format!("TBCOL{ii}");
                if let Ok(tb) = header.get_i64(&name) {
                    if tb > firstcol_0 {
                        header.update_long_keep_comment(&name, tb - delbyte)?;
                    }
                }
            }
            header.update_long_keep_comment("TFIELDS", i64::from(info.tfields) - 1)?;
            header.update_long_keep_comment("NAXIS1", info.rowlen - delbyte)?;
            header.shift_table_col_keys(colnum, info.tfields, -1)?;
            inner.dirty = true;
        }
        self.flush()?;
        Ok(())
    }

    fn write_col_numbers<I>(&mut self, colnum: i32, firstrow: i64, values: I) -> Result<()>
    where
        I: IntoIterator<Item = f64>,
    {
        self.require_write()?;
        let values: Vec<f64> = values.into_iter().collect();
        if values.is_empty() {
            return Ok(());
        }
        let col = self.ascii_col(colnum)?;
        if col.tform.kind == AsciiKind::A {
            return Err(FitsError::new(BAD_ATABLE_FORMAT));
        }
        if col.tscal == 0.0 {
            return Err(FitsError::new(ZERO_SCALE));
        }
        self.ensure_rows(firstrow, values.len() as i64)?;
        let (data_start, rowlen) = self.table_geom()?;
        let off = (col.tbcol - 1) as u64;
        for (i, v) in values.iter().enumerate() {
            let stored = (*v - col.tzero) / col.tscal;
            let buf = format_ascii_number(stored, &col.tform)?;
            let pos = data_start + ((firstrow - 1 + i as i64) as u64) * rowlen + off;
            self.inner_mut()?.io.write_at(pos, &buf)?;
        }
        Ok(())
    }

    fn ascii_col(&self, colnum: i32) -> Result<AsciiCol> {
        self.require_table()?;
        let inner = self.inner()?;
        let header = &inner.hdus[inner.current].header;
        let tfields = header.get_i64("TFIELDS").unwrap_or(0) as i32;
        if colnum < 1 || colnum > tfields {
            return Err(FitsError::new(BAD_COL_NUM));
        }
        let tform_raw = header
            .get_string(&format!("TFORM{colnum}"))
            .map(|(v, _)| v)
            .map_err(|_| FitsError::new(crate::status::NO_TFORM))?;
        let tform = AsciiTform::parse(&tform_raw)?;
        let tbcol = header
            .get_i64(&format!("TBCOL{colnum}"))
            .map_err(|_| FitsError::new(crate::status::NO_TBCOL))?;
        let tscal = header.get_f64(&format!("TSCAL{colnum}")).unwrap_or(1.0);
        let tzero = header.get_f64(&format!("TZERO{colnum}")).unwrap_or(0.0);
        let tnull = match header.get_string(&format!("TNULL{colnum}")) {
            Ok((v, _)) => v,
            Err(_) => {
                let n = tform.width.min(17);
                " ".repeat(n)
            }
        };
        Ok(AsciiCol {
            tbcol,
            tform,
            tscal,
            tzero,
            tnull,
        })
    }

    pub(crate) fn table_geom(&self) -> Result<(u64, u64)> {
        let inner = self.inner()?;
        let hdu = &inner.hdus[inner.current];
        let rowlen = hdu.header.get_i64("NAXIS1")?.max(0) as u64;
        Ok((hdu.data_start, rowlen))
    }

    pub(crate) fn require_table(&self) -> Result<()> {
        let inner = self.inner()?;
        match inner.hdus[inner.current].hdu_type {
            HduType::AsciiTable | HduType::BinaryTable => Ok(()),
            HduType::Image => Err(FitsError::new(NOT_TABLE)),
        }
    }

    pub(crate) fn require_last_hdu(&self) -> Result<()> {
        let inner = self.inner()?;
        if inner.current + 1 != inner.hdus.len() {
            return Err(FitsError::with_message(
                HEADER_NOT_EMPTY,
                "row/column edit only supports the last HDU",
            ));
        }
        Ok(())
    }

    pub(crate) fn check_read_rows(&self, firstrow: i64, nelem: i64) -> Result<()> {
        if firstrow < 1 {
            return Err(FitsError::new(BAD_ROW_NUM));
        }
        let naxis2 = self.nrows()?;
        if firstrow - 1 + nelem > naxis2 {
            return Err(FitsError::new(BAD_ROW_NUM));
        }
        Ok(())
    }

    pub(crate) fn ensure_rows(&mut self, firstrow: i64, nelem: i64) -> Result<()> {
        if firstrow < 1 {
            return Err(FitsError::new(BAD_ROW_NUM));
        }
        let endrow = firstrow - 1 + nelem;
        let naxis2 = self.nrows()?;
        if endrow <= naxis2 {
            return Ok(());
        }
        self.require_last_hdu()?;
        let extra = endrow - naxis2;
        self.insert_rows(naxis2, extra)
    }
}

fn shift_tail(io: &mut dyn Driver, from: u64, nshift: i64, fill: u8) -> Result<()> {
    let len = io.len()?;
    if from > len {
        return Ok(());
    }
    let tail_len = (len - from) as usize;
    let mut tail = vec![0u8; tail_len];
    if tail_len > 0 {
        let n = io.read_at(from, &mut tail)?;
        tail.truncate(n);
    }
    if nshift > 0 {
        let dest = from + nshift as u64;
        if !tail.is_empty() {
            io.write_at(dest, &tail)?;
        }
        io::write_fill(io, from, nshift as u64, fill)?;
    } else if nshift < 0 {
        let dest = from.saturating_sub((-nshift) as u64);
        if !tail.is_empty() {
            io.write_at(dest, &tail)?;
        }
    }
    Ok(())
}

pub(crate) fn insert_in_rows(
    io: &mut dyn Driver,
    data_start: u64,
    rowlen: usize,
    nrows: usize,
    at: usize,
    nbytes: usize,
    fill: u8,
) -> Result<()> {
    if nrows == 0 || nbytes == 0 {
        return Ok(());
    }
    let old_len = rowlen * nrows;
    let mut data = vec![0u8; old_len];
    let n = io.read_at(data_start, &mut data)?;
    data.truncate(n);
    data.resize(old_len, fill);
    let newlen = rowlen + nbytes;
    let mut out = vec![fill; newlen * nrows];
    for r in 0..nrows {
        let src = &data[r * rowlen..(r + 1) * rowlen];
        let dst = &mut out[r * newlen..(r + 1) * newlen];
        dst[..at].copy_from_slice(&src[..at]);
        dst[at + nbytes..].copy_from_slice(&src[at..]);
    }
    io.write_at(data_start, &out)?;
    Ok(())
}

fn delete_in_rows(
    io: &mut dyn Driver,
    data_start: u64,
    rowlen: usize,
    nrows: usize,
    at: usize,
    nbytes: usize,
) -> Result<()> {
    if nrows == 0 || nbytes == 0 {
        return Ok(());
    }
    let old_len = rowlen * nrows;
    let mut data = vec![0u8; old_len];
    let n = io.read_at(data_start, &mut data)?;
    data.truncate(n);
    data.resize(old_len, b' ');
    let newlen = rowlen - nbytes;
    let mut out = vec![b' '; newlen * nrows];
    for r in 0..nrows {
        let src = &data[r * rowlen..(r + 1) * rowlen];
        let dst = &mut out[r * newlen..(r + 1) * newlen];
        dst[..at].copy_from_slice(&src[..at]);
        dst[at..].copy_from_slice(&src[at + nbytes..]);
    }
    io.write_at(data_start, &out)?;
    Ok(())
}

fn col_glob(pat: &str, name: &str, casesen: bool) -> bool {
    let (p, n) = if casesen {
        (pat.to_string(), name.to_string())
    } else {
        (pat.to_ascii_uppercase(), name.to_ascii_uppercase())
    };
    let pb = p.as_bytes();
    let nb = n.as_bytes();
    let mut pi = 0;
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

/// `fits_create_tbl` for an ASCII table.
pub fn fits_create_tbl(
    f: &mut FitsFile,
    kind: HduType,
    nrows: i64,
    ttype: &[&str],
    tform: &[&str],
    tunit: &[Option<&str>],
    extname: Option<&str>,
) -> Result<()> {
    f.create_tbl(kind, nrows, ttype, tform, tunit, extname)
}

/// `fits_movabs_hdu`.
pub fn fits_movabs_hdu(f: &mut FitsFile, hdunum: usize) -> Result<HduType> {
    f.movabs_hdu(hdunum)
}
