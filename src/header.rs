//! In-memory header: a sequence of 80-byte cards plus record padding.

use crate::card::{
    Card, format_fixed_double, format_string_value, make_card_string, parse_value_comment,
};
use crate::error::{FitsError, Result};
use crate::status::{BAD_BITPIX, BAD_KEYCHAR, BAD_NAXIS, KEY_NO_EXIST, NEG_AXIS, NO_END};
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
