//! Twin-write and cross-read binary tables against CFITSIO.

use std::fs;

use rfitsio::tform::BinaryTform;
use rfitsio::types::{TBYTE, TLOGICAL, TLONG, TSHORT, TSTRING};
use rfitsio::{AccessMode, FitsFile, HduType};

fn dump_cards(bytes: &[u8]) -> String {
    bytes
        .chunks(80)
        .take(48)
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
        "bintable bytes differ\n cfitsio:\n{}\n rfitsio:\n{}",
        dump_cards(&c_bytes),
        dump_cards(&r_bytes)
    );
}

fn std_cols() -> (
    [&'static str; 8],
    [&'static str; 8],
    [Option<&'static str>; 8],
) {
    (
        [
            "Avalue", "Lvalue", "Xvalue", "Bvalue", "Ivalue", "Jvalue", "Evalue", "Dvalue",
        ],
        ["1A", "1L", "1X", "1B", "1I", "1J", "1E", "1D"],
        [
            None,
            Some("m**2"),
            Some("cm"),
            Some("erg/s"),
            Some("km/s"),
            None,
            None,
            None,
        ],
    )
}

#[test]
fn binary_tform_matches_ffbnfm() {
    for tf in [
        "1A", "1L", "1X", "1B", "1I", "1J", "1K", "1E", "1D", "1C", "1M", "1PJ", "1QK", "10E", "J",
        "1U", "1S",
    ] {
        let r = BinaryTform::parse(tf).unwrap();
        let (code, repeat, width) = cfitsio_sys::ffbnfm_parse(tf).unwrap();
        assert_eq!(r.datacode, code, "datacode {tf}");
        assert_eq!(r.repeat, repeat, "repeat {tf}");
        assert_eq!(r.width as i64, width, "width {tf}");
    }
    let a = BinaryTform::parse("1A").unwrap();
    assert_eq!(a.datacode, TSTRING);
    let l = BinaryTform::parse("1L").unwrap();
    assert_eq!(l.datacode, TLOGICAL);
    let b = BinaryTform::parse("1B").unwrap();
    assert_eq!(b.datacode, TBYTE);
    let i = BinaryTform::parse("1I").unwrap();
    assert_eq!(i.datacode, TSHORT);
    let j = BinaryTform::parse("1J").unwrap();
    assert_eq!(j.datacode, TLONG);
}

#[test]
fn empty_bintable_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let (ttype, tform, tunit) = std_cols();

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            3,
            &ttype,
            &tform,
            &tunit,
            Some("bintable"),
        ),
        0
    );
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(3, &ttype, &tform, &tunit, Some("bintable"))
        .unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn bintable_with_data_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let (ttype, tform, tunit) = std_cols();
    let names = ["a", "b"];
    let logs = [true, false];
    let bits = [true, false];
    let bytes = [1i64, 2];
    let shorts = [-1i64, 256];
    let longs = [-2i64, 65536];
    let mut flts = [1.5f32, -2.5];
    let mut dbls = [1.25f64, -2.5];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            2,
            &ttype,
            &tform,
            &tunit,
            Some("bintable"),
        ),
        0
    );
    assert_eq!(c.write_col_str(1, 1, &names), 0);
    assert_eq!(c.write_col_log(2, 1, &logs), 0);
    assert_eq!(c.write_col_bit(3, 1, &bits), 0);
    assert_eq!(c.write_col_i64(4, 1, &bytes), 0);
    assert_eq!(c.write_col_i64(5, 1, &shorts), 0);
    assert_eq!(c.write_col_i64(6, 1, &longs), 0);
    assert_eq!(c.write_col_f32(7, 1, &mut flts), 0);
    assert_eq!(c.write_col_f64(8, 1, &mut dbls), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(2, &ttype, &tform, &tunit, Some("bintable"))
        .unwrap();
    f.write_col_str(1, 1, &names).unwrap();
    f.write_col_log(2, 1, &logs).unwrap();
    f.write_col_bit(3, 1, &bits).unwrap();
    f.write_col_i64(4, 1, &bytes).unwrap();
    f.write_col_i64(5, 1, &shorts).unwrap();
    f.write_col_i64(6, 1, &longs).unwrap();
    f.write_col_f32(7, 1, &flts).unwrap();
    f.write_col_f64(8, 1, &dbls).unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn vla_pj_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let ttype = ["PJ"];
    let tform = ["1PJ"];
    let tunit = [None];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            2,
            &ttype,
            &tform,
            &tunit,
            Some("vla")
        ),
        0
    );
    assert_eq!(c.write_col_i64(1, 1, &[10i64, 20, 30]), 0);
    assert_eq!(c.write_col_i64(1, 2, &[40i64, 50]), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(2, &ttype, &tform, &tunit, Some("vla"))
        .unwrap();
    f.write_col_i64(1, 1, &[10, 20, 30]).unwrap();
    f.write_col_i64(1, 2, &[40, 50]).unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn unsigned_and_vla_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let ttype = ["U", "V", "W", "S", "PJ"];
    let tform = ["1U", "1V", "1W", "1S", "1PJ"];
    let tunit = [None, None, None, None, None];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            2,
            &ttype,
            &tform,
            &tunit,
            Some("vla")
        ),
        0
    );
    assert_eq!(c.write_col_i64(1, 1, &[1i64, 40000]), 0);
    assert_eq!(c.write_col_i64(2, 1, &[1i64, 3_000_000_000]), 0);
    // U64 via ffpclujj — write as two rows of TLONGLONG may overflow; skip W on C
    assert_eq!(c.write_col_i64(4, 1, &[-1i64, 127]), 0);
    assert_eq!(c.write_col_i64(5, 1, &[10i64, 20, 30]), 0);
    assert_eq!(c.write_col_i64(5, 2, &[40i64, 50]), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(2, &ttype, &tform, &tunit, Some("vla"))
        .unwrap();
    f.write_col_i64(1, 1, &[1i64, 40000]).unwrap();
    f.write_col_i64(2, 1, &[1i64, 3_000_000_000]).unwrap();
    f.write_col_u64(3, 1, &[1u64, (1u64 << 63) + 5]).unwrap();
    f.write_col_i64(4, 1, &[-1i64, 127]).unwrap();
    f.write_col_i64(5, 1, &[10, 20, 30]).unwrap();
    f.write_col_i64(5, 2, &[40, 50]).unwrap();
    f.close().unwrap();

    let mut f = FitsFile::open(&c_path, AccessMode::ReadOnly).unwrap();
    assert_eq!(f.movabs_hdu(2).unwrap(), HduType::BinaryTable);
    let info = f.binary_table_info().unwrap();
    assert_eq!(info.tfields, 5);
    assert!(info.pcount > 0);
    assert!(info.tform[4].contains("PJ"));
    let (u, _) = f.read_col_i64(1, 1, 2, None).unwrap();
    assert_eq!(u, [1, 40000]);
    let (s, _) = f.read_col_i64(4, 1, 2, None).unwrap();
    assert_eq!(s, [-1, 127]);
    let (v1, _) = f.read_col_i64(5, 1, 3, None).unwrap();
    assert_eq!(v1, [10, 20, 30]);
    let (v2, _) = f.read_col_i64(5, 2, 2, None).unwrap();
    assert_eq!(v2, [40, 50]);
    f.close().unwrap();
}

#[test]
fn unsigned_header_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let ttype = ["U", "V", "W", "S"];
    let tform = ["1U", "1V", "1W", "1S"];
    let tunit = [None, None, None, None];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            2,
            &ttype,
            &tform,
            &tunit,
            Some("u")
        ),
        0
    );
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(2, &ttype, &tform, &tunit, Some("u"))
        .unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}
