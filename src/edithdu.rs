//! Copy, insert, and delete HDUs (`edithdu.c`).

use std::io::Write;

use crate::convert::pad_data_len;
use crate::error::{FitsError, Result};
use crate::file::FitsFile;
use crate::hdu::Hdu;
use crate::header::Header;
use crate::io::{self, Driver};
use crate::status::{BAD_BITPIX, BAD_HDU_NUM, BAD_NAXES, BAD_NAXIS};
use crate::types::{HduType, ImageType};

impl FitsFile {
    /// `fits_create_hdu` / `ffcrhd`: append an empty HDU and make it current.
    ///
    /// No-op if the current header has no keywords yet.
    pub fn create_hdu(&mut self) -> Result<()> {
        self.require_write()?;
        self.flush()?;
        if self.header()?.is_empty() {
            return Ok(());
        }
        let inner = self.inner_mut()?;
        let at = inner.last_end()?;
        inner.hdus.push(Hdu {
            hdu_type: HduType::Image,
            header: Header::new(),
            header_start: at,
            data_start: at,
        });
        inner.current = inner.hdus.len() - 1;
        inner.dirty = false;
        Ok(())
    }

    /// `fits_delete_hdu` / `ffdhdu`.
    ///
    /// Deleting the primary array replaces it with a null (`NAXIS=0`) primary
    /// and leaves any following HDUs in place. Deleting an extension removes
    /// it and makes the following HDU current, or the previous HDU if this
    /// was the last one. Returns the type of the HDU that is current afterwards.
    pub fn delete_hdu(&mut self) -> Result<HduType> {
        self.require_write()?;
        self.flush()?;
        let inner = self.inner_mut()?;
        let idx = inner.current;
        if idx == 0 {
            let old_next = if inner.hdus.len() > 1 {
                inner.hdus[1].header_start
            } else {
                inner.io.len()?
            };
            let new_primary = Hdu::empty_primary()?;
            let hb = new_primary.header.to_record_bytes();
            inner.io.write_at(0, &hb)?;
            if old_next > hb.len() as u64 {
                let shrink = old_next - hb.len() as u64;
                io::delete_bytes(&mut inner.io, hb.len() as u64, shrink)?;
                inner.bump_offsets_from(1, -(shrink as i64));
            } else if inner.hdus.len() == 1 {
                inner.io.truncate(hb.len() as u64)?;
            }
            inner.hdus[0] = new_primary;
            inner.dirty = false;
            inner.io.flush()?;
            return Ok(HduType::Image);
        }
        let start = inner.hdus[idx].header_start;
        let end = inner.hdus[idx].end()?;
        let shrink = end.saturating_sub(start);
        io::delete_bytes(&mut inner.io, start, shrink)?;
        inner.hdus.remove(idx);
        inner.bump_offsets_from(idx, -(shrink as i64));
        if inner.hdus.is_empty() {
            return Err(FitsError::new(BAD_HDU_NUM));
        }
        if inner.current >= inner.hdus.len() {
            inner.current = inner.hdus.len() - 1;
        }
        inner.dirty = false;
        inner.io.flush()?;
        Ok(inner.hdus[inner.current].hdu_type)
    }

    /// `fits_copy_hdu` / `ffcopy`: copy the current HDU onto `dest`.
    ///
    /// A non-empty destination header causes a new HDU to be appended
    /// (`ffcrhd`). A primary array copied onto an extension is rewritten as
    /// `XTENSION='IMAGE'` with `PCOUNT`/`GCOUNT`. `morekeys` reserves extra
    /// header slots (`ffhdef`); `0` preserves the source's remaining space.
    pub fn copy_hdu(&mut self, dest: &mut FitsFile, morekeys: i32) -> Result<()> {
        dest.require_write()?;
        self.flush()?;
        dest.flush()?;
        let (src_type, src_primary, src_header, nmore) = {
            let inner = self.inner()?;
            let idx = inner.current;
            let hdu = &inner.hdus[idx];
            let header_len = hdu.data_start.saturating_sub(hdu.header_start);
            (
                hdu.hdu_type,
                idx == 0 && hdu.hdu_type == HduType::Image,
                hdu.header.clone(),
                hdu.header.nmore_in(header_len),
            )
        };
        let data = {
            let inner = self.inner_mut()?;
            let idx = inner.current;
            read_data_unit(inner, idx)?
        };
        if !dest.header()?.is_empty() {
            dest.create_hdu()?;
        }
        if dest.hdunum()? == 1 && src_type != HduType::Image && dest.header()?.is_empty() {
            let dummy = Header::empty_primary()?;
            dest.replace_current_header(HduType::Image, dummy, 0)?;
            dest.create_hdu()?;
        }
        let dest_primary = dest.hdunum()? == 1;
        let out_header = convert_copied_header(&src_header, src_primary, src_type, dest_primary)?;
        let pad_keys = if morekeys > 0 { morekeys } else { nmore };
        dest.replace_current_header(src_type, out_header, pad_keys)?;
        dest.write_current_data(&data)?;
        Ok(())
    }

    /// `fits_copy_header` / `ffcphd`.
    pub fn copy_header(&mut self, dest: &mut FitsFile) -> Result<()> {
        dest.require_write()?;
        self.flush()?;
        dest.flush()?;
        let (src_type, src_primary, src_header) = {
            let inner = self.inner()?;
            let hdu = &inner.hdus[inner.current];
            (
                hdu.hdu_type,
                inner.current == 0 && hdu.hdu_type == HduType::Image,
                hdu.header.clone(),
            )
        };
        if !dest.header()?.is_empty() {
            dest.create_hdu()?;
        }
        if dest.hdunum()? == 1 && src_type != HduType::Image && dest.header()?.is_empty() {
            let dummy = Header::empty_primary()?;
            dest.replace_current_header(HduType::Image, dummy, 0)?;
            dest.create_hdu()?;
        }
        let dest_primary = dest.hdunum()? == 1;
        let out_header = convert_copied_header(&src_header, src_primary, src_type, dest_primary)?;
        dest.replace_current_header(src_type, out_header, 0)?;
        Ok(())
    }

    /// `fits_copy_data` / `ffcpdt`.
    pub fn copy_data(&mut self, dest: &mut FitsFile) -> Result<()> {
        dest.require_write()?;
        self.flush()?;
        dest.flush()?;
        let data = {
            let inner = self.inner_mut()?;
            let idx = inner.current;
            read_data_unit(inner, idx)?
        };
        dest.write_current_data(&data)?;
        Ok(())
    }

    /// `fits_copy_file` / `ffcpfl`.
    pub fn copy_file(
        &mut self,
        dest: &mut FitsFile,
        previous: bool,
        current: bool,
        following: bool,
    ) -> Result<()> {
        dest.require_write()?;
        self.flush()?;
        dest.flush()?;
        let here = self.hdunum()?;
        let n = self.hdu_count()?;
        if previous {
            for i in 1..here {
                self.movabs_hdu(i)?;
                self.copy_hdu(dest, 0)?;
            }
        }
        if current {
            self.movabs_hdu(here)?;
            self.copy_hdu(dest, 0)?;
        }
        if following {
            for i in (here + 1)..=n {
                self.movabs_hdu(i)?;
                self.copy_hdu(dest, 0)?;
            }
        }
        self.movabs_hdu(here)?;
        Ok(())
    }

    /// `fits_write_hdu` / `ffwrhdu`: write the current HDU to `writer`.
    pub fn write_hdu_to<W: Write>(&mut self, writer: &mut W) -> Result<()> {
        self.flush()?;
        let inner = self.inner_mut()?;
        let idx = inner.current;
        let start = inner.hdus[idx].header_start;
        let end = inner.hdus[idx].end()?;
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];
        if len > 0 {
            let n = inner.io.read_at(start, &mut buf)?;
            buf.truncate(n);
        }
        writer.write_all(&buf).map_err(io::map_write_err)?;
        writer.flush().map_err(io::map_write_err)?;
        Ok(())
    }

    /// `fits_insert_img` / `ffiimg`: insert an IMAGE HDU after the current one.
    ///
    /// Unlike [`Self::create_image`], a null primary is not replaced: a new
    /// IMAGE extension is appended (or inserted) instead.
    pub fn insert_image(&mut self, ty: ImageType, naxes: &[i64]) -> Result<()> {
        self.require_write()?;
        self.flush()?;
        if naxes.len() > 999 {
            return Err(FitsError::new(BAD_NAXIS));
        }
        for &n in naxes {
            if n < 0 {
                return Err(FitsError::new(BAD_NAXES));
            }
        }
        if self.header()?.is_empty() {
            let header = if self.hdunum()? == 1 {
                Header::primary_image(ty, naxes)?
            } else {
                Header::image_extension(ty, naxes)?
            };
            let fill = image_payload_len(ty, naxes);
            self.replace_current_header(HduType::Image, header, 0)?;
            self.fill_current_data(fill, 0)?;
            return Ok(());
        }
        let header = Header::image_extension(ty, naxes)?;
        let payload = image_payload_len(ty, naxes);
        self.insert_constructed_hdu(HduType::Image, header, payload, 0)
    }

    /// `fits_insert_atbl` / `ffitab`: insert an ASCII table after the current HDU.
    pub fn insert_ascii_table(
        &mut self,
        nrows: i64,
        ttype: &[&str],
        tform: &[&str],
        tunit: &[Option<&str>],
        extname: Option<&str>,
    ) -> Result<()> {
        self.require_write()?;
        self.flush()?;
        if self.header()?.is_empty() {
            let header = Header::ascii_table(nrows, ttype, tform, tunit, extname)?;
            let payload = ascii_payload_len(&header, nrows);
            self.replace_current_header(HduType::AsciiTable, header, 0)?;
            return self.fill_current_data(payload, b' ');
        }
        if self.is_last_hdu()? {
            return self.create_ascii_table(nrows, ttype, tform, tunit, extname);
        }
        let header = Header::ascii_table(nrows, ttype, tform, tunit, extname)?;
        let payload = ascii_payload_len(&header, nrows);
        self.insert_constructed_hdu(HduType::AsciiTable, header, payload, b' ')
    }

    /// `fits_insert_btbl` / `ffibin`: insert a binary table after the current HDU.
    pub fn insert_binary_table(
        &mut self,
        nrows: i64,
        ttype: &[&str],
        tform: &[&str],
        tunit: &[Option<&str>],
        extname: Option<&str>,
    ) -> Result<()> {
        self.require_write()?;
        self.flush()?;
        if self.header()?.is_empty() {
            let header = Header::binary_table(nrows, ttype, tform, tunit, extname)?;
            let payload = binary_payload_len(&header, nrows);
            self.replace_current_header(HduType::BinaryTable, header, 0)?;
            return self.fill_current_data(payload, 0);
        }
        if self.is_last_hdu()? {
            return self.create_binary_table(nrows, ttype, tform, tunit, extname);
        }
        let header = Header::binary_table(nrows, ttype, tform, tunit, extname)?;
        let payload = binary_payload_len(&header, nrows);
        self.insert_constructed_hdu(HduType::BinaryTable, header, payload, 0)
    }

    fn is_last_hdu(&self) -> Result<bool> {
        let inner = self.inner()?;
        Ok(inner.current + 1 == inner.hdus.len())
    }

    fn replace_current_header(
        &mut self,
        hdu_type: HduType,
        header: Header,
        morekeys: i32,
    ) -> Result<()> {
        let hb = header.to_record_bytes_with_morekeys(morekeys);
        let inner = self.inner_mut()?;
        let idx = inner.current;
        let at = inner.hdus[idx].header_start;
        let old_end = inner.hdus[idx].end()?;
        let new_data_start = at + hb.len() as u64;
        let old_data_start = inner.hdus[idx].data_start;
        if new_data_start > old_data_start {
            io::insert_bytes(
                &mut inner.io,
                old_data_start,
                new_data_start - old_data_start,
                b' ',
            )?;
            inner.bump_offsets_from(idx + 1, (new_data_start - old_data_start) as i64);
        } else if new_data_start < old_data_start && old_end == old_data_start {
            io::delete_bytes(
                &mut inner.io,
                new_data_start,
                old_data_start - new_data_start,
            )?;
            inner.bump_offsets_from(idx + 1, -((old_data_start - new_data_start) as i64));
        }
        inner.io.write_at(at, &hb)?;
        inner.hdus[idx].header = header;
        inner.hdus[idx].hdu_type = hdu_type;
        inner.hdus[idx].data_start = new_data_start;
        inner.dirty = false;
        Ok(())
    }

    fn write_current_data(&mut self, data: &[u8]) -> Result<()> {
        let inner = self.inner_mut()?;
        let idx = inner.current;
        let data_start = inner.hdus[idx].data_start;
        let old_end = inner.last_end().unwrap_or(data_start);
        let new_end = data_start + data.len() as u64;
        if idx + 1 < inner.hdus.len() {
            let next = inner.hdus[idx + 1].header_start;
            if new_end > next {
                io::insert_bytes(&mut inner.io, next, new_end - next, 0)?;
                inner.bump_offsets_from(idx + 1, (new_end - next) as i64);
            } else if new_end < next {
                io::delete_bytes(&mut inner.io, new_end, next - new_end)?;
                inner.bump_offsets_from(idx + 1, -((next - new_end) as i64));
            }
        }
        if !data.is_empty() {
            inner.io.write_at(data_start, data)?;
        }
        if idx + 1 == inner.hdus.len() && new_end != old_end {
            inner.io.truncate(new_end)?;
        }
        inner.io.flush()?;
        inner.dirty = false;
        Ok(())
    }

    fn fill_current_data(&mut self, payload: u64, fill: u8) -> Result<()> {
        let padded = pad_data_len(payload);
        if padded == 0 {
            let inner = self.inner_mut()?;
            if inner.current + 1 == inner.hdus.len() {
                let end = inner.hdus[inner.current].data_start;
                inner.io.truncate(end)?;
            }
            return Ok(());
        }
        let buf = vec![fill; padded as usize];
        self.write_current_data(&buf)
    }

    fn insert_constructed_hdu(
        &mut self,
        hdu_type: HduType,
        header: Header,
        payload: u64,
        fill: u8,
    ) -> Result<()> {
        let hb = header.to_record_bytes();
        let padded = pad_data_len(payload);
        let hdu_len = hb.len() as u64 + padded;
        let inner = self.inner_mut()?;
        let idx = inner.current;
        let at = if idx + 1 < inner.hdus.len() {
            inner.hdus[idx + 1].header_start
        } else {
            inner.last_end()?
        };
        io::insert_bytes(&mut inner.io, at, hdu_len, fill)?;
        inner.io.write_at(at, &hb)?;
        inner.bump_offsets_from(idx + 1, hdu_len as i64);
        inner.hdus.insert(
            idx + 1,
            Hdu {
                hdu_type,
                header,
                header_start: at,
                data_start: at + hb.len() as u64,
            },
        );
        inner.current = idx + 1;
        inner.dirty = false;
        inner.io.flush()?;
        Ok(())
    }
}

fn convert_copied_header(
    src: &Header,
    src_primary: bool,
    src_type: HduType,
    dest_primary: bool,
) -> Result<Header> {
    if src_type != HduType::Image {
        return Ok(src.clone());
    }
    match (src_primary, dest_primary) {
        (true, false) => src.primary_as_image_extension(),
        (false, true) => src.image_extension_as_primary(),
        _ => Ok(src.clone()),
    }
}

fn read_data_unit(inner: &mut crate::file::Inner, idx: usize) -> Result<Vec<u8>> {
    let hdu = &inner.hdus[idx];
    let len = hdu.data_unit_len()?;
    if len == 0 {
        return Ok(Vec::new());
    }
    let mut buf = vec![0u8; len as usize];
    let n = inner.io.read_at(hdu.data_start, &mut buf)?;
    buf.truncate(n);
    if (buf.len() as u64) < len {
        let fill = if hdu.hdu_type == HduType::AsciiTable {
            b' '
        } else {
            0
        };
        buf.resize(len as usize, fill);
    }
    Ok(buf)
}

fn image_payload_len(ty: ImageType, naxes: &[i64]) -> u64 {
    if naxes.is_empty() {
        return 0;
    }
    let npix = naxes
        .iter()
        .fold(1u64, |acc, &n| acc.saturating_mul(n.max(0) as u64));
    npix.saturating_mul(ty.bytes_per_pixel() as u64)
}

fn ascii_payload_len(header: &Header, nrows: i64) -> u64 {
    let rowlen = header.get_i64("NAXIS1").unwrap_or(0).max(0) as u64;
    rowlen.saturating_mul(nrows.max(0) as u64)
}

fn binary_payload_len(header: &Header, nrows: i64) -> u64 {
    let rowlen = header.get_i64("NAXIS1").unwrap_or(0).max(0) as u64;
    let pcount = header.get_i64("PCOUNT").unwrap_or(0).max(0) as u64;
    rowlen
        .saturating_mul(nrows.max(0) as u64)
        .saturating_add(pcount)
}

/// `fits_delete_hdu` / `ffdhdu`.
pub fn fits_delete_hdu(f: &mut FitsFile) -> Result<HduType> {
    f.delete_hdu()
}

/// `fits_copy_hdu` / `ffcopy`.
pub fn fits_copy_hdu(src: &mut FitsFile, dest: &mut FitsFile, morekeys: i32) -> Result<()> {
    src.copy_hdu(dest, morekeys)
}

/// `fits_copy_file` / `ffcpfl`.
pub fn fits_copy_file(
    src: &mut FitsFile,
    dest: &mut FitsFile,
    previous: bool,
    current: bool,
    following: bool,
) -> Result<()> {
    src.copy_file(dest, previous, current, following)
}

/// `fits_copy_header` / `ffcphd`.
pub fn fits_copy_header(src: &mut FitsFile, dest: &mut FitsFile) -> Result<()> {
    src.copy_header(dest)
}

/// `fits_copy_data` / `ffcpdt`.
pub fn fits_copy_data(src: &mut FitsFile, dest: &mut FitsFile) -> Result<()> {
    src.copy_data(dest)
}

/// `fits_create_hdu` / `ffcrhd`.
pub fn fits_create_hdu(f: &mut FitsFile) -> Result<()> {
    f.create_hdu()
}

/// `fits_write_hdu` / `ffwrhdu`.
pub fn fits_write_hdu<W: Write>(f: &mut FitsFile, writer: &mut W) -> Result<()> {
    f.write_hdu_to(writer)
}

/// `fits_insert_img` / `ffiimg`.
pub fn fits_insert_img(f: &mut FitsFile, bitpix: i32, naxes: &[i64]) -> Result<()> {
    let ty = ImageType::from_code(bitpix).ok_or_else(|| FitsError::new(BAD_BITPIX))?;
    f.insert_image(ty, naxes)
}

/// `fits_insert_atbl` / `ffitab`.
pub fn fits_insert_atbl(
    f: &mut FitsFile,
    nrows: i64,
    ttype: &[&str],
    tform: &[&str],
    tunit: &[Option<&str>],
    extname: Option<&str>,
) -> Result<()> {
    f.insert_ascii_table(nrows, ttype, tform, tunit, extname)
}

/// `fits_insert_btbl` / `ffibin`.
pub fn fits_insert_btbl(
    f: &mut FitsFile,
    nrows: i64,
    ttype: &[&str],
    tform: &[&str],
    tunit: &[Option<&str>],
    extname: Option<&str>,
) -> Result<()> {
    f.insert_binary_table(nrows, ttype, tform, tunit, extname)
}
