//! Twin-write empty primary HDU against CFITSIO `ffinit` + `ffphps` + `ffclos`.

use std::fs;

use rfitsio::status::FILE_NOT_CREATED;
use rfitsio::types::RECORD_LEN;
use rfitsio::{AccessMode, FitsFile, fits_create_file, fits_open_file};

#[test]
fn empty_primary_byte_identical_to_cfitsio() {
    let dir = tempfile::tempdir().unwrap();
    let c_path = dir.path().join("c.fits");
    let r_path = dir.path().join("r.fits");

    cfitsio_sys::write_empty_primary(c_path.to_str().unwrap()).expect("cfitsio write");
    {
        let f = FitsFile::create(&r_path).expect("rfitsio create");
        f.close().expect("rfitsio close");
    }

    let c_bytes = fs::read(&c_path).unwrap();
    let r_bytes = fs::read(&r_path).unwrap();
    assert_eq!(
        c_bytes.len(),
        RECORD_LEN,
        "cfitsio empty primary is one record"
    );
    assert_eq!(
        r_bytes,
        c_bytes,
        "empty primary bytes differ\n cfitsio cards:\n{}\n rfitsio cards:\n{}",
        dump_cards(&c_bytes),
        dump_cards(&r_bytes)
    );
}

#[test]
fn memory_matches_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("d.fits");
    {
        let f = FitsFile::create(&path).unwrap();
        f.close().unwrap();
    }
    let disk = fs::read(&path).unwrap();
    let mut mem = FitsFile::create_memory().unwrap();
    let mem_bytes = mem.to_bytes().unwrap();
    mem.close().unwrap();
    assert_eq!(disk, mem_bytes);
}

#[test]
fn open_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rt.fits");
    {
        let f = FitsFile::create(&path).unwrap();
        f.close().unwrap();
    }
    let original = fs::read(&path).unwrap();
    let mut f = fits_open_file(path.to_str().unwrap(), AccessMode::ReadOnly).unwrap();
    assert_eq!(f.hdu_count().unwrap(), 1);
    assert_eq!(f.header().unwrap().len(), 6);
    let bytes = f.to_bytes().unwrap();
    f.close().unwrap();
    assert_eq!(bytes, original);
}

#[test]
fn create_refuses_existing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("once.fits");
    FitsFile::create(&path).unwrap().close().unwrap();
    let err = FitsFile::create(&path).unwrap_err();
    assert_eq!(err.status, FILE_NOT_CREATED);
}

#[test]
fn clobber_bang_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clobber.fits");
    FitsFile::create(&path).unwrap().close().unwrap();
    let bang = format!("!{}", path.to_str().unwrap());
    fits_create_file(&bang).unwrap().close().unwrap();
    assert!(path.exists());
}

#[test]
fn named_api_create() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("named.fits");
    let f = fits_create_file(path.to_str().unwrap()).unwrap();
    f.close().unwrap();
    assert_eq!(fs::metadata(&path).unwrap().len(), RECORD_LEN as u64);
}

fn dump_cards(bytes: &[u8]) -> String {
    bytes
        .chunks(80)
        .take(10)
        .map(|c| String::from_utf8_lossy(c).trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}
