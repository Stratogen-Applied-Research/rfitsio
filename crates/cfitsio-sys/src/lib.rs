//! Thin, test-only FFI to vendored CFITSIO 4.7.0.
//!
//! Only the helpers needed to lock rfitsio against the C oracle are bound
//! here. Application code must not depend on this crate.

#![allow(non_camel_case_types)]

use std::ffi::{CStr, CString, c_char, c_int, c_long, c_longlong, c_void};
use std::sync::Mutex;

/// CFITSIO process-global state (I/O drivers, error stack) is not reentrant
/// unless built with `_REENTRANT`. Serialise all oracle calls.
static CFITSIO_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    CFITSIO_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub const FLEN_CARD: usize = 81;
pub const FLEN_VALUE: usize = 71;
pub const FLEN_COMMENT: usize = 73;
pub const FLEN_KEYWORD: usize = 75;
pub const FLEN_ERRMSG: usize = 81;
pub const FLEN_STATUS: usize = 31;

unsafe extern "C" {
    fn ffgerr(status: c_int, errtext: *mut c_char);
    fn ffmkky(
        keyname: *const c_char,
        value: *mut c_char,
        comm: *const c_char,
        card: *mut c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffpsvc(
        card: *mut c_char,
        value: *mut c_char,
        comm: *mut c_char,
        status: *mut c_int,
    ) -> c_int;
    fn fftkey(keyword: *const c_char, status: *mut c_int) -> c_int;
    fn fftrec(card: *mut c_char, status: *mut c_int) -> c_int;
    fn ffgkcl(tcard: *mut c_char) -> c_int;
    fn ffvers(version: *mut f32) -> f32;
    fn ffgknm(
        card: *mut c_char,
        name: *mut c_char,
        length: *mut c_int,
        status: *mut c_int,
    ) -> c_int;
    fn ffinit(fptr: *mut *mut c_void, filename: *const c_char, status: *mut c_int) -> c_int;
    fn ffclos(fptr: *mut c_void, status: *mut c_int) -> c_int;
    fn ffphps(
        fptr: *mut c_void,
        bitpix: c_int,
        naxis: c_int,
        naxes: *mut c_long,
        status: *mut c_int,
    ) -> c_int;
    fn ffopen(
        fptr: *mut *mut c_void,
        filename: *const c_char,
        iomode: c_int,
        status: *mut c_int,
    ) -> c_int;
    fn ffppr(
        fptr: *mut c_void,
        datatype: c_int,
        firstelem: c_longlong,
        nelem: c_longlong,
        array: *mut c_void,
        status: *mut c_int,
    ) -> c_int;
    fn ffd2e(dval: f64, decim: c_int, cval: *mut c_char, status: *mut c_int) -> c_int;
    fn ffd2f(dval: f64, decim: c_int, cval: *mut c_char, status: *mut c_int) -> c_int;
    fn ffpkys(
        fptr: *mut c_void,
        keyname: *const c_char,
        value: *const c_char,
        comm: *const c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffpkyj(
        fptr: *mut c_void,
        keyname: *const c_char,
        value: c_longlong,
        comm: *const c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffpkyl(
        fptr: *mut c_void,
        keyname: *const c_char,
        value: c_int,
        comm: *const c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffpkye(
        fptr: *mut c_void,
        keyname: *const c_char,
        value: f32,
        decim: c_int,
        comm: *const c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffpkyd(
        fptr: *mut c_void,
        keyname: *const c_char,
        value: f64,
        decim: c_int,
        comm: *const c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffpkyg(
        fptr: *mut c_void,
        keyname: *const c_char,
        value: f64,
        decim: c_int,
        comm: *const c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffpkyu(
        fptr: *mut c_void,
        keyname: *const c_char,
        comm: *const c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffpcom(fptr: *mut c_void, comm: *const c_char, status: *mut c_int) -> c_int;
    fn ffphis(fptr: *mut c_void, history: *const c_char, status: *mut c_int) -> c_int;
    fn ffukys(
        fptr: *mut c_void,
        keyname: *const c_char,
        value: *const c_char,
        comm: *const c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffmkys(
        fptr: *mut c_void,
        keyname: *const c_char,
        value: *const c_char,
        comm: *const c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffdkey(fptr: *mut c_void, keyname: *const c_char, status: *mut c_int) -> c_int;
    fn ffpunt(
        fptr: *mut c_void,
        keyname: *const c_char,
        unit: *const c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffgpv(
        fptr: *mut c_void,
        datatype: c_int,
        firstelem: c_longlong,
        nelem: c_longlong,
        nulval: *mut c_void,
        array: *mut c_void,
        anynul: *mut c_int,
        status: *mut c_int,
    ) -> c_int;
    fn ffcrtb(
        fptr: *mut c_void,
        tbltype: c_int,
        naxis2: c_longlong,
        tfields: c_int,
        ttype: *mut *mut c_char,
        tform: *mut *mut c_char,
        tunit: *mut *mut c_char,
        extnm: *const c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffpcls(
        fptr: *mut c_void,
        colnum: c_int,
        firstrow: c_longlong,
        firstelem: c_longlong,
        nelem: c_longlong,
        array: *mut *mut c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffpcljj(
        fptr: *mut c_void,
        colnum: c_int,
        firstrow: c_longlong,
        firstelem: c_longlong,
        nelem: c_longlong,
        array: *mut c_longlong,
        status: *mut c_int,
    ) -> c_int;
    fn ffpcld(
        fptr: *mut c_void,
        colnum: c_int,
        firstrow: c_longlong,
        firstelem: c_longlong,
        nelem: c_longlong,
        array: *mut f64,
        status: *mut c_int,
    ) -> c_int;
    fn ffpcle(
        fptr: *mut c_void,
        colnum: c_int,
        firstrow: c_longlong,
        firstelem: c_longlong,
        nelem: c_longlong,
        array: *mut f32,
        status: *mut c_int,
    ) -> c_int;
    fn ffgcvs(
        fptr: *mut c_void,
        colnum: c_int,
        firstrow: c_longlong,
        firstelem: c_longlong,
        nelem: c_longlong,
        nulval: *mut c_char,
        array: *mut *mut c_char,
        anynul: *mut c_int,
        status: *mut c_int,
    ) -> c_int;
    fn ffgcvjj(
        fptr: *mut c_void,
        colnum: c_int,
        firstrow: c_longlong,
        firstelem: c_longlong,
        nelem: c_longlong,
        nulval: c_longlong,
        array: *mut c_longlong,
        anynul: *mut c_int,
        status: *mut c_int,
    ) -> c_int;
    fn ffgcvd(
        fptr: *mut c_void,
        colnum: c_int,
        firstrow: c_longlong,
        firstelem: c_longlong,
        nelem: c_longlong,
        nulval: f64,
        array: *mut f64,
        anynul: *mut c_int,
        status: *mut c_int,
    ) -> c_int;
    fn ffirow(
        fptr: *mut c_void,
        firstrow: c_longlong,
        nrows: c_longlong,
        status: *mut c_int,
    ) -> c_int;
    fn ffdrow(
        fptr: *mut c_void,
        firstrow: c_longlong,
        nrows: c_longlong,
        status: *mut c_int,
    ) -> c_int;
    fn fficol(
        fptr: *mut c_void,
        numcol: c_int,
        ttype: *mut c_char,
        tform: *mut c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffdcol(fptr: *mut c_void, colnum: c_int, status: *mut c_int) -> c_int;
    fn ffpclu(
        fptr: *mut c_void,
        colnum: c_int,
        firstrow: c_longlong,
        firstelem: c_longlong,
        nelem: c_longlong,
        status: *mut c_int,
    ) -> c_int;
    fn ffmahd(fptr: *mut c_void, hdunum: c_int, exttype: *mut c_int, status: *mut c_int) -> c_int;
    fn ffasfm(
        tform: *mut c_char,
        dtcode: *mut c_int,
        twidth: *mut c_long,
        decimals: *mut c_int,
        status: *mut c_int,
    ) -> c_int;
    fn ffgabc(
        tfields: c_int,
        tform: *mut *mut c_char,
        space: c_int,
        rowlen: *mut c_long,
        tbcol: *mut c_long,
        status: *mut c_int,
    ) -> c_int;
    fn ffbnfm(
        tform: *mut c_char,
        dtcode: *mut c_int,
        trepeat: *mut c_long,
        twidth: *mut c_long,
        status: *mut c_int,
    ) -> c_int;
    fn ffpcll(
        fptr: *mut c_void,
        colnum: c_int,
        firstrow: c_longlong,
        firstelem: c_longlong,
        nelem: c_longlong,
        array: *mut c_char,
        status: *mut c_int,
    ) -> c_int;
    fn ffpclx(
        fptr: *mut c_void,
        colnum: c_int,
        frow: c_longlong,
        fbit: c_long,
        nbit: c_long,
        larray: *mut c_char,
        status: *mut c_int,
    ) -> c_int;
}

pub const ASCII_TBL: c_int = 1;
pub const BINARY_TBL: c_int = 2;

fn cstr_to_string(buf: &[u8]) -> String {
    let cstr = CStr::from_bytes_until_nul(buf).unwrap_or(c"unknown error status");
    cstr.to_string_lossy().into_owned()
}

/// `fits_get_errstatus` / `ffgerr`.
pub fn ffgerr_str(status: i32) -> String {
    let _g = lock();
    let mut buf = [0u8; FLEN_ERRMSG];
    unsafe {
        ffgerr(status as c_int, buf.as_mut_ptr().cast::<c_char>());
    }
    cstr_to_string(&buf)
}

/// `fits_get_version` / `ffvers`.
pub fn ffvers_f32() -> f32 {
    let _g = lock();
    let mut v = 0.0f32;
    unsafe { ffvers(&raw mut v) }
}

/// `fits_make_key` / `ffmkky`. Returns `(card_without_trailing_blanks, status)`.
pub fn ffmkky_str(keyname: &str, value: &str, comm: Option<&str>) -> (String, i32) {
    let _g = lock();
    let key = CString::new(keyname).expect("keyname contains NUL");
    let mut val = CString::new(value)
        .expect("value contains NUL")
        .into_bytes_with_nul();
    let comm_c = comm.map(|c| CString::new(c).expect("comment contains NUL"));
    let comm_ptr = comm_c
        .as_ref()
        .map(|c| c.as_ptr())
        .unwrap_or(std::ptr::null());
    let mut card = [0u8; FLEN_CARD];
    let mut status: c_int = 0;
    unsafe {
        ffmkky(
            key.as_ptr(),
            val.as_mut_ptr().cast::<c_char>(),
            comm_ptr,
            card.as_mut_ptr().cast::<c_char>(),
            &raw mut status,
        );
    }
    (cstr_to_string(&card), status as i32)
}

/// `fits_parse_value` / `ffpsvc`.
pub fn ffpsvc_str(card: &str) -> (String, String, i32) {
    let _g = lock();
    let mut card_buf = {
        let mut v = CString::new(card)
            .expect("card contains NUL")
            .into_bytes_with_nul();
        v.resize(FLEN_CARD, 0);
        v
    };
    let mut value = [0u8; FLEN_VALUE];
    let mut comm = [0u8; FLEN_COMMENT];
    let mut status: c_int = 0;
    unsafe {
        ffpsvc(
            card_buf.as_mut_ptr().cast::<c_char>(),
            value.as_mut_ptr().cast::<c_char>(),
            comm.as_mut_ptr().cast::<c_char>(),
            &raw mut status,
        );
    }
    (cstr_to_string(&value), cstr_to_string(&comm), status as i32)
}

/// `fits_test_keyword` / `fftkey`. `initial_status` is the inbound `*status`.
pub fn fftkey_status(keyword: &str, initial_status: i32) -> i32 {
    let _g = lock();
    let key = CString::new(keyword).expect("keyword contains NUL");
    let mut status = initial_status as c_int;
    unsafe {
        fftkey(key.as_ptr(), &raw mut status);
    }
    status as i32
}

/// `fits_test_record` / `fftrec`.
pub fn fftrec_status(card: &str, initial_status: i32) -> i32 {
    let _g = lock();
    let mut buf = {
        let mut v = CString::new(card)
            .expect("card contains NUL")
            .into_bytes_with_nul();
        v.resize(FLEN_CARD, 0);
        v
    };
    let mut status = initial_status as c_int;
    unsafe {
        fftrec(buf.as_mut_ptr().cast::<c_char>(), &raw mut status);
    }
    status as i32
}

/// `fits_get_keyclass` / `ffgkcl`.
pub fn ffgkcl_i32(card: &str) -> i32 {
    let _g = lock();
    let mut buf = {
        let mut v = CString::new(card)
            .expect("card contains NUL")
            .into_bytes_with_nul();
        v.resize(FLEN_CARD, 0);
        v
    };
    unsafe { ffgkcl(buf.as_mut_ptr().cast::<c_char>()) as i32 }
}

/// `fits_get_keyname` / `ffgknm`.
pub fn ffgknm_str(card: &str) -> (String, i32, i32) {
    let _g = lock();
    let mut buf = {
        let mut v = CString::new(card)
            .expect("card contains NUL")
            .into_bytes_with_nul();
        v.resize(FLEN_CARD, 0);
        v
    };
    let mut name = [0u8; FLEN_KEYWORD];
    let mut length: c_int = 0;
    let mut status: c_int = 0;
    unsafe {
        ffgknm(
            buf.as_mut_ptr().cast::<c_char>(),
            name.as_mut_ptr().cast::<c_char>(),
            &raw mut length,
            &raw mut status,
        );
    }
    (cstr_to_string(&name), length as i32, status as i32)
}

/// `fits_create_file` + `fits_write_imghdr`(BITPIX=8, NAXIS=0) + `fits_close_file`.
///
/// This is CFITSIO's standard empty primary array: SIMPLE/BITPIX/NAXIS/EXTEND
/// plus the two FITS-definition COMMENT cards, END, and space fill to 2880.
pub fn write_empty_primary(path: &str) -> Result<(), i32> {
    let _g = lock();
    let cpath = CString::new(path).map_err(|_| 105)?;
    let mut fptr: *mut c_void = std::ptr::null_mut();
    let mut status: c_int = 0;
    unsafe {
        ffinit(&raw mut fptr, cpath.as_ptr(), &raw mut status);
        if status != 0 {
            return Err(status as i32);
        }
        ffphps(fptr, 8, 0, std::ptr::null_mut(), &raw mut status);
        let mut close_status = status;
        ffclos(fptr, &raw mut close_status);
        if status != 0 {
            return Err(status as i32);
        }
        if close_status != 0 {
            return Err(close_status as i32);
        }
    }
    Ok(())
}

/// `fits_create_file` + `fits_close_file` with no keywords written.
pub fn create_and_close(path: &str) -> Result<(), i32> {
    let _g = lock();
    let cpath = CString::new(path).map_err(|_| 105)?;
    let mut fptr: *mut c_void = std::ptr::null_mut();
    let mut status: c_int = 0;
    unsafe {
        ffinit(&raw mut fptr, cpath.as_ptr(), &raw mut status);
        if status != 0 {
            return Err(status as i32);
        }
        ffclos(fptr, &raw mut status);
    }
    if status != 0 {
        Err(status as i32)
    } else {
        Ok(())
    }
}

/// `ffinit` + `ffphps` + `ffppr` + `ffclos`.
pub fn write_primary_image<T>(
    path: &str,
    bitpix: i32,
    naxes: &[i64],
    datatype: i32,
    data: &[T],
) -> Result<(), i32> {
    let _g = lock();
    let cpath = CString::new(path).map_err(|_| 105)?;
    let mut axes: Vec<c_long> = naxes.iter().map(|&n| n as c_long).collect();
    let mut fptr: *mut c_void = std::ptr::null_mut();
    let mut status: c_int = 0;
    unsafe {
        ffinit(&raw mut fptr, cpath.as_ptr(), &raw mut status);
        if status != 0 {
            return Err(status as i32);
        }
        let naxis = naxes.len() as c_int;
        let axes_ptr = if axes.is_empty() {
            std::ptr::null_mut()
        } else {
            axes.as_mut_ptr()
        };
        ffphps(fptr, bitpix as c_int, naxis, axes_ptr, &raw mut status);
        if status == 0 && !data.is_empty() {
            ffppr(
                fptr,
                datatype as c_int,
                1,
                data.len() as c_longlong,
                data.as_ptr().cast::<c_void>().cast_mut(),
                &raw mut status,
            );
        }
        let mut close_status = status;
        ffclos(fptr, &raw mut close_status);
        if status != 0 {
            return Err(status as i32);
        }
        if close_status != 0 {
            return Err(close_status as i32);
        }
    }
    Ok(())
}

/// `ffopen` + `ffgpv` + `ffclos`.
pub fn read_primary_image<T>(path: &str, datatype: i32, out: &mut [T]) -> Result<i32, i32> {
    let _g = lock();
    let cpath = CString::new(path).map_err(|_| 104)?;
    let mut fptr: *mut c_void = std::ptr::null_mut();
    let mut status: c_int = 0;
    let mut anynul: c_int = 0;
    unsafe {
        ffopen(&raw mut fptr, cpath.as_ptr(), 0, &raw mut status);
        if status != 0 {
            return Err(status as i32);
        }
        ffgpv(
            fptr,
            datatype as c_int,
            1,
            out.len() as c_longlong,
            std::ptr::null_mut(),
            out.as_mut_ptr().cast::<c_void>(),
            &raw mut anynul,
            &raw mut status,
        );
        let mut close_status = status;
        ffclos(fptr, &raw mut close_status);
        if status != 0 {
            return Err(status as i32);
        }
        if close_status != 0 {
            return Err(close_status as i32);
        }
    }
    Ok(anynul as i32)
}

/// Open a new empty-primary FITS file via CFITSIO.
pub struct CFile {
    fptr: *mut c_void,
}

impl CFile {
    /// `ffinit` + `ffphps`(BITPIX=8, NAXIS=0).
    pub fn create_empty(path: &str) -> Result<Self, i32> {
        let _g = lock();
        let cpath = CString::new(path).map_err(|_| 105)?;
        let mut fptr: *mut c_void = std::ptr::null_mut();
        let mut status: c_int = 0;
        unsafe {
            ffinit(&raw mut fptr, cpath.as_ptr(), &raw mut status);
            if status != 0 {
                return Err(status as i32);
            }
            ffphps(fptr, 8, 0, std::ptr::null_mut(), &raw mut status);
            if status != 0 {
                let mut cs = status;
                ffclos(fptr, &raw mut cs);
                return Err(status as i32);
            }
        }
        Ok(Self { fptr })
    }

    /// `ffopen`.
    pub fn open(path: &str, iomode: i32) -> Result<Self, i32> {
        let _g = lock();
        let cpath = CString::new(path).map_err(|_| 104)?;
        let mut fptr: *mut c_void = std::ptr::null_mut();
        let mut status: c_int = 0;
        unsafe {
            ffopen(
                &raw mut fptr,
                cpath.as_ptr(),
                iomode as c_int,
                &raw mut status,
            );
            if status != 0 {
                return Err(status as i32);
            }
        }
        Ok(Self { fptr })
    }

    fn cstr(s: &str) -> CString {
        CString::new(s).unwrap_or_else(|_| CString::new("").unwrap())
    }

    fn comm_ptr(comm: Option<&str>) -> (*const c_char, Option<CString>) {
        match comm {
            Some(c) => {
                let cs = Self::cstr(c);
                (cs.as_ptr(), Some(cs))
            }
            None => (std::ptr::null(), None),
        }
    }

    /// `ffpkys`.
    pub fn write_str(&mut self, name: &str, value: &str, comm: Option<&str>) -> i32 {
        let _g = lock();
        let n = Self::cstr(name);
        let v = Self::cstr(value);
        let (cp, _hold) = Self::comm_ptr(comm);
        let mut status: c_int = 0;
        unsafe { ffpkys(self.fptr, n.as_ptr(), v.as_ptr(), cp, &raw mut status) };
        status as i32
    }

    /// `ffpkyj`.
    pub fn write_lng(&mut self, name: &str, value: i64, comm: Option<&str>) -> i32 {
        let _g = lock();
        let n = Self::cstr(name);
        let (cp, _hold) = Self::comm_ptr(comm);
        let mut status: c_int = 0;
        unsafe {
            ffpkyj(
                self.fptr,
                n.as_ptr(),
                value as c_longlong,
                cp,
                &raw mut status,
            )
        };
        status as i32
    }

    /// `ffpkyl`.
    pub fn write_log(&mut self, name: &str, value: bool, comm: Option<&str>) -> i32 {
        let _g = lock();
        let n = Self::cstr(name);
        let (cp, _hold) = Self::comm_ptr(comm);
        let mut status: c_int = 0;
        unsafe {
            ffpkyl(
                self.fptr,
                n.as_ptr(),
                i32::from(value) as c_int,
                cp,
                &raw mut status,
            )
        };
        status as i32
    }

    /// `ffpkyd`.
    pub fn write_dbl(&mut self, name: &str, value: f64, decim: i32, comm: Option<&str>) -> i32 {
        let _g = lock();
        let n = Self::cstr(name);
        let (cp, _hold) = Self::comm_ptr(comm);
        let mut status: c_int = 0;
        unsafe {
            ffpkyd(
                self.fptr,
                n.as_ptr(),
                value,
                decim as c_int,
                cp,
                &raw mut status,
            )
        };
        status as i32
    }

    /// `ffpkyg`.
    pub fn write_fixdbl(&mut self, name: &str, value: f64, decim: i32, comm: Option<&str>) -> i32 {
        let _g = lock();
        let n = Self::cstr(name);
        let (cp, _hold) = Self::comm_ptr(comm);
        let mut status: c_int = 0;
        unsafe {
            ffpkyg(
                self.fptr,
                n.as_ptr(),
                value,
                decim as c_int,
                cp,
                &raw mut status,
            )
        };
        status as i32
    }

    /// `ffpkye`.
    pub fn write_flt(&mut self, name: &str, value: f32, decim: i32, comm: Option<&str>) -> i32 {
        let _g = lock();
        let n = Self::cstr(name);
        let (cp, _hold) = Self::comm_ptr(comm);
        let mut status: c_int = 0;
        unsafe {
            ffpkye(
                self.fptr,
                n.as_ptr(),
                value,
                decim as c_int,
                cp,
                &raw mut status,
            )
        };
        status as i32
    }

    /// `ffpkyu`.
    pub fn write_null(&mut self, name: &str, comm: Option<&str>) -> i32 {
        let _g = lock();
        let n = Self::cstr(name);
        let (cp, _hold) = Self::comm_ptr(comm);
        let mut status: c_int = 0;
        unsafe { ffpkyu(self.fptr, n.as_ptr(), cp, &raw mut status) };
        status as i32
    }

    /// `ffpcom`.
    pub fn write_comment(&mut self, comm: &str) -> i32 {
        let _g = lock();
        let c = Self::cstr(comm);
        let mut status: c_int = 0;
        unsafe { ffpcom(self.fptr, c.as_ptr(), &raw mut status) };
        status as i32
    }

    /// `ffphis`.
    pub fn write_history(&mut self, hist: &str) -> i32 {
        let _g = lock();
        let c = Self::cstr(hist);
        let mut status: c_int = 0;
        unsafe { ffphis(self.fptr, c.as_ptr(), &raw mut status) };
        status as i32
    }

    /// `ffpunt`.
    pub fn write_unit(&mut self, name: &str, unit: &str) -> i32 {
        let _g = lock();
        let n = Self::cstr(name);
        let u = Self::cstr(unit);
        let mut status: c_int = 0;
        unsafe { ffpunt(self.fptr, n.as_ptr(), u.as_ptr(), &raw mut status) };
        status as i32
    }

    /// `ffukys`.
    pub fn update_str(&mut self, name: &str, value: &str, comm: Option<&str>) -> i32 {
        let _g = lock();
        let n = Self::cstr(name);
        let v = Self::cstr(value);
        let (cp, _hold) = Self::comm_ptr(comm);
        let mut status: c_int = 0;
        unsafe { ffukys(self.fptr, n.as_ptr(), v.as_ptr(), cp, &raw mut status) };
        status as i32
    }

    /// `ffmkys`.
    pub fn modify_str(&mut self, name: &str, value: &str, comm: Option<&str>) -> i32 {
        let _g = lock();
        let n = Self::cstr(name);
        let v = Self::cstr(value);
        let (cp, _hold) = Self::comm_ptr(comm);
        let mut status: c_int = 0;
        unsafe { ffmkys(self.fptr, n.as_ptr(), v.as_ptr(), cp, &raw mut status) };
        status as i32
    }

    /// `ffdkey`.
    pub fn delete_key(&mut self, name: &str) -> i32 {
        let _g = lock();
        let n = Self::cstr(name);
        let mut status: c_int = 0;
        unsafe { ffdkey(self.fptr, n.as_ptr(), &raw mut status) };
        status as i32
    }

    /// `ffcrtb`. `tunit` entries of `None` become empty strings.
    pub fn create_tbl(
        &mut self,
        tbltype: i32,
        nrows: i64,
        ttype: &[&str],
        tform: &[&str],
        tunit: &[Option<&str>],
        extname: Option<&str>,
    ) -> i32 {
        let _g = lock();
        let tfields = tform.len() as c_int;
        let mut ttype_c: Vec<CString> = ttype.iter().map(|s| Self::cstr(s)).collect();
        while ttype_c.len() < tform.len() {
            ttype_c.push(Self::cstr(""));
        }
        let tform_c: Vec<CString> = tform.iter().map(|s| Self::cstr(s)).collect();
        let tunit_c: Vec<CString> = (0..tform.len())
            .map(|i| Self::cstr(tunit.get(i).copied().flatten().unwrap_or("")))
            .collect();
        let mut ttype_p: Vec<*mut c_char> = ttype_c.iter().map(|c| c.as_ptr().cast_mut()).collect();
        let mut tform_p: Vec<*mut c_char> = tform_c.iter().map(|c| c.as_ptr().cast_mut()).collect();
        let mut tunit_p: Vec<*mut c_char> = tunit_c.iter().map(|c| c.as_ptr().cast_mut()).collect();
        let ext = extname.map(Self::cstr);
        let ext_ptr = ext.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());
        let mut status: c_int = 0;
        unsafe {
            ffcrtb(
                self.fptr,
                tbltype as c_int,
                nrows as c_longlong,
                tfields,
                ttype_p.as_mut_ptr(),
                tform_p.as_mut_ptr(),
                tunit_p.as_mut_ptr(),
                ext_ptr,
                &raw mut status,
            );
        }
        status as i32
    }

    /// `ffpcls`.
    pub fn write_col_str(&mut self, colnum: i32, firstrow: i64, values: &[&str]) -> i32 {
        let _g = lock();
        let owned: Vec<CString> = values.iter().map(|s| Self::cstr(s)).collect();
        let mut ptrs: Vec<*mut c_char> = owned.iter().map(|c| c.as_ptr().cast_mut()).collect();
        let mut status: c_int = 0;
        unsafe {
            ffpcls(
                self.fptr,
                colnum as c_int,
                firstrow as c_longlong,
                1,
                values.len() as c_longlong,
                ptrs.as_mut_ptr(),
                &raw mut status,
            );
        }
        status as i32
    }

    /// `ffpcljj`.
    pub fn write_col_i64(&mut self, colnum: i32, firstrow: i64, values: &[i64]) -> i32 {
        let _g = lock();
        let mut data: Vec<c_longlong> = values.iter().map(|&v| v as c_longlong).collect();
        let mut status: c_int = 0;
        unsafe {
            ffpcljj(
                self.fptr,
                colnum as c_int,
                firstrow as c_longlong,
                1,
                values.len() as c_longlong,
                data.as_mut_ptr(),
                &raw mut status,
            );
        }
        status as i32
    }

    /// `ffpcld`.
    pub fn write_col_f64(&mut self, colnum: i32, firstrow: i64, values: &mut [f64]) -> i32 {
        let _g = lock();
        let mut status: c_int = 0;
        unsafe {
            ffpcld(
                self.fptr,
                colnum as c_int,
                firstrow as c_longlong,
                1,
                values.len() as c_longlong,
                values.as_mut_ptr(),
                &raw mut status,
            );
        }
        status as i32
    }

    /// `ffpcle`.
    pub fn write_col_f32(&mut self, colnum: i32, firstrow: i64, values: &mut [f32]) -> i32 {
        let _g = lock();
        let mut status: c_int = 0;
        unsafe {
            ffpcle(
                self.fptr,
                colnum as c_int,
                firstrow as c_longlong,
                1,
                values.len() as c_longlong,
                values.as_mut_ptr(),
                &raw mut status,
            );
        }
        status as i32
    }

    /// `ffgcvs`.
    pub fn read_col_str(
        &mut self,
        colnum: i32,
        firstrow: i64,
        nelem: usize,
    ) -> Result<(Vec<String>, i32), i32> {
        let _g = lock();
        let mut bufs: Vec<Vec<u8>> = (0..nelem).map(|_| vec![0u8; FLEN_VALUE]).collect();
        let mut ptrs: Vec<*mut c_char> = bufs
            .iter_mut()
            .map(|b| b.as_mut_ptr().cast::<c_char>())
            .collect();
        let mut anynul: c_int = 0;
        let mut status: c_int = 0;
        let mut nul = [0u8; 1];
        unsafe {
            ffgcvs(
                self.fptr,
                colnum as c_int,
                firstrow as c_longlong,
                1,
                nelem as c_longlong,
                nul.as_mut_ptr().cast::<c_char>(),
                ptrs.as_mut_ptr(),
                &raw mut anynul,
                &raw mut status,
            );
        }
        if status != 0 {
            return Err(status as i32);
        }
        let out = bufs.iter().map(|b| cstr_to_string(b)).collect();
        Ok((out, anynul as i32))
    }

    /// `ffgcvjj`.
    pub fn read_col_i64(
        &mut self,
        colnum: i32,
        firstrow: i64,
        out: &mut [i64],
    ) -> Result<i32, i32> {
        let _g = lock();
        let mut anynul: c_int = 0;
        let mut status: c_int = 0;
        unsafe {
            ffgcvjj(
                self.fptr,
                colnum as c_int,
                firstrow as c_longlong,
                1,
                out.len() as c_longlong,
                0,
                out.as_mut_ptr().cast::<c_longlong>(),
                &raw mut anynul,
                &raw mut status,
            );
        }
        if status != 0 {
            Err(status as i32)
        } else {
            Ok(anynul as i32)
        }
    }

    /// `ffgcvd`.
    pub fn read_col_f64(
        &mut self,
        colnum: i32,
        firstrow: i64,
        out: &mut [f64],
    ) -> Result<i32, i32> {
        let _g = lock();
        let mut anynul: c_int = 0;
        let mut status: c_int = 0;
        unsafe {
            ffgcvd(
                self.fptr,
                colnum as c_int,
                firstrow as c_longlong,
                1,
                out.len() as c_longlong,
                0.0,
                out.as_mut_ptr(),
                &raw mut anynul,
                &raw mut status,
            );
        }
        if status != 0 {
            Err(status as i32)
        } else {
            Ok(anynul as i32)
        }
    }

    /// `ffirow`.
    pub fn insert_rows(&mut self, firstrow: i64, nrows: i64) -> i32 {
        let _g = lock();
        let mut status: c_int = 0;
        unsafe {
            ffirow(
                self.fptr,
                firstrow as c_longlong,
                nrows as c_longlong,
                &raw mut status,
            )
        };
        status as i32
    }

    /// `ffdrow`.
    pub fn delete_rows(&mut self, firstrow: i64, nrows: i64) -> i32 {
        let _g = lock();
        let mut status: c_int = 0;
        unsafe {
            ffdrow(
                self.fptr,
                firstrow as c_longlong,
                nrows as c_longlong,
                &raw mut status,
            )
        };
        status as i32
    }

    /// `fficol`.
    pub fn insert_col(&mut self, numcol: i32, ttype: &str, tform: &str) -> i32 {
        let _g = lock();
        let mut tn = Self::cstr(ttype).into_bytes_with_nul();
        let mut tf = Self::cstr(tform).into_bytes_with_nul();
        let mut status: c_int = 0;
        unsafe {
            fficol(
                self.fptr,
                numcol as c_int,
                tn.as_mut_ptr().cast::<c_char>(),
                tf.as_mut_ptr().cast::<c_char>(),
                &raw mut status,
            );
        }
        status as i32
    }

    /// `ffdcol`.
    pub fn delete_col(&mut self, colnum: i32) -> i32 {
        let _g = lock();
        let mut status: c_int = 0;
        unsafe { ffdcol(self.fptr, colnum as c_int, &raw mut status) };
        status as i32
    }

    /// `ffpclu`.
    pub fn write_col_null(&mut self, colnum: i32, firstrow: i64, nelem: i64) -> i32 {
        let _g = lock();
        let mut status: c_int = 0;
        unsafe {
            ffpclu(
                self.fptr,
                colnum as c_int,
                firstrow as c_longlong,
                1,
                nelem as c_longlong,
                &raw mut status,
            );
        }
        status as i32
    }

    /// `ffpcll`.
    pub fn write_col_log(&mut self, colnum: i32, firstrow: i64, values: &[bool]) -> i32 {
        let _g = lock();
        let mut data: Vec<c_char> = values.iter().map(|&v| i8::from(v) as c_char).collect();
        let mut status: c_int = 0;
        unsafe {
            ffpcll(
                self.fptr,
                colnum as c_int,
                firstrow as c_longlong,
                1,
                values.len() as c_longlong,
                data.as_mut_ptr(),
                &raw mut status,
            );
        }
        status as i32
    }

    /// `ffpclx`, one bit per row (scalar `1X` columns).
    pub fn write_col_bit(&mut self, colnum: i32, firstrow: i64, values: &[bool]) -> i32 {
        let _g = lock();
        let mut status: c_int = 0;
        for (i, &v) in values.iter().enumerate() {
            let mut data = [i8::from(v) as c_char];
            unsafe {
                ffpclx(
                    self.fptr,
                    colnum as c_int,
                    (firstrow + i as i64) as c_longlong,
                    1,
                    1,
                    data.as_mut_ptr(),
                    &raw mut status,
                );
            }
            if status != 0 {
                break;
            }
        }
        status as i32
    }

    /// `ffmahd`.
    pub fn movabs_hdu(&mut self, hdunum: i32) -> i32 {
        let _g = lock();
        let mut hdutype: c_int = 0;
        let mut status: c_int = 0;
        unsafe {
            ffmahd(
                self.fptr,
                hdunum as c_int,
                &raw mut hdutype,
                &raw mut status,
            )
        };
        status as i32
    }

    /// `ffclos`.
    pub fn close(mut self) -> i32 {
        let _g = lock();
        let mut status: c_int = 0;
        unsafe { ffclos(self.fptr, &raw mut status) };
        self.fptr = std::ptr::null_mut();
        status as i32
    }
}

impl Drop for CFile {
    fn drop(&mut self) {
        if !self.fptr.is_null() {
            let _g = lock();
            let mut status: c_int = 0;
            unsafe { ffclos(self.fptr, &raw mut status) };
            self.fptr = std::ptr::null_mut();
        }
    }
}

/// `ffd2e` text.
pub fn ffd2e_str(value: f64, decim: i32) -> (String, i32) {
    let _g = lock();
    let mut buf = [0u8; FLEN_VALUE];
    let mut status: c_int = 0;
    unsafe {
        ffd2e(
            value,
            decim as c_int,
            buf.as_mut_ptr().cast::<c_char>(),
            &raw mut status,
        );
    }
    (cstr_to_string(&buf), status as i32)
}

/// `ffasfm`. Returns `(datacode, width, decimals)`.
pub fn ffasfm_parse(tform: &str) -> Result<(i32, i64, i32), i32> {
    let _g = lock();
    let mut buf = {
        let mut v = CString::new(tform)
            .expect("tform contains NUL")
            .into_bytes_with_nul();
        v.resize(FLEN_VALUE, 0);
        v
    };
    let mut dtcode: c_int = 0;
    let mut twidth: c_long = 0;
    let mut decimals: c_int = 0;
    let mut status: c_int = 0;
    unsafe {
        ffasfm(
            buf.as_mut_ptr().cast::<c_char>(),
            &raw mut dtcode,
            &raw mut twidth,
            &raw mut decimals,
            &raw mut status,
        );
    }
    if status != 0 {
        Err(status as i32)
    } else {
        Ok((dtcode as i32, twidth as i64, decimals as i32))
    }
}

/// `ffbnfm`. Returns `(datacode, repeat, width)`.
pub fn ffbnfm_parse(tform: &str) -> Result<(i32, i64, i64), i32> {
    let _g = lock();
    let mut buf = {
        let mut v = CString::new(tform)
            .expect("tform contains NUL")
            .into_bytes_with_nul();
        v.resize(FLEN_VALUE, 0);
        v
    };
    let mut dtcode: c_int = 0;
    let mut trepeat: c_long = 0;
    let mut twidth: c_long = 0;
    let mut status: c_int = 0;
    unsafe {
        ffbnfm(
            buf.as_mut_ptr().cast::<c_char>(),
            &raw mut dtcode,
            &raw mut trepeat,
            &raw mut twidth,
            &raw mut status,
        );
    }
    if status != 0 {
        Err(status as i32)
    } else {
        Ok((dtcode as i32, trepeat as i64, twidth as i64))
    }
}

/// `ffgabc`. Returns `(rowlen, tbcol)`.
#[allow(clippy::unnecessary_cast, clippy::useless_conversion)]
pub fn ffgabc_vals(tforms: &[&str], space: i32) -> Result<(i64, Vec<i64>), i32> {
    let _g = lock();
    let owned: Vec<CString> = tforms
        .iter()
        .map(|s| CString::new(*s).expect("tform contains NUL"))
        .collect();
    let mut ptrs: Vec<*mut c_char> = owned.iter().map(|c| c.as_ptr().cast_mut()).collect();
    let mut tbcol = vec![0 as c_long; tforms.len().max(1)];
    let mut rowlen: c_long = 0;
    let mut status: c_int = 0;
    unsafe {
        ffgabc(
            tforms.len() as c_int,
            ptrs.as_mut_ptr(),
            space as c_int,
            &raw mut rowlen,
            tbcol.as_mut_ptr(),
            &raw mut status,
        );
    }
    if status != 0 {
        Err(status as i32)
    } else {
        Ok((
            rowlen as i64,
            tbcol
                .into_iter()
                .take(tforms.len())
                .map(i64::from)
                .collect(),
        ))
    }
}

/// `ffd2f` text.
pub fn ffd2f_str(value: f64, decim: i32) -> (String, i32) {
    let _g = lock();
    let mut buf = [0u8; FLEN_VALUE];
    let mut status: c_int = 0;
    unsafe {
        ffd2f(
            value,
            decim as c_int,
            buf.as_mut_ptr().cast::<c_char>(),
            &raw mut status,
        );
    }
    (cstr_to_string(&buf), status as i32)
}
