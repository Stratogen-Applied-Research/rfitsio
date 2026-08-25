//! Header CRUD vs CFITSIO putkey / getkey / modkey.

use std::fs;

use rfitsio::card::{format_exp_double, format_fixed_double};
use rfitsio::datetime::is_fits_date;
use rfitsio::status::KEY_NO_EXIST;
use rfitsio::{FitsFile, ImageType};

fn dump_cards(bytes: &[u8]) -> String {
    bytes
        .chunks(80)
        .take(20)
        .map(|c| String::from_utf8_lossy(c).trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_files_eq(c: &std::path::Path, r: &std::path::Path) {
    let cb = fs::read(c).unwrap();
    let rb = fs::read(r).unwrap();
    assert_eq!(
        rb,
        cb,
        "header bytes differ\n cfitsio:\n{}\n rfitsio:\n{}",
        dump_cards(&cb),
        dump_cards(&rb)
    );
}

#[test]
fn format_exp_matches_ffd2e() {
    for (v, d) in [
        (2.5f64, 6),
        (1.23e10, 4),
        (1.23456789e20, 8),
        (1.125, 15),
        (-0.5, 1),
        (1.0, 0),
    ] {
        let (c, st) = cfitsio_sys::ffd2e_str(v, d);
        assert_eq!(st, 0, "ffd2e failed for {v} {d}");
        assert_eq!(format_exp_double(v, d as usize), c, "exp {v} decim={d}");
    }
}

#[test]
fn format_fixed_matches_ffd2f() {
    for (v, d) in [(123.456789f64, 10), (1.25, 5), (1.0, 0), (32768.0, 0)] {
        let (c, st) = cfitsio_sys::ffd2f_str(v, d);
        assert_eq!(st, 0);
        assert_eq!(format_fixed_double(v, d as usize), c, "fixed {v} decim={d}");
    }
}

#[test]
fn write_string_logical_long_comment_history() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.write_str("STRKEY", "test value", Some("string keyword")),
        0
    );
    assert_eq!(c.write_log("BOOLKEY", true, Some("logical keyword")), 0);
    assert_eq!(c.write_lng("LONGKEY", 123456789, Some("long keyword")), 0);
    assert_eq!(c.write_comment("This is a comment keyword"), 0);
    assert_eq!(c.write_history("This is a history keyword"), 0);
    assert_eq!(c.write_null("NULLKEY", Some("undefined value")), 0);
    assert_eq!(c.close(), 0);

    let mut r = FitsFile::create(&r_path).unwrap();
    r.write_key_str("STRKEY", "test value", Some("string keyword"))
        .unwrap();
    r.write_key_log("BOOLKEY", true, Some("logical keyword"))
        .unwrap();
    r.write_key_lng("LONGKEY", 123456789, Some("long keyword"))
        .unwrap();
    r.write_comment("This is a comment keyword").unwrap();
    r.write_history("This is a history keyword").unwrap();
    r.write_key_null("NULLKEY", Some("undefined value"))
        .unwrap();
    r.close().unwrap();

    assert_files_eq(&c_path, &r_path);

    let f = FitsFile::open(&r_path, rfitsio::AccessMode::ReadOnly).unwrap();
    let (s, _) = f.read_key_str("STRKEY").unwrap();
    assert_eq!(s, "test value");
    assert!(f.read_key_log("BOOLKEY").unwrap().0);
    assert_eq!(f.read_key_lng("LONGKEY").unwrap(), 123456789);
    f.close().unwrap();
}

#[test]
fn write_float_double_units() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(c.write_flt("FLTKEY", 2.5, 6, Some("float keyword")), 0);
    assert_eq!(c.write_dbl("DBLKEY", 1.125, 15, Some("double keyword")), 0);
    assert_eq!(c.write_fixdbl("GKEY", 123.456789, 10, Some("G format")), 0);
    assert_eq!(c.write_lng("EXPTIME", 300, Some("Exposure time")), 0);
    assert_eq!(c.write_unit("EXPTIME", "seconds"), 0);
    assert_eq!(c.close(), 0);

    let mut r = FitsFile::create(&r_path).unwrap();
    r.write_key_flt("FLTKEY", 2.5, 6, Some("float keyword"))
        .unwrap();
    r.write_key_dbl("DBLKEY", 1.125, 15, Some("double keyword"))
        .unwrap();
    r.write_key_fixdbl("GKEY", 123.456789, 10, Some("G format"))
        .unwrap();
    r.write_key_lng("EXPTIME", 300, Some("Exposure time"))
        .unwrap();
    r.write_key_unit("EXPTIME", "seconds").unwrap();
    r.close().unwrap();

    assert_files_eq(&c_path, &r_path);

    let f = FitsFile::open(&r_path, rfitsio::AccessMode::ReadOnly).unwrap();
    assert!((f.read_key_dbl("FLTKEY").unwrap() - 2.5).abs() < 1e-5);
    assert_eq!(f.read_key_unit("EXPTIME").unwrap(), "seconds");
    let info = f.read_imghdr().unwrap();
    assert_eq!(info.bitpix, 8);
    assert_eq!(info.naxis, 0);
    assert!(info.extend);
    f.close().unwrap();
}

#[test]
fn update_modify_delete() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(c.write_str("TESTKEY", "value1", Some("comment1")), 0);
    assert_eq!(c.update_str("TESTKEY", "value2", Some("comment2")), 0);
    assert_eq!(c.update_str("NEWKEY", "inserted", Some("new")), 0);
    assert_eq!(c.modify_str("NEWKEY", "changed", Some("mod")), 0);
    assert_eq!(c.delete_key("TESTKEY"), 0);
    assert_eq!(c.close(), 0);

    let mut r = FitsFile::create(&r_path).unwrap();
    r.write_key_str("TESTKEY", "value1", Some("comment1"))
        .unwrap();
    r.update_key_str("TESTKEY", "value2", Some("comment2"))
        .unwrap();
    r.update_key_str("NEWKEY", "inserted", Some("new")).unwrap();
    r.modify_key_str("NEWKEY", "changed", Some("mod")).unwrap();
    r.delete_key("TESTKEY").unwrap();
    r.close().unwrap();

    assert_files_eq(&c_path, &r_path);

    let mut r = FitsFile::create(dir.path().join("m.fits")).unwrap();
    let err = r.modify_key_str("NOSUCH", "x", None).unwrap_err();
    assert_eq!(err.status, KEY_NO_EXIST);
    let err = r.delete_key("NOSUCH").unwrap_err();
    assert_eq!(err.status, KEY_NO_EXIST);
    r.close().unwrap();
}

#[test]
fn insert_and_indexed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ins.fits");
    let mut f = FitsFile::create(&path).unwrap();
    f.write_key_lng("A", 1, None).unwrap();
    f.write_key_lng("C", 3, None).unwrap();
    f.insert_key_str(2, "B", "mid", None).unwrap();
    assert_eq!(f.header().unwrap().len(), 6 + 3); // empty primary 6 + 3
    let names: Vec<_> = f
        .header()
        .unwrap()
        .cards()
        .iter()
        .map(|c| c.as_str().unwrap()[..8].trim().to_string())
        .collect();
    assert!(names.contains(&"B".to_string()));
    f.header_mut()
        .unwrap()
        .write_keys_long("IDX", 1, &[10, 20, 30], &[None, None, None])
        .unwrap();
    let (vals, n) = f.header().unwrap().get_keys_long("IDX", 1, 5);
    assert_eq!(n, 3);
    assert_eq!(vals, vec![10, 20, 30]);
    f.close().unwrap();
}

#[test]
fn date_keyword_format_exempt_from_byte_compare() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("date.fits");
    let mut f = FitsFile::create(&path).unwrap();
    f.write_date().unwrap();
    let (val, comm) = f.read_key_str("DATE").unwrap();
    assert!(is_fits_date(&val), "DATE={val}");
    assert!(val.contains('T'), "CFITSIO writes YYYY-MM-DDThh:mm:ss");
    assert!(comm.contains("file creation date"));
    f.close().unwrap();

    let mut f = FitsFile::create(dir.path().join("date2.fits")).unwrap();
    f.write_date_value("2026-08-25T12:00:00").unwrap();
    let (val, _) = f.read_key_str("DATE").unwrap();
    assert_eq!(val, "2026-08-25T12:00:00");
    f.close().unwrap();
}

#[test]
fn create_image_then_user_keys_still_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("img.fits");
    let mut f = FitsFile::create(&path).unwrap();
    f.create_image(ImageType::I16, &[4, 2]).unwrap();
    f.write_image(1, &[1i16, 2, 3, 4, 5, 6, 7, 8]).unwrap();
    f.write_key_str("OBJECT", "NGC3372", Some("target"))
        .unwrap();
    f.close().unwrap();

    let mut f = FitsFile::open(&path, rfitsio::AccessMode::ReadOnly).unwrap();
    let (obj, _) = f.read_key_str("OBJECT").unwrap();
    assert_eq!(obj, "NGC3372");
    let pix: Vec<i16> = f.read_image_all().unwrap();
    assert_eq!(pix, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    f.close().unwrap();
}
