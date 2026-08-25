//! Binary table create / column I/O / variable-length heap.

use crate::convert::pad_data_len;
use crate::error::{FitsError, Result};
use crate::file::FitsFile;
use crate::hdu::Hdu;
use crate::header::Header;
use crate::io::{self, Driver};
use crate::status::{
    BAD_COL_NUM, BAD_ELEM_NUM, BAD_HDU_NUM, BAD_ROW_NUM, BAD_TFIELDS, END_OF_FILE, NEG_BYTES,
    NOT_LOGICAL_COL, NUM_OVERFLOW, ZERO_SCALE,
};
use crate::tform::{BinaryKind, BinaryTform, VariableKind, parse_binary_tform};
use crate::types::HduType;

#[derive(Debug, Clone)]
struct BinCol {
    offset: u64,
    tform: BinaryTform,
    tscal: f64,
    tzero: f64,
    tnull: Option<i64>,
}

impl FitsFile {
    /// `fits_create_tbl` with `BINARY_TBL`.
    pub fn create_binary_table(
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
        let header = Header::binary_table(nrows, ttype, tform, tunit, extname)?;
        let header_bytes = header.to_record_bytes();
        let data_start = end + header_bytes.len() as u64;
        let data_len = header.get_i64("NAXIS1")?.max(0) as u64 * nrows.max(0) as u64;
        let padded = pad_data_len(data_len);
        let inner = self.inner_mut()?;
        inner.io.write_at(end, &header_bytes)?;
        if padded > 0 {
            io::write_fill(&mut inner.io, data_start, padded, 0)?;
        }
        inner.io.truncate(data_start + padded)?;
        inner.io.flush()?;
        inner.hdus.push(Hdu {
            hdu_type: HduType::BinaryTable,
            header,
            header_start: end,
            data_start,
        });
        inner.current = inner.hdus.len() - 1;
        inner.dirty = false;
        Ok(())
    }

    /// `fits_read_btblhdr` / `ffghbn`.
    pub fn binary_table_info(&self) -> Result<crate::header::BinaryTableInfo> {
        let inner = self.inner()?;
        inner.hdus[inner.current].header.binary_table_info()
    }

    pub(crate) fn write_bin_col_str(
        &mut self,
        colnum: i32,
        firstrow: i64,
        values: &[&str],
    ) -> Result<()> {
        self.require_write()?;
        if values.is_empty() {
            return Ok(());
        }
        let col = self.bin_col(colnum)?;
        if col.tform.kind != BinaryKind::A {
            return Err(FitsError::new(crate::status::NOT_ASCII_COL));
        }
        if col.tform.is_variable() {
            for (i, s) in values.iter().enumerate() {
                let bytes = if s.is_empty() {
                    vec![0u8]
                } else {
                    s.as_bytes().to_vec()
                };
                self.write_vla_bytes(colnum, firstrow + i as i64, &bytes, bytes.len() as i64)?;
            }
            return Ok(());
        }
        self.ensure_rows(firstrow, values.len() as i64)?;
        let width = col.tform.repeat as usize;
        let (data_start, rowlen) = self.table_geom()?;
        for (i, s) in values.iter().enumerate() {
            let mut buf = vec![b' '; width];
            let n = s.len().min(width);
            buf[..n].copy_from_slice(&s.as_bytes()[..n]);
            let pos = data_start + ((firstrow - 1 + i as i64) as u64) * rowlen + col.offset;
            self.inner_mut()?.io.write_at(pos, &buf)?;
        }
        Ok(())
    }

    pub(crate) fn write_bin_col_i64_at(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[i64],
    ) -> Result<()> {
        self.require_write()?;
        if values.is_empty() {
            return Ok(());
        }
        let col = self.bin_col(colnum)?;
        if col.tform.is_variable() {
            return self.write_vla_ints_at(colnum, firstrow, firstelem, values);
        }
        match col.tform.kind {
            BinaryKind::L => {
                let flags: Vec<bool> = values.iter().map(|&v| v != 0).collect();
                return self.write_bin_col_log_at(colnum, firstrow, firstelem, &flags);
            }
            BinaryKind::A | BinaryKind::E | BinaryKind::D | BinaryKind::C | BinaryKind::M => {
                return self.write_bin_col_f64_at(
                    colnum,
                    firstrow,
                    firstelem,
                    &values.iter().map(|&v| v as f64).collect::<Vec<_>>(),
                );
            }
            BinaryKind::X => {
                let flags: Vec<bool> = values.iter().map(|&v| v != 0).collect();
                return self.write_bin_col_bit_at(colnum, firstrow, firstelem, &flags);
            }
            _ => {}
        }
        let repeat = col.tform.repeat.max(1);
        self.ensure_elem_span(firstrow, firstelem, values.len() as i64, repeat)?;
        let width = col.tform.elem_nbytes();
        let (data_start, rowlen) = self.table_geom()?;
        let mut row = firstrow;
        let mut elem = firstelem;
        for &v in values {
            let buf = encode_int_value(v, &col)?;
            let pos = elem_file_pos(data_start, rowlen, col.offset, row, elem, width);
            self.inner_mut()?.io.write_at(pos, &buf)?;
            advance_elem(&mut row, &mut elem, repeat);
        }
        Ok(())
    }

    pub(crate) fn write_bin_col_u64_at(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[u64],
    ) -> Result<()> {
        self.require_write()?;
        if values.is_empty() {
            return Ok(());
        }
        let col = self.bin_col(colnum)?;
        let repeat = col.tform.repeat.max(1);
        self.ensure_elem_span(firstrow, firstelem, values.len() as i64, repeat)?;
        let (data_start, rowlen) = self.table_geom()?;
        let n = col.tform.elem_nbytes().min(8);
        let mut row = firstrow;
        let mut elem = firstelem;
        for &v in values {
            let stored = if col.tform.kind == BinaryKind::W {
                v.wrapping_sub(1u64 << 63)
            } else {
                let phys = v as i64;
                stored_from_physical(phys, &col)? as u64
            };
            let buf = stored.to_be_bytes();
            let pos = elem_file_pos(data_start, rowlen, col.offset, row, elem, n);
            self.inner_mut()?.io.write_at(pos, &buf[8 - n..])?;
            advance_elem(&mut row, &mut elem, repeat);
        }
        Ok(())
    }

    pub(crate) fn write_bin_col_f64(
        &mut self,
        colnum: i32,
        firstrow: i64,
        values: &[f64],
    ) -> Result<()> {
        self.write_bin_col_f64_at(colnum, firstrow, 1, values)
    }

    pub(crate) fn write_bin_col_f64_at(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[f64],
    ) -> Result<()> {
        self.require_write()?;
        if values.is_empty() {
            return Ok(());
        }
        let col = self.bin_col(colnum)?;
        if col.tscal == 0.0 {
            return Err(FitsError::new(ZERO_SCALE));
        }
        if col.tform.is_variable() {
            return self.write_vla_floats_at(colnum, firstrow, firstelem, values);
        }
        let repeat = col.tform.repeat.max(1);
        self.ensure_elem_span(firstrow, firstelem, values.len() as i64, repeat)?;
        let width = col.tform.elem_nbytes();
        let (data_start, rowlen) = self.table_geom()?;
        let mut row = firstrow;
        let mut elem = firstelem;
        for &v in values {
            let buf = encode_float_value(v, &col)?;
            let pos = elem_file_pos(data_start, rowlen, col.offset, row, elem, width);
            self.inner_mut()?.io.write_at(pos, &buf)?;
            advance_elem(&mut row, &mut elem, repeat);
        }
        Ok(())
    }

    pub(crate) fn write_bin_col_f32_at(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[f32],
    ) -> Result<()> {
        self.write_bin_col_f64_at(
            colnum,
            firstrow,
            firstelem,
            &values.iter().map(|&v| f64::from(v)).collect::<Vec<_>>(),
        )
    }

    pub(crate) fn insert_bin_col(&mut self, numcol: i32, ttype: &str, tform: &str) -> Result<()> {
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
        let parsed = parse_binary_tform(tform)?;
        let nbytes = parsed.row_nbytes() as u64;
        let forms: Vec<String> = (1..=tfields)
            .map(|i| {
                self.header()
                    .ok()
                    .and_then(|h| h.get_string(&format!("TFORM{i}")).ok().map(|(v, _)| v))
                    .unwrap_or_default()
            })
            .collect();
        let form_refs: Vec<&str> = forms.iter().map(String::as_str).collect();
        let at = if colnum > tfields {
            self.header()?.get_i64("NAXIS1").unwrap_or(0).max(0) as u64
        } else {
            crate::tform::binary_column_offsets(&form_refs)
                .ok()
                .and_then(|(_, offs)| offs.get((colnum - 1) as usize).copied())
                .unwrap_or(0) as u64
        };
        let naxis2 = self.nrows()?.max(0) as usize;
        let naxis1 = self.header()?.get_i64("NAXIS1").unwrap_or(0).max(0) as usize;
        let (data_start, _) = self.table_geom()?;
        if naxis2 > 0 && nbytes > 0 {
            crate::table::insert_in_rows(
                &mut self.inner_mut()?.io,
                data_start,
                naxis1,
                naxis2,
                at as usize,
                nbytes as usize,
                0,
            )?;
        }
        {
            let inner = self.inner_mut()?;
            let header = &mut inner.hdus[inner.current].header;
            header.update_long_keep_comment("TFIELDS", i64::from(tfields) + 1)?;
            header.update_long_keep_comment("NAXIS1", naxis1 as i64 + nbytes as i64)?;
            if colnum <= tfields {
                header.shift_table_col_keys(colnum, tfields, 1)?;
            }
            let stored = BinaryTform::stored_code(tform);
            header.write_string(&format!("TTYPE{colnum}"), ttype, Some("label for field"))?;
            header.write_string(
                &format!("TFORM{colnum}"),
                &stored,
                Some("data format of field"),
            )?;
            inner.dirty = true;
        }
        self.flush()?;
        Ok(())
    }

    /// Write logical `T`/`F` values.
    pub fn write_col_log(&mut self, colnum: i32, firstrow: i64, values: &[bool]) -> Result<()> {
        self.require_write()?;
        match self.hdu_type()? {
            HduType::BinaryTable => self.write_bin_col_log(colnum, firstrow, values),
            HduType::AsciiTable | HduType::Image => Err(FitsError::new(NOT_LOGICAL_COL)),
        }
    }

    /// Write bit values into an `X` (or `B`) column.
    pub fn write_col_bit(&mut self, colnum: i32, firstrow: i64, values: &[bool]) -> Result<()> {
        self.require_write()?;
        match self.hdu_type()? {
            HduType::BinaryTable => self.write_bin_col_bit(colnum, firstrow, values),
            _ => Err(FitsError::new(NOT_LOGICAL_COL)),
        }
    }

    fn write_bin_col_log(&mut self, colnum: i32, firstrow: i64, values: &[bool]) -> Result<()> {
        self.write_bin_col_log_at(colnum, firstrow, 1, values)
    }

    fn write_bin_col_log_at(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[bool],
    ) -> Result<()> {
        if values.is_empty() {
            return Ok(());
        }
        let col = self.bin_col(colnum)?;
        if col.tform.kind != BinaryKind::L {
            return Err(FitsError::new(NOT_LOGICAL_COL));
        }
        if col.tform.is_variable() {
            let bytes: Vec<u8> = values
                .iter()
                .map(|&v| if v { b'T' } else { b'F' })
                .collect();
            self.write_vla_bytes(colnum, firstrow, &bytes, values.len() as i64)?;
            return Ok(());
        }
        let repeat = col.tform.repeat.max(1);
        self.ensure_elem_span(firstrow, firstelem, values.len() as i64, repeat)?;
        let (data_start, rowlen) = self.table_geom()?;
        let mut row = firstrow;
        let mut elem = firstelem;
        for &v in values {
            let b = [if v { b'T' } else { b'F' }];
            let pos = elem_file_pos(data_start, rowlen, col.offset, row, elem, 1);
            self.inner_mut()?.io.write_at(pos, &b)?;
            advance_elem(&mut row, &mut elem, repeat);
        }
        Ok(())
    }

    fn write_bin_col_bit(&mut self, colnum: i32, firstrow: i64, values: &[bool]) -> Result<()> {
        self.write_bin_col_bit_at(colnum, firstrow, 1, values)
    }

    fn write_bin_col_bit_at(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[bool],
    ) -> Result<()> {
        if values.is_empty() {
            return Ok(());
        }
        let col = self.bin_col(colnum)?;
        if col.tform.kind != BinaryKind::X && col.tform.kind != BinaryKind::B {
            return Err(FitsError::new(NOT_LOGICAL_COL));
        }
        if col.tform.is_variable() {
            let nbytes = values.len().div_ceil(8);
            let mut bytes = vec![0u8; nbytes];
            for (i, &v) in values.iter().enumerate() {
                if v {
                    bytes[i / 8] |= 1 << (7 - (i % 8));
                }
            }
            self.write_vla_bytes(colnum, firstrow, &bytes, values.len() as i64)?;
            return Ok(());
        }
        let repeat = col.tform.repeat.max(1);
        self.ensure_elem_span(firstrow, firstelem, values.len() as i64, repeat)?;
        let (data_start, rowlen) = self.table_geom()?;
        let mut row = firstrow;
        let mut elem = firstelem;
        for &v in values {
            if col.tform.kind == BinaryKind::X {
                self.write_one_bit(data_start, rowlen, col.offset, row, elem, v)?;
            } else {
                let b = [u8::from(v)];
                let pos = elem_file_pos(data_start, rowlen, col.offset, row, elem, 1);
                self.inner_mut()?.io.write_at(pos, &b)?;
            }
            advance_elem(&mut row, &mut elem, repeat);
        }
        Ok(())
    }

    pub(crate) fn read_bin_col_str(
        &mut self,
        colnum: i32,
        firstrow: i64,
        nelem: usize,
        nulval: Option<&str>,
    ) -> Result<(Vec<String>, bool)> {
        let col = self.bin_col(colnum)?;
        if col.tform.kind != BinaryKind::A {
            return Err(FitsError::new(crate::status::NOT_ASCII_COL));
        }
        if col.tform.is_variable() {
            let mut out = Vec::new();
            let mut anynul = false;
            for i in 0..nelem {
                let bytes = self.read_vla_bytes(colnum, firstrow + i as i64)?;
                if bytes.is_empty() || bytes.iter().all(|&b| b == 0) {
                    if let Some(n) = nulval {
                        anynul = true;
                        out.push(n.to_string());
                        continue;
                    }
                }
                out.push(String::from_utf8_lossy(&bytes).trim_end().to_string());
            }
            return Ok((out, anynul));
        }
        self.check_read_rows(firstrow, nelem as i64)?;
        let width = col.tform.repeat as usize;
        let (data_start, rowlen) = self.table_geom()?;
        let mut out = Vec::with_capacity(nelem);
        for i in 0..nelem {
            let pos = data_start + ((firstrow - 1 + i as i64) as u64) * rowlen + col.offset;
            let mut buf = vec![0u8; width];
            let n = self.inner_mut()?.io.read_at(pos, &mut buf)?;
            if n < width {
                return Err(FitsError::new(END_OF_FILE));
            }
            let mut end = buf.len();
            while end > 0 && buf[end - 1] == b' ' {
                end -= 1;
            }
            out.push(String::from_utf8_lossy(&buf[..end]).into_owned());
        }
        Ok((out, false))
    }

    pub(crate) fn read_bin_col_i64_at(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        nelem: usize,
        nulval: Option<i64>,
    ) -> Result<(Vec<i64>, bool)> {
        let col = self.bin_col(colnum)?;
        if col.tform.is_variable() {
            let bytes = self.read_vla_bytes(colnum, firstrow)?;
            let vals = decode_int_slice(&bytes, &col)?;
            let start = (firstelem.max(1) as usize).saturating_sub(1);
            let end = (start + nelem).min(vals.len());
            return Ok((vals.get(start..end).unwrap_or(&[]).to_vec(), false));
        }
        let repeat = col.tform.repeat.max(1);
        self.check_elem_span(firstrow, firstelem, nelem as i64, repeat)?;
        let (data_start, rowlen) = self.table_geom()?;
        let width = col.tform.elem_nbytes();
        let mut out = Vec::with_capacity(nelem);
        let mut anynul = false;
        let mut row = firstrow;
        let mut elem = firstelem;
        for _ in 0..nelem {
            let pos = elem_file_pos(data_start, rowlen, col.offset, row, elem, width);
            let mut buf = vec![0u8; width];
            let n = self.inner_mut()?.io.read_at(pos, &mut buf)?;
            if n < width {
                return Err(FitsError::new(END_OF_FILE));
            }
            let stored = decode_int_raw(&buf, col.tform.kind)?;
            if let (Some(nv), Some(tn)) = (nulval, col.tnull) {
                if stored == tn {
                    anynul = true;
                    out.push(nv);
                    advance_elem(&mut row, &mut elem, repeat);
                    continue;
                }
            }
            out.push(physical_from_stored(stored, &col)?);
            advance_elem(&mut row, &mut elem, repeat);
        }
        Ok((out, anynul))
    }

    pub(crate) fn read_bin_col_f64(
        &mut self,
        colnum: i32,
        firstrow: i64,
        nelem: usize,
        _nulval: Option<f64>,
    ) -> Result<(Vec<f64>, bool)> {
        self.read_bin_col_f64_at(colnum, firstrow, 1, nelem, _nulval)
    }

    pub(crate) fn read_bin_col_f64_at(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        nelem: usize,
        _nulval: Option<f64>,
    ) -> Result<(Vec<f64>, bool)> {
        let col = self.bin_col(colnum)?;
        if col.tform.is_variable() {
            let bytes = self.read_vla_bytes(colnum, firstrow)?;
            let vals = decode_float_slice(&bytes, &col)?;
            let start = (firstelem.max(1) as usize).saturating_sub(1);
            let end = (start + nelem).min(vals.len());
            return Ok((vals.get(start..end).unwrap_or(&[]).to_vec(), false));
        }
        let repeat = col.tform.repeat.max(1);
        self.check_elem_span(firstrow, firstelem, nelem as i64, repeat)?;
        let (data_start, rowlen) = self.table_geom()?;
        let width = col.tform.elem_nbytes();
        let mut out = Vec::with_capacity(nelem);
        let mut row = firstrow;
        let mut elem = firstelem;
        for _ in 0..nelem {
            let pos = elem_file_pos(data_start, rowlen, col.offset, row, elem, width);
            let mut buf = vec![0u8; width];
            let n = self.inner_mut()?.io.read_at(pos, &mut buf)?;
            if n < width {
                return Err(FitsError::new(END_OF_FILE));
            }
            out.push(decode_float_value(&buf, &col)?);
            advance_elem(&mut row, &mut elem, repeat);
        }
        Ok((out, false))
    }

    pub(crate) fn write_bin_col_null(
        &mut self,
        colnum: i32,
        firstrow: i64,
        nelem: i64,
    ) -> Result<()> {
        if nelem < 0 {
            return Err(FitsError::new(NEG_BYTES));
        }
        if nelem == 0 {
            return Ok(());
        }
        let col = self.bin_col(colnum)?;
        self.ensure_rows(firstrow, nelem)?;
        let (data_start, rowlen) = self.table_geom()?;
        let width = col.tform.row_nbytes();
        let buf = if let Some(tn) = col.tnull {
            encode_int_raw(tn, col.tform.kind)?
        } else if matches!(col.tform.kind, BinaryKind::E | BinaryKind::D) {
            vec![0xff; width]
        } else if matches!(col.tform.kind, BinaryKind::L | BinaryKind::A) {
            vec![0u8; width]
        } else {
            return Err(FitsError::new(crate::status::NO_NULL));
        };
        for i in 0..nelem {
            let pos = data_start + ((firstrow - 1 + i) as u64) * rowlen + col.offset;
            self.inner_mut()?.io.write_at(pos, &buf)?;
        }
        Ok(())
    }

    /// Update VLA `TFORMn(max)` before a header flush (`ffuptf`).
    pub(crate) fn sync_binary_heap_keywords(&mut self) -> Result<()> {
        if self.hdu_type()? != HduType::BinaryTable {
            return Ok(());
        }
        let pcount = self.header()?.get_i64("PCOUNT").unwrap_or(0);
        if pcount <= 0 {
            return Ok(());
        }
        let tfields = self.ncols()?;
        let naxis2 = self.nrows()?;
        let mut vla: Vec<(i32, String, String)> = Vec::new();
        for colnum in 1..=tfields {
            let tform = self
                .header()?
                .get_string(&format!("TFORM{colnum}"))
                .map(|(v, _)| v)
                .unwrap_or_default();
            let parsed = match parse_binary_tform(&tform) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if !parsed.is_variable() {
                continue;
            }
            let comm = self
                .header()?
                .raw_value_comment(&format!("TFORM{colnum}"))
                .ok()
                .map(|(_, c)| c)
                .unwrap_or_default();
            vla.push((colnum, tform, comm));
        }
        let mut updates: Vec<(i32, String, String)> = Vec::new();
        for (colnum, tform, comm) in vla {
            let mut maxlen = 0i64;
            for row in 1..=naxis2 {
                let (len, _) = self.read_descriptor(colnum, row)?;
                if len > maxlen {
                    maxlen = len;
                }
            }
            let mut base = tform;
            if let Some(i) = base.find('(') {
                base.truncate(i);
            }
            updates.push((colnum, format!("{base}({maxlen})"), comm));
        }
        if updates.is_empty() {
            return Ok(());
        }
        {
            let inner = self.inner_mut()?;
            for (colnum, newform, comm) in updates {
                let c = if comm.is_empty() { None } else { Some(comm) };
                inner.hdus[inner.current].header.update_with(
                    &format!("TFORM{colnum}"),
                    &crate::card::format_string_value(&newform),
                    c.as_deref(),
                )?;
            }
            inner.dirty = true;
        }
        Ok(())
    }

    pub(crate) fn write_vla_bytes(
        &mut self,
        colnum: i32,
        row: i64,
        bytes: &[u8],
        nelem: i64,
    ) -> Result<()> {
        if row < 1 {
            return Err(FitsError::new(BAD_ROW_NUM));
        }
        self.ensure_rows(row, 1)?;
        let col = self.bin_col(colnum)?;
        let (data_start, rowlen) = self.table_geom()?;
        let naxis2 = self.nrows()?;
        let heap0 = data_start + rowlen * naxis2 as u64;
        let mut pcount = self.inner()?.hdus[self.inner()?.current]
            .header
            .get_i64("PCOUNT")
            .unwrap_or(0)
            .max(0) as u64;
        let heap_off = pcount;
        if !bytes.is_empty() {
            self.inner_mut()?.io.write_at(heap0 + heap_off, bytes)?;
        }
        pcount += bytes.len() as u64;
        self.write_descriptor(colnum, row, col.tform.variable, nelem, heap_off as i64)?;
        {
            let inner = self.inner_mut()?;
            inner.hdus[inner.current]
                .header
                .update_long_keep_comment("PCOUNT", pcount as i64)?;
            inner.dirty = true;
        }
        let padded = pad_data_len(rowlen * naxis2 as u64 + pcount);
        let inner = self.inner_mut()?;
        let end = data_start + padded;
        if end > inner.io.len()? {
            io::write_fill(&mut inner.io, heap0 + pcount, end - (heap0 + pcount), 0)?;
        }
        inner.io.truncate(end)?;
        Ok(())
    }

    pub(crate) fn read_vla_bytes(&mut self, colnum: i32, row: i64) -> Result<Vec<u8>> {
        let col = self.bin_col(colnum)?;
        let (len, heap_off) = self.read_descriptor(colnum, row)?;
        if len <= 0 {
            return Ok(Vec::new());
        }
        let nbytes = if col.tform.kind == BinaryKind::X {
            (len as usize).div_ceil(8)
        } else {
            len as usize * col.tform.elem_nbytes()
        };
        let (data_start, rowlen) = self.table_geom()?;
        let naxis2 = self.nrows()?;
        let heap0 = data_start + rowlen * naxis2 as u64;
        let mut buf = vec![0u8; nbytes];
        let n = self
            .inner_mut()?
            .io
            .read_at(heap0 + heap_off as u64, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    fn write_descriptor(
        &mut self,
        colnum: i32,
        row: i64,
        var: VariableKind,
        len: i64,
        heap_off: i64,
    ) -> Result<()> {
        let col = self.bin_col(colnum)?;
        let (data_start, rowlen) = self.table_geom()?;
        let pos = data_start + ((row - 1) as u64) * rowlen + col.offset;
        match var {
            VariableKind::P => {
                let mut buf = [0u8; 8];
                buf[..4].copy_from_slice(&(len as i32).to_be_bytes());
                buf[4..].copy_from_slice(&(heap_off as i32).to_be_bytes());
                self.inner_mut()?.io.write_at(pos, &buf)?;
            }
            VariableKind::Q => {
                let mut buf = [0u8; 16];
                buf[..8].copy_from_slice(&len.to_be_bytes());
                buf[8..].copy_from_slice(&heap_off.to_be_bytes());
                self.inner_mut()?.io.write_at(pos, &buf)?;
            }
            VariableKind::None => {}
        }
        Ok(())
    }

    pub(crate) fn read_descriptor(&mut self, colnum: i32, row: i64) -> Result<(i64, i64)> {
        let col = self.bin_col(colnum)?;
        let (data_start, rowlen) = self.table_geom()?;
        let pos = data_start + ((row - 1) as u64) * rowlen + col.offset;
        match col.tform.variable {
            VariableKind::P => {
                let mut buf = [0u8; 8];
                let n = self.inner_mut()?.io.read_at(pos, &mut buf)?;
                if n < 8 {
                    return Ok((0, 0));
                }
                let len = i32::from_be_bytes(buf[..4].try_into().unwrap()) as i64;
                let off = i32::from_be_bytes(buf[4..].try_into().unwrap()) as i64;
                Ok((len, off))
            }
            VariableKind::Q => {
                let mut buf = [0u8; 16];
                let n = self.inner_mut()?.io.read_at(pos, &mut buf)?;
                if n < 16 {
                    return Ok((0, 0));
                }
                let len = i64::from_be_bytes(buf[..8].try_into().unwrap());
                let off = i64::from_be_bytes(buf[8..].try_into().unwrap());
                Ok((len, off))
            }
            VariableKind::None => Ok((0, 0)),
        }
    }

    fn ensure_elem_span(
        &mut self,
        firstrow: i64,
        firstelem: i64,
        nvalues: i64,
        repeat: i64,
    ) -> Result<()> {
        if firstrow < 1 {
            return Err(FitsError::new(BAD_ROW_NUM));
        }
        if firstelem < 1 || (repeat > 0 && firstelem > repeat) {
            return Err(FitsError::new(BAD_ELEM_NUM));
        }
        if nvalues <= 0 {
            return Ok(());
        }
        let start = (firstrow - 1) * repeat + (firstelem - 1);
        let last_row = (start + nvalues - 1) / repeat + 1;
        self.ensure_rows(1, last_row)
    }

    fn check_elem_span(
        &self,
        firstrow: i64,
        firstelem: i64,
        nvalues: i64,
        repeat: i64,
    ) -> Result<()> {
        if firstrow < 1 {
            return Err(FitsError::new(BAD_ROW_NUM));
        }
        if firstelem < 1 || (repeat > 0 && firstelem > repeat) {
            return Err(FitsError::new(BAD_ELEM_NUM));
        }
        if nvalues < 0 {
            return Err(FitsError::new(NEG_BYTES));
        }
        if nvalues == 0 {
            return Ok(());
        }
        let start = (firstrow - 1) * repeat + (firstelem - 1);
        let last_row = (start + nvalues - 1) / repeat + 1;
        if last_row > self.nrows()? {
            return Err(FitsError::new(BAD_ELEM_NUM));
        }
        Ok(())
    }

    fn write_vla_ints_at(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[i64],
    ) -> Result<()> {
        let col = self.bin_col(colnum)?;
        if firstelem <= 1 {
            let bytes = encode_int_slice(values, &col)?;
            return self.write_vla_bytes(colnum, firstrow, &bytes, values.len() as i64);
        }
        let bytes = self.read_vla_bytes(colnum, firstrow)?;
        let mut vals = decode_int_slice(&bytes, &col)?;
        let start = (firstelem as usize).saturating_sub(1);
        if vals.len() < start + values.len() {
            vals.resize(start + values.len(), 0);
        }
        vals[start..start + values.len()].copy_from_slice(values);
        let out = encode_int_slice(&vals, &col)?;
        self.write_vla_bytes(colnum, firstrow, &out, vals.len() as i64)
    }

    fn write_vla_floats_at(
        &mut self,
        colnum: i32,
        firstrow: i64,
        firstelem: i64,
        values: &[f64],
    ) -> Result<()> {
        let col = self.bin_col(colnum)?;
        if firstelem <= 1 {
            let bytes = encode_float_slice(values, &col)?;
            return self.write_vla_bytes(colnum, firstrow, &bytes, values.len() as i64);
        }
        let bytes = self.read_vla_bytes(colnum, firstrow)?;
        let mut vals = decode_float_slice(&bytes, &col)?;
        let start = (firstelem as usize).saturating_sub(1);
        if vals.len() < start + values.len() {
            vals.resize(start + values.len(), 0.0);
        }
        vals[start..start + values.len()].copy_from_slice(values);
        let out = encode_float_slice(&vals, &col)?;
        self.write_vla_bytes(colnum, firstrow, &out, vals.len() as i64)
    }

    fn write_one_bit(
        &mut self,
        data_start: u64,
        rowlen: u64,
        col_off: u64,
        row: i64,
        elem: i64,
        on: bool,
    ) -> Result<()> {
        let bit_index = elem - 1;
        let byte_off = (bit_index / 8) as u64;
        let shift = 7 - (bit_index % 8);
        let pos = data_start + ((row - 1) as u64) * rowlen + col_off + byte_off;
        let mut b = [0u8];
        let _ = self.inner_mut()?.io.read_at(pos, &mut b)?;
        if on {
            b[0] |= 1 << shift;
        } else {
            b[0] &= !(1 << shift);
        }
        self.inner_mut()?.io.write_at(pos, &b)
    }

    fn bin_col(&self, colnum: i32) -> Result<BinCol> {
        self.require_table()?;
        let inner = self.inner()?;
        let header = &inner.hdus[inner.current].header;
        let tfields = header.get_i64("TFIELDS").unwrap_or(0) as i32;
        if colnum < 1 || colnum > tfields {
            return Err(FitsError::new(BAD_COL_NUM));
        }
        let mut offset = 0u64;
        let mut parsed = BinaryTform::parse("1B")?;
        for i in 1..=colnum {
            let tf = header
                .get_string(&format!("TFORM{i}"))
                .map(|(v, _)| v)
                .map_err(|_| FitsError::new(crate::status::NO_TFORM))?;
            parsed = BinaryTform::parse(&tf)?;
            if i < colnum {
                offset += parsed.row_nbytes() as u64;
            }
        }
        let tscal = header.get_f64(&format!("TSCAL{colnum}")).unwrap_or(1.0);
        let tzero = header.get_f64(&format!("TZERO{colnum}")).unwrap_or(0.0);
        let tnull = header.get_i64(&format!("TNULL{colnum}")).ok();
        Ok(BinCol {
            offset,
            tform: parsed,
            tscal,
            tzero,
            tnull,
        })
    }
}

fn elem_file_pos(
    data_start: u64,
    rowlen: u64,
    col_off: u64,
    row: i64,
    elem: i64,
    elem_nbytes: usize,
) -> u64 {
    data_start + ((row - 1) as u64) * rowlen + col_off + ((elem - 1) as u64) * elem_nbytes as u64
}

fn advance_elem(row: &mut i64, elem: &mut i64, repeat: i64) {
    *elem += 1;
    if *elem > repeat {
        *elem = 1;
        *row += 1;
    }
}

fn stored_from_physical(physical: i64, col: &BinCol) -> Result<i64> {
    if col.tscal == 0.0 {
        return Err(FitsError::new(ZERO_SCALE));
    }
    if (col.tscal - 1.0).abs() < f64::EPSILON {
        let raw = (physical as f64 - col.tzero).round();
        return Ok(raw as i64);
    }
    Ok(((physical as f64 - col.tzero) / col.tscal).round() as i64)
}

fn physical_from_stored(stored: i64, col: &BinCol) -> Result<i64> {
    if col.tscal == 0.0 {
        return Err(FitsError::new(ZERO_SCALE));
    }
    Ok((stored as f64 * col.tscal + col.tzero).round() as i64)
}

fn encode_int_raw(stored: i64, kind: BinaryKind) -> Result<Vec<u8>> {
    match kind {
        BinaryKind::B | BinaryKind::S => {
            if !(0..=255).contains(&stored) {
                return Err(FitsError::new(NUM_OVERFLOW));
            }
            Ok(vec![stored as u8])
        }
        BinaryKind::I | BinaryKind::U => Ok((stored as i16).to_be_bytes().to_vec()),
        BinaryKind::J | BinaryKind::V => Ok((stored as i32).to_be_bytes().to_vec()),
        BinaryKind::K | BinaryKind::W => Ok(stored.to_be_bytes().to_vec()),
        _ => Err(FitsError::new(crate::status::BAD_BTABLE_FORMAT)),
    }
}

fn encode_int_value(physical: i64, col: &BinCol) -> Result<Vec<u8>> {
    encode_int_raw(stored_from_physical(physical, col)?, col.tform.kind)
}

fn encode_int_slice(values: &[i64], col: &BinCol) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for &v in values {
        out.extend_from_slice(&encode_int_value(v, col)?);
    }
    Ok(out)
}

fn decode_int_raw(buf: &[u8], kind: BinaryKind) -> Result<i64> {
    match kind {
        BinaryKind::B | BinaryKind::S => Ok(i64::from(*buf.first().unwrap_or(&0))),
        BinaryKind::I | BinaryKind::U => {
            let a: [u8; 2] = buf
                .get(..2)
                .and_then(|s| s.try_into().ok())
                .ok_or_else(|| FitsError::new(END_OF_FILE))?;
            Ok(i64::from(i16::from_be_bytes(a)))
        }
        BinaryKind::J | BinaryKind::V => {
            let a: [u8; 4] = buf
                .get(..4)
                .and_then(|s| s.try_into().ok())
                .ok_or_else(|| FitsError::new(END_OF_FILE))?;
            Ok(i64::from(i32::from_be_bytes(a)))
        }
        BinaryKind::K | BinaryKind::W => {
            let a: [u8; 8] = buf
                .get(..8)
                .and_then(|s| s.try_into().ok())
                .ok_or_else(|| FitsError::new(END_OF_FILE))?;
            Ok(i64::from_be_bytes(a))
        }
        BinaryKind::L => Ok(i64::from(buf.first() == Some(&b'T'))),
        _ => Err(FitsError::new(crate::status::BAD_BTABLE_FORMAT)),
    }
}

fn decode_int_slice(bytes: &[u8], col: &BinCol) -> Result<Vec<i64>> {
    let w = col.tform.elem_nbytes();
    if w == 0 {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for chunk in bytes.chunks_exact(w) {
        let stored = decode_int_raw(chunk, col.tform.kind)?;
        out.push(physical_from_stored(stored, col)?);
    }
    Ok(out)
}

fn encode_float_value(physical: f64, col: &BinCol) -> Result<Vec<u8>> {
    if col.tscal == 0.0 {
        return Err(FitsError::new(ZERO_SCALE));
    }
    let raw = (physical - col.tzero) / col.tscal;
    match col.tform.kind {
        BinaryKind::E | BinaryKind::C => Ok((raw as f32).to_be_bytes().to_vec()),
        BinaryKind::D | BinaryKind::M => Ok(raw.to_be_bytes().to_vec()),
        _ => encode_int_value(raw.round() as i64, col),
    }
}

fn encode_float_slice(values: &[f64], col: &BinCol) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for &v in values {
        out.extend_from_slice(&encode_float_value(v, col)?);
    }
    Ok(out)
}

fn decode_float_value(buf: &[u8], col: &BinCol) -> Result<f64> {
    if col.tscal == 0.0 {
        return Err(FitsError::new(ZERO_SCALE));
    }
    let raw = match col.tform.kind {
        BinaryKind::E | BinaryKind::C => {
            let a: [u8; 4] = buf
                .get(..4)
                .and_then(|s| s.try_into().ok())
                .ok_or_else(|| FitsError::new(END_OF_FILE))?;
            f64::from(f32::from_be_bytes(a))
        }
        BinaryKind::D | BinaryKind::M => {
            let a: [u8; 8] = buf
                .get(..8)
                .and_then(|s| s.try_into().ok())
                .ok_or_else(|| FitsError::new(END_OF_FILE))?;
            f64::from_be_bytes(a)
        }
        _ => decode_int_raw(buf, col.tform.kind)? as f64,
    };
    Ok(raw * col.tscal + col.tzero)
}

fn decode_float_slice(bytes: &[u8], col: &BinCol) -> Result<Vec<f64>> {
    let w = col.tform.elem_nbytes();
    if w == 0 {
        return Ok(Vec::new());
    }
    bytes
        .chunks_exact(w)
        .map(|c| decode_float_value(c, col))
        .collect()
}
