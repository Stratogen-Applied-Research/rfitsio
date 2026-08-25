//! Header Data Unit metadata.

use crate::convert::pad_data_len;
use crate::header::Header;
use crate::types::HduType;

/// One HDU: header plus the file offset of its data unit.
#[derive(Clone, Debug)]
pub struct Hdu {
    /// IMAGE / ASCII table / binary table.
    pub hdu_type: HduType,
    /// Keyword cards for this HDU.
    pub header: Header,
    /// Byte offset of the start of this HDU (header).
    pub header_start: u64,
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
            header_start: 0,
            data_start,
        })
    }

    /// Unpadded data-unit size in bytes.
    pub fn data_bytes(&self) -> crate::error::Result<u64> {
        match self.hdu_type {
            HduType::AsciiTable | HduType::BinaryTable => {
                let naxis1 = self.header.get_i64("NAXIS1").unwrap_or(0).max(0) as u64;
                let naxis2 = self.header.get_i64("NAXIS2").unwrap_or(0).max(0) as u64;
                let pcount = self.header.get_i64("PCOUNT").unwrap_or(0).max(0) as u64;
                Ok(naxis1.saturating_mul(naxis2).saturating_add(pcount))
            }
            HduType::Image => {
                let naxes = self.header.naxes()?;
                if naxes.is_empty() {
                    return Ok(0);
                }
                let bpp = self.header.image_type()?.bytes_per_pixel() as u64;
                let npix = naxes.iter().try_fold(1u64, |acc, &n| {
                    u64::try_from(n)
                        .ok()
                        .and_then(|n| acc.checked_mul(n))
                        .ok_or_else(|| crate::error::FitsError::new(crate::status::ARRAY_TOO_BIG))
                })?;
                Ok(bpp.saturating_mul(npix))
            }
        }
    }

    /// Data unit size including 2880-byte padding (0 if no data).
    pub fn data_unit_len(&self) -> crate::error::Result<u64> {
        Ok(pad_data_len(self.data_bytes()?))
    }

    /// True if this is a null primary array (NAXIS = 0).
    pub fn is_null_image(&self) -> bool {
        self.hdu_type == HduType::Image && self.header.get_i64("NAXIS").unwrap_or(-1) == 0
    }
}
