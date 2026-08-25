//! Pure-Rust FITS I/O, validated against CFITSIO 4.7.0.
//!
//! This crate reimplements the FITS Standard 4.0 and CFITSIO's documented
//! behaviour. It does not wrap `libcfitsio`. Status codes, `ffgerr` text,
//! and header-card formatting are locked to CFITSIO 4.7.0 by tests that
//! call the vendored C library.

#![forbid(unsafe_code)]

pub mod card;
pub mod convert;
pub mod datetime;
pub mod error;
pub mod file;
pub mod hdu;
pub mod header;
pub mod image;
pub mod io;
pub mod keys;
pub mod status;
pub mod table;
pub mod tform;
pub mod types;

pub use card::{
    Card, fits_get_keyclass, fits_get_keyname, fits_make_key, fits_parse_value, fits_test_keyword,
    make_card_string,
};
pub use error::{FitsError, Result};
pub use file::{
    AccessMode, FitsFile, fits_close_file, fits_create_file, fits_create_memfile, fits_open_file,
};
pub use header::{AsciiTableInfo, Header, PrimaryInfo};
pub use image::{Pixel, fits_create_img, fits_read_img, fits_write_img};
pub use keys::{fits_read_key_str, fits_write_date, fits_write_key_str};
pub use status::{fits_get_errstatus, status_text};
pub use table::{fits_create_tbl, fits_movabs_hdu};
pub use tform::{AsciiKind, AsciiTform, ascii_column_starts, parse_ascii_tform};
pub use types::{
    CARD_LEN, CFITSIO_MAJOR, CFITSIO_MICRO, CFITSIO_MINOR, HduType, ImageType, KeyClass,
    RECORD_LEN, cfitsio_version_float,
};
