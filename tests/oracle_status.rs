//! Differential tests: `fits_get_errstatus` vs CFITSIO `ffgerr`.

use rfitsio::status_text;

#[test]
fn ffgerr_202_keyword_not_found() {
    assert_eq!(status_text(202), "keyword not found in header");
    assert_eq!(status_text(202), cfitsio_sys::ffgerr_str(202));
}

#[test]
fn ffgerr_matches_cfitsio_for_all_codes() {
    // Cover the first switch, the second switch, negatives, and the
    // "unknown" default — including named constants CFITSIO does not
    // describe (415, 506, -11, …).
    let mut codes: Vec<i32> = (-120..=650).collect();
    codes.extend([
        999,
        1000,
        i32::MIN,
        i32::MAX,
        -11,
        -9,
        -106,
        415,
        506,
        349,
        350,
    ]);
    for code in codes {
        assert_eq!(
            status_text(code),
            cfitsio_sys::ffgerr_str(code),
            "ffgerr mismatch for status {code}"
        );
    }
}

#[test]
fn oracle_version_is_4_7_0() {
    let v = cfitsio_sys::ffvers_f32();
    let expected = rfitsio::cfitsio_version_float();
    assert!(
        (v - expected).abs() < 1e-5,
        "oracle ffvers={v} rfitsio={expected}"
    );
}
