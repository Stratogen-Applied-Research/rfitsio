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

/// Image BITPIX / CFITSIO unsigned-image codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageType {
    /// `BYTE_IMG` / BITPIX = 8.
    U8,
    /// `SBYTE_IMG` / BITPIX = 8 + BZERO = -128.
    I8,
    /// `SHORT_IMG` / BITPIX = 16.
    I16,
    /// `USHORT_IMG` / BITPIX = 16 + BZERO = 32768.
    U16,
    /// `LONG_IMG` / BITPIX = 32.
    I32,
    /// `ULONG_IMG` / BITPIX = 32 + BZERO = 2^31.
    U32,
    /// `LONGLONG_IMG` / BITPIX = 64.
    I64,
    /// `ULONGLONG_IMG` / BITPIX = 64 + BZERO = 2^63.
    U64,
    /// `FLOAT_IMG` / BITPIX = -32.
    F32,
    /// `DOUBLE_IMG` / BITPIX = -64.
    F64,
}

impl ImageType {
    /// CFITSIO `ffcrim` / `ffphps` bitpix argument (includes 10/20/40/80).
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::U8 => BYTE_IMG,
            Self::I8 => SBYTE_IMG,
            Self::I16 => SHORT_IMG,
            Self::U16 => USHORT_IMG,
            Self::I32 => LONG_IMG,
            Self::U32 => ULONG_IMG,
            Self::I64 => LONGLONG_IMG,
            Self::U64 => ULONGLONG_IMG,
            Self::F32 => FLOAT_IMG,
            Self::F64 => DOUBLE_IMG,
        }
    }

    /// BITPIX actually stored in the header.
    #[must_use]
    pub const fn bitpix(self) -> i32 {
        match self {
            Self::U8 | Self::I8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::I32 | Self::U32 => 32,
            Self::I64 | Self::U64 => 64,
            Self::F32 => -32,
            Self::F64 => -64,
        }
    }

    /// Bytes per stored pixel.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> usize {
        match self.bitpix().unsigned_abs() {
            8 => 1,
            16 => 2,
            32 => 4,
            64 => 8,
            _ => 1,
        }
    }

    /// Default BSCALE.
    #[must_use]
    pub const fn bscale(self) -> f64 {
        1.0
    }

    /// Default BZERO for unsigned / signed-byte images.
    #[must_use]
    pub const fn bzero(self) -> f64 {
        match self {
            Self::I8 => -128.0,
            Self::U16 => 32768.0,
            Self::U32 => 2_147_483_648.0,
            Self::U64 => 9_223_372_036_854_775_808.0,
            _ => 0.0,
        }
    }

    /// Parse a CFITSIO image-type code (including 10/20/40/80).
    #[must_use]
    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            BYTE_IMG => Some(Self::U8),
            SBYTE_IMG => Some(Self::I8),
            SHORT_IMG => Some(Self::I16),
            USHORT_IMG => Some(Self::U16),
            LONG_IMG => Some(Self::I32),
            ULONG_IMG => Some(Self::U32),
            LONGLONG_IMG => Some(Self::I64),
            ULONGLONG_IMG => Some(Self::U64),
            FLOAT_IMG => Some(Self::F32),
            DOUBLE_IMG => Some(Self::F64),
            _ => None,
        }
    }
}

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

/// Maximum number of axes in a tiled compressed image (`MAX_COMPRESS_DIM`).
pub const MAX_COMPRESS_DIM: usize = 6;

/// Rice coding (`RICE_1`).
pub const RICE_1: i32 = 11;
/// gzip of the raw tile bytes (`GZIP_1`).
pub const GZIP_1: i32 = 21;
/// byte-shuffle then gzip (`GZIP_2`).
pub const GZIP_2: i32 = 22;
/// IRAF pixel-list coding (`PLIO_1`).
pub const PLIO_1: i32 = 31;
/// H-transform coding (`HCOMPRESS_1`).
pub const HCOMPRESS_1: i32 = 41;
/// Internal CFITSIO test codec; not a public FITS type.
pub const BZIP2_1: i32 = 51;
/// Store tiles uncompressed (`NOCOMPRESS`).
pub const NOCOMPRESS: i32 = -1;

/// No dither when quantizing floats (`NO_DITHER`).
pub const NO_DITHER: i32 = -1;
/// Subtractive dithering, zeros treated as data (`SUBTRACTIVE_DITHER_1`).
pub const SUBTRACTIVE_DITHER_1: i32 = 1;
/// Subtractive dithering, exact zeros preserved (`SUBTRACTIVE_DITHER_2`).
pub const SUBTRACTIVE_DITHER_2: i32 = 2;

/// Sentinel meaning "do not quantize floats" (`NO_QUANTIZE` in `imcompress.c`).
pub const NO_QUANTIZE: f32 = 9999.0;

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
