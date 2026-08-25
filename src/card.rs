//! 80-byte FITS header cards: parse, format, classify.
//!
//! Formatting matches CFITSIO 4.7.0 `ffmkky`. Parsing matches `ffpsvc`.
//! Keyword classification matches `ffgkcl`.

use crate::error::{FitsError, Result};
use crate::status::BAD_KEYCHAR;
use crate::types::{
    CARD_LEN, FLEN_CARD, FLEN_COMMENT, FLEN_KEYWORD, FLEN_VALUE, KeyClass, TYP_CKSUM_KEY,
    TYP_CMPRS_KEY, TYP_COMM_KEY, TYP_CONT_KEY, TYP_DIM_KEY, TYP_DISP_KEY, TYP_HDUID_KEY,
    TYP_NULL_KEY, TYP_RANG_KEY, TYP_REFSYS_KEY, TYP_SCAL_KEY, TYP_STRUC_KEY, TYP_UNIT_KEY,
    TYP_USER_KEY, TYP_WCS_KEY,
};

/// An 80-byte FITS header card (space-padded, no trailing NUL).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Card {
    bytes: [u8; CARD_LEN],
}

impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.as_str().unwrap_or("<non-ascii>");
        f.debug_tuple("Card").field(&s).finish()
    }
}

impl Card {
    /// Space-filled card.
    #[must_use]
    pub fn blank() -> Self {
        Self {
            bytes: [b' '; CARD_LEN],
        }
    }

    /// Pad `text` to 80 bytes with spaces. Truncates if longer.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut bytes = [b' '; CARD_LEN];
        let src = text.as_bytes();
        let n = src.len().min(CARD_LEN);
        bytes[..n].copy_from_slice(&src[..n]);
        Self { bytes }
    }

    /// Construct a card from name, pre-formatted value, and optional comment.
    ///
    /// `value` is the CFITSIO value string (e.g. `'NGC3372'`, `T`,
    /// `                  16`). This is `fits_make_key` / `ffmkky`.
    pub fn make(name: &str, value: &str, comment: Option<&str>) -> Result<Self> {
        let s = make_card_string(name, value, comment)?;
        Ok(Self::from_text(&s))
    }

    /// Bytes of the on-disk card.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; CARD_LEN] {
        &self.bytes
    }

    /// UTF-8 view; FITS cards are ASCII.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes).ok()
    }

    /// CFITSIO `ffmkky` form: trailing spaces stripped.
    #[must_use]
    pub fn to_cfitsio_string(&self) -> String {
        let s = self.as_str().unwrap_or("");
        s.trim_end_matches(' ').to_string()
    }

    /// `fits_get_keyclass` / `ffgkcl`.
    #[must_use]
    pub fn class(&self) -> KeyClass {
        KeyClass::from_code(key_class_code(&self.bytes))
    }

    /// `fits_parse_value` / `ffpsvc`.
    pub fn parse_value_comment(&self) -> Result<(String, String)> {
        parse_value_comment(self.as_str().unwrap_or(""))
    }

    /// `fits_get_keyname` / `ffgknm`.
    pub fn keyword_name(&self) -> Result<(String, usize)> {
        keyword_name(self.as_str().unwrap_or(""))
    }
}

impl AsRef<[u8]> for Card {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

/// `fits_make_key` / `ffmkky`: assemble name + value + comment.
///
/// The returned string is **not** padded to 80 bytes (same as CFITSIO).
pub fn make_card_string(keyname: &str, value: &str, comm: Option<&str>) -> Result<String> {
    let key_b = keyname.as_bytes();
    let nblank = key_b.iter().take_while(|&&b| b == b' ').count();
    let mut tmpname: Vec<u8> = key_b[nblank..]
        .iter()
        .copied()
        .take(FLEN_KEYWORD - 1)
        .collect();
    while tmpname.last() == Some(&b' ') {
        tmpname.pop();
    }

    if tmpname.contains(&b'=') {
        return Err(FitsError::with_message(
            BAD_KEYCHAR,
            "Illegal keyword name; contains an equals sign (=)",
        ));
    }

    let name = ascii(&tmpname);
    let vlen = value.len();
    let mut card = String::new();

    if tmpname.len() <= 8 && test_keyword_bytes(&tmpname, true).is_ok() {
        card.push_str(name);
        for _ in tmpname.len()..8 {
            card.push(' ');
        }
        card.push_str("= ");
    } else if tmpname.starts_with(b"HIERARCH ") || tmpname.starts_with(b"hierarch ") {
        card.push_str(name);
        if card.len() + 3 + vlen > 80 {
            card.push_str("= ");
        } else {
            card.push_str(" = ");
        }
    } else {
        if tmpname.len() + 11 > FLEN_CARD - 1 {
            return Err(FitsError::with_message(
                BAD_KEYCHAR,
                "The following keyword is too long to fit on a card:",
            ));
        }
        card.push_str("HIERARCH ");
        card.push_str(name);
        if card.len() + 3 + vlen > 80 {
            card.push_str("= ");
        } else {
            card.push_str(" = ");
        }
    }

    // `namelen` in CFITSIO is the length of the name + equals prefix, before
    // the value is appended. Comment placement is computed from that.
    let prefix_len = card.len();

    if vlen > 0 {
        if value.as_bytes().first() == Some(&b'\'') {
            if prefix_len > 77 {
                return Err(FitsError::with_message(
                    BAD_KEYCHAR,
                    "The following keyword + value is too long to fit on a card:",
                ));
            }
            let room = 80 - prefix_len;
            card.push_str(&value[..vlen.min(room)]);
            let mut len = (prefix_len + vlen).min(80);
            if len == 80 {
                let mut bytes = card.into_bytes();
                bytes[79] = b'\'';
                card = ascii_owned(bytes);
            }
            if comm.is_some_and(|c| !c.is_empty()) && len < 30 {
                for _ in len..30 {
                    card.push(' ');
                }
                len = 30;
            }
            append_comment(&mut card, len, comm);
        } else {
            if prefix_len + vlen > 80 {
                return Err(FitsError::with_message(
                    BAD_KEYCHAR,
                    "The following keyword + value is too long to fit on a card:",
                ));
            }
            if prefix_len + vlen < 30 {
                for _ in 0..(30 - (prefix_len + vlen)) {
                    card.push(' ');
                }
            }
            let room = 80 - card.len();
            card.push_str(&value[..vlen.min(room)]);
            let len = (prefix_len + vlen).clamp(30, 80);
            append_comment(&mut card, len, comm);
        }
    } else if prefix_len == 10 {
        let mut bytes = card.into_bytes();
        bytes[8] = b' ';
        card = ascii_owned(bytes);
        if let Some(c) = comm {
            let room = 80 - 10;
            let n = c.len().min(room);
            card.push_str(&c[..n]);
        }
    }

    Ok(card)
}

fn append_comment(card: &mut String, len: usize, comm: Option<&str>) {
    if let Some(c) = comm {
        if len < 77 && !c.is_empty() {
            card.push_str(" / ");
            let room = 77usize.saturating_sub(len);
            let n = c.len().min(room);
            card.push_str(&c[..n]);
        }
    }
}

fn ascii(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap_or("")
}

fn ascii_owned(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap_or_default()
}

/// `fits_test_keyword` / `fftkey`.
pub fn test_keyword(keyword: &str) -> Result<()> {
    test_keyword_bytes(keyword.as_bytes(), false)
}

fn test_keyword_bytes(keyword: &[u8], allow_lower: bool) -> Result<()> {
    let maxchr = keyword.len().min(8);
    let mut spaces = false;
    for &raw in keyword.iter().take(maxchr) {
        let test = if allow_lower {
            raw.to_ascii_uppercase()
        } else {
            raw
        };
        if (test.is_ascii_uppercase() || test.is_ascii_digit() || test == b'-' || test == b'_')
            && spaces
        {
            return Err(FitsError::new(BAD_KEYCHAR));
        } else if raw == b' ' {
            spaces = true;
        } else if !(test.is_ascii_uppercase()
            || test.is_ascii_digit()
            || test == b'-'
            || test == b'_')
        {
            return Err(FitsError::new(BAD_KEYCHAR));
        }
    }
    Ok(())
}

/// `fits_test_record` / `fftrec`: bytes 9–end must be printable ASCII.
pub fn test_record(card: &str) -> Result<()> {
    for (i, &b) in card.as_bytes().iter().enumerate().skip(8) {
        if !(32..=126).contains(&b) {
            return Err(FitsError::with_message(
                BAD_KEYCHAR,
                format!(
                    "Character {} in this keyword is illegal. Hex Value = {b:X}",
                    i + 1
                ),
            ));
        }
    }
    Ok(())
}

/// `fits_parse_value` / `ffpsvc`.
pub fn parse_value_comment(card: &str) -> Result<(String, String)> {
    if card.len() >= FLEN_CARD {
        return Err(FitsError::with_message(
            BAD_KEYCHAR,
            "The card string starting with the chars below is too long:",
        ));
    }
    let bytes = card.as_bytes();
    let cardlen = bytes.len();
    let mut value = String::new();
    let mut comm = String::new();

    let starts = |s: &str| cardlen >= s.len() && &bytes[..s.len()] == s.as_bytes();

    let valpos = if starts("HIERARCH ") {
        match bytes.iter().position(|&b| b == b'=') {
            None => {
                if cardlen > 8 {
                    comm = trim_trailing_spaces(&card[8..]);
                }
                return Ok((value, comm));
            }
            Some(p) => p + 1,
        }
    } else if cardlen < 9
        || starts("COMMENT ")
        || starts("HISTORY ")
        || starts("END     ")
        || starts("CONTINUE")
        || starts("        ")
    {
        if cardlen > 8 {
            comm = trim_trailing_spaces(&card[8..]);
        }
        return Ok((value, comm));
    } else if cardlen >= 10 && &bytes[8..10] == b"= " {
        10
    } else {
        match bytes.iter().position(|&b| b == b'=') {
            None => {
                if cardlen > 8 {
                    comm = trim_trailing_spaces(&card[8..]);
                }
                return Ok((value, comm));
            }
            Some(p) => p + 1,
        }
    };

    let nblank = bytes
        .get(valpos..)
        .map(|s| s.iter().take_while(|&&b| b == b' ').count())
        .unwrap_or(0);
    if nblank + valpos == cardlen {
        return Ok((value, comm));
    }
    let mut ii = valpos + nblank;

    if bytes[ii] == b'/' {
        ii += 1;
    } else if bytes[ii] == b'\'' {
        let (v, next) = parse_quoted(bytes, ii);
        value = v;
        ii = next;
    } else if bytes[ii] == b'(' {
        let rest = &bytes[ii..];
        let close = rest.iter().position(|&b| b == b')').unwrap_or(rest.len());
        if close == rest.len() || close >= FLEN_VALUE - 1 {
            return Err(FitsError::with_message(
                crate::status::NO_QUOTE,
                "This complex keyword value has no closing ')' within range:",
            ));
        }
        let n = close + 1;
        value = ascii(&bytes[ii..ii + n]).to_string();
        ii += n;
    } else {
        let rest = &bytes[ii..];
        let n = rest
            .iter()
            .position(|&b| b == b' ' || b == b'/')
            .unwrap_or(rest.len())
            .min(FLEN_VALUE - 1);
        value = ascii(&rest[..n]).to_string();
        ii += n;
    }

    let nblank = bytes
        .get(ii..)
        .map(|s| s.iter().take_while(|&&b| b == b' ').count())
        .unwrap_or(0);
    ii += nblank;
    if ii < 80 && ii < cardlen {
        if bytes[ii] == b'/' {
            ii += 1;
            if ii < cardlen && bytes[ii] == b' ' {
                ii += 1;
            }
        }
        if ii < cardlen {
            let take = (cardlen - ii).min(FLEN_COMMENT - 1);
            comm = trim_trailing_spaces(&card[ii..ii + take]);
        }
    }
    Ok((value, comm))
}

/// Parse a FITS quoted string starting at `start` (the opening quote).
/// Returns (value including quotes, index of first char after the value).
fn parse_quoted(bytes: &[u8], start: usize) -> (String, usize) {
    let cardlen = bytes.len();
    let mut value = Vec::new();
    value.push(b'\'');
    let mut ii = start + 1;
    let mut jj = 1usize;
    while ii < cardlen && jj < FLEN_VALUE - 1 {
        if bytes[ii] == b'\'' {
            if ii + 1 < cardlen && bytes[ii + 1] == b'\'' {
                value.push(b'\'');
                ii += 1;
                jj += 1;
            } else {
                value.push(b'\'');
                break;
            }
        }
        if jj < FLEN_VALUE - 1 {
            value.push(bytes[ii]);
        }
        ii += 1;
        jj += 1;
    }
    if ii == cardlen || jj >= FLEN_VALUE - 1 {
        let jj = jj.min(FLEN_VALUE - 2);
        value.truncate(jj);
        value.push(b'\'');
        return (ascii_owned(value), ii);
    }
    (ascii_owned(value), ii + 1)
}

fn trim_trailing_spaces(s: &str) -> String {
    s.trim_end_matches(' ').to_string()
}

/// `fits_get_keyname` / `ffgknm`.
pub fn keyword_name(card: &str) -> Result<(String, usize)> {
    let bytes = card.as_bytes();
    if bytes.len() >= 9 && (bytes[..9].eq_ignore_ascii_case(b"HIERARCH ")) {
        if let Some(eq) = bytes.iter().position(|&b| b == b'=') {
            let mut ptr1 = 9usize;
            while ptr1 < eq && bytes[ptr1] == b' ' {
                ptr1 += 1;
            }
            let mut name = String::from_utf8_lossy(&bytes[ptr1..eq]).into_owned();
            while name.ends_with(' ') {
                name.pop();
            }
            let len = name.len();
            Ok((name, len))
        } else {
            Ok(("HIERARCH".to_string(), 8))
        }
    } else {
        let namelength = FLEN_KEYWORD - 1;
        let mut name = String::new();
        for ii in 0..namelength {
            let b = *bytes.get(ii).unwrap_or(&0);
            if b != b' ' && b != b'=' && b != 0 {
                name.push(b as char);
            } else {
                return Ok((name, ii));
            }
        }
        Ok((name, namelength))
    }
}

/// `fits_get_keyclass` / `ffgkcl` integer code.
#[must_use]
pub fn key_class_code(tcard: &[u8]) -> i32 {
    let mut card = [b' '; 8];
    let n = tcard.len().min(8);
    card[..n].copy_from_slice(&tcard[..n]);
    for b in &mut card {
        *b = b.to_ascii_uppercase();
    }
    let c0 = card[0];
    let c = |i: usize| card[i];
    let rest = |start: usize, lit: &[u8]| card[start..].starts_with(lit);

    if c0 == b'Z' {
        if rest(1, b"IMAGE  ")
            || rest(1, b"CMPTYPE")
            || (rest(1, b"NAME") && c(5).is_ascii_digit())
            || (rest(1, b"VAL") && c(4).is_ascii_digit())
            || (rest(1, b"TILE") && c(5).is_ascii_digit())
            || rest(1, b"BITPIX ")
            || (rest(1, b"NAXIS") && (c(6).is_ascii_digit() || c(6) == b' '))
            || rest(1, b"SCALE  ")
            || rest(1, b"ZERO   ")
            || rest(1, b"BLANK  ")
            || rest(1, b"SIMPLE ")
            || rest(1, b"TENSION")
            || rest(1, b"EXTEND ")
            || rest(1, b"BLOCKED")
            || rest(1, b"PCOUNT ")
            || rest(1, b"GCOUNT ")
            || rest(1, b"QUANTIZ")
            || rest(1, b"DITHER0")
        {
            return TYP_CMPRS_KEY;
        }
    } else if c0 == b' ' {
        return TYP_COMM_KEY;
    } else if c0 == b'B' {
        if rest(1, b"ITPIX  ") || rest(1, b"LOCKED ") {
            return TYP_STRUC_KEY;
        }
        if rest(1, b"LANK   ") {
            return TYP_NULL_KEY;
        }
        if rest(1, b"SCALE  ") || rest(1, b"ZERO   ") {
            return TYP_SCAL_KEY;
        }
        if rest(1, b"UNIT   ") {
            return TYP_UNIT_KEY;
        }
    } else if c0 == b'C' {
        if rest(1, b"OMMENT") {
            if starts_raw(tcard, b"COMMENT   and Astrophysics', volume 376, page 3")
                || starts_raw(tcard, b"COMMENT   FITS (Flexible Image Transport System")
                || starts_raw(tcard, b"COMMENT   Astrophysics Supplement Series v44/p3")
                || starts_raw(tcard, b"COMMENT   Contact the NASA Science Office of St")
                || starts_raw(tcard, b"COMMENT   FITS Definition document #100 and oth")
            {
                return TYP_STRUC_KEY;
            }
            return if c(7) == b' ' {
                TYP_COMM_KEY
            } else {
                TYP_USER_KEY
            };
        }
        if rest(1, b"HECKSUM") {
            return TYP_CKSUM_KEY;
        }
        if rest(1, b"ONTINUE") {
            return TYP_CONT_KEY;
        }
        if rest(1, b"TYPE") && c(5).is_ascii_digit()
            || rest(1, b"UNIT") && c(5).is_ascii_digit()
            || rest(1, b"RVAL") && c(5).is_ascii_digit()
            || rest(1, b"RPIX") && c(5).is_ascii_digit()
            || rest(1, b"ROTA") && c(5).is_ascii_digit()
            || rest(1, b"RDER") && c(5).is_ascii_digit()
            || rest(1, b"SYER") && c(5).is_ascii_digit()
            || rest(1, b"DELT") && c(5).is_ascii_digit()
            || (c(1) == b'D' && c(2).is_ascii_digit())
        {
            return TYP_WCS_KEY;
        }
    } else if c0 == b'D' {
        if rest(1, b"ATASUM ") {
            return TYP_CKSUM_KEY;
        }
        if rest(1, b"ATAMIN ") || rest(1, b"ATAMAX ") {
            return TYP_RANG_KEY;
        }
        if rest(1, b"ATE-OBS") {
            return TYP_REFSYS_KEY;
        }
    } else if c0 == b'E' {
        if rest(1, b"XTEND  ") || rest(1, b"ND     ") {
            return TYP_STRUC_KEY;
        }
        if rest(1, b"XTNAME ") {
            if starts_raw(tcard, b"EXTNAME = 'COMPRESSED_IMAGE'") {
                return TYP_CMPRS_KEY;
            }
            return TYP_HDUID_KEY;
        }
        if rest(1, b"XTVER  ") || rest(1, b"XTLEVEL") {
            return TYP_HDUID_KEY;
        }
        if rest(1, b"QUINOX") || (rest(1, b"QUI") && c(4).is_ascii_digit()) || rest(1, b"POCH   ") {
            return TYP_REFSYS_KEY;
        }
    } else if c0 == b'G' {
        if rest(1, b"COUNT  ") || rest(1, b"ROUPS  ") {
            return TYP_STRUC_KEY;
        }
    } else if c0 == b'H' {
        if rest(1, b"DUNAME ") || rest(1, b"DUVER  ") || rest(1, b"DULEVEL") {
            return TYP_HDUID_KEY;
        }
        if rest(1, b"ISTORY") {
            return if c(7) == b' ' {
                TYP_COMM_KEY
            } else {
                TYP_USER_KEY
            };
        }
    } else if c0 == b'L' {
        if rest(1, b"ONPOLE")
            || rest(1, b"ATPOLE")
            || (rest(1, b"ONP") && c(4).is_ascii_digit())
            || (rest(1, b"ATP") && c(4).is_ascii_digit())
        {
            return TYP_WCS_KEY;
        }
    } else if c0 == b'M' {
        if rest(1, b"JD-OBS ") || (rest(1, b"JDOB") && c(5).is_ascii_digit()) {
            return TYP_REFSYS_KEY;
        }
    } else if c0 == b'N' {
        if rest(1, b"AXIS") && (c(5).is_ascii_digit() || c(5) == b' ') {
            return TYP_STRUC_KEY;
        }
    } else if c0 == b'P' {
        if rest(1, b"COUNT  ") {
            return TYP_STRUC_KEY;
        }
        if (c(1) == b'C' || c(1) == b'V' || c(1) == b'S') && c(2).is_ascii_digit() {
            return TYP_WCS_KEY;
        }
    } else if c0 == b'R' {
        if rest(1, b"ADECSYS") || rest(1, b"ADESYS") || (rest(1, b"ADE") && c(4).is_ascii_digit()) {
            return TYP_REFSYS_KEY;
        }
    } else if c0 == b'S' {
        if rest(1, b"IMPLE  ") {
            return TYP_STRUC_KEY;
        }
    } else if c0 == b'T' {
        if (rest(1, b"TYPE") && c(5).is_ascii_digit())
            || (rest(1, b"FORM") && c(5).is_ascii_digit())
            || (rest(1, b"BCOL") && c(5).is_ascii_digit())
            || rest(1, b"FIELDS ")
            || rest(1, b"HEAP   ")
        {
            return TYP_STRUC_KEY;
        }
        if rest(1, b"NULL") && c(5).is_ascii_digit() {
            return TYP_NULL_KEY;
        }
        if rest(1, b"DIM") && c(4).is_ascii_digit() {
            return TYP_DIM_KEY;
        }
        if rest(1, b"UNIT") && c(5).is_ascii_digit() {
            return TYP_UNIT_KEY;
        }
        if rest(1, b"DISP") && c(5).is_ascii_digit() {
            return TYP_DISP_KEY;
        }
        if (rest(1, b"SCAL") && c(5).is_ascii_digit())
            || (rest(1, b"ZERO") && c(5).is_ascii_digit())
        {
            return TYP_SCAL_KEY;
        }
        if (rest(1, b"LMIN") && c(5).is_ascii_digit())
            || (rest(1, b"LMAX") && c(5).is_ascii_digit())
            || (rest(1, b"DMIN") && c(5).is_ascii_digit())
            || (rest(1, b"DMAX") && c(5).is_ascii_digit())
        {
            return TYP_RANG_KEY;
        }
        if wcs_t_keyword(&card) {
            return TYP_WCS_KEY;
        }
    } else if c0 == b'X' {
        if rest(1, b"TENSION") {
            return TYP_STRUC_KEY;
        }
    } else if c0 == b'W' {
        if rest(1, b"CSAXES")
            || rest(1, b"CSNAME")
            || (rest(1, b"CAX") && c(4).is_ascii_digit())
            || (rest(1, b"CSN") && c(4).is_ascii_digit())
        {
            return TYP_WCS_KEY;
        }
    } else if c0.is_ascii_digit() && wcs_digit_keyword(&card) {
        return TYP_WCS_KEY;
    }

    TYP_USER_KEY
}

fn wcs_t_keyword(card: &[u8; 8]) -> bool {
    let c = |i: usize| card[i];
    let rest = |start: usize, lit: &[u8]| card[start..].starts_with(lit);
    (rest(1, b"CTYP") && c(5).is_ascii_digit())
        || (rest(1, b"CTY") && c(4).is_ascii_digit())
        || (rest(1, b"CUNI") && c(5).is_ascii_digit())
        || (rest(1, b"CUN") && c(4).is_ascii_digit())
        || (rest(1, b"CRVL") && c(5).is_ascii_digit())
        || (rest(1, b"CRV") && c(4).is_ascii_digit())
        || (rest(1, b"CRPX") && c(5).is_ascii_digit())
        || (rest(1, b"CRP") && c(4).is_ascii_digit())
        || (rest(1, b"CROT") && c(5).is_ascii_digit())
        || (rest(1, b"CDLT") && c(5).is_ascii_digit())
        || (rest(1, b"CDE") && c(4).is_ascii_digit())
        || (rest(1, b"CRD") && c(4).is_ascii_digit())
        || (rest(1, b"CSY") && c(4).is_ascii_digit())
        || (rest(1, b"WCS") && c(4).is_ascii_digit())
        || (rest(1, b"C") && c(2).is_ascii_digit())
        || (rest(1, b"P") && c(2).is_ascii_digit())
        || (rest(1, b"V") && c(2).is_ascii_digit())
        || (rest(1, b"S") && c(2).is_ascii_digit())
}

fn wcs_digit_keyword(card: &[u8; 8]) -> bool {
    let c = |i: usize| card[i];
    let rest = |start: usize, lit: &[u8]| card[start..].starts_with(lit);
    if c(1) == b'C' {
        return (rest(1, b"CTYP") && c(5).is_ascii_digit())
            || (rest(1, b"CTY") && c(4).is_ascii_digit())
            || (rest(1, b"CUNI") && c(5).is_ascii_digit())
            || (rest(1, b"CUN") && c(4).is_ascii_digit())
            || (rest(1, b"CRVL") && c(5).is_ascii_digit())
            || (rest(1, b"CRV") && c(4).is_ascii_digit())
            || (rest(1, b"CRPX") && c(5).is_ascii_digit())
            || (rest(1, b"CRP") && c(4).is_ascii_digit())
            || (rest(1, b"CROT") && c(5).is_ascii_digit())
            || (rest(1, b"CDLT") && c(5).is_ascii_digit())
            || (rest(1, b"CDE") && c(4).is_ascii_digit())
            || (rest(1, b"CRD") && c(4).is_ascii_digit())
            || (rest(1, b"CSY") && c(4).is_ascii_digit());
    }
    if rest(1, b"V") && c(2).is_ascii_digit() {
        return true;
    }
    if rest(1, b"S") && c(2).is_ascii_digit() {
        return true;
    }
    if c(1).is_ascii_digit() {
        return (c(2) == b'P' && c(3) == b'C' && c(4).is_ascii_digit())
            || (c(2) == b'C' && c(3) == b'D' && c(4).is_ascii_digit());
    }
    false
}

fn starts_raw(tcard: &[u8], prefix: &[u8]) -> bool {
    tcard.len() >= prefix.len() && &tcard[..prefix.len()] == prefix
}

/// `fits_make_key` / `ffmkky`.
pub fn fits_make_key(name: &str, value: &str, comm: Option<&str>) -> Result<String> {
    make_card_string(name, value, comm)
}

/// `fits_parse_value` / `ffpsvc`.
pub fn fits_parse_value(card: &str) -> Result<(String, String)> {
    parse_value_comment(card)
}

/// `fits_get_keyclass` / `ffgkcl`.
pub fn fits_get_keyclass(card: &str) -> KeyClass {
    KeyClass::from_code(key_class_code(card.as_bytes()))
}

/// `fits_test_keyword` / `fftkey`.
pub fn fits_test_keyword(keyword: &str) -> Result<()> {
    test_keyword(keyword)
}

/// `fits_get_keyname` / `ffgknm`.
pub fn fits_get_keyname(card: &str) -> Result<(String, usize)> {
    keyword_name(card)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_integer_card_right_justified() {
        let s = make_card_string("BITPIX", "16", Some("number of bits per data pixel")).unwrap();
        assert!(s.starts_with("BITPIX  ="));
        assert!(s.contains("16"));
        assert!(s.contains("/ number of bits per data pixel"));
        assert!(s.len() <= 80);
        // value right-justified to column 30 (1-based): bytes 0-9 are name+eq,
        // bytes 10-29 are the 20-char value field.
        assert_eq!(&s.as_bytes()[28..30], b"16");
    }

    #[test]
    fn quoted_string_left_aligned() {
        let s = make_card_string("OBJECT", "'NGC3372'", Some("target")).unwrap();
        assert!(s.starts_with("OBJECT  = 'NGC3372'"));
        assert!(s.contains("/ target"));
    }

    #[test]
    fn hierarch_implicit() {
        let s = make_card_string("ESO DET DIT", "5.0", Some("sec")).unwrap();
        assert!(s.starts_with("HIERARCH ESO DET DIT"));
        assert!(s.contains("5.0"));
    }

    #[test]
    fn keyclass_structure() {
        assert_eq!(
            key_class_code(b"SIMPLE  =                    T"),
            TYP_STRUC_KEY
        );
        assert_eq!(
            key_class_code(b"NAXIS1  =                  100"),
            TYP_STRUC_KEY
        );
        assert_eq!(
            key_class_code(b"EXPTIME =                  1.0"),
            TYP_USER_KEY
        );
        assert_eq!(key_class_code(b"CONTINUE  'abc'"), TYP_CONT_KEY);
        assert_eq!(key_class_code(b"HISTORY processed"), TYP_COMM_KEY);
    }

    #[test]
    fn parse_simple_int() {
        let card = "BITPIX  =                   16 / number of bits per data pixel";
        let (v, c) = parse_value_comment(card).unwrap();
        assert_eq!(v, "16");
        assert_eq!(c, "number of bits per data pixel");
    }
}
