//! PSRFITS SUBINT scale/offset, packed search DATA, and disk-backed gzip.

use rfitsio::{
    AccessMode, CubeLayout, FitsFile, HduType, apply_scale, pack_samples, unpack_samples,
};

fn write_mini_search(path: &std::path::Path) -> (Vec<u8>, Vec<f32>, Vec<f32>, Vec<f64>) {
    let nchan = 4i64;
    let npol = 1i64;
    let nsblk = 8i64;
    let nbit = 2u32;
    let nsamp = (nchan * npol * nsblk) as usize;
    let nbytes = nsamp * nbit as usize / 8;
    let samples: Vec<u8> = (0..nsamp).map(|i| (i as u8) % 4).collect();
    let packed = pack_samples(&samples, nbit).unwrap();
    let scl = vec![0.5f32, 1.0, 1.5, 2.0];
    let offs = vec![10.0f32, 0.0, -1.0, 0.5];
    let layout = CubeLayout::Search {
        nchan: nchan as usize,
        npol: npol as usize,
        nsblk: nsblk as usize,
    };
    let raw_f: Vec<f64> = samples.iter().map(|&s| f64::from(s)).collect();
    let phys = apply_scale(&raw_f, &scl, &offs, layout, 0.0).unwrap();

    let tform_data = format!("{nbytes}B");
    let tform_scl = format!("{nchan}E");
    let mut f = FitsFile::create(path).unwrap();
    f.create_binary_table(
        1,
        &["DATA", "DAT_SCL", "DAT_OFFS"],
        &[&tform_data, &tform_scl, &tform_scl],
        &[Some("Jy"), None, None],
        Some("SUBINT"),
    )
    .unwrap();
    f.write_key_lng("NPOL", npol, Some("Nr of polarisations"))
        .unwrap();
    f.write_key_lng("NBIN", 0, Some("Nr of bins (SEARCH)"))
        .unwrap();
    f.write_key_lng("NCHAN", nchan, Some("Number of channels"))
        .unwrap();
    f.write_key_lng("NBITS", i64::from(nbit), Some("Nr of bits"))
        .unwrap();
    f.write_key_lng("NSBLK", nsblk, Some("Samples/row"))
        .unwrap();
    f.write_key_dbl("ZERO_OFF", 0.0, 6, Some("Zero offset"))
        .unwrap();
    f.write_key_lng("SIGNINT", 0, Some("unsigned packed"))
        .unwrap();
    let as_i64: Vec<i64> = packed.iter().map(|&b| i64::from(b)).collect();
    f.write_col_i64(1, 1, &as_i64).unwrap();
    f.write_col_f32(2, 1, &scl).unwrap();
    f.write_col_f32(3, 1, &offs).unwrap();
    f.close().unwrap();
    (packed, scl, offs, phys)
}

#[test]
fn search_subint_read_applies_scale() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("psr.fits");
    let (packed, _, _, phys) = write_mini_search(&path);

    let mut f = FitsFile::open(&path, AccessMode::ReadOnly).unwrap();
    assert_eq!(f.movnam_hdu(-1, "SUBINT", 0).unwrap(), HduType::BinaryTable);
    let info = f.read_subint_info().unwrap();
    assert_eq!(info.nchan, 4);
    assert_eq!(info.nbits, 2);
    assert_eq!(info.nsblk, 8);
    assert!(!info.signint);
    let got = f.read_subint_data(1).unwrap();
    assert_eq!(got.len(), phys.len());
    for (a, b) in got.iter().zip(phys.iter()) {
        assert!((a - b).abs() < 1e-6, "{a} vs {b}");
    }
    let (raw, _) = f.read_col_i64(1, 1, packed.len(), None).unwrap();
    let bytes: Vec<u8> = raw.iter().map(|&v| v as u8).collect();
    assert_eq!(bytes, packed);
    assert_eq!(unpack_samples(&bytes, 2, 32).unwrap().len(), 32);
    f.close().unwrap();
}

#[test]
fn search_subint_gzip_scratch_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("psr.fits.gz");
    let (_, _, _, phys) = write_mini_search(&path);
    let raw = std::fs::read(&path).unwrap();
    assert_eq!(&raw[..2], &[0x1f, 0x8b]);

    let mut f = FitsFile::open(&path, AccessMode::ReadOnly).unwrap();
    f.movnam_hdu(-1, "SUBINT", 0).unwrap();
    let got = f.read_subint_data(1).unwrap();
    f.close().unwrap();
    for (a, b) in got.iter().zip(phys.iter()) {
        assert!((a - b).abs() < 1e-6);
    }

    let unsuffixed = dir.path().join("psr.fits");
    let mut f = FitsFile::open(&unsuffixed, AccessMode::ReadOnly).unwrap();
    f.movnam_hdu(-1, "SUBINT", 0).unwrap();
    let got = f.read_subint_data(1).unwrap();
    f.close().unwrap();
    assert_eq!(got.len(), phys.len());
}

#[test]
fn fold_subint_i16_scale() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fold.fits");
    let nbin = 4i64;
    let nchan = 2i64;
    let npol = 1i64;
    let nsamp = (nbin * nchan * npol) as usize;
    let data: Vec<i64> = (0..nsamp as i64).collect();
    let scl = vec![2.0f32, 0.5];
    let offs = vec![1.0f32, -4.0];
    let mut f = FitsFile::create(&path).unwrap();
    f.create_binary_table(
        1,
        &["DATA", "DAT_SCL", "DAT_OFFS"],
        &[&format!("{nsamp}I"), "2E", "2E"],
        &[Some("Jy"), None, None],
        Some("SUBINT"),
    )
    .unwrap();
    f.write_key_lng("NPOL", npol, None).unwrap();
    f.write_key_lng("NBIN", nbin, None).unwrap();
    f.write_key_lng("NCHAN", nchan, None).unwrap();
    f.write_key_lng("NBITS", 16, None).unwrap();
    f.write_key_lng("NSBLK", 1, None).unwrap();
    f.write_tdim(1, &[nbin, nchan, npol]).unwrap();
    f.write_col_i64(1, 1, &data).unwrap();
    f.write_col_f32(2, 1, &scl).unwrap();
    f.write_col_f32(3, 1, &offs).unwrap();
    f.close().unwrap();

    let mut f = FitsFile::open(&path, AccessMode::ReadOnly).unwrap();
    f.movabs_hdu(2).unwrap();
    let got = f.read_subint_data(1).unwrap();
    let layout = CubeLayout::Fold {
        nbin: nbin as usize,
        nchan: nchan as usize,
        npol: npol as usize,
    };
    let raw: Vec<f64> = data.iter().map(|&v| v as f64).collect();
    let expect = apply_scale(&raw, &scl, &offs, layout, 0.0).unwrap();
    assert_eq!(got, expect);
    f.close().unwrap();

    let mut c = cfitsio_sys::CFile::open(path.to_str().unwrap(), 0).unwrap();
    assert_eq!(c.movabs_hdu(2), 0);
    assert_eq!(c.close(), 0);
}
