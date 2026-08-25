//! Twin-write HDU copy / delete / insert against CFITSIO.

use std::fs;

use rfitsio::status::{BAD_NAXES, BAD_NAXIS, READONLY_FILE};
use rfitsio::types::{SHORT_IMG, TSHORT};
use rfitsio::{AccessMode, FitsFile, HduType, ImageType};

fn dump_cards(bytes: &[u8]) -> String {
    bytes
        .chunks(80)
        .take(72)
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
        "HDU bytes differ\n cfitsio:\n{}\n rfitsio:\n{}",
        dump_cards(&c_bytes),
        dump_cards(&r_bytes)
    );
}

fn tbl() -> (
    [&'static str; 1],
    [&'static str; 1],
    [Option<&'static str>; 1],
) {
    (["COL1"], ["1J"], [None])
}

#[test]
fn insert_image_after_empty_primary_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let naxes = [10i64, 10];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(c.insert_img(SHORT_IMG, &naxes), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.insert_image(ImageType::I16, &naxes).unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn insert_bintable_after_empty_primary_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let (ttype, tform, tunit) = tbl();

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(c.insert_btbl(5, &ttype, &tform, &tunit, Some("TABLE")), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.insert_binary_table(5, &ttype, &tform, &tunit, Some("TABLE"))
        .unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn insert_ascii_table_after_empty_primary_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let ttype = ["COL1"];
    let tform = ["F10.2"];
    let tunit = [Some("m")];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(c.insert_atbl(0, 1, &ttype, &tform, &tunit, Some("TEST")), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.insert_ascii_table(1, &ttype, &tform, &tunit, Some("TEST"))
        .unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn insert_image_in_middle_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let (ttype, tform, tunit) = tbl();
    let naxes = [8i64, 4];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            2,
            &ttype,
            &tform,
            &tunit,
            Some("A")
        ),
        0
    );
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            2,
            &ttype,
            &tform,
            &tunit,
            Some("B")
        ),
        0
    );
    assert_eq!(c.movabs_hdu(1), 0);
    assert_eq!(c.insert_img(SHORT_IMG, &naxes), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(2, &ttype, &tform, &tunit, Some("A"))
        .unwrap();
    f.create_binary_table(2, &ttype, &tform, &tunit, Some("B"))
        .unwrap();
    f.movabs_hdu(1).unwrap();
    f.insert_image(ImageType::I16, &naxes).unwrap();
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn delete_middle_hdu_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let (ttype, tform, tunit) = tbl();

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            1,
            &ttype,
            &tform,
            &tunit,
            Some("TABLE1")
        ),
        0
    );
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            1,
            &ttype,
            &tform,
            &tunit,
            Some("TABLE2")
        ),
        0
    );
    assert_eq!(c.movabs_hdu(2), 0);
    assert!(c.delete_hdu().is_ok());
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(1, &ttype, &tform, &tunit, Some("TABLE1"))
        .unwrap();
    f.create_binary_table(1, &ttype, &tform, &tunit, Some("TABLE2"))
        .unwrap();
    f.movabs_hdu(2).unwrap();
    let ty = f.delete_hdu().unwrap();
    assert_eq!(ty, HduType::BinaryTable);
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn delete_primary_keeps_extension_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let (ttype, tform, tunit) = tbl();

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            3,
            &ttype,
            &tform,
            &tunit,
            Some("KEEP")
        ),
        0
    );
    assert_eq!(c.movabs_hdu(1), 0);
    assert!(c.delete_hdu().is_ok());
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(3, &ttype, &tform, &tunit, Some("KEEP"))
        .unwrap();
    f.movabs_hdu(1).unwrap();
    assert_eq!(f.delete_hdu().unwrap(), HduType::Image);
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn copy_table_hdu_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_src = dir.path().join("cs.fits");
    let c_dst = dir.path().join("cd.fits");
    let r_src = dir.path().join("rs.fits");
    let r_dst = dir.path().join("rd.fits");
    let (ttype, tform, tunit) = tbl();
    let vals = [10i64, 20, 30];

    let mut c = cfitsio_sys::CFile::create_empty(c_src.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            3,
            &ttype,
            &tform,
            &tunit,
            Some("EXT1")
        ),
        0
    );
    assert_eq!(c.write_col_i64(1, 1, &vals), 0);
    assert_eq!(c.close(), 0);
    let mut cin = cfitsio_sys::CFile::open(c_src.to_str().unwrap(), 0).unwrap();
    let mut cout = cfitsio_sys::CFile::create_empty(c_dst.to_str().unwrap()).unwrap();
    assert_eq!(cin.movabs_hdu(2), 0);
    assert_eq!(cin.copy_hdu(&mut cout, 0), 0);
    assert_eq!(cin.close(), 0);
    assert_eq!(cout.close(), 0);

    let mut f = FitsFile::create(&r_src).unwrap();
    f.create_binary_table(3, &ttype, &tform, &tunit, Some("EXT1"))
        .unwrap();
    f.write_col_i64(1, 1, &vals).unwrap();
    f.close().unwrap();
    let mut rin = FitsFile::open(&r_src, AccessMode::ReadOnly).unwrap();
    let mut rout = FitsFile::create(&r_dst).unwrap();
    rin.movabs_hdu(2).unwrap();
    rin.copy_hdu(&mut rout, 0).unwrap();
    rin.close().unwrap();
    rout.close().unwrap();
    assert_files_eq(&c_dst, &r_dst);
}

#[test]
fn copy_primary_image_as_extension_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_src = dir.path().join("cs.fits");
    let c_dst = dir.path().join("cd.fits");
    let r_src = dir.path().join("rs.fits");
    let r_dst = dir.path().join("rd.fits");
    let naxes = [8i64, 4];
    let data: Vec<i16> = (0..32).map(|i| i * 10 - 20).collect();

    cfitsio_sys::write_primary_image(c_src.to_str().unwrap(), SHORT_IMG, &naxes, TSHORT, &data)
        .unwrap();
    let mut cin = cfitsio_sys::CFile::open(c_src.to_str().unwrap(), 0).unwrap();
    let mut cout = cfitsio_sys::CFile::create_empty(c_dst.to_str().unwrap()).unwrap();
    assert_eq!(cin.copy_hdu(&mut cout, 0), 0);
    assert_eq!(cin.close(), 0);
    assert_eq!(cout.close(), 0);

    let mut f = FitsFile::create(&r_src).unwrap();
    f.create_image(ImageType::I16, &naxes).unwrap();
    f.write_image(1, &data).unwrap();
    f.close().unwrap();
    let mut rin = FitsFile::open(&r_src, AccessMode::ReadOnly).unwrap();
    let mut rout = FitsFile::create(&r_dst).unwrap();
    rin.copy_hdu(&mut rout, 0).unwrap();
    rin.close().unwrap();
    rout.close().unwrap();
    assert_files_eq(&c_dst, &r_dst);
}

#[test]
fn copy_file_all_hdus_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_src = dir.path().join("cs.fits");
    let c_dst = dir.path().join("cd.fits");
    let r_src = dir.path().join("rs.fits");
    let r_dst = dir.path().join("rd.fits");
    let (ttype, tform, tunit) = tbl();

    let mut c = cfitsio_sys::CFile::create_empty(c_src.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            2,
            &ttype,
            &tform,
            &tunit,
            Some("A")
        ),
        0
    );
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            2,
            &ttype,
            &tform,
            &tunit,
            Some("B")
        ),
        0
    );
    assert_eq!(c.close(), 0);
    let mut cin = cfitsio_sys::CFile::open(c_src.to_str().unwrap(), 0).unwrap();
    let mut cout = cfitsio_sys::CFile::create_empty(c_dst.to_str().unwrap()).unwrap();
    assert_eq!(cin.movabs_hdu(2), 0);
    assert_eq!(cin.copy_file(&mut cout, true, true, true), 0);
    assert_eq!(cin.close(), 0);
    assert_eq!(cout.close(), 0);

    let mut f = FitsFile::create(&r_src).unwrap();
    f.create_binary_table(2, &ttype, &tform, &tunit, Some("A"))
        .unwrap();
    f.create_binary_table(2, &ttype, &tform, &tunit, Some("B"))
        .unwrap();
    f.close().unwrap();
    let mut rin = FitsFile::open(&r_src, AccessMode::ReadOnly).unwrap();
    let mut rout = FitsFile::create(&r_dst).unwrap();
    rin.movabs_hdu(2).unwrap();
    rin.copy_file(&mut rout, true, true, true).unwrap();
    rin.close().unwrap();
    rout.close().unwrap();
    assert_files_eq(&c_dst, &r_dst);
}

#[test]
fn keyword_on_first_hdu_does_not_clobber_extension() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let (ttype, tform, tunit) = tbl();

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            2,
            &ttype,
            &tform,
            &tunit,
            Some("EXT")
        ),
        0
    );
    assert_eq!(c.movabs_hdu(1), 0);
    for i in 0..30 {
        assert_eq!(c.write_comment(&format!("extra keyword {i:02}")), 0);
    }
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(2, &ttype, &tform, &tunit, Some("EXT"))
        .unwrap();
    f.movabs_hdu(1).unwrap();
    for i in 0..30 {
        f.write_comment(&format!("extra keyword {i:02}")).unwrap();
    }
    f.close().unwrap();
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn insert_image_rejects_bad_naxis_and_naxes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.fits");
    let mut f = FitsFile::create(&path).unwrap();
    let too_many = vec![2i64; 1000];
    assert_eq!(
        f.insert_image(ImageType::I16, &too_many)
            .unwrap_err()
            .status,
        BAD_NAXIS
    );
    assert_eq!(
        f.insert_image(ImageType::I16, &[-10]).unwrap_err().status,
        BAD_NAXES
    );
}

#[test]
fn delete_hdu_readonly_fails() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.fits");
    {
        let mut f = FitsFile::create(&path).unwrap();
        let (ttype, tform, tunit) = tbl();
        f.create_binary_table(1, &ttype, &tform, &tunit, Some("X"))
            .unwrap();
        f.close().unwrap();
    }
    let mut f = FitsFile::open(&path, AccessMode::ReadOnly).unwrap();
    f.movabs_hdu(2).unwrap();
    assert_eq!(f.delete_hdu().unwrap_err().status, READONLY_FILE);
}

#[test]
fn delete_last_extension_moves_to_previous() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.fits");
    let (ttype, tform, tunit) = tbl();
    let mut f = FitsFile::create(&path).unwrap();
    f.create_binary_table(1, &ttype, &tform, &tunit, Some("A"))
        .unwrap();
    f.create_binary_table(1, &ttype, &tform, &tunit, Some("B"))
        .unwrap();
    assert_eq!(f.hdu_count().unwrap(), 3);
    let ty = f.delete_hdu().unwrap();
    assert_eq!(ty, HduType::BinaryTable);
    assert_eq!(f.hdunum().unwrap(), 2);
    assert_eq!(f.hdu_count().unwrap(), 2);
    let info = f.binary_table_info().unwrap();
    assert_eq!(info.extname, "A");
}
