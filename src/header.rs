//! In-memory header: a sequence of 80-byte cards plus record padding.

use crate::card::{
    Card, format_complex_exp, format_complex_fixed, format_exp_double, format_fixed_double,
    format_string_value, make_card_string, parse_value_comment, unquote_fits_string,
};
use crate::error::{FitsError, Result};
use crate::status::{
    BAD_BITPIX, BAD_KEYCHAR, BAD_NAXES, BAD_NAXIS, BAD_TBCOL, BAD_TFIELDS, BAD_TFORM, KEY_NO_EXIST,
    KEY_OUT_BOUNDS, NEG_AXIS, NEG_ROWS, NEG_WIDTH, NO_END, NO_TBCOL, NO_TFORM, NO_XTENSION,
    NOT_ATABLE, NOT_BTABLE,
};
use crate::tform::{BinaryKind, BinaryTform, ascii_column_starts, binary_column_offsets};
use crate::types::{CARD_LEN, ImageType, RECORD_LEN};

/// CFITSIO `ffphpr` self-documenting COMMENT cards (primary array).
pub const COMMENT_FITS_1: &str =
    "COMMENT   FITS (Flexible Image Transport System) format is defined in 'Astronomy";
pub const COMMENT_FITS_2: &str =
    "COMMENT   and Astrophysics', volume 376, page 359; bibcode: 2001A&A...376..359H";

pub const COMM_SIMPLE: &str = "file does conform to FITS standard";
pub const COMM_BITPIX: &str = "number of bits per data pixel";
pub const COMM_NAXIS: &str = "number of data axes";
pub const COMM_EXTEND: &str = "FITS dataset may contain extensions";
pub const COMM_NAXIS_N: &str = "length of data axis ";
pub const COMM_XTENSION_IMAGE: &str = "IMAGE extension";
pub const COMM_PCOUNT: &str = "required keyword; must = 0";
pub const COMM_GCOUNT: &str = "required keyword; must = 1";
pub const COMM_PCOUNT_GROUP: &str = "number of random group parameters";
pub const COMM_GCOUNT_GROUP: &str = "number of random groups";
pub const COMM_BSCALE: &str = "default scaling factor";
pub const COMM_BZERO_USHORT: &str = "offset data range to that of unsigned short";
pub const COMM_BZERO_ULONG: &str = "offset data range to that of unsigned long";
pub const COMM_BZERO_SBYTE: &str = "offset data range to that of signed byte";
pub const BZERO_ULONGLONG_CARD: &str =
    "BZERO   =  9223372036854775808 / offset data range to that of unsigned long long";

pub const COMM_XTENSION_TABLE: &str = "ASCII table extension";
pub const COMM_BITPIX_ASCII: &str = "8-bit ASCII characters";
pub const COMM_NAXIS_ASCII: &str = "2-dimensional ASCII table";
pub const COMM_NAXIS1_ASCII: &str = "width of table in characters";
pub const COMM_NAXIS2_ASCII: &str = "number of rows in table";
pub const COMM_PCOUNT_ASCII: &str = "no group parameters (required keyword)";
pub const COMM_GCOUNT_ASCII: &str = "one data group (required keyword)";
pub const COMM_TFIELDS: &str = "number of fields in each row";
pub const COMM_TFORM_ASCII: &str = "Fortran-77 format of field";
pub const COMM_TUNIT: &str = "physical unit of field";
pub const COMM_EXTNAME_ASCII: &str = "name of this ASCII table extension";
pub const COMM_TTYPE_INSERTED: &str = "label for field";
pub const COMM_TFORM_INSERTED: &str = "format of field";
pub const COMM_TBCOL_INSERTED: &str = "beginning column of field";

pub const COMM_XTENSION_BINTABLE: &str = "binary table extension";
pub const COMM_BITPIX_BIN: &str = "8-bit bytes";
pub const COMM_NAXIS_BIN: &str = "2-dimensional binary table";
pub const COMM_NAXIS1_BIN: &str = "width of table in bytes";
pub const COMM_NAXIS2_BIN: &str = "number of rows in table";
pub const COMM_PCOUNT_BIN: &str = "size of special data area";
pub const COMM_GCOUNT_BIN: &str = "one data group (required keyword)";
pub const COMM_TFORM_BIN: &str = "data format of field";
pub const COMM_TZERO_UNSIGNED: &str = "offset for unsigned integers";
pub const COMM_TZERO_SBYTE: &str = "offset for signed bytes";
pub const COMM_TSCAL_BIN: &str = "data are not scaled";
pub const COMM_EXTNAME_BIN: &str = "name of this binary table extension";

/// Result of [`Header::primary_info`] (`ffghpr`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryInfo {
    /// SIMPLE.
    pub simple: bool,
    /// BITPIX.
    pub bitpix: i32,
    /// NAXIS.
    pub naxis: i32,
    /// NAXISn.
    pub naxes: Vec<i64>,
    /// PCOUNT (0 if absent).
    pub pcount: i64,
    /// GCOUNT (1 if absent).
    pub gcount: i64,
    /// EXTEND (false if absent).
    pub extend: bool,
}

/// Result of [`Header::ascii_table_info`] (`ffghtb`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsciiTableInfo {
    /// NAXIS1: characters per row.
    pub rowlen: i64,
    /// NAXIS2: number of rows.
    pub nrows: i64,
    /// TFIELDS.
    pub tfields: i32,
    /// TTYPEn (empty string if absent).
    pub ttype: Vec<String>,
    /// TBCOLn (1-based).
    pub tbcol: Vec<i64>,
    /// TFORMn.
    pub tform: Vec<String>,
    /// TUNITn (empty string if absent).
    pub tunit: Vec<String>,
    /// EXTNAME (empty if absent).
    pub extname: String,
}

/// Result of [`Header::binary_table_info`] (`ffghbn`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryTableInfo {
    /// NAXIS2.
    pub nrows: i64,
    /// TFIELDS.
    pub tfields: i32,
    /// TTYPEn.
    pub ttype: Vec<String>,
    /// TFORMn.
    pub tform: Vec<String>,
    /// TUNITn.
    pub tunit: Vec<String>,
    /// EXTNAME.
    pub extname: String,
    /// PCOUNT (heap size).
    pub pcount: i64,
    /// NAXIS1 (row width in bytes).
    pub rowlen: i64,
}

/// Ordered list of header cards, without the END card or fill.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    cards: Vec<Card>,
}

impl Header {
    /// Empty header (no cards yet).
    #[must_use]
    pub fn new() -> Self {
        Self { cards: Vec::new() }
    }

    /// Header from an ordered list of cards (no END).
    #[must_use]
    pub fn from_cards(cards: Vec<Card>) -> Self {
        Self { cards }
    }

    /// True if this header is a primary array (`SIMPLE` present).
    #[must_use]
    pub fn is_primary(&self) -> bool {
        self.card_by_name("SIMPLE").is_some()
    }

    /// CFITSIO empty primary: SIMPLE/BITPIX=8/NAXIS=0/EXTEND plus COMMENTs.
    pub fn empty_primary() -> Result<Self> {
        let mut h = Self::new();
        h.write_logical("SIMPLE", true, Some(COMM_SIMPLE))?;
        h.write_long("BITPIX", 8, Some(COMM_BITPIX))?;
        h.write_long("NAXIS", 0, Some(COMM_NAXIS))?;
        h.write_logical("EXTEND", true, Some(COMM_EXTEND))?;
        h.push(Card::from_text(COMMENT_FITS_1));
        h.push(Card::from_text(COMMENT_FITS_2));
        Ok(h)
    }

    /// Number of keyword cards (not counting END or fill).
    #[must_use]
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// True if no cards have been written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Append a fully formatted card.
    pub fn push(&mut self, card: Card) {
        self.cards.push(card);
    }

    /// Cards in order.
    #[must_use]
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    /// `fits_write_key_log` / `ffpkyl`.
    pub fn write_logical(&mut self, name: &str, value: bool, comm: Option<&str>) -> Result<()> {
        let v = if value { "T" } else { "F" };
        let s = make_card_string(name, v, comm)?;
        self.cards.push(Card::from_text(&s));
        Ok(())
    }

    /// `fits_write_key_lng` / `ffpkyj`.
    pub fn write_long(&mut self, name: &str, value: i64, comm: Option<&str>) -> Result<()> {
        let v = value.to_string();
        let s = make_card_string(name, &v, comm)?;
        self.cards.push(Card::from_text(&s));
        Ok(())
    }

    /// `fits_write_key_str` / `ffpkys`.
    pub fn write_string(&mut self, name: &str, value: &str, comm: Option<&str>) -> Result<()> {
        let v = format_string_value(value);
        let s = make_card_string(name, &v, comm)?;
        self.cards.push(Card::from_text(&s));
        Ok(())
    }

    /// `fits_write_key_fixdbl` / `ffpkyg`.
    pub fn write_fixed_double(
        &mut self,
        name: &str,
        value: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        let v = format_fixed_double(value, decim);
        let s = make_card_string(name, &v, comm)?;
        self.cards.push(Card::from_text(&s));
        Ok(())
    }

    /// Required keywords for a primary image (`ffphps` / `ffcrim` on HDU 0).
    pub fn primary_image(ty: ImageType, naxes: &[i64]) -> Result<Self> {
        if naxes.len() > 999 {
            return Err(FitsError::new(BAD_NAXIS));
        }
        let mut h = Self::new();
        h.write_logical("SIMPLE", true, Some(COMM_SIMPLE))?;
        h.write_long("BITPIX", i64::from(ty.bitpix()), Some(COMM_BITPIX))?;
        h.write_long("NAXIS", naxes.len() as i64, Some(COMM_NAXIS))?;
        for (i, &len) in naxes.iter().enumerate() {
            if len < 1 {
                return Err(FitsError::new(NEG_AXIS));
            }
            let name = format!("NAXIS{}", i + 1);
            let comm = format!("{COMM_NAXIS_N}{}", i + 1);
            h.write_long(&name, len, Some(&comm))?;
        }
        h.write_logical("EXTEND", true, Some(COMM_EXTEND))?;
        h.push(Card::from_text(COMMENT_FITS_1));
        h.push(Card::from_text(COMMENT_FITS_2));
        write_unsigned_scaling(&mut h, ty)?;
        Ok(h)
    }

    /// Required keywords for an IMAGE extension (`ffphpr` when not HDU 0).
    pub fn image_extension(ty: ImageType, naxes: &[i64]) -> Result<Self> {
        if naxes.len() > 999 {
            return Err(FitsError::new(BAD_NAXIS));
        }
        let mut h = Self::new();
        h.write_string("XTENSION", "IMAGE", Some(COMM_XTENSION_IMAGE))?;
        h.write_long("BITPIX", i64::from(ty.bitpix()), Some(COMM_BITPIX))?;
        h.write_long("NAXIS", naxes.len() as i64, Some(COMM_NAXIS))?;
        for (i, &len) in naxes.iter().enumerate() {
            if len < 0 {
                return Err(FitsError::new(BAD_NAXES));
            }
            let name = format!("NAXIS{}", i + 1);
            let comm = format!("{COMM_NAXIS_N}{}", i + 1);
            h.write_long(&name, len, Some(&comm))?;
        }
        h.write_long("PCOUNT", 0, Some(COMM_PCOUNT))?;
        h.write_long("GCOUNT", 1, Some(COMM_GCOUNT))?;
        write_unsigned_scaling(&mut h, ty)?;
        Ok(h)
    }

    /// Required keywords for an ASCII table extension (`ffphtb`).
    ///
    /// TBCOL values and NAXIS1 are computed with one blank between columns
    /// (`ffgabc` with `space = 1`).
    pub fn ascii_table(
        nrows: i64,
        ttype: &[&str],
        tform: &[&str],
        tunit: &[Option<&str>],
        extname: Option<&str>,
    ) -> Result<Self> {
        if nrows < 0 {
            return Err(FitsError::new(NEG_ROWS));
        }
        let tfields = tform.len();
        if tfields > 999 {
            return Err(FitsError::new(BAD_TFIELDS));
        }
        for tf in tform {
            if tf.len() > 29 {
                return Err(FitsError::with_message(
                    BAD_TFORM,
                    "Error: ASCII table TFORM code is too long (ffphtb)",
                ));
            }
        }
        let (rowlen, tbcol) = ascii_column_starts(tform, 1)?;
        if rowlen < 0 {
            return Err(FitsError::new(NEG_WIDTH));
        }
        let mut h = Self::new();
        h.write_string("XTENSION", "TABLE", Some(COMM_XTENSION_TABLE))?;
        h.write_long("BITPIX", 8, Some(COMM_BITPIX_ASCII))?;
        h.write_long("NAXIS", 2, Some(COMM_NAXIS_ASCII))?;
        h.write_long("NAXIS1", rowlen, Some(COMM_NAXIS1_ASCII))?;
        h.write_long("NAXIS2", nrows, Some(COMM_NAXIS2_ASCII))?;
        h.write_long("PCOUNT", 0, Some(COMM_PCOUNT_ASCII))?;
        h.write_long("GCOUNT", 1, Some(COMM_GCOUNT_ASCII))?;
        h.write_long("TFIELDS", tfields as i64, Some(COMM_TFIELDS))?;
        for (ii, tf) in tform.iter().enumerate() {
            let n = ii + 1;
            let ttype_s = ttype.get(ii).copied().unwrap_or("");
            if !ttype_s.is_empty() {
                let comm = format!("label for field {n:3}");
                h.write_string(&format!("TTYPE{n}"), ttype_s, Some(&comm))?;
            }
            let col = tbcol[ii];
            if col < 1 || (rowlen > 0 && col > rowlen) {
                return Err(FitsError::new(BAD_TBCOL));
            }
            let comm = format!("beginning column of field {n:3}");
            h.write_long(&format!("TBCOL{n}"), col, Some(&comm))?;
            let tfmt = tf.to_ascii_uppercase();
            h.write_string(&format!("TFORM{n}"), &tfmt, Some(COMM_TFORM_ASCII))?;
            if let Some(&Some(unit)) = tunit.get(ii) {
                if !unit.is_empty() {
                    h.write_string(&format!("TUNIT{n}"), unit, Some(COMM_TUNIT))?;
                }
            }
        }
        if let Some(name) = extname {
            if !name.is_empty() {
                h.write_string("EXTNAME", name, Some(COMM_EXTNAME_ASCII))?;
            }
        }
        Ok(h)
    }

    /// ASCII-table required keywords (`fits_read_atblhdr` / `ffghtb`).
    pub fn ascii_table_info(&self) -> Result<AsciiTableInfo> {
        let first = self
            .cards
            .first()
            .ok_or_else(|| FitsError::new(NO_XTENSION))?;
        let (fname, _) = first.keyword_name()?;
        if !fname.eq_ignore_ascii_case("XTENSION") {
            return Err(FitsError::new(NO_XTENSION));
        }
        let (xt, _) = self.get_string("XTENSION")?;
        if xt.trim() != "TABLE" {
            return Err(FitsError::new(NOT_ATABLE));
        }
        let rowlen = self.get_i64("NAXIS1")?;
        let nrows = self.get_i64("NAXIS2")?;
        let tfields_i = self.get_i64("TFIELDS")?;
        if !(0..=999).contains(&tfields_i) {
            return Err(FitsError::new(BAD_TFIELDS));
        }
        let tfields = tfields_i as i32;
        let mut ttype = Vec::new();
        let mut tbcol = Vec::new();
        let mut tform = Vec::new();
        let mut tunit = Vec::new();
        for i in 1..=tfields {
            ttype.push(
                self.get_string(&format!("TTYPE{i}"))
                    .map(|(v, _)| v)
                    .unwrap_or_default(),
            );
            tbcol.push(
                self.get_i64(&format!("TBCOL{i}"))
                    .map_err(|_| FitsError::new(NO_TBCOL))?,
            );
            tform.push(
                self.get_string(&format!("TFORM{i}"))
                    .map(|(v, _)| v)
                    .map_err(|_| FitsError::new(NO_TFORM))?,
            );
            tunit.push(
                self.get_string(&format!("TUNIT{i}"))
                    .map(|(v, _)| v)
                    .unwrap_or_default(),
            );
        }
        let extname = self
            .get_string("EXTNAME")
            .map(|(v, _)| v)
            .unwrap_or_default();
        Ok(AsciiTableInfo {
            rowlen,
            nrows,
            tfields,
            ttype,
            tbcol,
            tform,
            tunit,
            extname,
        })
    }

    /// Required keywords for a binary table extension (`ffphbn`).
    pub fn binary_table(
        nrows: i64,
        ttype: &[&str],
        tform: &[&str],
        tunit: &[Option<&str>],
        extname: Option<&str>,
    ) -> Result<Self> {
        if nrows < 0 {
            return Err(FitsError::new(NEG_ROWS));
        }
        let tfields = tform.len();
        if tfields > 999 {
            return Err(FitsError::new(BAD_TFIELDS));
        }
        for tf in tform {
            if tf.len() > 29 {
                return Err(FitsError::with_message(
                    BAD_TFORM,
                    "Error: BIN table TFORM code is too long (ffphbn)",
                ));
            }
        }
        let (rowlen, _) = binary_column_offsets(tform)?;
        let mut h = Self::new();
        h.write_string("XTENSION", "BINTABLE", Some(COMM_XTENSION_BINTABLE))?;
        h.write_long("BITPIX", 8, Some(COMM_BITPIX_BIN))?;
        h.write_long("NAXIS", 2, Some(COMM_NAXIS_BIN))?;
        h.write_long("NAXIS1", rowlen, Some(COMM_NAXIS1_BIN))?;
        h.write_long("NAXIS2", nrows, Some(COMM_NAXIS2_BIN))?;
        h.write_long("PCOUNT", 0, Some(COMM_PCOUNT_BIN))?;
        h.write_long("GCOUNT", 1, Some(COMM_GCOUNT_BIN))?;
        h.write_long("TFIELDS", tfields as i64, Some(COMM_TFIELDS))?;
        for (ii, tf) in tform.iter().enumerate() {
            let n = ii + 1;
            let ttype_s = ttype.get(ii).copied().unwrap_or("");
            if !ttype_s.is_empty() {
                let comm = format!("label for field {n:3}");
                h.write_string(&format!("TTYPE{n}"), ttype_s, Some(&comm))?;
            }
            let parsed = BinaryTform::parse(tf)?;
            let stored = BinaryTform::stored_code(tf);
            let comm = format!("{COMM_TFORM_BIN}{}", parsed.tform_comment_suffix());
            h.write_string(&format!("TFORM{n}"), &stored, Some(&comm))?;
            match parsed.kind {
                BinaryKind::S => {
                    h.write_fixed_double(&format!("TZERO{n}"), -128.0, 0, Some(COMM_TZERO_SBYTE))?;
                    h.write_fixed_double(&format!("TSCAL{n}"), 1.0, 0, Some(COMM_TSCAL_BIN))?;
                }
                BinaryKind::U => {
                    h.write_fixed_double(
                        &format!("TZERO{n}"),
                        32768.0,
                        0,
                        Some(COMM_TZERO_UNSIGNED),
                    )?;
                    h.write_fixed_double(&format!("TSCAL{n}"), 1.0, 0, Some(COMM_TSCAL_BIN))?;
                }
                BinaryKind::V => {
                    h.write_fixed_double(
                        &format!("TZERO{n}"),
                        2_147_483_648.0,
                        0,
                        Some(COMM_TZERO_UNSIGNED),
                    )?;
                    h.write_fixed_double(&format!("TSCAL{n}"), 1.0, 0, Some(COMM_TSCAL_BIN))?;
                }
                BinaryKind::W => {
                    let name = format!("TZERO{n}");
                    let card =
                        format!("{name:<8}=  9223372036854775808 / offset for unsigned integers");
                    h.push(Card::from_text(&card));
                    h.write_fixed_double(&format!("TSCAL{n}"), 1.0, 0, Some(COMM_TSCAL_BIN))?;
                }
                _ => {}
            }
            if let Some(&Some(unit)) = tunit.get(ii) {
                if !unit.is_empty() {
                    h.write_string(&format!("TUNIT{n}"), unit, Some(COMM_TUNIT))?;
                }
            }
        }
        if let Some(name) = extname {
            if !name.is_empty() {
                h.write_string("EXTNAME", name, Some(COMM_EXTNAME_BIN))?;
            }
        }
        Ok(h)
    }

    /// Binary-table required keywords (`fits_read_btblhdr` / `ffghbn`).
    pub fn binary_table_info(&self) -> Result<BinaryTableInfo> {
        let first = self
            .cards
            .first()
            .ok_or_else(|| FitsError::new(NO_XTENSION))?;
        let (fname, _) = first.keyword_name()?;
        if !fname.eq_ignore_ascii_case("XTENSION") {
            return Err(FitsError::new(NO_XTENSION));
        }
        let (xt, _) = self.get_string("XTENSION")?;
        let xt = xt.trim();
        if xt != "BINTABLE" && xt != "A3DTABLE" && xt != "3DTABLE" {
            return Err(FitsError::new(NOT_BTABLE));
        }
        let rowlen = self.get_i64("NAXIS1")?;
        let nrows = self.get_i64("NAXIS2")?;
        let pcount = self.get_i64("PCOUNT").unwrap_or(0);
        let tfields_i = self.get_i64("TFIELDS")?;
        if !(0..=999).contains(&tfields_i) {
            return Err(FitsError::new(BAD_TFIELDS));
        }
        let tfields = tfields_i as i32;
        let mut ttype = Vec::new();
        let mut tform = Vec::new();
        let mut tunit = Vec::new();
        for i in 1..=tfields {
            ttype.push(
                self.get_string(&format!("TTYPE{i}"))
                    .map(|(v, _)| v)
                    .unwrap_or_default(),
            );
            tform.push(
                self.get_string(&format!("TFORM{i}"))
                    .map(|(v, _)| v)
                    .map_err(|_| FitsError::new(NO_TFORM))?,
            );
            tunit.push(
                self.get_string(&format!("TUNIT{i}"))
                    .map(|(v, _)| v)
                    .unwrap_or_default(),
            );
        }
        let extname = self
            .get_string("EXTNAME")
            .map(|(v, _)| v)
            .unwrap_or_default();
        Ok(BinaryTableInfo {
            nrows,
            tfields,
            ttype,
            tform,
            tunit,
            extname,
            pcount,
            rowlen,
        })
    }

    /// Replace a long keyword, keeping its existing comment (`ffmkyj` with `"&"`).
    pub fn update_long_keep_comment(&mut self, name: &str, value: i64) -> Result<()> {
        let comm = match self.raw_value_comment(name) {
            Ok((_, c)) if !c.is_empty() => Some(c),
            Ok(_) => None,
            Err(e) => return Err(e),
        };
        let s = make_card_string(name, &value.to_string(), comm.as_deref())?;
        self.replace_name(name, Card::from_text(&s))
    }

    /// Shift `Txxxn` keyword indices (`ffkshf`). Starts at the 9th card.
    ///
    /// If `incre <= 0`, keywords with index `colmin` are deleted.
    pub fn shift_table_col_keys(&mut self, colmin: i32, colmax: i32, incre: i32) -> Result<()> {
        let mut i = 8usize;
        while i < self.cards.len() {
            let bytes = *self.cards[i].as_bytes();
            if bytes[0] != b'T' {
                i += 1;
                continue;
            }
            let Some((prefix_len, ivalue)) = tkey_index(&bytes) else {
                i += 1;
                continue;
            };
            if ivalue < colmin || ivalue > colmax {
                i += 1;
                continue;
            }
            if incre <= 0 && ivalue == colmin {
                self.cards.remove(i);
                continue;
            }
            let new_idx = ivalue + incre;
            let prefix = std::str::from_utf8(&bytes[0..prefix_len]).unwrap_or("");
            let new_name = format!("{prefix}{new_idx}");
            self.cards[i].set_keyword_name(&new_name);
            i += 1;
        }
        Ok(())
    }

    /// First card whose 8-character name matches `name` (case-insensitive).
    #[must_use]
    pub fn card_by_name(&self, name: &str) -> Option<&Card> {
        let key = name8(name);
        self.cards
            .iter()
            .find(|c| name8_bytes(&c.as_bytes()[..8]) == key)
    }

    /// Integer keyword value (`fits_read_key_lng`).
    pub fn get_i64(&self, name: &str) -> Result<i64> {
        let card = self
            .card_by_name(name)
            .ok_or_else(|| FitsError::new(KEY_NO_EXIST))?;
        let text = card.as_str().unwrap_or("");
        let (val, _) = parse_value_comment(text)?;
        val.trim()
            .parse::<i64>()
            .map_err(|_| FitsError::new(crate::status::BAD_INTKEY))
    }

    /// Floating keyword value (`fits_read_key_dbl`).
    pub fn get_f64(&self, name: &str) -> Result<f64> {
        let card = self
            .card_by_name(name)
            .ok_or_else(|| FitsError::new(KEY_NO_EXIST))?;
        let text = card.as_str().unwrap_or("");
        let (val, _) = parse_value_comment(text)?;
        val.trim()
            .parse::<f64>()
            .map_err(|_| FitsError::new(crate::status::BAD_FLOATKEY))
    }

    /// NAXIS axis lengths.
    pub fn naxes(&self) -> Result<Vec<i64>> {
        let n = self.get_i64("NAXIS")?;
        if n < 0 {
            return Err(FitsError::new(BAD_NAXIS));
        }
        let mut out = Vec::with_capacity(n as usize);
        for i in 1..=n {
            out.push(self.get_i64(&format!("NAXIS{i}"))?);
        }
        Ok(out)
    }

    /// Stored BITPIX.
    pub fn bitpix(&self) -> Result<i32> {
        let v = self.get_i64("BITPIX")?;
        i32::try_from(v).map_err(|_| FitsError::new(BAD_BITPIX))
    }

    /// BSCALE, default 1.0.
    pub fn bscale(&self) -> f64 {
        self.get_f64("BSCALE").unwrap_or(1.0)
    }

    /// BZERO, default 0.0. ULONGLONG's integer BZERO is returned as f64.
    pub fn bzero(&self) -> f64 {
        self.get_f64("BZERO").unwrap_or(0.0)
    }

    /// BLANK keyword if present.
    pub fn blank(&self) -> Option<i64> {
        self.get_i64("BLANK").ok()
    }

    /// Replace or append a BLANK keyword.
    pub fn set_blank(&mut self, value: i64) -> Result<()> {
        self.upsert_long("BLANK", value, Some("undefined pixel value"))
    }

    /// Replace the first card of this name, or append.
    pub fn upsert_long(&mut self, name: &str, value: i64, comm: Option<&str>) -> Result<()> {
        let v = value.to_string();
        let s = make_card_string(name, &v, comm)?;
        let card = Card::from_text(&s);
        let key = name8(name);
        if let Some(i) = self
            .cards
            .iter()
            .position(|c| name8_bytes(&c.as_bytes()[..8]) == key)
        {
            self.cards[i] = card;
        } else {
            self.cards.push(card);
        }
        Ok(())
    }

    /// `fits_write_key_flt` / `ffpkye` (exponential).
    pub fn write_float_exp(
        &mut self,
        name: &str,
        value: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        let v = format_exp_double(value, decim);
        self.push_made(name, &v, comm)
    }

    /// `fits_write_key_null` / `ffpkyu`.
    pub fn write_null(&mut self, name: &str, comm: Option<&str>) -> Result<()> {
        self.push_made(name, " ", comm)
    }

    /// `fits_write_comment` / `ffpcom`.
    pub fn write_comment(&mut self, comm: &str) {
        for chunk in comm.as_bytes().chunks(72) {
            let mut card = String::from("COMMENT ");
            card.push_str(std::str::from_utf8(chunk).unwrap_or(""));
            self.cards.push(Card::from_text(&card));
        }
    }

    /// `fits_write_history` / `ffphis`.
    pub fn write_history(&mut self, hist: &str) {
        for chunk in hist.as_bytes().chunks(72) {
            let mut card = String::from("HISTORY ");
            card.push_str(std::str::from_utf8(chunk).unwrap_or(""));
            self.cards.push(Card::from_text(&card));
        }
    }

    /// `fits_write_record` / `ffprec`.
    pub fn write_record(&mut self, card: &str) {
        self.cards.push(Card::from_text(card));
    }

    /// Complex exponential (`ffpkyc` / `ffpkym`).
    pub fn write_complex_exp(
        &mut self,
        name: &str,
        real: f64,
        imag: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        let v = format_complex_exp(real, imag, decim);
        self.push_made(name, &v, comm)
    }

    /// Complex fixed (`ffpkfc` / `ffpkfm`).
    pub fn write_complex_fixed(
        &mut self,
        name: &str,
        real: f64,
        imag: f64,
        decim: usize,
        comm: Option<&str>,
    ) -> Result<()> {
        let v = format_complex_fixed(real, imag, decim);
        self.push_made(name, &v, comm)
    }

    /// Unsigned 64-bit integer keyword.
    pub fn write_ulong(&mut self, name: &str, value: u64, comm: Option<&str>) -> Result<()> {
        self.push_made(name, &value.to_string(), comm)
    }

    fn push_made(&mut self, name: &str, value: &str, comm: Option<&str>) -> Result<()> {
        let s = make_card_string(name, value, comm)?;
        self.cards.push(Card::from_text(&s));
        Ok(())
    }

    /// 1-based keyword record (`fits_read_record`). Does not include END.
    pub fn record(&self, n: usize) -> Result<Card> {
        self.cards
            .get(n.wrapping_sub(1))
            .copied()
            .ok_or_else(|| FitsError::new(KEY_OUT_BOUNDS))
    }

    /// Unquoted string value (`fits_read_key_str`).
    pub fn get_string(&self, name: &str) -> Result<(String, String)> {
        let (val, comm) = self.raw_value_comment(name)?;
        Ok((unquote_fits_string(&val), comm))
    }

    /// Logical value (`fits_read_key_log`).
    pub fn get_logical(&self, name: &str) -> Result<(bool, String)> {
        let (val, comm) = self.raw_value_comment(name)?;
        let v = val.trim();
        match v.as_bytes().first() {
            Some(b'T' | b't') => Ok((true, comm)),
            Some(b'F' | b'f') => Ok((false, comm)),
            _ => Err(FitsError::new(crate::status::BAD_LOGICALKEY)),
        }
    }

    /// Raw value + comment strings (`fits_read_keyword`).
    pub fn raw_value_comment(&self, name: &str) -> Result<(String, String)> {
        let card = self
            .card_by_name(name)
            .ok_or_else(|| FitsError::new(KEY_NO_EXIST))?;
        parse_value_comment(card.as_str().unwrap_or(""))
    }

    /// Units from `[...]` in the comment (`fits_read_key_unit`).
    pub fn get_unit(&self, name: &str) -> Result<String> {
        let (_, comm) = self.raw_value_comment(name)?;
        if let Some(rest) = comm.strip_prefix('[') {
            if let Some(end) = rest.find(']') {
                return Ok(rest[..end].to_string());
            }
        }
        Ok(String::new())
    }

    /// Prepend `[unit] ` to the comment (`fits_write_key_unit`).
    pub fn set_unit(&mut self, name: &str, unit: &str) -> Result<()> {
        let card = *self
            .card_by_name(name)
            .ok_or_else(|| FitsError::new(KEY_NO_EXIST))?;
        let text = card.as_str().unwrap_or("");
        let (val, oldcomm) = parse_value_comment(text)?;
        let rest = if let Some(stripped) = oldcomm.strip_prefix('[') {
            if let Some(idx) = stripped.find(']') {
                stripped[idx + 1..].trim_start().to_string()
            } else {
                oldcomm
            }
        } else {
            oldcomm
        };
        let newcomm = if unit.is_empty() {
            rest
        } else {
            let mut c = format!("[{unit}] ");
            c.push_str(&rest);
            c
        };
        let s = make_card_string(name, &val, Some(&newcomm))?;
        self.replace_name(name, Card::from_text(&s))?;
        Ok(())
    }

    /// Update: replace if present, otherwise append (`fits_update_key`).
    pub fn update_with(&mut self, name: &str, value: &str, comm: Option<&str>) -> Result<()> {
        let s = make_card_string(name, value, comm)?;
        let card = Card::from_text(&s);
        if self.card_by_name(name).is_some() {
            self.replace_name(name, card)
        } else {
            self.cards.push(card);
            Ok(())
        }
    }

    /// Modify an existing keyword; error if missing (`fits_modify_key`).
    pub fn modify_with(&mut self, name: &str, value: &str, comm: Option<&str>) -> Result<()> {
        let s = make_card_string(name, value, comm)?;
        self.replace_name(name, Card::from_text(&s))
    }

    /// Insert `card` at 1-based `pos` (before that record). `pos > nkeys`
    /// appends.
    pub fn insert_record_at(&mut self, pos: usize, card: Card) -> Result<()> {
        let idx = pos.saturating_sub(1);
        if idx >= self.cards.len() {
            self.cards.push(card);
        } else {
            self.cards.insert(idx, card);
        }
        Ok(())
    }

    /// Delete first keyword with this name (`fits_delete_key`).
    pub fn delete_key(&mut self, name: &str) -> Result<()> {
        let key = name8(name);
        let idx = self
            .cards
            .iter()
            .position(|c| name8_bytes(&c.as_bytes()[..8]) == key)
            .ok_or_else(|| FitsError::new(KEY_NO_EXIST))?;
        self.cards.remove(idx);
        Ok(())
    }

    /// Delete 1-based record (`fits_delete_record`).
    pub fn delete_record(&mut self, n: usize) -> Result<()> {
        let idx = n.wrapping_sub(1);
        if idx >= self.cards.len() {
            return Err(FitsError::new(KEY_OUT_BOUNDS));
        }
        self.cards.remove(idx);
        Ok(())
    }

    /// Replace first matching name.
    pub fn replace_name(&mut self, name: &str, card: Card) -> Result<()> {
        let key = name8(name);
        let idx = self
            .cards
            .iter()
            .position(|c| name8_bytes(&c.as_bytes()[..8]) == key)
            .ok_or_else(|| FitsError::new(KEY_NO_EXIST))?;
        self.cards[idx] = card;
        Ok(())
    }

    /// Write DATE with the given timestamp value (`fits_write_date` body).
    pub fn write_date_value(&mut self, datetime: &str) -> Result<()> {
        let comm = "file creation date (YYYY-MM-DDThh:mm:ss UT)";
        let v = format_string_value(datetime);
        self.update_with("DATE", &v, Some(comm))
    }

    /// `fits_write_keys_lng`: KEY1, KEY2, ... from `start`.
    pub fn write_keys_long(
        &mut self,
        root: &str,
        start: i32,
        values: &[i64],
        comms: &[Option<&str>],
    ) -> Result<()> {
        for (i, val) in values.iter().enumerate() {
            let name = format!("{root}{}", start as usize + i);
            let comm = comms.get(i).copied().flatten();
            self.write_long(&name, *val, comm)?;
        }
        Ok(())
    }

    /// Read KEY{start}.. as integers. Returns how many were found.
    pub fn get_keys_long(&self, root: &str, start: i32, nmax: usize) -> (Vec<i64>, usize) {
        let mut out = Vec::new();
        for i in 0..nmax {
            let name = format!("{root}{}", start as usize + i);
            match self.get_i64(&name) {
                Ok(v) => out.push(v),
                Err(_) => break,
            }
        }
        let n = out.len();
        (out, n)
    }

    /// Primary-array required keywords (`fits_read_imghdr` / `ffghpr`).
    pub fn primary_info(&self) -> Result<PrimaryInfo> {
        let simple = self.get_logical("SIMPLE").map(|(v, _)| v).unwrap_or(true);
        let bitpix = self.bitpix()?;
        let naxes = self.naxes()?;
        let naxis = naxes.len() as i32;
        let pcount = self.get_i64("PCOUNT").unwrap_or(0);
        let gcount = self.get_i64("GCOUNT").unwrap_or(1);
        let extend = self.get_logical("EXTEND").map(|(v, _)| v).unwrap_or(false);
        Ok(PrimaryInfo {
            simple,
            bitpix,
            naxis,
            naxes,
            pcount,
            gcount,
            extend,
        })
    }

    /// Infer [`ImageType`] from BITPIX + BZERO.
    pub fn image_type(&self) -> Result<ImageType> {
        let bp = self.bitpix()?;
        let z = self.bzero();
        Ok(match bp {
            8 if (z + 128.0).abs() < 0.5 => ImageType::I8,
            8 => ImageType::U8,
            16 if (z - 32768.0).abs() < 0.5 => ImageType::U16,
            16 => ImageType::I16,
            32 if (z - 2_147_483_648.0).abs() < 0.5 => ImageType::U32,
            32 => ImageType::I32,
            64 if z > 1e18 => ImageType::U64,
            64 => ImageType::I64,
            -32 => ImageType::F32,
            -64 => ImageType::F64,
            _ => return Err(FitsError::new(BAD_BITPIX)),
        })
    }

    /// Serialize cards + END + space fill to a multiple of 2880 bytes.
    #[must_use]
    pub fn to_record_bytes(&self) -> Vec<u8> {
        self.to_record_bytes_with_morekeys(0)
    }

    /// Like [`Self::to_record_bytes`], reserving `morekeys` extra 80-byte slots
    /// after `END` (`fits_write_hdu_space` / `ffhdef`).
    #[must_use]
    pub fn to_record_bytes_with_morekeys(&self, morekeys: i32) -> Vec<u8> {
        let mut out = Vec::with_capacity(RECORD_LEN);
        for card in &self.cards {
            out.extend_from_slice(card.as_bytes());
        }
        out.extend_from_slice(end_card().as_bytes());
        let extra = morekeys.max(0) as usize * CARD_LEN;
        let min_len = out.len() + extra;
        let padded = min_len.div_ceil(RECORD_LEN).max(1) * RECORD_LEN;
        out.resize(padded, b' ');
        out
    }

    /// Remaining keyword slots in an on-disk header of `header_len` bytes
    /// (`ffghsp` `nmore`, not counting `END`).
    #[must_use]
    pub fn nmore_in(&self, header_len: u64) -> i32 {
        let nmore = (header_len / CARD_LEN as u64) as i32 - self.cards.len() as i32 - 1;
        nmore.max(0)
    }

    /// Convert a primary-array header into an IMAGE extension (`ffcphd`).
    pub fn primary_as_image_extension(&self) -> Result<Self> {
        let naxis = self.get_i64("NAXIS").unwrap_or(0).max(0) as usize;
        let mut out = Self::new();
        out.write_string("XTENSION", "IMAGE", Some(COMM_XTENSION_IMAGE))?;
        let start = 1.min(self.cards.len());
        let struct_end = (3 + naxis).min(self.cards.len());
        if start < struct_end {
            for card in &self.cards[start..struct_end] {
                out.cards.push(*card);
            }
        }
        out.write_long("PCOUNT", 0, Some(COMM_PCOUNT_GROUP))?;
        out.write_long("GCOUNT", 1, Some(COMM_GCOUNT_GROUP))?;
        for card in &self.cards[struct_end..] {
            if card_name_is(card, "EXTEND") || is_std_fits_comment(card) {
                continue;
            }
            out.cards.push(*card);
        }
        Ok(out)
    }

    /// Convert an IMAGE extension header into a primary array (`ffcphd`).
    pub fn image_extension_as_primary(&self) -> Result<Self> {
        let naxis = self.get_i64("NAXIS").unwrap_or(0).max(0) as usize;
        let mut out = Self::new();
        out.write_logical("SIMPLE", true, Some(COMM_SIMPLE))?;
        let struct_end = (3 + naxis).min(self.cards.len());
        let start = 1.min(self.cards.len());
        for card in &self.cards[start..struct_end] {
            out.cards.push(*card);
        }
        out.write_logical("EXTEND", true, Some(COMM_EXTEND))?;
        out.push(Card::from_text(COMMENT_FITS_1));
        out.push(Card::from_text(COMMENT_FITS_2));
        for card in &self.cards[struct_end..] {
            if card_name_is(card, "PCOUNT") || card_name_is(card, "GCOUNT") {
                continue;
            }
            out.cards.push(*card);
        }
        Ok(out)
    }

    /// Parse a header from the start of a FITS file image.
    ///
    /// Reads 80-byte cards until END. Returns the header and the byte
    /// offset of the following data unit (a multiple of 2880).
    pub fn parse(bytes: &[u8]) -> Result<(Self, u64)> {
        if bytes.len() < CARD_LEN {
            return Err(FitsError::new(NO_END));
        }
        let mut cards = Vec::new();
        let mut off = 0usize;
        loop {
            if off + CARD_LEN > bytes.len() {
                return Err(FitsError::new(NO_END));
            }
            let slice = &bytes[off..off + CARD_LEN];
            off += CARD_LEN;
            if is_end_card(slice) {
                break;
            }
            let mut card_bytes = [b' '; CARD_LEN];
            card_bytes.copy_from_slice(slice);
            cards.push(Card::from_text(
                std::str::from_utf8(&card_bytes).map_err(|_| FitsError::new(BAD_KEYCHAR))?,
            ));
        }
        let recs = off.div_ceil(RECORD_LEN).max(1);
        let data_start = (recs * RECORD_LEN) as u64;
        Ok((Self { cards }, data_start))
    }
}

fn end_card() -> Card {
    Card::from_text("END")
}

fn card_name_is(card: &Card, name: &str) -> bool {
    name8_bytes(&card.as_bytes()[..8]) == name8(name)
}

fn is_std_fits_comment(card: &Card) -> bool {
    let s = card.as_str().unwrap_or("");
    s.starts_with("COMMENT   FITS (Flexible Image Transport System) format is")
        || s.starts_with("COMMENT   and Astrophysics', volume 376, page 3")
}

fn is_end_card(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes.starts_with(b"END") && bytes[3..8].iter().all(|&b| b == b' ')
}

fn write_unsigned_scaling(h: &mut Header, ty: ImageType) -> Result<()> {
    match ty {
        ImageType::U16 => {
            h.write_fixed_double("BZERO", 32768.0, 0, Some(COMM_BZERO_USHORT))?;
            h.write_fixed_double("BSCALE", 1.0, 0, Some(COMM_BSCALE))?;
        }
        ImageType::U32 => {
            h.write_fixed_double("BZERO", 2_147_483_648.0, 0, Some(COMM_BZERO_ULONG))?;
            h.write_fixed_double("BSCALE", 1.0, 0, Some(COMM_BSCALE))?;
        }
        ImageType::U64 => {
            h.push(Card::from_text(BZERO_ULONGLONG_CARD));
            h.write_fixed_double("BSCALE", 1.0, 0, Some(COMM_BSCALE))?;
        }
        ImageType::I8 => {
            h.write_fixed_double("BZERO", -128.0, 0, Some(COMM_BZERO_SBYTE))?;
            h.write_fixed_double("BSCALE", 1.0, 0, Some(COMM_BSCALE))?;
        }
        _ => {}
    }
    Ok(())
}

fn name8(name: &str) -> [u8; 8] {
    name8_bytes(name.as_bytes())
}

/// Parse a `Txxxn` / `TDIMn` name. Returns `(prefix_len, index)`.
fn tkey_index(bytes: &[u8; 80]) -> Option<(usize, i32)> {
    let rec = bytes;
    let q = &rec[1..5];
    let i1 = if q == b"BCOL"
        || q == b"FORM"
        || q == b"TYPE"
        || q == b"SCAL"
        || q == b"UNIT"
        || q == b"NULL"
        || q == b"ZERO"
        || q == b"DISP"
        || q == b"LMIN"
        || q == b"LMAX"
        || q == b"DMIN"
        || q == b"DMAX"
        || q == b"CTYP"
        || q == b"CRPX"
        || q == b"CRVL"
        || q == b"CDLT"
        || q == b"CROT"
        || q == b"CUNI"
    {
        5usize
    } else if rec.starts_with(b"TDIM") {
        4usize
    } else {
        return None;
    };
    let mut suffix = String::new();
    for &b in rec[i1..8].iter() {
        if b == b' ' {
            break;
        }
        suffix.push(b as char);
    }
    let ivalue = suffix.parse::<i32>().ok()?;
    Some((i1, ivalue))
}

fn name8_bytes(bytes: &[u8]) -> [u8; 8] {
    let mut n = [b' '; 8];
    let len = bytes.len().min(8);
    n[..len].copy_from_slice(&bytes[..len]);
    for b in &mut n {
        *b = b.to_ascii_uppercase();
    }
    n
}
