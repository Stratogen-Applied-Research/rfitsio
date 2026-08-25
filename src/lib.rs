//! Pure-Rust FITS I/O, validated against CFITSIO 4.7.0.
//!
//! This crate reimplements the FITS Standard 4.0 and CFITSIO's documented
//! behaviour. It does not wrap `libcfitsio`. Status codes, `ffgerr` text,
//! and header-card formatting are locked to CFITSIO 4.7.0 by tests that
//! call the vendored C library.

#![forbid(unsafe_code)]

pub mod card;
pub mod error;
pub mod file;
pub mod hdu;
pub mod header;
pub mod io;
pub mod status;
pub mod types;

pub use card::{
    Card, fits_get_keyclass, fits_get_keyname, fits_make_key, fits_parse_value, fits_test_keyword,
    make_card_string,
};
pub use error::{FitsError, Result};
pub use file::{
    AccessMode, FitsFile, fits_close_file, fits_create_file, fits_create_memfile, fits_open_file,
};
pub use header::Header;
pub use status::{fits_get_errstatus, status_text};
pub use types::{
    CARD_LEN, CFITSIO_MAJOR, CFITSIO_MICRO, CFITSIO_MINOR, HduType, KeyClass, RECORD_LEN,
    cfitsio_version_float,
};
