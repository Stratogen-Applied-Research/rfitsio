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
}

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
