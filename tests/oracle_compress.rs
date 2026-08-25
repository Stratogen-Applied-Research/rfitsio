//! Cross-read tiled compressed images against CFITSIO (pixel identity).

use rfitsio::types::{
    BYTE_IMG, FLOAT_IMG, GZIP_1, GZIP_2, HCOMPRESS_1, LONG_IMG, PLIO_1, RICE_1, SHORT_IMG, TBYTE,
    TFLOAT, TINT, TSHORT,
};
use rfitsio::{AccessMode, CompressionType, FitsFile, ImageType};

fn gradient_i16(n: usize) -> Vec<i16> {
    (0..n).map(|i| (i as i16 % 1000) - 20).collect()
}

fn gradient_i32(n: usize) -> Vec<i32> {
    (0..n).map(|i| (i as i32 % 1000) - 20).collect()
}

fn gradient_u8(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 3) as u8).collect()
}

fn cfitsio_write_compressed<T>(
    path: &std::path::Path,
    bitpix: i32,
    naxes: &[i64],
    datatype: i32,
    data: &[T],
    ctype: i32,
) {
    let mut f = cfitsio_sys::CFile::create_empty(path.to_str().unwrap()).unwrap();
    assert_eq!(f.set_compression_type(ctype), 0);
    assert_eq!(f.create_img(bitpix, naxes), 0);
    assert_eq!(f.write_img(datatype, 1, data), 0);
    assert_eq!(f.close(), 0);
}

fn rfitsio_write_compressed<T: rfitsio::Pixel>(
    path: &std::path::Path,
    ty: ImageType,
    naxes: &[i64],
    data: &[T],
    ctype: i32,
) {
    let mut f = FitsFile::create(path).unwrap();
    f.set_compression_type(ctype).unwrap();
    f.create_image(ty, naxes).unwrap();
    f.write_image(1, data).unwrap();
    f.close().unwrap();
}

fn find_compressed_hdu(f: &mut FitsFile) {
    let n = f.hdu_count().unwrap();
    for i in 1..=n {
        f.movabs_hdu(i).unwrap();
        if f.is_compressed_image().unwrap() {
            return;
        }
    }
    panic!("no compressed image HDU");
}

fn cfitsio_read_i16(path: &std::path::Path, npix: usize) -> Vec<i16> {
    let mut f = cfitsio_sys::CFile::open(path.to_str().unwrap(), 0).unwrap();
    let n = f.num_hdus().unwrap();
    for i in 1..=n {
        assert_eq!(f.movabs_hdu(i), 0);
        if f.is_compressed_image().unwrap() {
            let mut out = vec![0i16; npix];
            f.read_img(TSHORT, 1, &mut out).unwrap();
            f.close();
            return out;
        }
    }
    f.close();
    panic!("cfitsio: no compressed image HDU in {}", path.display());
}

fn cfitsio_read_i32(path: &std::path::Path, npix: usize) -> Vec<i32> {
    let mut f = cfitsio_sys::CFile::open(path.to_str().unwrap(), 0).unwrap();
    let n = f.num_hdus().unwrap();
    for i in 1..=n {
        assert_eq!(f.movabs_hdu(i), 0);
        if f.is_compressed_image().unwrap() {
            let mut out = vec![0i32; npix];
            f.read_img(TINT, 1, &mut out).unwrap();
            f.close();
            return out;
        }
    }
    f.close();
    panic!("cfitsio: no compressed image HDU");
}

fn cfitsio_read_u8(path: &std::path::Path, npix: usize) -> Vec<u8> {
    let mut f = cfitsio_sys::CFile::open(path.to_str().unwrap(), 0).unwrap();
    let n = f.num_hdus().unwrap();
    for i in 1..=n {
        assert_eq!(f.movabs_hdu(i), 0);
        if f.is_compressed_image().unwrap() {
            let mut out = vec![0u8; npix];
            f.read_img(TBYTE, 1, &mut out).unwrap();
            f.close();
            return out;
        }
    }
    f.close();
    panic!("cfitsio: no compressed image HDU");
}

fn rfitsio_read_i16(path: &std::path::Path, npix: usize) -> Vec<i16> {
    let mut f = FitsFile::open(path, AccessMode::ReadOnly).unwrap();
    find_compressed_hdu(&mut f);
    f.read_image(1, npix).unwrap()
}

fn rfitsio_read_i32(path: &std::path::Path, npix: usize) -> Vec<i32> {
    let mut f = FitsFile::open(path, AccessMode::ReadOnly).unwrap();
    find_compressed_hdu(&mut f);
    f.read_image(1, npix).unwrap()
}

fn rfitsio_read_u8(path: &std::path::Path, npix: usize) -> Vec<u8> {
    let mut f = FitsFile::open(path, AccessMode::ReadOnly).unwrap();
    find_compressed_hdu(&mut f);
    f.read_image(1, npix).unwrap()
}

#[test]
fn set_get_compression_type() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.fits");
    let mut f = FitsFile::create(&path).unwrap();
    f.create_image(ImageType::I16, &[8, 8]).unwrap();
    for code in [RICE_1, GZIP_1, GZIP_2, PLIO_1, HCOMPRESS_1] {
        f.set_compression_type(code).unwrap();
        assert_eq!(f.get_compression_type().unwrap(), code);
    }
    f.close().unwrap();
}

#[test]
fn rice_i16_cfitsio_write_rfitsio_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c.fits");
    let naxes = [32i64, 16];
    let data = gradient_i16(32 * 16);
    cfitsio_write_compressed(&path, SHORT_IMG, &naxes, TSHORT, &data, RICE_1);
    let out = rfitsio_read_i16(&path, data.len());
    assert_eq!(out, data);
}

#[test]
fn rice_i16_rfitsio_write_cfitsio_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("r.fits");
    let naxes = [32i64, 16];
    let data = gradient_i16(32 * 16);
    rfitsio_write_compressed(&path, ImageType::I16, &naxes, &data, RICE_1);
    let mut f = FitsFile::open(&path, AccessMode::ReadOnly).unwrap();
    find_compressed_hdu(&mut f);
    assert!(f.is_compressed_image().unwrap());
    assert_eq!(f.hdu_type().unwrap(), rfitsio::HduType::Image);
    drop(f);
    let out = cfitsio_read_i16(&path, data.len());
    assert_eq!(out, data);
}

#[test]
fn rice_i16_self_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("r.fits");
    let naxes = [24i64, 12];
    let data = gradient_i16(24 * 12);
    rfitsio_write_compressed(&path, ImageType::I16, &naxes, &data, RICE_1);
    let out = rfitsio_read_i16(&path, data.len());
    assert_eq!(out, data);
}

#[test]
fn gzip1_i32_cross() {
    let dir = tempfile::tempdir().unwrap();
    let naxes = [16i64, 8];
    let data = gradient_i32(16 * 8);
    let c = dir.path().join("c.fits");
    cfitsio_write_compressed(&c, LONG_IMG, &naxes, TINT, &data, GZIP_1);
    assert_eq!(rfitsio_read_i32(&c, data.len()), data);
    let r = dir.path().join("r.fits");
    rfitsio_write_compressed(&r, ImageType::I32, &naxes, &data, GZIP_1);
    assert_eq!(cfitsio_read_i32(&r, data.len()), data);
}

#[test]
fn gzip2_i16_cross() {
    let dir = tempfile::tempdir().unwrap();
    let naxes = [16i64, 8];
    let data = gradient_i16(16 * 8);
    let c = dir.path().join("c.fits");
    cfitsio_write_compressed(&c, SHORT_IMG, &naxes, TSHORT, &data, GZIP_2);
    assert_eq!(rfitsio_read_i16(&c, data.len()), data);
    let r = dir.path().join("r.fits");
    rfitsio_write_compressed(&r, ImageType::I16, &naxes, &data, GZIP_2);
    assert_eq!(cfitsio_read_i16(&r, data.len()), data);
}

#[test]
fn plio_i16_cross() {
    let dir = tempfile::tempdir().unwrap();
    let naxes = [20i64, 8];
    let data: Vec<i16> = (0..160).map(|i| (i % 40) as i16).collect();
    let c = dir.path().join("c.fits");
    cfitsio_write_compressed(&c, SHORT_IMG, &naxes, TSHORT, &data, PLIO_1);
    assert_eq!(rfitsio_read_i16(&c, data.len()), data);
    let r = dir.path().join("r.fits");
    rfitsio_write_compressed(&r, ImageType::I16, &naxes, &data, PLIO_1);
    assert_eq!(cfitsio_read_i16(&r, data.len()), data);
}

#[test]
fn hcompress_i16_cross() {
    let dir = tempfile::tempdir().unwrap();
    let naxes = [16i64, 16];
    let data = gradient_i16(16 * 16);
    let c = dir.path().join("c.fits");
    cfitsio_write_compressed(&c, SHORT_IMG, &naxes, TSHORT, &data, HCOMPRESS_1);
    assert_eq!(rfitsio_read_i16(&c, data.len()), data);
    let r = dir.path().join("r.fits");
    rfitsio_write_compressed(&r, ImageType::I16, &naxes, &data, HCOMPRESS_1);
    assert_eq!(cfitsio_read_i16(&r, data.len()), data);
}

#[test]
fn rice_u8_cross() {
    let dir = tempfile::tempdir().unwrap();
    let naxes = [32i64, 8];
    let data = gradient_u8(32 * 8);
    let c = dir.path().join("c.fits");
    cfitsio_write_compressed(&c, BYTE_IMG, &naxes, TBYTE, &data, RICE_1);
    assert_eq!(rfitsio_read_u8(&c, data.len()), data);
    let r = dir.path().join("r.fits");
    rfitsio_write_compressed(&r, ImageType::U8, &naxes, &data, RICE_1);
    assert_eq!(cfitsio_read_u8(&r, data.len()), data);
}

#[test]
fn gzip_float_lossless_cross() {
    let dir = tempfile::tempdir().unwrap();
    let naxes = [8i64, 8];
    let data: Vec<f32> = (0..64).map(|i| i as f32 * 0.25).collect();
    let c = dir.path().join("c.fits");
    let mut f = cfitsio_sys::CFile::create_empty(c.to_str().unwrap()).unwrap();
    assert_eq!(f.set_compression_type(GZIP_1), 0);
    assert_eq!(f.set_quantize_level(0.0), 0);
    assert_eq!(f.create_img(FLOAT_IMG, &naxes), 0);
    assert_eq!(f.write_img(TFLOAT, 1, &data), 0);
    assert_eq!(f.close(), 0);
    let mut rf = FitsFile::open(&c, AccessMode::ReadOnly).unwrap();
    find_compressed_hdu(&mut rf);
    let out: Vec<f32> = rf.read_image(1, data.len()).unwrap();
    assert_eq!(out, data);

    let r = dir.path().join("r.fits");
    let mut wf = FitsFile::create(&r).unwrap();
    wf.set_compression_type(GZIP_1).unwrap();
    wf.set_quantize_level(0.0).unwrap();
    wf.create_image(ImageType::F32, &naxes).unwrap();
    wf.write_image(1, &data).unwrap();
    wf.close().unwrap();
    let mut cf = cfitsio_sys::CFile::open(r.to_str().unwrap(), 0).unwrap();
    let n = cf.num_hdus().unwrap();
    let mut got = vec![0f32; data.len()];
    for i in 1..=n {
        assert_eq!(cf.movabs_hdu(i), 0);
        if cf.is_compressed_image().unwrap() {
            cf.read_img(TFLOAT, 1, &mut got).unwrap();
            break;
        }
    }
    cf.close();
    assert_eq!(got, data);
}

#[test]
fn unknown_compression_type_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut f = FitsFile::create(dir.path().join("t.fits")).unwrap();
    assert_eq!(
        f.set_compression_type(99).unwrap_err().status,
        rfitsio::status::DATA_COMPRESSION_ERR
    );
}

#[test]
fn compression_type_names() {
    assert_eq!(
        CompressionType::from_code(RICE_1).unwrap().zcmptype(),
        "RICE_1"
    );
    assert_eq!(CompressionType::Gzip2.zcmptype(), "GZIP_2");
}

#[test]
fn rice_i32_and_hcompress_i32_cross() {
    let dir = tempfile::tempdir().unwrap();
    let naxes = [16i64, 8];
    let data = gradient_i32(16 * 8);
    let c = dir.path().join("c32.fits");
    cfitsio_write_compressed(&c, LONG_IMG, &naxes, TINT, &data, RICE_1);
    assert_eq!(rfitsio_read_i32(&c, data.len()), data);
    let r = dir.path().join("r32.fits");
    rfitsio_write_compressed(&r, ImageType::I32, &naxes, &data, RICE_1);
    assert_eq!(cfitsio_read_i32(&r, data.len()), data);

    let naxes = [16i64, 16];
    let data = gradient_i32(16 * 16);
    let c = dir.path().join("ch.fits");
    cfitsio_write_compressed(&c, LONG_IMG, &naxes, TINT, &data, HCOMPRESS_1);
    assert_eq!(rfitsio_read_i32(&c, data.len()), data);
    let r = dir.path().join("rh.fits");
    rfitsio_write_compressed(&r, ImageType::I32, &naxes, &data, HCOMPRESS_1);
    assert_eq!(cfitsio_read_i32(&r, data.len()), data);
}

#[test]
fn quantized_float_cfitsio_write_rfitsio_read() {
    let dir = tempfile::tempdir().unwrap();
    let naxes = [16i64, 8];
    let data: Vec<f32> = (0..128).map(|i| (i as f32) * 0.1 + 3.0).collect();
    let c = dir.path().join("q.fits");
    let mut f = cfitsio_sys::CFile::create_empty(c.to_str().unwrap()).unwrap();
    assert_eq!(f.set_compression_type(RICE_1), 0);
    assert_eq!(f.set_quantize_level(16.0), 0);
    assert_eq!(f.create_img(FLOAT_IMG, &naxes), 0);
    assert_eq!(f.write_img(TFLOAT, 1, &data), 0);
    assert_eq!(f.close(), 0);

    let mut cf = cfitsio_sys::CFile::open(c.to_str().unwrap(), 0).unwrap();
    let n = cf.num_hdus().unwrap();
    let mut c_out = vec![0f32; data.len()];
    for i in 1..=n {
        assert_eq!(cf.movabs_hdu(i), 0);
        if cf.is_compressed_image().unwrap() {
            cf.read_img(TFLOAT, 1, &mut c_out).unwrap();
            break;
        }
    }
    cf.close();

    let mut rf = FitsFile::open(&c, AccessMode::ReadOnly).unwrap();
    find_compressed_hdu(&mut rf);
    let r_out: Vec<f32> = rf.read_image(1, data.len()).unwrap();
    for (a, b) in c_out.iter().zip(r_out.iter()) {
        assert!((a - b).abs() < 1e-3, "{a} vs {b}");
    }
}
