//! Twin-test CHECKSUM / gzip / stream I/O against CFITSIO.

use std::fs;

use rfitsio::types::{BYTE_IMG, TBYTE};
use rfitsio::{
    AccessMode, FitsFile, ImageType, decode_checksum, encode_checksum, fits_decode_chksum,
    fits_encode_chksum,
};

#[test]
fn encode_decode_matches_ffesum_ffdsum() {
    for &sum in &[
        0u32,
        1,
        0x1234_5678,
        0xabcd_ef00,
        0xffff_ffff,
        42,
        0x8000_0000,
        0x00ff_00ff,
        0x0102_0304,
    ] {
        for complement in [false, true] {
            let ours = encode_checksum(sum, complement);
            let c = cfitsio_sys::ffesum_str(sum, complement);
            assert_eq!(ours, c, "encode {sum:#x} complement={complement}");
            assert_eq!(ours.len(), 16);
            assert_eq!(decode_checksum(&ours, complement), sum, "decode {sum:#x}");
            assert_eq!(cfitsio_sys::ffdsum_u32(&c, complement), sum);
            assert_eq!(fits_encode_chksum(sum, complement), ours);
            assert_eq!(fits_decode_chksum(&ours, complement), sum);
        }
    }
}

#[test]
fn get_chksum_matches_ffgcks() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let naxes = [10i64];
    let data: Vec<u8> = (1..=10).collect();

    cfitsio_sys::write_primary_image(c_path.to_str().unwrap(), BYTE_IMG, &naxes, TBYTE, &data)
        .unwrap();
    let mut c = cfitsio_sys::CFile::open(c_path.to_str().unwrap(), 0).unwrap();
    let (cd, ch) = c.get_chksum().unwrap();
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_image(ImageType::U8, &naxes).unwrap();
    f.write_image(1, &data).unwrap();
    let (rd, rh) = f.get_chksum().unwrap();
    f.close().unwrap();
    assert_eq!(rd, cd, "datasum");
    assert_eq!(rh, ch, "hdusum");
}

#[test]
fn write_chksum_verifies_and_cfitsio_agrees() {
    let dir = tempfile::tempdir().unwrap();
    let r_path = dir.path().join("r.fits");
    let naxes = [10i64];
    let data: Vec<u8> = (1..=10).collect();

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_image(ImageType::U8, &naxes).unwrap();
    f.write_image(1, &data).unwrap();
    f.write_chksum_at("2026-08-25T00:00:00").unwrap();
    let (ds, hs) = f.verify_chksum().unwrap();
    assert_eq!(ds, 1);
    assert_eq!(hs, 1);
    let check = f.read_key_str("CHECKSUM").unwrap().0;
    let data_s = f.read_key_str("DATASUM").unwrap().0;
    f.close().unwrap();

    let mut c = cfitsio_sys::CFile::open(r_path.to_str().unwrap(), 0).unwrap();
    let (cds, chs) = c.verify_chksum().unwrap();
    assert_eq!(cds, 1, "cfitsio data verify of rfitsio file");
    assert_eq!(chs, 1, "cfitsio hdu verify of rfitsio file");
    assert_eq!(c.close(), 0);
    assert_eq!(check.len(), 16);
    assert!(!data_s.trim().is_empty());
}

#[test]
fn cfitsio_chksum_verifies_in_rfitsio() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let naxes = [10i64];
    let data: Vec<u8> = (1..=10).collect();

    cfitsio_sys::write_primary_image(c_path.to_str().unwrap(), BYTE_IMG, &naxes, TBYTE, &data)
        .unwrap();
    let mut c = cfitsio_sys::CFile::open(c_path.to_str().unwrap(), 1).unwrap();
    assert_eq!(c.write_chksum(), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::open(&c_path, AccessMode::ReadOnly).unwrap();
    let (ds, hs) = f.verify_chksum().unwrap();
    assert_eq!(ds, 1);
    assert_eq!(hs, 1);
}

#[test]
fn verify_without_keywords_is_zero() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.fits");
    let mut f = FitsFile::create(&path).unwrap();
    f.create_image(ImageType::U8, &[5]).unwrap();
    let (ds, hs) = f.verify_chksum().unwrap();
    assert_eq!(ds, 0);
    assert_eq!(hs, 0);
}

#[test]
fn gzip_round_trip_cross_read() {
    let dir = tempfile::tempdir().unwrap();
    let c_gz = dir.path().join("c.fits.gz");
    let r_gz = dir.path().join("r.fits.gz");
    let naxes = [8i64];
    let data: Vec<u8> = (0..8).map(|i| i * 3).collect();

    cfitsio_sys::write_primary_image(c_gz.to_str().unwrap(), BYTE_IMG, &naxes, TBYTE, &data)
        .unwrap();
    let raw = fs::read(&c_gz).unwrap();
    assert_eq!(&raw[..2], &[0x1f, 0x8b], "cfitsio wrote gzip");

    let mut f = FitsFile::open(&c_gz, AccessMode::ReadOnly).unwrap();
    let got: Vec<u8> = f.read_image(1, 8).unwrap();
    f.close().unwrap();
    assert_eq!(got, data);

    // Open by unsuffixed name (CFITSIO appends .gz).
    let unsuffixed = dir.path().join("c.fits");
    let mut f = FitsFile::open(&unsuffixed, AccessMode::ReadOnly).unwrap();
    let got: Vec<u8> = f.read_image(1, 8).unwrap();
    f.close().unwrap();
    assert_eq!(got, data);

    let mut w = FitsFile::create(&r_gz).unwrap();
    w.create_image(ImageType::U8, &naxes).unwrap();
    w.write_image(1, &data).unwrap();
    w.close().unwrap();
    let raw = fs::read(&r_gz).unwrap();
    assert_eq!(&raw[..2], &[0x1f, 0x8b], "rfitsio wrote gzip");

    let c = cfitsio_sys::CFile::open(r_gz.to_str().unwrap(), 0).unwrap();
    assert_eq!(c.close(), 0);
    let mut f = FitsFile::open(&r_gz, AccessMode::ReadOnly).unwrap();
    let got: Vec<u8> = f.read_image(1, 8).unwrap();
    f.close().unwrap();
    assert_eq!(got, data);
}

#[test]
fn from_bytes_and_write_hdu_to_stream() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.fits");
    let naxes = [10i64];
    let data: Vec<u8> = (1..=10).collect();
    {
        let mut f = FitsFile::create(&path).unwrap();
        f.create_image(ImageType::U8, &naxes).unwrap();
        f.write_image(1, &data).unwrap();
        f.close().unwrap();
    }
    let disk = fs::read(&path).unwrap();
    let mut mem = FitsFile::from_bytes(&disk).unwrap();
    let got: Vec<u8> = mem.read_image(1, 10).unwrap();
    assert_eq!(got, data);

    let mut hdu_bytes = Vec::new();
    mem.write_hdu_to(&mut hdu_bytes).unwrap();
    mem.close().unwrap();
    assert_eq!(hdu_bytes, disk);

    let mut rdr = FitsFile::open_reader(disk.as_slice()).unwrap();
    let got: Vec<u8> = rdr.read_image(1, 10).unwrap();
    rdr.close().unwrap();
    assert_eq!(got, data);
}

#[test]
fn gzip_from_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let gz = dir.path().join("t.fits.gz");
    let naxes = [4i64];
    let data: Vec<u8> = vec![9, 8, 7, 6];
    {
        let mut f = FitsFile::create(&gz).unwrap();
        f.create_image(ImageType::U8, &naxes).unwrap();
        f.write_image(1, &data).unwrap();
        f.close().unwrap();
    }
    let raw = fs::read(&gz).unwrap();
    let mut f = FitsFile::from_bytes(&raw).unwrap();
    let got: Vec<u8> = f.read_image(1, 4).unwrap();
    f.close().unwrap();
    assert_eq!(got, data);
}
