//! Vector-column, TDIM, and PSRFITS-style packed-bit oracle tests.

use rfitsio::{
    AccessMode, FitsFile, HduType, pack_samples, tdim_coords, tdim_elem, unpack_samples,
};

fn dump_cards(bytes: &[u8]) -> String {
    bytes
        .chunks(80)
        .take(36)
        .map(|c| String::from_utf8_lossy(c).trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn vector_float_column_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let ttype = ["DAT_FREQ"];
    let tform = ["4E"];
    let tunit = [Some("MHz")];
    let mut data = [100.0f32, 101.0, 102.0, 103.0];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            1,
            &ttype,
            &tform,
            &tunit,
            Some("SUBINT")
        ),
        0
    );
    assert_eq!(c.write_col_f32(1, 1, &mut data), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(1, &ttype, &tform, &tunit, Some("SUBINT"))
        .unwrap();
    f.write_col_f32(1, 1, &data).unwrap();
    f.close().unwrap();

    let c_bytes = std::fs::read(&c_path).unwrap();
    let r_bytes = std::fs::read(&r_path).unwrap();
    assert_eq!(
        r_bytes,
        c_bytes,
        "vector col bytes differ\n cfitsio:\n{}\n rfitsio:\n{}",
        dump_cards(&c_bytes),
        dump_cards(&r_bytes)
    );
}

#[test]
fn vector_firstelem_cross_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v.fits");
    let data: Vec<i32> = (0..8).collect();
    let mut c = cfitsio_sys::CFile::create_empty(path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            2,
            &["DATA"],
            &["4J"],
            &[None],
            Some("SUBINT")
        ),
        0
    );
    let row1: Vec<i64> = data[..4].iter().map(|&v| i64::from(v)).collect();
    let row2: Vec<i64> = data[4..].iter().map(|&v| i64::from(v)).collect();
    assert_eq!(c.write_col_i64(1, 1, &row1), 0);
    assert_eq!(c.write_col_i64(1, 2, &row2), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::open(&path, AccessMode::ReadOnly).unwrap();
    assert_eq!(f.movabs_hdu(2).unwrap(), HduType::BinaryTable);
    let (all, _) = f.read_col_i64(1, 1, 8, None).unwrap();
    assert_eq!(all, [0, 1, 2, 3, 4, 5, 6, 7]);
    let (mid, _) = f.read_col_i64_elem(1, 1, 3, 4, None).unwrap();
    assert_eq!(mid, [2, 3, 4, 5]);
    f.close().unwrap();
}

#[test]
fn tdim_write_matches_cfitsio() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");
    let dims = [2i64, 3];

    let mut c = cfitsio_sys::CFile::create_empty(c_path.to_str().unwrap()).unwrap();
    assert_eq!(
        c.create_tbl(
            cfitsio_sys::BINARY_TBL,
            1,
            &["DATA"],
            &["6I"],
            &[None],
            Some("SUBINT")
        ),
        0
    );
    assert_eq!(c.write_tdim(1, &dims), 0);
    assert_eq!(c.close(), 0);

    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(1, &["DATA"], &["6I"], &[None], Some("SUBINT"))
        .unwrap();
    f.write_tdim(1, &dims).unwrap();
    f.close().unwrap();

    let c_bytes = std::fs::read(&c_path).unwrap();
    let r_bytes = std::fs::read(&r_path).unwrap();
    assert_eq!(
        r_bytes,
        c_bytes,
        "TDIM cards differ\n cfitsio:\n{}\n rfitsio:\n{}",
        dump_cards(&c_bytes),
        dump_cards(&r_bytes)
    );

    let mut f = FitsFile::open(&c_path, AccessMode::ReadOnly).unwrap();
    f.movabs_hdu(2).unwrap();
    assert_eq!(f.read_tdim(1).unwrap(), dims);
    assert_eq!(tdim_elem(&dims, &[2, 1]).unwrap(), 2);
    assert_eq!(tdim_coords(&dims, 3).unwrap(), vec![1, 2]);
    f.close().unwrap();
}

#[test]
fn tdim_default_is_repeat() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.fits");
    let mut f = FitsFile::create(&path).unwrap();
    f.create_binary_table(1, &["X"], &["8E"], &[None], Some("T"))
        .unwrap();
    assert_eq!(f.read_tdim(1).unwrap(), vec![8]);
    f.close().unwrap();
}

#[test]
fn search_mode_data_cell_with_tdim_and_nbit() {
    // Mini search-mode SUBINT: nchan=4, npol=1, nsblk=8, nbit=2 → 8 bytes/row.
    let nchan = 4i64;
    let npol = 1i64;
    let nsblk = 8i64;
    let nbit = 2u32;
    let nsamp = (nchan * npol * nsblk) as usize;
    // Packed byte count = nsamp * nbit / 8 = 8. TFORM/TDIM product is bytes.
    let nbytes = nsamp * nbit as usize / 8;
    assert_eq!(nbytes, 8);
    let byte_dims = [nbytes as i64];

    let samples: Vec<u8> = (0..nsamp).map(|i| (i as u8) % 4).collect();
    let packed = pack_samples(&samples, nbit).unwrap();
    assert_eq!(packed.len(), nbytes);
    assert_eq!(unpack_samples(&packed, nbit, nsamp).unwrap(), samples);

    let dir = tempfile::tempdir().unwrap();
    let r_path = dir.path().join("psr.fits");
    let tform = format!("{nbytes}B");
    let mut f = FitsFile::create(&r_path).unwrap();
    f.create_binary_table(1, &["DATA"], &[&tform], &[Some("Jy")], Some("SUBINT"))
        .unwrap();
    f.write_tdim(1, &byte_dims).unwrap();
    let as_i64: Vec<i64> = packed.iter().map(|&b| i64::from(b)).collect();
    f.write_col_i64(1, 1, &as_i64).unwrap();
    f.close().unwrap();

    let mut f = FitsFile::open(&r_path, AccessMode::ReadOnly).unwrap();
    f.movabs_hdu(2).unwrap();
    assert_eq!(f.read_tdim(1).unwrap(), byte_dims);
    let (raw, _) = f.read_col_i64(1, 1, nbytes, None).unwrap();
    let bytes: Vec<u8> = raw.iter().map(|&v| v as u8).collect();
    assert_eq!(unpack_samples(&bytes, nbit, nsamp).unwrap(), samples);

    // Index sample (chan=2, pol=1, t=1) via TDIM on the *sample* cube
    // (nchan, npol, nsblk), then pack offset.
    let samp_dims = [nchan, npol, nsblk];
    let elem = tdim_elem(&samp_dims, &[2, 1, 1]).unwrap();
    assert_eq!(elem, 2);
    assert_eq!(samples[elem as usize - 1], 1);

    let mut c = cfitsio_sys::CFile::open(r_path.to_str().unwrap(), 0).unwrap();
    assert_eq!(c.movabs_hdu(2), 0);
    let (naxis, cdims) = c.read_tdim(1, 8).unwrap();
    assert_eq!(naxis, 1);
    assert_eq!(cdims, byte_dims);
    assert_eq!(c.close(), 0);
    f.close().unwrap();
}
