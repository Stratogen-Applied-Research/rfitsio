//! In-memory header: a sequence of 80-byte cards plus record padding.

use crate::card::{
    Card, format_complex_exp, format_complex_fixed, format_exp_double, format_fixed_double,
    format_string_value, make_card_string, parse_value_comment, unquote_fits_string,
};
use crate::error::{FitsError, Result};
use crate::status::{
    BAD_BITPIX, BAD_KEYCHAR, BAD_NAXIS, KEY_NO_EXIST, KEY_OUT_BOUNDS, NEG_AXIS, NO_END,
};
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
pub const COMM_BSCALE: &str = "default scaling factor";
pub const COMM_BZERO_USHORT: &str = "offset data range to that of unsigned short";
pub const COMM_BZERO_ULONG: &str = "offset data range to that of unsigned long";
pub const COMM_BZERO_SBYTE: &str = "offset data range to that of signed byte";
pub const BZERO_ULONGLONG_CARD: &str =
    "BZERO   =  9223372036854775808 / offset data range to that of unsigned long long";

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
            if len < 1 {
                return Err(FitsError::new(NEG_AXIS));
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
        let mut out = Vec::with_capacity(RECORD_LEN);
        for card in &self.cards {
            out.extend_from_slice(card.as_bytes());
        }
        out.extend_from_slice(end_card().as_bytes());
        let rem = out.len() % RECORD_LEN;
        if rem != 0 {
            out.resize(out.len() + (RECORD_LEN - rem), b' ');
        }
        if out.is_empty() {
            // END alone still occupies one record.
            out.resize(RECORD_LEN, b' ');
            out[..CARD_LEN].copy_from_slice(end_card().as_bytes());
        }
        out
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

fn name8_bytes(bytes: &[u8]) -> [u8; 8] {
    let mut n = [b' '; 8];
    let len = bytes.len().min(8);
    n[..len].copy_from_slice(&bytes[..len]);
    for b in &mut n {
        *b = b.to_ascii_uppercase();
    }
    n
}
