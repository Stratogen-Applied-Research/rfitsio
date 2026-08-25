//! FITS / CFITSIO constants and datatype enumerations.

/// Behavioral oracle: CFITSIO 4.7.0.
pub const CFITSIO_MAJOR: u32 = 4;
pub const CFITSIO_MINOR: u32 = 7;
pub const CFITSIO_MICRO: u32 = 0;
pub const CFITSIO_SONAME: u32 = 10;

/// `ffvers` encoding: `major + 0.01 * minor + 0.0001 * micro`.
#[must_use]
pub fn cfitsio_version_float() -> f32 {
    CFITSIO_MAJOR as f32 + 0.01 * CFITSIO_MINOR as f32 + 0.0001 * CFITSIO_MICRO as f32
}

/// FITS logical record size in bytes. Do not change.
pub const RECORD_LEN: usize = 2880;
/// Cards per header record (`RECORD_LEN / CARD_LEN`).
pub const CARDS_PER_RECORD: usize = 36;

pub const FLEN_FILENAME: usize = 1025;
pub const FLEN_KEYWORD: usize = 75;
pub const FLEN_CARD: usize = 81;
pub const CARD_LEN: usize = 80;
pub const FLEN_VALUE: usize = 71;
pub const FLEN_COMMENT: usize = 73;
pub const FLEN_ERRMSG: usize = 81;

pub const TBIT: i32 = 1;
pub const TBYTE: i32 = 11;
pub const TSBYTE: i32 = 12;
pub const TLOGICAL: i32 = 14;
pub const TSTRING: i32 = 16;
pub const TUSHORT: i32 = 20;
pub const TSHORT: i32 = 21;
pub const TUINT: i32 = 30;
pub const TINT: i32 = 31;
pub const TULONG: i32 = 40;
pub const TLONG: i32 = 41;
pub const TINT32BIT: i32 = 41;
pub const TFLOAT: i32 = 42;
pub const TULONGLONG: i32 = 80;
pub const TLONGLONG: i32 = 81;
pub const TDOUBLE: i32 = 82;
pub const TCOMPLEX: i32 = 83;
pub const TDBLCOMPLEX: i32 = 163;

pub const TYP_STRUC_KEY: i32 = 10;
pub const TYP_CMPRS_KEY: i32 = 20;
pub const TYP_SCAL_KEY: i32 = 30;
pub const TYP_NULL_KEY: i32 = 40;
pub const TYP_DIM_KEY: i32 = 50;
pub const TYP_RANG_KEY: i32 = 60;
pub const TYP_UNIT_KEY: i32 = 70;
pub const TYP_DISP_KEY: i32 = 80;
pub const TYP_HDUID_KEY: i32 = 90;
pub const TYP_CKSUM_KEY: i32 = 100;
pub const TYP_WCS_KEY: i32 = 110;
pub const TYP_REFSYS_KEY: i32 = 120;
pub const TYP_COMM_KEY: i32 = 130;
pub const TYP_CONT_KEY: i32 = 140;
pub const TYP_USER_KEY: i32 = 150;

pub const BYTE_IMG: i32 = 8;
pub const SHORT_IMG: i32 = 16;
pub const LONG_IMG: i32 = 32;
pub const LONGLONG_IMG: i32 = 64;
pub const FLOAT_IMG: i32 = -32;
pub const DOUBLE_IMG: i32 = -64;
pub const SBYTE_IMG: i32 = 10;
pub const USHORT_IMG: i32 = 20;
pub const ULONG_IMG: i32 = 40;
pub const ULONGLONG_IMG: i32 = 80;

pub const IMAGE_HDU: i32 = 0;
pub const ASCII_TBL: i32 = 1;
pub const BINARY_TBL: i32 = 2;
pub const ANY_HDU: i32 = -1;

pub const READONLY: i32 = 0;
pub const READWRITE: i32 = 1;

pub const CASESEN: i32 = 1;
pub const CASEINSEN: i32 = 0;

/// HDU class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HduType {
    Image = 0,
    AsciiTable = 1,
    BinaryTable = 2,
}

impl HduType {
    /// CFITSIO `IMAGE_HDU` / `ASCII_TBL` / `BINARY_TBL` code.
    #[must_use]
    pub const fn code(self) -> i32 {
        self as i32
    }
}

/// Keyword classification (`ffgkcl`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum KeyClass {
    Structure = TYP_STRUC_KEY,
    Compressed = TYP_CMPRS_KEY,
    Scaling = TYP_SCAL_KEY,
    Null = TYP_NULL_KEY,
    Dimension = TYP_DIM_KEY,
    Range = TYP_RANG_KEY,
    Unit = TYP_UNIT_KEY,
    Display = TYP_DISP_KEY,
    HduId = TYP_HDUID_KEY,
    Checksum = TYP_CKSUM_KEY,
    Wcs = TYP_WCS_KEY,
    Refsys = TYP_REFSYS_KEY,
    Comment = TYP_COMM_KEY,
    Continue = TYP_CONT_KEY,
    User = TYP_USER_KEY,
}

impl KeyClass {
    /// CFITSIO `TYP_*_KEY` integer.
    #[must_use]
    pub const fn code(self) -> i32 {
        self as i32
    }

    /// Reconstruct from a CFITSIO class code. Unknown codes become [`KeyClass::User`].
    #[must_use]
    pub const fn from_code(code: i32) -> Self {
        match code {
            TYP_STRUC_KEY => Self::Structure,
            TYP_CMPRS_KEY => Self::Compressed,
            TYP_SCAL_KEY => Self::Scaling,
            TYP_NULL_KEY => Self::Null,
            TYP_DIM_KEY => Self::Dimension,
            TYP_RANG_KEY => Self::Range,
            TYP_UNIT_KEY => Self::Unit,
            TYP_DISP_KEY => Self::Display,
            TYP_HDUID_KEY => Self::HduId,
            TYP_CKSUM_KEY => Self::Checksum,
            TYP_WCS_KEY => Self::Wcs,
            TYP_REFSYS_KEY => Self::Refsys,
            TYP_COMM_KEY => Self::Comment,
            TYP_CONT_KEY => Self::Continue,
            _ => Self::User,
        }
    }
}
