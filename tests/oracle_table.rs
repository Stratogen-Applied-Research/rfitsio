//! Twin-write and cross-read ASCII tables against CFITSIO.

use std::fs;

use rfitsio::status::{BAD_TFORM, BAD_TFORM_DTYPE};
use rfitsio::tform::AsciiTform;
use rfitsio::types::{TDOUBLE, TFLOAT, TLONG, TSHORT, TSTRING};
use rfitsio::{AccessMode, FitsFile, HduType};

fn dump_cards(bytes: &[u8]) -> String {
    bytes
        .chunks(80)
        .take(40)
        .map(|c| String::from_utf8_lossy(c).trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_files_eq(c_path: &std::path::Path, r_path: &std::path::Path) {
    let c_bytes = fs::read(c_path).unwrap();
    let r_bytes = fs::read(r_path).unwrap();
    assert_eq!(
        r_bytes,
        c_bytes,
        "ASCII table bytes differ\n cfitsio:\n{}\n rfitsio:\n{}",
        dump_cards(&c_bytes),
        dump_cards(&r_bytes)
    );
}

fn cols() -> (
    [&'static str; 5],
    [&'static str; 5],
    [Option<&'static str>; 5],
) {
    (
        ["Name", "Ivalue", "Fvalue", "Evalue", "Dvalue"],
        ["A15", "I11", "F15.6", "E13.5", "D22.14"],
        [None, Some("m**2"), Some("cm"), Some("erg/s"), Some("km/s")],
    )
}

#[test]
fn tform_parse_matches_ffasfm() {
    for tf in [
        "A15", "I11", "I4", "F15.6", "E13.5", "D22.14", "F8.2", "E12.7",
    ] {
        let r = AsciiTform::parse(tf).unwrap();
        let (code, width, dec) = cfitsio_sys::ffasfm_parse(tf).unwrap();
        assert_eq!(r.datacode, code, "datacode {tf}");
        assert_eq!(r.width as i64, width, "width {tf}");
        assert_eq!(r.decimals as i32, dec, "decimals {tf}");
    }
    assert_eq!(
        AsciiTform::parse("X10").unwrap_err().status,
        cfitsio_sys::ffasfm_parse("X10").unwrap_err()
    );
    assert_eq!(
        AsciiTform::parse("X10").unwrap_err().status,
        BAD_TFORM_DTYPE
    );
    assert_eq!(AsciiTform::parse("I0").unwrap_err().status, BAD_TFORM);
    assert_eq!(
        AsciiTform::parse("I0").unwrap_err().status,
        cfitsio_sys::ffasfm_parse("I0").unwrap_err()
    );
    let i4 = AsciiTform::parse("I4").unwrap();
    assert_eq!(i4.datacode, TSHORT);
    let a = AsciiTform::parse("A15").unwrap();
    assert_eq!(a.datacode, TSTRING);
    let f = AsciiTform::parse("F15.6").unwrap();
    assert_eq!(f.datacode, TDOUBLE);
    let e = AsciiTform::parse("E13.5").unwrap();
    assert_eq!(e.datacode, TFLOAT);
    let i = AsciiTform::parse("I11").unwrap();
    assert_eq!(i.datacode, TLONG);
}

#[test]
fn ffgabc_matches_cfitsio() {
    let tforms = ["A15", "I11", "F15.6", "E13.5", "D22.14"];
    let (rowlen, tbcol) = rfitsio::ascii_column_starts(&tforms, 1).unwrap();
    let (crow, ctb) = cfitsio_sys::ffgabc_vals(&tforms, 1).unwrap();
    assert_eq!(rowlen, crow);
    assert_eq!(tbcol, ctb);
    assert_eq!(tbcol, vec![1, 17, 29, 45, 59]);
    assert_eq!(rowlen, 80);
}

#[test]
fn empty_ascii_table_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let (ttype, tform, tunit) = cols();

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::ASCII_TBL,
            3,
            &ttype,
            &tform,
            &tunit,
            Some("new_table"),
        ),
        0
    );
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_ascii_table(3, &ttype, &tform, &tunit, Some("new_table"))
        .unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn ascii_table_with_data_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let (ttype, tform, tunit) = cols();
    let names = ["alpha", "beta", "gamma"];
    let ivals = [1i64, 22, 333];
    let mut fvals = [1.25f32, 2.5, -3.75];
    let mut evals = [1.25f64, 2.5e10, -3.75e-4];
    let mut dvals = [1.25f64, 2.5e10, -3.75e-4];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::ASCII_TBL,
            3,
            &ttype,
            &tform,
            &tunit,
            Some("new_table"),
        ),
        0
    );
    assert_eq!(c.write_col_str(1, 1, &names), 0);
    assert_eq!(c.write_col_i64(2, 1, &ivals), 0);
    assert_eq!(c.write_col_f32(3, 1, &mut fvals), 0);
    assert_eq!(c.write_col_f64(4, 1, &mut evals), 0);
    assert_eq!(c.write_col_f64(5, 1, &mut dvals), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_ascii_table(3, &ttype, &tform, &tunit, Some("new_table"))
        .unwrap();
    f.write_col_str(1, 1, &names).unwrap();
    f.write_col_i64(2, 1, &ivals).unwrap();
    f.write_col_f32(3, 1, &fvals).unwrap();
    f.write_col_f64(4, 1, &evals).unwrap();
    f.write_col_f64(5, 1, &dvals).unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn ascii_table_cross_read() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let (ttype, tform, tunit) = cols();
    let names = ["alpha", "beta", "gamma"];
    let ivals = [1i64, 22, 333];
    let mut fvals = [1.25f64, 2.5, -3.75];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(cfitsio_sys::ASCII_TBL, 3, &ttype, &tform, &tunit, Some("t"),),
        0
    );
    assert_eq!(c.write_col_str(1, 1, &names), 0);
    assert_eq!(c.write_col_i64(2, 1, &ivals), 0);
    assert_eq!(c.write_col_f64(3, 1, &mut fvals), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::open(&c_path, AccessMode::ReadOnly).unwrap();
    assert_eq!(f.hdu_count().unwrap(), 2);
    assert_eq!(f.movabs_hdu(2).unwrap(), HduType::AsciiTable);
    let info = f.ascii_table_info().unwrap();
    assert_eq!(info.nrows, 3);
    assert_eq!(info.tfields, 5);
    assert_eq!(info.tbcol, vec![1, 17, 29, 45, 59]);
    assert_eq!(info.extname, "t");
    let (got, _) = f.read_col_str(1, 1, 3, None).unwrap();
    assert_eq!(got, names);
    let (goti, _) = f.read_col_i64(2, 1, 3, None).unwrap();
    assert_eq!(goti, ivals);
    let (gotf, _) = f.read_col_f64(3, 1, 3, None).unwrap();
    for (a, b) in gotf.iter().zip(fvals.iter()) {
        assert!((a - b).abs() < 1e-6, "{a} vs {b}");
    }
    f.close().unwrap();
}

#[test]
fn rfitsio_table_readable_by_cfitsio() {
    let dir = tempfile::tempdir().unwrap();
    let r_path = dir.path().join("r.fits");
    let (ttype, tform, tunit) = cols();
    let names = ["alpha", "beta", "gamma"];
    let ivals = [1i64, 22, 333];
    let evals = [1.25f64, 2.5e10, -3.75e-4];

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_ascii_table(3, &ttype, &tform, &tunit, Some("new_table"))
        .unwrap();
    f.write_col_str(1, 1, &names).unwrap();
    f.write_col_i64(2, 1, &ivals).unwrap();
    f.write_col_f64(4, 1, &evals).unwrap();
    f.close().unwrap();

    let mut c = cfitsio_sys::CFile::open(r_path.to_str().unwrap(), 0).unwrap();
    assert_eq!(c.movabs_hdu(2), 0);
    let (got, _) = c.read_col_str(1, 1, 3).unwrap();
    assert_eq!(got, names);
    let mut buf = vec![0i64; 3];
    c.read_col_i64(2, 1, &mut buf).unwrap();
    assert_eq!(buf, ivals);
    let mut dbuf = vec![0f64; 3];
    c.read_col_f64(4, 1, &mut dbuf).unwrap();
    for (a, b) in dbuf.iter().zip(evals.iter()) {
        assert!((a - b).abs() / b.abs().max(1.0) < 1e-12, "{a} vs {b}");
    }
    assert_eq!(c.close(), 0);
}

#[test]
fn tnull_and_blank_rows() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let ttype = ["Name", "Ivalue"];
    let tform = ["A15", "I11"];
    let tunit = [None, None];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(cfitsio_sys::ASCII_TBL, 2, &ttype, &tform, &tunit, Some("t")),
        0
    );
    assert_eq!(c.write_str("TNULL2", "NULL", Some("null")), 0);
    assert_eq!(c.write_col_str(1, 1, &["a", "b"]), 0);
    assert_eq!(c.write_col_i64(2, 1, &[5i64, 6]), 0);
    assert_eq!(c.write_col_null(2, 2, 1), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_ascii_table(2, &ttype, &tform, &tunit, Some("t"))
        .unwrap();
    f.write_key_str("TNULL2", "NULL", Some("null")).unwrap();
    f.write_col_str(1, 1, &["a", "b"]).unwrap();
    f.write_col_i64(2, 1, &[5i64, 6]).unwrap();
    f.write_col_null(2, 2, 1).unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);

    let mut f = FitsFile::open(&r_path, AccessMode::ReadOnly).unwrap();
    f.movabs_hdu(2).unwrap();
    let (vals, anynul) = f.read_col_i64(2, 1, 2, Some(-999)).unwrap();
    assert!(anynul);
    assert_eq!(vals, vec![5, -999]);
}

#[test]
fn insert_delete_rows_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let ttype = ["Name", "Ivalue"];
    let tform = ["A15", "I11"];
    let tunit = [None, None];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(cfitsio_sys::ASCII_TBL, 2, &ttype, &tform, &tunit, Some("t")),
        0
    );
    assert_eq!(c.write_col_str(1, 1, &["alpha", "beta"]), 0);
    assert_eq!(c.write_col_i64(2, 1, &[1i64, 2]), 0);
    assert_eq!(c.insert_rows(1, 1), 0);
    assert_eq!(c.delete_rows(3, 1), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_ascii_table(2, &ttype, &tform, &tunit, Some("t"))
        .unwrap();
    f.write_col_str(1, 1, &["alpha", "beta"]).unwrap();
    f.write_col_i64(2, 1, &[1i64, 2]).unwrap();
    f.insert_rows(1, 1).unwrap();
    f.delete_rows(3, 1).unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn insert_delete_columns_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let ttype = ["Name", "Ivalue"];
    let tform = ["A15", "I11"];
    let tunit = [None, None];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(cfitsio_sys::ASCII_TBL, 2, &ttype, &tform, &tunit, Some("t")),
        0
    );
    assert_eq!(c.write_col_str(1, 1, &["alpha", "beta"]), 0);
    assert_eq!(c.write_col_i64(2, 1, &[1i64, 2]), 0);
    assert_eq!(c.insert_col(2, "New", "F8.2"), 0);
    assert_eq!(c.delete_col(2), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_ascii_table(2, &ttype, &tform, &tunit, Some("t"))
        .unwrap();
    f.write_col_str(1, 1, &["alpha", "beta"]).unwrap();
    f.write_col_i64(2, 1, &[1i64, 2]).unwrap();
    f.insert_col(2, "New", "F8.2").unwrap();
    f.delete_col(2).unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn insert_column_header_comments() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let ttype = ["Name"];
    let tform = ["A15"];
    let tunit = [None];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(cfitsio_sys::ASCII_TBL, 0, &ttype, &tform, &tunit, None),
        0
    );
    assert_eq!(c.insert_col(2, "New", "I6"), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_ascii_table(0, &ttype, &tform, &tunit, None)
        .unwrap();
    f.insert_col(2, "New", "I6").unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn zero_row_table_then_write_extends() {
    let dir = tempfile::tempdir().unwrap();
    let r_path = dir.path().join("r.fits");
    let ttype = ["Name", "Ivalue"];
    let tform = ["A15", "I11"];
    let tunit = [None, None];
    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_ascii_table(0, &ttype, &tform, &tunit, Some("t"))
        .unwrap();
    f.write_col_str(1, 1, &["alpha", "beta"]).unwrap();
    f.write_col_i64(2, 1, &[10i64, 20]).unwrap();
    assert_eq!(f.nrows().unwrap(), 2);
    let (got, _) = f.read_col_str(1, 1, 2, None).unwrap();
    assert_eq!(got, ["alpha", "beta"]);
    f.close().unwrap();
}
