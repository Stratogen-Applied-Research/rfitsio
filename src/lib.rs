//! Pure-Rust FITS I/O, validated against CFITSIO 4.7.0.
//!
//! This crate reimplements the FITS Standard 4.0 and CFITSIO's documented
//! behaviour. It does not wrap `libcfitsio`. Status codes, `ffgerr` text,
//! and header-card formatting are locked to CFITSIO 4.7.0 by tests that
//! call the vendored C library.

#![forbid(unsafe_code)]

pub mod bintable;
pub mod card;
pub mod checksum;
pub mod compress;
pub mod convert;
pub mod datetime;
pub mod edithdu;
pub mod error;
pub mod file;
pub mod hdu;
pub mod header;
pub mod image;
pub mod io;
pub mod keys;
pub mod nbit;
pub mod status;
pub mod table;
pub mod tdim;
pub mod tform;
pub mod types;
pub mod wcs;

pub use card::{
    Card, fits_get_keyclass, fits_get_keyname, fits_make_key, fits_parse_value, fits_test_keyword,
    make_card_string,
};
pub use checksum::{
    add_checksum_record, decode_checksum, encode_checksum, fits_decode_chksum, fits_encode_chksum,
    fits_get_chksum, fits_update_chksum, fits_verify_chksum, fits_write_chksum,
};
pub use compress::{CompressionType, fits_is_compressed_image, fits_set_compression_type};
pub use edithdu::{
    fits_copy_data, fits_copy_file, fits_copy_hdu, fits_copy_header, fits_create_hdu,
    fits_delete_hdu, fits_insert_atbl, fits_insert_btbl, fits_insert_img, fits_write_hdu,
};
pub use error::{FitsError, Result, clear_err_msg, pop_err_msg, push_err_msg};
pub use file::{
    AccessMode, FitsFile, fits_close_file, fits_create_file, fits_create_memfile, fits_open_file,
};
pub use header::{AsciiTableInfo, BinaryTableInfo, Header, PrimaryInfo};
pub use image::{Pixel, fits_create_img, fits_read_img, fits_write_img};
pub use keys::{fits_read_key_str, fits_write_date, fits_write_key_str};
pub use nbit::{pack_samples, samples_per_byte, unpack_samples};
pub use status::{fits_get_errstatus, status_text};
pub use table::{fits_create_tbl, fits_movabs_hdu};
pub use tdim::{
    decode_tdim, fits_read_tdim, fits_write_tdim, format_tdim, tdim_coords, tdim_elem, tdim_product,
};
pub use tform::{
    AsciiKind, AsciiTform, BinaryKind, BinaryTform, VariableKind, ascii_column_starts,
    binary_column_offsets, parse_ascii_tform, parse_binary_tform,
};
pub use types::{
    CARD_LEN, CFITSIO_MAJOR, CFITSIO_MICRO, CFITSIO_MINOR, GZIP_1, GZIP_2, HCOMPRESS_1, HduType,
    ImageType, KeyClass, NO_DITHER, NO_QUANTIZE, NOCOMPRESS, PLIO_1, RECORD_LEN, RICE_1,
    SUBTRACTIVE_DITHER_1, SUBTRACTIVE_DITHER_2, cfitsio_version_float,
};
pub use wcs::{pix_to_world, read_img_coord, world_to_pix};
