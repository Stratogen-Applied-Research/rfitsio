//! Twin-write and cross-read image HDUs against CFITSIO.

use std::fs;

use rfitsio::types::{
    BYTE_IMG, DOUBLE_IMG, FLOAT_IMG, LONG_IMG, LONGLONG_IMG, SBYTE_IMG, SHORT_IMG, TBYTE, TDOUBLE,
    TFLOAT, TINT, TLONGLONG, TSBYTE, TSHORT, TUINT, TULONGLONG, TUSHORT, ULONG_IMG, ULONGLONG_IMG,
    USHORT_IMG,
};
use rfitsio::{AccessMode, FitsFile, ImageType, fits_create_img, fits_write_img};

fn dump_cards(bytes: &[u8]) -> String {
    bytes
        .chunks(80)
        .take(16)
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
        "image bytes differ\n cfitsio:\n{}\n rfitsio:\n{}",
        dump_cards(&c_bytes),
        dump_cards(&r_bytes)
    );
}

fn rfitsio_write<T: rfitsio::Pixel>(
    path: &std::path::Path,
    ty: ImageType,
    naxes: &[i64],
    data: &[T],
) {
    let mut f = FitsFile::create(path).unwrap();
    f.create_image(ty, naxes).unwrap();
    f.write_image(1, data).unwrap();
    f.close().unwrap();
}

#[test]
fn i16_image_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let naxes = [8i64, 4];
    let data: Vec<i16> = (0..32).map(|i| i * 10 - 20).collect();
    cfitsio_sys::write_primary_image(c_path.to_str().unwrap(), SHORT_IMG, &naxes, TSHORT, &data)
        .unwrap();
    rfitsio_write(&r_path, ImageType::I16, &naxes, &data);
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn u8_image_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let naxes = [16i64, 2];
    let data: Vec<u8> = (0..32).map(|i| (i * 7) as u8).collect();
    cfitsio_sys::write_primary_image(c_path.to_str().unwrap(), BYTE_IMG, &naxes, TBYTE, &data)
        .unwrap();
    rfitsio_write(&r_path, ImageType::U8, &naxes, &data);
    assert_files_eq(&c_path, &r_path);
}

#[test]
fn i32_f32_f64_i64_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let naxes = [5i64, 3];

    let i32s: Vec<i32> = (0..15).map(|i| i * 1000 - 5000).collect();
    let c = dir.path().join("c32.fits");
    let r = dir.path().join("r32.fits");
    cfitsio_sys::write_primary_image(c.to_str().unwrap(), LONG_IMG, &naxes, TINT, &i32s).unwrap();
    rfitsio_write(&r, ImageType::I32, &naxes, &i32s);
    assert_files_eq(&c, &r);

    let f32s: Vec<f32> = (0..15).map(|i| i as f32 * 0.5).collect();
    let c = dir.path().join("cf32.fits");
    let r = dir.path().join("rf32.fits");
    cfitsio_sys::write_primary_image(c.to_str().unwrap(), FLOAT_IMG, &naxes, TFLOAT, &f32s)
        .unwrap();
    rfitsio_write(&r, ImageType::F32, &naxes, &f32s);
    assert_files_eq(&c, &r);

    let f64s: Vec<f64> = (0..15).map(|i| i as f64 * 0.25).collect();
    let c = dir.path().join("cf64.fits");
    let r = dir.path().join("rf64.fits");
    cfitsio_sys::write_primary_image(c.to_str().unwrap(), DOUBLE_IMG, &naxes, TDOUBLE, &f64s)
        .unwrap();
    rfitsio_write(&r, ImageType::F64, &naxes, &f64s);
    assert_files_eq(&c, &r);

    let i64s: Vec<i64> = (0..15).map(|i| i * 1_000_000_000_000i64 - 3).collect();
    let c = dir.path().join("c64.fits");
    let r = dir.path().join("r64.fits");
    cfitsio_sys::write_primary_image(c.to_str().unwrap(), LONGLONG_IMG, &naxes, TLONGLONG, &i64s)
        .unwrap();
    rfitsio_write(&r, ImageType::I64, &naxes, &i64s);
    assert_files_eq(&c, &r);
}

#[test]
fn ushort_byte_identical_and_cross_read() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let naxes = [6i64, 2];
    let data: Vec<u16> = (0..12).map(|i| 40_000 + i * 100).collect();
    cfitsio_sys::write_primary_image(c_path.to_str().unwrap(), USHORT_IMG, &naxes, TUSHORT, &data)
        .unwrap();
    rfitsio_write(&r_path, ImageType::U16, &naxes, &data);
    assert_files_eq(&c_path, &r_path);

    let mut f = FitsFile::open(&c_path, AccessMode::ReadOnly).unwrap();
    let got: Vec<u16> = f.read_image_all().unwrap();
    f.close().unwrap();
    assert_eq!(got, data);

    let mut buf = vec![0u16; data.len()];
    cfitsio_sys::read_primary_image(r_path.to_str().unwrap(), TUSHORT, &mut buf).unwrap();
    assert_eq!(buf, data);
}

#[test]
fn sbyte_ulong_ulonglong() {
    let dir = tempfile::tempdir().unwrap();
    let naxes = [4i64, 2];

    let i8s: Vec<i8> = vec![-128, -1, 0, 1, 2, 3, 4, 127];
    let c = dir.path().join("c8.fits");
    let r = dir.path().join("r8.fits");
    cfitsio_sys::write_primary_image(c.to_str().unwrap(), SBYTE_IMG, &naxes, TSBYTE, &i8s).unwrap();
    rfitsio_write(&r, ImageType::I8, &naxes, &i8s);
    assert_files_eq(&c, &r);

    let u32s: Vec<u32> = vec![0, 1, 2, 3, 4_000_000_000, 4_000_000_001, 10, 20];
    let c = dir.path().join("cu32.fits");
    let r = dir.path().join("ru32.fits");
    cfitsio_sys::write_primary_image(c.to_str().unwrap(), ULONG_IMG, &naxes, TUINT, &u32s).unwrap();
    rfitsio_write(&r, ImageType::U32, &naxes, &u32s);
    assert_files_eq(&c, &r);

    let u64s: Vec<u64> = vec![0, 1, u64::MAX, 1u64 << 63, (1u64 << 63) + 5, 99, 100, 101];
    let c = dir.path().join("cu64.fits");
    let r = dir.path().join("ru64.fits");
    cfitsio_sys::write_primary_image(
        c.to_str().unwrap(),
        ULONGLONG_IMG,
        &naxes,
        TULONGLONG,
        &u64s,
    )
    .unwrap();
    rfitsio_write(&r, ImageType::U64, &naxes, &u64s);
    assert_files_eq(&c, &r);
}

#[test]
fn cross_read_i16() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let naxes = [10i64, 3];
    let data: Vec<i16> = (0..30).map(|i| i * 3 - 15).collect();
    cfitsio_sys::write_primary_image(c_path.to_str().unwrap(), SHORT_IMG, &naxes, TSHORT, &data)
        .unwrap();
    rfitsio_write(&r_path, ImageType::I16, &naxes, &data);

    let mut f = FitsFile::open(&c_path, AccessMode::ReadOnly).unwrap();
    let got: Vec<i16> = f.read_image_all().unwrap();
    assert_eq!(got, data);
    f.close().unwrap();

    let mut buf = vec![0i16; data.len()];
    cfitsio_sys::read_primary_image(r_path.to_str().unwrap(), TSHORT, &mut buf).unwrap();
    assert_eq!(buf, data);
}

#[test]
fn implicit_conversion_f32_to_i16() {
    let dir = tempfile::tempdir().unwrap();
    let naxes = [8i64, 1];
    let floats: Vec<f32> = (0..8).map(|i| i as f32 + 0.4).collect();
    let path = dir.path().join("conv.fits");
    let mut f = FitsFile::create(&path).unwrap();
    f.create_image(ImageType::I16, &naxes).unwrap();
    f.write_image(1, &floats).unwrap();
    let got: Vec<i16> = f.read_image_all().unwrap();
    f.close().unwrap();
    let expect: Vec<i16> = floats.iter().map(|v| v.round() as i16).collect();
    assert_eq!(got, expect);

    let mut buf = vec![0i16; 8];
    cfitsio_sys::read_primary_image(path.to_str().unwrap(), TSHORT, &mut buf).unwrap();
    assert_eq!(buf, expect);
}

#[test]
fn subset_write_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub.fits");
    let naxes = [10i64, 2];
    let mut f = FitsFile::create(&path).unwrap();
    f.create_image(ImageType::I16, &naxes).unwrap();
    let chunk: Vec<i16> = (0..5).map(|i| 100 + i).collect();
    f.write_image(6, &chunk).unwrap();
    let all: Vec<i16> = f.read_image_all().unwrap();
    f.close().unwrap();
    assert_eq!(&all[5..10], chunk.as_slice());
    assert!(all[..5].iter().all(|&x| x == 0));
    assert!(all[10..].iter().all(|&x| x == 0));
}

#[test]
fn named_api_and_second_hdu() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("two.fits");
    let mut f = FitsFile::create(&path).unwrap();
    fits_create_img(&mut f, SHORT_IMG, &[4, 4]).unwrap();
    let data: Vec<i16> = (0..16).collect();
    fits_write_img(&mut f, 1, &data).unwrap();
    f.create_image(ImageType::U8, &[8, 1]).unwrap();
    let u8s: Vec<u8> = (0..8).map(|i| i * 3).collect();
    f.write_image(1, &u8s).unwrap();
    assert_eq!(f.hdu_count().unwrap(), 2);
    f.close().unwrap();

    let f = FitsFile::open(&path, AccessMode::ReadOnly).unwrap();
    assert_eq!(f.hdu_count().unwrap(), 2);
    f.close().unwrap();
}

#[test]
fn resize_last_hdu() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rz.fits");
    let mut f = FitsFile::create(&path).unwrap();
    f.create_image(ImageType::I16, &[4, 2]).unwrap();
    f.write_image(1, &[1i16, 2, 3, 4, 5, 6, 7, 8]).unwrap();
    f.resize_image(ImageType::I16, &[2, 2]).unwrap();
    let (ty, naxes) = f.image_size().unwrap();
    assert_eq!(ty, ImageType::I16);
    assert_eq!(naxes, vec![2, 2]);
    f.close().unwrap();
}
