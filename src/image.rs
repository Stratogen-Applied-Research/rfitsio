//! Image HDU create / read / write.

use crate::convert::{
    decode_physical, decode_sbyte_i8, decode_ulong_u32, decode_ulonglong_u64, decode_ushort_u16,
    encode_i8_sbyte, encode_physical, encode_u16_ushort, encode_u32_ulong, encode_u64_ulonglong,
    pad_data_len,
};
use crate::error::{FitsError, Result};
use crate::file::FitsFile;
use crate::hdu::Hdu;
use crate::header::Header;
use crate::io::Driver;
use crate::status::{
    BAD_BITPIX, BAD_DATATYPE, BAD_ELEM_NUM, BAD_PIX_NUM, HEADER_NOT_EMPTY, NEG_BYTES, NOT_IMAGE,
    NUM_OVERFLOW,
};
use crate::types::{HduType, ImageType, RECORD_LEN};

/// A native pixel type that can be written to or read from a FITS image.
pub trait Pixel: Copy + Default {
    /// CFITSIO `TBYTE` / `TSHORT` / … code.
    fn datatype() -> i32;
    /// Physical value as f64 for conversion.
    fn to_f64(self) -> f64;
    /// Convert from a physical value, or `NUM_OVERFLOW`.
    fn from_f64(v: f64) -> Result<Self>;
    /// Native big-endian payload (no BZERO).
    fn to_be_bytes(self) -> Vec<u8>;
    /// Inverse of [`Pixel::to_be_bytes`].
    fn from_be_bytes(bytes: &[u8]) -> Result<Self>;
}

macro_rules! int_pixel {
    ($t:ty, $code:expr, $to:expr, $n:expr) => {
        impl Pixel for $t {
            fn datatype() -> i32 {
                $code
            }
            fn to_f64(self) -> f64 {
                $to(self)
            }
            fn from_f64(v: f64) -> Result<Self> {
                let r = v.round();
                if r < (Self::MIN as f64) || r > (Self::MAX as f64) {
                    return Err(FitsError::new(NUM_OVERFLOW));
                }
                Ok(r as Self)
            }
            fn to_be_bytes(self) -> Vec<u8> {
                <$t>::to_be_bytes(self).to_vec()
            }
            fn from_be_bytes(bytes: &[u8]) -> Result<Self> {
                let arr: [u8; $n] = bytes
                    .get(..$n)
                    .and_then(|s| s.try_into().ok())
                    .ok_or_else(|| FitsError::new(crate::status::READ_ERROR))?;
                Ok(<$t>::from_be_bytes(arr))
            }
        }
    };
}

int_pixel!(u8, crate::types::TBYTE, f64::from, 1);
int_pixel!(i8, crate::types::TSBYTE, f64::from, 1);
int_pixel!(u16, crate::types::TUSHORT, f64::from, 2);
int_pixel!(i16, crate::types::TSHORT, f64::from, 2);
int_pixel!(u32, crate::types::TUINT, f64::from, 4);
int_pixel!(i32, crate::types::TINT, f64::from, 4);
int_pixel!(u64, crate::types::TULONGLONG, |x| x as f64, 8);
int_pixel!(i64, crate::types::TLONGLONG, |x| x as f64, 8);

impl Pixel for f32 {
    fn datatype() -> i32 {
        crate::types::TFLOAT
    }
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_f64(v: f64) -> Result<Self> {
        Ok(v as f32)
    }
    fn to_be_bytes(self) -> Vec<u8> {
        f32::to_be_bytes(self).to_vec()
    }
    fn from_be_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(f32::from_be_bytes(copy4(bytes)?))
    }
}

impl Pixel for f64 {
    fn datatype() -> i32 {
        crate::types::TDOUBLE
    }
    fn to_f64(self) -> f64 {
        self
    }
    fn from_f64(v: f64) -> Result<Self> {
        Ok(v)
    }
    fn to_be_bytes(self) -> Vec<u8> {
        f64::to_be_bytes(self).to_vec()
    }
    fn from_be_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(f64::from_be_bytes(copy8(bytes)?))
    }
}

impl FitsFile {
    /// `fits_create_img` / `ffcrim`.
    ///
    /// If the current HDU is a null (NAXIS=0) primary, it is replaced.
    /// Otherwise a new IMAGE extension is appended.
    pub fn create_image(&mut self, ty: ImageType, naxes: &[i64]) -> Result<()> {
        self.require_write()?;
        let replace = {
            let inner = self.inner()?;
            inner.current == 0
                && inner.hdus.len() == 1
                && inner.hdus.first().is_some_and(Hdu::is_null_image)
        };

        if replace {
            let header = Header::primary_image(ty, naxes)?;
            let header_bytes = header.to_record_bytes();
            let data_start = header_bytes.len() as u64;
            let data_len = data_len_bytes(ty, naxes)?;
            let padded = pad_data_len(data_len);
            let inner = self.inner_mut()?;
            inner.io.write_at(0, &header_bytes)?;
            if padded > 0 {
                write_zeros(&mut inner.io, data_start, padded)?;
            }
            inner.io.truncate(data_start + padded)?;
            inner.io.flush()?;
            inner.hdus[0] = Hdu {
                hdu_type: HduType::Image,
                header,
                header_start: 0,
                data_start,
            };
            inner.dirty = false;
            return Ok(());
        }

        // New IMAGE extension at end of file.
        let inner = self.inner()?;
        let last = inner
            .hdus
            .last()
            .ok_or_else(|| FitsError::new(BAD_BITPIX))?;
        let end = last.data_start + last.data_unit_len()?;
        let header = Header::image_extension(ty, naxes)?;
        let header_bytes = header.to_record_bytes();
        let data_start = end + header_bytes.len() as u64;
        let padded = pad_data_len(data_len_bytes(ty, naxes)?);
        let inner = self.inner_mut()?;
        inner.io.write_at(end, &header_bytes)?;
        if padded > 0 {
            write_zeros(&mut inner.io, data_start, padded)?;
        }
        inner.io.truncate(data_start + padded)?;
        inner.io.flush()?;
        inner.hdus.push(Hdu {
            hdu_type: HduType::Image,
            header,
            header_start: end,
            data_start,
        });
        inner.current = inner.hdus.len() - 1;
        inner.dirty = false;
        Ok(())
    }

    /// Write `data` starting at 1-based `firstelem` (`fits_write_img` / `ffppr`).
    pub fn write_image<T: Pixel>(&mut self, firstelem: i64, data: &[T]) -> Result<()> {
        self.require_write()?;
        if data.is_empty() {
            return Ok(());
        }
        if firstelem < 1 {
            return Err(FitsError::new(BAD_ELEM_NUM));
        }
        let (ty, bscale, bzero, data_start, npix, bpp) = self.image_layout()?;
        let nelem = data.len() as i64;
        if firstelem - 1 + nelem > npix {
            return Err(FitsError::new(BAD_PIX_NUM));
        }
        let mut buf = Vec::with_capacity(data.len() * bpp);
        for &px in data {
            buf.extend_from_slice(&encode_pixel(px, ty, bscale, bzero)?);
        }
        let pos = data_start + ((firstelem - 1) as u64) * bpp as u64;
        let inner = self.inner_mut()?;
        inner.io.write_at(pos, &buf)?;
        Ok(())
    }

    /// Read `nelem` pixels starting at 1-based `firstelem` (`fits_read_img` / `ffgpv`).
    pub fn read_image<T: Pixel>(&mut self, firstelem: i64, nelem: usize) -> Result<Vec<T>> {
        if nelem == 0 {
            return Ok(Vec::new());
        }
        if firstelem < 1 {
            return Err(FitsError::new(BAD_ELEM_NUM));
        }
        let (ty, bscale, bzero, data_start, npix, bpp) = self.image_layout()?;
        if firstelem - 1 + nelem as i64 > npix {
            return Err(FitsError::new(BAD_PIX_NUM));
        }
        let pos = data_start + ((firstelem - 1) as u64) * bpp as u64;
        let nbytes = nelem * bpp;
        let mut buf = vec![0u8; nbytes];
        let n = self.inner_mut()?.io.read_at(pos, &mut buf)?;
        if n < nbytes {
            return Err(FitsError::new(crate::status::END_OF_FILE));
        }
        let mut out = Vec::with_capacity(nelem);
        for chunk in buf.chunks_exact(bpp) {
            out.push(decode_pixel(chunk, ty, bscale, bzero)?);
        }
        Ok(out)
    }

    /// Read the entire current image.
    pub fn read_image_all<T: Pixel>(&mut self) -> Result<Vec<T>> {
        let npix = self.image_layout()?.4;
        if npix < 0 {
            return Err(FitsError::new(NEG_BYTES));
        }
        self.read_image(1, npix as usize)
    }

    /// Write `BLANK` (`fits_set_imgnull` / `ffpnul`).
    pub fn set_imgnull(&mut self, blank: i64) -> Result<()> {
        self.require_write()?;
        let inner = self.inner_mut()?;
        if inner.hdus[inner.current].hdu_type != HduType::Image {
            return Err(FitsError::new(NOT_IMAGE));
        }
        inner.hdus[inner.current].header.set_blank(blank)?;
        let bytes = inner.hdus[inner.current].header.to_record_bytes();
        let start = inner.hdus[inner.current].header_start;
        inner.io.write_at(start, &bytes)?;
        Ok(())
    }

    /// `fits_resize_img` / `ffrsim` for the current (last) image HDU.
    pub fn resize_image(&mut self, ty: ImageType, naxes: &[i64]) -> Result<()> {
        self.require_write()?;
        let inner = self.inner()?;
        if inner.current + 1 != inner.hdus.len() {
            return Err(FitsError::with_message(
                HEADER_NOT_EMPTY,
                "resize_image only supports the last HDU",
            ));
        }
        let header_start = inner.hdus[inner.current].header_start;
        let header = if header_start == 0 {
            Header::primary_image(ty, naxes)?
        } else {
            Header::image_extension(ty, naxes)?
        };
        let header_bytes = header.to_record_bytes();
        let data_start = header_start + header_bytes.len() as u64;
        let padded = pad_data_len(data_len_bytes(ty, naxes)?);
        let inner = self.inner_mut()?;
        inner.io.write_at(header_start, &header_bytes)?;
        if padded > 0 {
            write_zeros(&mut inner.io, data_start, padded)?;
        }
        inner.io.truncate(data_start + padded)?;
        inner.hdus[inner.current] = Hdu {
            hdu_type: HduType::Image,
            header,
            header_start,
            data_start,
        };
        inner.dirty = false;
        Ok(())
    }

    /// BITPIX / naxes of the current image.
    pub fn image_size(&self) -> Result<(ImageType, Vec<i64>)> {
        let inner = self.inner()?;
        let hdu = &inner.hdus[inner.current];
        if hdu.hdu_type != HduType::Image {
            return Err(FitsError::new(NOT_IMAGE));
        }
        Ok((hdu.header.image_type()?, hdu.header.naxes()?))
    }

    fn image_layout(&self) -> Result<(ImageType, f64, f64, u64, i64, usize)> {
        let inner = self.inner()?;
        let hdu = &inner.hdus[inner.current];
        if hdu.hdu_type != HduType::Image {
            return Err(FitsError::new(NOT_IMAGE));
        }
        let ty = hdu.header.image_type()?;
        let bscale = hdu.header.bscale();
        let bzero = hdu.header.bzero();
        let npix = hdu.data_bytes()? / ty.bytes_per_pixel() as u64;
        Ok((
            ty,
            bscale,
            bzero,
            hdu.data_start,
            npix as i64,
            ty.bytes_per_pixel(),
        ))
    }
}

fn data_len_bytes(ty: ImageType, naxes: &[i64]) -> Result<u64> {
    if naxes.is_empty() {
        return Ok(0);
    }
    let mut n = 1u64;
    for &a in naxes {
        if a < 1 {
            return Err(FitsError::new(crate::status::NEG_AXIS));
        }
        n = n
            .checked_mul(a as u64)
            .ok_or_else(|| FitsError::new(crate::status::ARRAY_TOO_BIG))?;
    }
    Ok(n * ty.bytes_per_pixel() as u64)
}

fn write_zeros(io: &mut dyn crate::io::Driver, pos: u64, len: u64) -> Result<()> {
    const CHUNK: usize = RECORD_LEN;
    let zeros = [0u8; CHUNK];
    let mut remaining = len;
    let mut at = pos;
    while remaining > 0 {
        let n = remaining.min(CHUNK as u64) as usize;
        io.write_at(at, &zeros[..n])?;
        at += n as u64;
        remaining -= n as u64;
    }
    Ok(())
}

fn encode_pixel<T: Pixel>(px: T, ty: ImageType, bscale: f64, bzero: f64) -> Result<Vec<u8>> {
    if (bscale - 1.0).abs() < f64::EPSILON {
        match (ty, T::datatype()) {
            (ImageType::U8, crate::types::TBYTE) if bzero == 0.0 => {
                return Ok(px.to_be_bytes());
            }
            (ImageType::I8, crate::types::TSBYTE) if (bzero + 128.0).abs() < 0.5 => {
                return Ok(vec![encode_i8_sbyte(i8::from_be_bytes(copy1(
                    &px.to_be_bytes(),
                )?))]);
            }
            (ImageType::I16, crate::types::TSHORT) if bzero == 0.0 => {
                return Ok(px.to_be_bytes());
            }
            (ImageType::U16, crate::types::TUSHORT) if (bzero - 32768.0).abs() < 0.5 => {
                let v = u16::from_be_bytes(copy2(&px.to_be_bytes())?);
                return Ok(encode_u16_ushort(v).to_vec());
            }
            (ImageType::I32, crate::types::TINT) if bzero == 0.0 => {
                return Ok(px.to_be_bytes());
            }
            (ImageType::U32, crate::types::TUINT) if (bzero - 2_147_483_648.0).abs() < 0.5 => {
                let v = u32::from_be_bytes(copy4(&px.to_be_bytes())?);
                return Ok(encode_u32_ulong(v).to_vec());
            }
            (ImageType::I64, crate::types::TLONGLONG) if bzero == 0.0 => {
                return Ok(px.to_be_bytes());
            }
            (ImageType::U64, crate::types::TULONGLONG) if bzero > 1e18 => {
                let v = u64::from_be_bytes(copy8(&px.to_be_bytes())?);
                return Ok(encode_u64_ulonglong(v).to_vec());
            }
            (ImageType::F32, crate::types::TFLOAT) if bzero == 0.0 => {
                return Ok(px.to_be_bytes());
            }
            (ImageType::F64, crate::types::TDOUBLE) if bzero == 0.0 => {
                return Ok(px.to_be_bytes());
            }
            _ => {}
        }
    }
    encode_physical(px.to_f64(), ty, bscale, bzero)
}

fn decode_pixel<T: Pixel>(bytes: &[u8], ty: ImageType, bscale: f64, bzero: f64) -> Result<T> {
    if (bscale - 1.0).abs() < f64::EPSILON {
        match (ty, T::datatype()) {
            (ImageType::U8, crate::types::TBYTE) if bzero == 0.0 => {
                return T::from_be_bytes(bytes);
            }
            (ImageType::I8, crate::types::TSBYTE) if (bzero + 128.0).abs() < 0.5 => {
                let v = decode_sbyte_i8(bytes[0]);
                return T::from_be_bytes(&i8::to_be_bytes(v));
            }
            (ImageType::I16, crate::types::TSHORT) if bzero == 0.0 => {
                return T::from_be_bytes(bytes);
            }
            (ImageType::U16, crate::types::TUSHORT) if (bzero - 32768.0).abs() < 0.5 => {
                let v = decode_ushort_u16(copy2(bytes)?);
                return T::from_be_bytes(&v.to_be_bytes());
            }
            (ImageType::I32, crate::types::TINT) if bzero == 0.0 => {
                return T::from_be_bytes(bytes);
            }
            (ImageType::U32, crate::types::TUINT) if (bzero - 2_147_483_648.0).abs() < 0.5 => {
                let v = decode_ulong_u32(copy4(bytes)?);
                return T::from_be_bytes(&v.to_be_bytes());
            }
            (ImageType::I64, crate::types::TLONGLONG) if bzero == 0.0 => {
                return T::from_be_bytes(bytes);
            }
            (ImageType::U64, crate::types::TULONGLONG) if bzero > 1e18 => {
                let v = decode_ulonglong_u64(copy8(bytes)?);
                return T::from_be_bytes(&v.to_be_bytes());
            }
            (ImageType::F32, crate::types::TFLOAT) if bzero == 0.0 => {
                return T::from_be_bytes(bytes);
            }
            (ImageType::F64, crate::types::TDOUBLE) if bzero == 0.0 => {
                return T::from_be_bytes(bytes);
            }
            _ => {}
        }
    }
    T::from_f64(decode_physical(bytes, ty, bscale, bzero)?)
}

fn copy1(b: &[u8]) -> Result<[u8; 1]> {
    b.get(..1)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| FitsError::new(crate::status::READ_ERROR))
}

fn copy2(b: &[u8]) -> Result<[u8; 2]> {
    b.get(..2)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| FitsError::new(crate::status::READ_ERROR))
}
fn copy4(b: &[u8]) -> Result<[u8; 4]> {
    b.get(..4)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| FitsError::new(crate::status::READ_ERROR))
}
fn copy8(b: &[u8]) -> Result<[u8; 8]> {
    b.get(..8)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| FitsError::new(crate::status::READ_ERROR))
}

/// `fits_create_img`.
pub fn fits_create_img(f: &mut FitsFile, bitpix: i32, naxes: &[i64]) -> Result<()> {
    let ty = ImageType::from_code(bitpix).ok_or_else(|| FitsError::new(BAD_DATATYPE))?;
    f.create_image(ty, naxes)
}

/// `fits_write_img`.
pub fn fits_write_img<T: Pixel>(f: &mut FitsFile, firstelem: i64, data: &[T]) -> Result<()> {
    f.write_image(firstelem, data)
}

/// `fits_read_img`.
pub fn fits_read_img<T: Pixel>(f: &mut FitsFile, firstelem: i64, nelem: usize) -> Result<Vec<T>> {
    f.read_image(firstelem, nelem)
}
