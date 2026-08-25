//! Header Data Unit metadata.

use crate::header::Header;
use crate::types::HduType;

/// One HDU: header plus the file offset of its data unit.
#[derive(Clone, Debug)]
pub struct Hdu {
    /// IMAGE / ASCII table / binary table.
    pub hdu_type: HduType,
    /// Keyword cards for this HDU.
    pub header: Header,
    /// Byte offset of the data unit (multiple of 2880).
    pub data_start: u64,
}

impl Hdu {
    /// Empty primary array (NAXIS=0, BITPIX=8).
    pub fn empty_primary() -> crate::error::Result<Self> {
        let header = Header::empty_primary()?;
        let data_start = header.to_record_bytes().len() as u64;
        Ok(Self {
            hdu_type: HduType::Image,
            header,
            data_start,
        })
    }
}
