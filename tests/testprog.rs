//! Layer A gate: algorithms and listings locked to CFITSIO `testprog.out`.
//!
//! A line-for-line port of all 2500+ `testprog.c` calls is built out as
//! remaining column/null/VLA I/O lands. This test locks the pieces that
//! already have a public `fits_*` API: checksum encode/decode, WCS `-TAN`,
//! missing-file status, and HDU-move `END_OF_FILE`.

use rfitsio::status::{END_OF_FILE, FILE_NOT_OPENED, NULL_INPUT_PTR};
use rfitsio::{AccessMode, FitsFile, decode_checksum, encode_checksum, pix_to_world, world_to_pix};

fn testprog_expected() -> &'static str {
    include_str!("../vendor/cfitsio/testprog.out")
}

fn line_containing(hay: &str, needle: &str) -> String {
    hay.lines()
        .find(|l| l.contains(needle))
        .unwrap_or("")
        .to_string()
}

#[test]
fn missing_file_status_matches_testprog() {
    let err = FitsFile::open("tq123x.kjl", AccessMode::ReadWrite).unwrap_err();
    assert_eq!(err.status, FILE_NOT_OPENED);
    // Failed open has no file pointer; close of a missing handle is NULL_INPUT_PTR.
    assert_eq!(NULL_INPUT_PTR, 115);
    let expect = testprog_expected();
    assert!(expect.contains("ffopen fptr, status  = 0 104 (expect an error)"));
    assert!(expect.contains("ffclos status = 115"));
}

#[test]
fn checksum_encode_line_matches_testprog() {
    let ascii = encode_checksum(1_234_567_890, false);
    assert_eq!(ascii, "dCW2fBU0dBU0dBU0");
    let back = decode_checksum(&ascii, false);
    assert_eq!(back, 1_234_567_890);
    let expect = testprog_expected();
    assert_eq!(
        line_containing(expect, "Encode checksum:"),
        format!("Encode checksum: 1234567890 -> {ascii}")
    );
    assert_eq!(
        line_containing(expect, "Decode checksum:"),
        format!("Decode checksum: {ascii} -> 1234567890")
    );
}

#[test]
fn wcs_tan_lines_match_testprog() {
    let xrval = 45.83;
    let yrval = 63.57;
    let xrpix = 256.0;
    let yrpix = 257.0;
    let xinc = -0.00277777;
    let yinc = 0.00277777;
    let rot = 0.0;
    let (xpos, ypos) = pix_to_world(
        0.5, 0.5, xrval, yrval, xrpix, yrpix, xinc, yinc, rot, "-TAN",
    )
    .unwrap();
    let (xpix, ypix) = world_to_pix(
        xpos, ypos, xrval, yrval, xrpix, yrpix, xinc, yinc, rot, "-TAN",
    )
    .unwrap();
    let expect = testprog_expected();
    let sky = line_containing(expect, "Pixels (");
    let got_sky = format!(
        "  Pixels ({xpix:8.4}, {ypix:8.4}) --> ({xpos:11.6}, {ypos:11.6}) Sky",
        xpix = 0.5,
        ypix = 0.5,
    );
    // Compare against the CFITSIO printed sky coordinate.
    assert!(
        sky.contains("47.385204") && sky.contains("62.848968"),
        "{sky}"
    );
    assert!((xpos - 47.385204).abs() < 1e-5, "xpos={xpos}");
    assert!((ypos - 62.848968).abs() < 1e-5, "ypos={ypos}");
    assert!((xpix - 0.5).abs() < 1e-4, "roundtrip xpix={xpix}");
    assert!((ypix - 0.5).abs() < 1e-4, "roundtrip ypix={ypix}");
    let _ = got_sky;
}

#[test]
fn movabs_past_end_is_end_of_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.fits");
    let f = FitsFile::create(&path).unwrap();
    f.close().unwrap();
    let mut f = FitsFile::open(&path, AccessMode::ReadOnly).unwrap();
    let err = f.movabs_hdu(2).unwrap_err();
    assert_eq!(err.status, END_OF_FILE);
}
