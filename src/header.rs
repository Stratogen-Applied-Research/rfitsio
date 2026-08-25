//! In-memory header: a sequence of 80-byte cards plus record padding.

use crate::card::{Card, make_card_string};
use crate::error::{FitsError, Result};
use crate::status::{BAD_KEYCHAR, NO_END};
use crate::types::{CARD_LEN, RECORD_LEN};

/// CFITSIO `ffphpr` self-documenting COMMENT cards (primary array).
pub const COMMENT_FITS_1: &str =
    "COMMENT   FITS (Flexible Image Transport System) format is defined in 'Astronomy";
pub const COMMENT_FITS_2: &str =
    "COMMENT   and Astrophysics', volume 376, page 359; bibcode: 2001A&A...376..359H";

pub const COMM_SIMPLE: &str = "file does conform to FITS standard";
pub const COMM_BITPIX: &str = "number of bits per data pixel";
pub const COMM_NAXIS: &str = "number of data axes";
pub const COMM_EXTEND: &str = "FITS dataset may contain extensions";

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
