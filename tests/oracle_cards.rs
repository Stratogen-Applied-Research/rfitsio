//! Differential tests: card format/parse/classify vs CFITSIO.

use rfitsio::card::{
    fits_get_keyclass, fits_get_keyname, fits_make_key, fits_parse_value, fits_test_keyword,
    make_card_string, test_keyword,
};
use rfitsio::status::BAD_KEYCHAR;

fn assert_make(name: &str, value: &str, comm: Option<&str>) {
    let (c_card, c_status) = cfitsio_sys::ffmkky_str(name, value, comm);
    let rust = make_card_string(name, value, comm);
    match rust {
        Ok(r_card) => {
            assert_eq!(
                c_status, 0,
                "CFITSIO rejected {name:?} {value:?} {comm:?}: {c_card}"
            );
            assert_eq!(
                r_card, c_card,
                "ffmkky mismatch name={name:?} value={value:?} comm={comm:?}"
            );
        }
        Err(e) => {
            assert_eq!(
                e.status, c_status,
                "status mismatch name={name:?} value={value:?} rust={} cfitsio={c_status} card={c_card}",
                e.status
            );
        }
    }
}

#[test]
fn make_key_matrix() {
    let long = "x".repeat(80);
    let comments: [Option<&str>; 4] = [None, Some(""), Some("a comment"), Some(&long)];

    for comm in comments {
        assert_make("BITPIX", "16", comm);
        assert_make("SIMPLE", "T", comm);
        assert_make("NAXIS", "2", comm);
        assert_make("NAXIS1", "100", comm);
        assert_make("OBJECT", "'NGC3372'", comm);
        assert_make("OBJECT", "'IT''S'", comm);
        assert_make("HISTORY", "", comm);
        assert_make("COMMENT", "", comm);
        assert_make("END", "", comm);
        assert_make("HIERARCH ESO DET DIT", "5.0", comm);
        assert_make("ESO DET DIT", "5.0", comm);
        assert_make("EXPTIME", "1.5", comm);
        assert_make("BLANK", "-32768", comm);
        assert_make("TTYPE1", "'ENERGY'", comm);
        assert_make("CONTINUE", "'abc&'", comm);
        assert_make("AZ", "T", comm);
        assert_make("A", "1", comm);
        assert_make("WAVELENG", "5000.0", comm);
    }
}

#[test]
fn make_key_rejects_equals_in_name() {
    let err = make_card_string("BAD=KEY", "1", None).unwrap_err();
    assert_eq!(err.status, BAD_KEYCHAR);
    let (_, c_status) = cfitsio_sys::ffmkky_str("BAD=KEY", "1", None);
    assert_eq!(c_status, BAD_KEYCHAR);
}

#[test]
fn parse_value_matrix() {
    let cards = [
        "BITPIX  =                   16 / number of bits per data pixel",
        "SIMPLE  =                    T / file does conform to FITS standard",
        "OBJECT  = 'NGC3372'           / target",
        "OBJECT  = 'IT''S'             / apostrophe",
        "HISTORY processed by pipeline",
        "COMMENT a comment",
        "END",
        "CONTINUE  'long string&'        ",
        "HIERARCH ESO DET DIT = 5.0 / sec",
        "EXPTIME =                  1.5",
        "EMPTY   =                      / undefined",
        "COMPLEX = (1.0, 2.0)          / pair",
    ];
    for card in cards {
        let (cv, cc, cs) = cfitsio_sys::ffpsvc_str(card);
        let rust = fits_parse_value(card);
        match rust {
            Ok((rv, rc)) => {
                assert_eq!(cs, 0, "CFITSIO parse failed for {card:?}: {cv:?} {cc:?}");
                assert_eq!(rv, cv, "value mismatch for {card:?}");
                assert_eq!(rc, cc, "comment mismatch for {card:?}");
            }
            Err(e) => {
                assert_eq!(e.status, cs, "status mismatch for {card:?}");
            }
        }
    }
}

#[test]
fn keyclass_matrix() {
    let cards = [
        "SIMPLE  =                    T",
        "BITPIX  =                   16",
        "NAXIS   =                    2",
        "NAXIS1  =                  100",
        "NAXIS2  =                  100",
        "EXTEND  =                    T",
        "XTENSION= 'IMAGE   '",
        "PCOUNT  =                    0",
        "GCOUNT  =                    1",
        "TFIELDS =                    3",
        "TTYPE1  = 'ENERGY  '",
        "TFORM1  = '1D      '",
        "TBCOL1  =                    1",
        "THEAP   =                 2880",
        "GROUPS  =                    T",
        "BSCALE  =                  1.0",
        "BZERO   =                    0",
        "TSCAL1  =                  1.0",
        "TZERO1  =                    0",
        "BLANK   =               -32768",
        "TNULL1  =                   -1",
        "TDIM1   = '(2,2)   '",
        "TLMIN1  =                    0",
        "TLMAX1  =                 4095",
        "TDMIN1  =                    0",
        "TDMAX1  =                 4095",
        "DATAMIN =                    0",
        "DATAMAX =                 255",
        "BUNIT   = 'adu     '",
        "TUNIT1  = 'keV     '",
        "TDISP1  = 'F8.3    '",
        "EXTNAME = 'EVENTS  '",
        "EXTVER  =                    1",
        "EXTLEVEL=                    1",
        "HDUNAME = 'PRIMARY '",
        "HDUVER  =                    1",
        "HDULEVEL=                    1",
        "CHECKSUM= '0000000000000000'",
        "DATASUM = '0       '",
        "CTYPE1  = 'RA---TAN'",
        "CUNIT1  = 'deg     '",
        "CRVAL1  =                  0.0",
        "CRPIX1  =                  1.0",
        "CDELT1  =             0.001",
        "CROTA1  =                  0.0",
        "EQUINOX =               2000.0",
        "EPOCH   =               2000.0",
        "MJD-OBS =              58000.0",
        "RADECSYS= 'FK5     '",
        "HISTORY processed",
        "COMMENT a comment",
        "CONTINUE  'abc&'",
        "        a blank-name comment",
        "EXPTIME =                  1.5",
        "ZIMAGE  =                    T",
        "ZCMPTYPE= 'RICE_1  '",
        "ZBITPIX =                   16",
        "ZNAXIS  =                    2",
        "ZNAXIS1 =                  100",
        "END",
        "EXTNAME = 'COMPRESSED_IMAGE'",
        "COMMENT   FITS (Flexible Image Transport System) format is defined in 'Astronomy",
    ];
    for card in cards {
        let c = cfitsio_sys::ffgkcl_i32(card);
        let r = fits_get_keyclass(card).code();
        assert_eq!(r, c, "ffgkcl mismatch for {card:?}");
    }
}

#[test]
fn test_keyword_matrix() {
    for name in ["BITPIX", "NAXIS1", "TTYPE12", "A", "GOOD_KEY", "A-B"] {
        assert!(test_keyword(name).is_ok(), "{name}");
        assert_eq!(cfitsio_sys::fftkey_status(name, 0), 0, "{name}");
    }
    for name in ["bad key", "lowercase", "EQ=UAL", "HAS.DOT", "TAB\tKEY"] {
        let rust = fits_test_keyword(name);
        let c = cfitsio_sys::fftkey_status(name, 0);
        match rust {
            Ok(()) => panic!("expected rust to reject {name:?}, cfitsio={c}"),
            Err(e) => assert_eq!(e.status, c, "{name:?}"),
        }
    }
}

#[test]
fn keyname_extraction() {
    for card in [
        "BITPIX  =                   16",
        "HIERARCH ESO DET DIT = 5.0 / sec",
        "NAXIS1  =                  100",
        "CONTINUE  'abc&'",
    ] {
        let (cn, cl, cs) = cfitsio_sys::ffgknm_str(card);
        let (rn, rl) = fits_get_keyname(card).unwrap();
        assert_eq!(cs, 0, "{card:?}");
        assert_eq!(rn, cn, "{card:?}");
        assert_eq!(rl as i32, cl, "{card:?}");
    }
}

#[test]
fn fits_make_key_wrapper_matches() {
    let s = fits_make_key("OBJECT", "'M31'", Some("galaxy")).unwrap();
    let (c, st) = cfitsio_sys::ffmkky_str("OBJECT", "'M31'", Some("galaxy"));
    assert_eq!(st, 0);
    assert_eq!(s, c);
}
