//! Tiled image compression (FITS ZIMAGE binary tables).
//!
//! Supported codecs: `RICE_1`, `GZIP_1`, `GZIP_2`, `PLIO_1`, `HCOMPRESS_1`,
//! plus lossless float gzip (`ZQUANTIZ = NONE`) and quantized floats.

#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_range_loop
)]

mod hcompress;
mod plio;
mod quantize;
mod rice;

#[cfg(feature = "gzip")]
mod gzip_tile;

use crate::error::{FitsError, Result};
use crate::file::FitsFile;
use crate::header::Header;
use crate::status::{BAD_DATATYPE, BAD_DIMEN, DATA_COMPRESSION_ERR, DATA_DECOMPRESSION_ERR};
use crate::types::{
    GZIP_1, GZIP_2, HCOMPRESS_1, ImageType, MAX_COMPRESS_DIM, NO_DITHER, NO_QUANTIZE, NOCOMPRESS,
    PLIO_1, RICE_1, SUBTRACTIVE_DITHER_1, SUBTRACTIVE_DITHER_2,
};

pub use quantize::{N_RANDOM, dither_table};

/// Requested tiled-compression algorithm (`fits_set_compression_type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompressionType {
    /// Uncompressed tiles (`NOCOMPRESS`).
    None,
    /// Rice coding.
    Rice1,
    /// gzip of big-endian pixels.
    Gzip1,
    /// byte-shuffle then gzip.
    Gzip2,
    /// IRAF pixel list (non-negative 24-bit).
    Plio1,
    /// H-transform.
    Hcompress1,
}

impl CompressionType {
    /// CFITSIO integer code.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::None => NOCOMPRESS,
            Self::Rice1 => RICE_1,
            Self::Gzip1 => GZIP_1,
            Self::Gzip2 => GZIP_2,
            Self::Plio1 => PLIO_1,
            Self::Hcompress1 => HCOMPRESS_1,
        }
    }

    /// Parse a CFITSIO code. `0` means "not set" (default Rice when writing).
    #[must_use]
    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            0 | NOCOMPRESS => Some(Self::None),
            RICE_1 => Some(Self::Rice1),
            GZIP_1 => Some(Self::Gzip1),
            GZIP_2 => Some(Self::Gzip2),
            PLIO_1 => Some(Self::Plio1),
            HCOMPRESS_1 => Some(Self::Hcompress1),
            _ => None,
        }
    }

    /// `ZCMPTYPE` string.
    #[must_use]
    pub const fn zcmptype(self) -> &'static str {
        match self {
            Self::None => "NOCOMPRESS",
            Self::Rice1 => "RICE_1",
            Self::Gzip1 => "GZIP_1",
            Self::Gzip2 => "GZIP_2",
            Self::Plio1 => "PLIO_1",
            Self::Hcompress1 => "HCOMPRESS_1",
        }
    }

    fn from_zcmptype(s: &str) -> Result<Self> {
        match s.trim() {
            "RICE_1" | "RICE_ONE" => Ok(Self::Rice1),
            "GZIP_1" => Ok(Self::Gzip1),
            "GZIP_2" => Ok(Self::Gzip2),
            "PLIO_1" => Ok(Self::Plio1),
            "HCOMPRESS_1" => Ok(Self::Hcompress1),
            "NOCOMPRESS" => Ok(Self::None),
            other => Err(FitsError::with_message(
                DATA_DECOMPRESSION_ERR,
                format!("Unknown image compression type: {other}"),
            )),
        }
    }
}

/// Parameters applied when the next image HDU is created.
#[derive(Debug, Clone)]
pub struct CompressionRequest {
    /// Algorithm; `None` means do not compress.
    pub kind: Option<CompressionType>,
    /// Tile size per axis; 0/negative follow CFITSIO defaults.
    pub tilesize: [i64; MAX_COMPRESS_DIM],
    /// Quantize level. `0.0` unset; [`NO_QUANTIZE`] lossless gzip.
    pub quantize_level: f32,
    /// Dither method (`NO_DITHER` / `SUBTRACTIVE_DITHER_1` / `_2`).
    pub quantize_method: i32,
    /// `ZDITHER0` seed offset (1-based).
    pub dither_seed: i32,
    /// Hcompress scale (0 = lossless).
    pub hcomp_scale: f32,
    /// Hcompress smooth flag.
    pub hcomp_smooth: i32,
}

impl Default for CompressionRequest {
    fn default() -> Self {
        Self {
            kind: None,
            tilesize: [0; MAX_COMPRESS_DIM],
            quantize_level: 0.0,
            quantize_method: 0,
            dither_seed: 1,
            hcomp_scale: 0.0,
            hcomp_smooth: 0,
        }
    }
}

/// Parsed ZIMAGE header.
#[derive(Debug, Clone)]
pub struct ZImageInfo {
    /// Compression algorithm.
    pub kind: CompressionType,
    /// Uncompressed BITPIX.
    pub zbitpix: i32,
    /// Uncompressed axis lengths.
    pub znaxis: Vec<i64>,
    /// Tile size per axis.
    pub tilesize: Vec<i64>,
    /// Rice block size (`ZVAL1`).
    pub rice_blocksize: i32,
    /// Rice bytes per pixel (`ZVAL2`).
    pub rice_bytepix: i32,
    /// Hcompress scale.
    pub hcomp_scale: f32,
    /// Hcompress smooth.
    pub hcomp_smooth: i32,
    /// Quantize method.
    pub quantize_method: i32,
    /// True when floats are stored losslessly.
    pub no_quantize: bool,
    /// Dither seed (`ZDITHER0`).
    pub dither_seed: i32,
    /// Conventional BSCALE of the integer image.
    pub bscale: f64,
    /// Conventional BZERO.
    pub bzero: f64,
}

impl ZImageInfo {
    /// Number of uncompressed pixels.
    #[must_use]
    pub fn npix(&self) -> i64 {
        self.znaxis
            .iter()
            .copied()
            .filter(|&n| n > 0)
            .product::<i64>()
            .max(0)
    }

    /// [`ImageType`] of the uncompressed image.
    pub fn image_type(&self) -> Result<ImageType> {
        let z = self.bzero;
        Ok(match self.zbitpix {
            8 if (z + 128.0).abs() < 0.5 => ImageType::I8,
            8 => ImageType::U8,
            16 if (z - 32768.0).abs() < 0.5 => ImageType::U16,
            16 => ImageType::I16,
            32 if (z - 2_147_483_648.0).abs() < 0.5 => ImageType::U32,
            32 => ImageType::I32,
            64 if z > 1e18 => ImageType::U64,
            64 => ImageType::I64,
            -32 => ImageType::F32,
            -64 => ImageType::F64,
            _ => return Err(FitsError::new(crate::status::BAD_BITPIX)),
        })
    }

    /// Bytes per uncompressed stored pixel.
    #[must_use]
    pub fn bytes_per_pixel(&self) -> usize {
        match self.zbitpix.unsigned_abs() {
            8 => 1,
            16 => 2,
            32 => 4,
            64 => 8,
            _ => 1,
        }
    }
}

impl Header {
    /// True when this HDU is a tiled compressed image (`ZIMAGE = T`).
    #[must_use]
    pub fn is_compressed_image(&self) -> bool {
        self.get_logical("ZIMAGE").map(|(v, _)| v).unwrap_or(false)
    }

    /// Parse ZIMAGE keywords. `nrows` is `NAXIS2` of the binary table.
    pub fn zimage_info(&self, nrows: i64) -> Result<ZImageInfo> {
        let zcmp = self.get_string("ZCMPTYPE").map(|(v, _)| v).map_err(|_| {
            FitsError::with_message(
                DATA_DECOMPRESSION_ERR,
                "required ZCMPTYPE compression keyword not found",
            )
        })?;
        let kind = CompressionType::from_zcmptype(&zcmp)?;
        let zbitpix = self.get_i64("ZBITPIX").map_err(|_| {
            FitsError::with_message(
                DATA_DECOMPRESSION_ERR,
                "required ZBITPIX compression keyword not found",
            )
        })? as i32;
        let zndim = self.get_i64("ZNAXIS").map_err(|_| {
            FitsError::with_message(
                DATA_DECOMPRESSION_ERR,
                "required ZNAXIS compression keyword not found",
            )
        })?;
        if zndim < 1 || zndim as usize > MAX_COMPRESS_DIM {
            return Err(FitsError::new(crate::status::BAD_NAXIS));
        }
        let mut znaxis = Vec::with_capacity(zndim as usize);
        let mut tilesize = Vec::with_capacity(zndim as usize);
        let mut expect_nrows = 1i64;
        for i in 1..=zndim {
            let n = self.get_i64(&format!("ZNAXIS{i}")).map_err(|_| {
                FitsError::with_message(
                    DATA_DECOMPRESSION_ERR,
                    "required ZNAXISn compression keyword not found",
                )
            })?;
            znaxis.push(n);
            let default = if i == 1 { n } else { 1 };
            let t = self.get_i64(&format!("ZTILE{i}")).unwrap_or(default);
            if t == 0 {
                return Err(FitsError::with_message(
                    DATA_DECOMPRESSION_ERR,
                    "invalid ZTILE value = 0 in compressed image",
                ));
            }
            tilesize.push(t);
            expect_nrows *= (n - 1) / t + 1;
        }
        if expect_nrows != nrows {
            return Err(FitsError::with_message(
                DATA_DECOMPRESSION_ERR,
                "number of table rows != the number of tiles in compressed image",
            ));
        }
        let has_zscale = self.card_by_name("ZSCALE").is_some()
            || (1..=self.get_i64("TFIELDS").unwrap_or(0)).any(|c| {
                self.get_string(&format!("TTYPE{c}"))
                    .map(|(v, _)| v.eq_ignore_ascii_case("ZSCALE"))
                    .unwrap_or(false)
            });
        let zquant = self.get_string("ZQUANTIZ").ok().map(|(v, _)| v);
        let (no_quantize, quantize_method) = if zbitpix < 0 && !has_zscale {
            (true, NO_DITHER)
        } else {
            match zquant.as_deref() {
                Some(s) if s.trim() == "NONE" => (true, NO_DITHER),
                Some(s) if s.trim() == "SUBTRACTIVE_DITHER_1" => (false, SUBTRACTIVE_DITHER_1),
                Some(s) if s.trim() == "SUBTRACTIVE_DITHER_2" => (false, SUBTRACTIVE_DITHER_2),
                Some(s) if s.trim() == "NO_DITHER" => (false, NO_DITHER),
                Some(_) => {
                    return Err(FitsError::with_message(
                        DATA_DECOMPRESSION_ERR,
                        "Unknown quantization type",
                    ));
                }
                None => (false, NO_DITHER),
            }
        };
        let mut rice_blocksize = 32;
        let mut rice_bytepix = 4;
        let mut hcomp_scale = 0.0f32;
        let mut hcomp_smooth = 0i32;
        if kind == CompressionType::Rice1 {
            rice_blocksize = self.get_i64("ZVAL1").unwrap_or(32) as i32;
            rice_bytepix = self.get_i64("ZVAL2").unwrap_or(4) as i32;
            if rice_blocksize < 16 && rice_bytepix > 8 {
                std::mem::swap(&mut rice_blocksize, &mut rice_bytepix);
            }
            if rice_blocksize == 0 {
                return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
            }
        } else if kind == CompressionType::Hcompress1 {
            hcomp_scale = self.get_f64("ZVAL1").unwrap_or(0.0) as f32;
            hcomp_smooth = self.get_i64("ZVAL2").unwrap_or(0) as i32;
        }
        Ok(ZImageInfo {
            kind,
            zbitpix,
            znaxis,
            tilesize,
            rice_blocksize,
            rice_bytepix,
            hcomp_scale,
            hcomp_smooth,
            quantize_method,
            no_quantize,
            dither_seed: self.get_i64("ZDITHER0").unwrap_or(1) as i32,
            bscale: self.get_f64("BSCALE").unwrap_or(1.0),
            bzero: self.get_f64("BZERO").unwrap_or(0.0),
        })
    }
}

impl FitsFile {
    /// `fits_set_compression_type`.
    pub fn set_compression_type(&mut self, ctype: i32) -> Result<()> {
        let kind = CompressionType::from_code(ctype).ok_or_else(|| {
            FitsError::with_message(
                DATA_COMPRESSION_ERR,
                "unknown compression algorithm (fits_set_compression_type)",
            )
        })?;
        let inner = self.inner_mut()?;
        inner.compression.kind = if ctype == 0 || ctype == NOCOMPRESS {
            None
        } else {
            Some(kind)
        };
        Ok(())
    }

    /// `fits_get_compression_type`.
    pub fn get_compression_type(&self) -> Result<i32> {
        Ok(self
            .inner()?
            .compression
            .kind
            .map_or(0, CompressionType::code))
    }

    /// `fits_set_tile_dim`.
    pub fn set_tile_dim(&mut self, dims: &[i64]) -> Result<()> {
        if dims.len() > MAX_COMPRESS_DIM {
            return Err(FitsError::new(BAD_DIMEN));
        }
        let inner = self.inner_mut()?;
        for (i, &d) in dims.iter().enumerate() {
            inner.compression.tilesize[i] = d;
        }
        Ok(())
    }

    /// `fits_get_tile_dim`.
    pub fn get_tile_dim(&self, ndim: usize) -> Result<Vec<i64>> {
        let c = &self.inner()?.compression;
        Ok(c.tilesize[..ndim.min(MAX_COMPRESS_DIM)].to_vec())
    }

    /// `fits_set_quantize_level`. `0.0` means lossless gzip of floats.
    pub fn set_quantize_level(&mut self, qlevel: f32) -> Result<()> {
        self.inner_mut()?.compression.quantize_level =
            if qlevel == 0.0 { NO_QUANTIZE } else { qlevel };
        Ok(())
    }

    /// `fits_get_quantize_level`.
    pub fn get_quantize_level(&self) -> Result<f32> {
        let q = self.inner()?.compression.quantize_level;
        Ok(if q == NO_QUANTIZE { 0.0 } else { q })
    }

    /// `fits_set_quantize_method`.
    pub fn set_quantize_method(&mut self, method: i32) -> Result<()> {
        self.inner_mut()?.compression.quantize_method = method;
        Ok(())
    }

    /// `fits_set_hcomp_scale`.
    pub fn set_hcomp_scale(&mut self, scale: f32) -> Result<()> {
        self.inner_mut()?.compression.hcomp_scale = scale;
        Ok(())
    }

    /// `fits_get_hcomp_scale`.
    pub fn get_hcomp_scale(&self) -> Result<f32> {
        Ok(self.inner()?.compression.hcomp_scale)
    }

    /// `fits_set_hcomp_smooth`.
    pub fn set_hcomp_smooth(&mut self, smooth: i32) -> Result<()> {
        self.inner_mut()?.compression.hcomp_smooth = smooth;
        Ok(())
    }

    /// `fits_is_compressed_image`.
    pub fn is_compressed_image(&self) -> Result<bool> {
        Ok(self.inner()?.hdus[self.inner()?.current]
            .header
            .is_compressed_image())
    }

    pub(crate) fn compression_requested(&self) -> bool {
        self.inner()
            .ok()
            .and_then(|i| i.compression.kind)
            .is_some_and(|k| k != CompressionType::None)
    }

    /// Create a ZIMAGE binary table for the next compressed image.
    pub(crate) fn create_compressed_image(&mut self, ty: ImageType, naxes: &[i64]) -> Result<()> {
        if matches!(ty, ImageType::I64 | ImageType::U64) {
            return Err(FitsError::new(BAD_DATATYPE));
        }
        let req = self.inner()?.compression.clone();
        let kind = req.kind.unwrap_or(CompressionType::Rice1);
        if ty.bitpix() < 0
            && req.quantize_level == NO_QUANTIZE
            && !matches!(kind, CompressionType::Gzip1 | CompressionType::Gzip2)
        {
            return Err(FitsError::with_message(
                DATA_COMPRESSION_ERR,
                "Lossless compression of floating point images must use GZIP (imcomp_init_table)",
            ));
        }
        let tiles = actual_tilesize(kind, naxes, &req.tilesize)?;
        let nrows = tile_count(naxes, &tiles);
        let bitpix = ty.bitpix();
        let quantize = bitpix < 0 && req.quantize_level != NO_QUANTIZE;
        let tform0 = if kind == CompressionType::Plio1 {
            "1PI"
        } else {
            "1PB"
        };
        let (ttype, tform): (Vec<&str>, Vec<&str>) = if quantize {
            (
                vec!["COMPRESSED_DATA", "ZSCALE", "ZZERO"],
                vec![tform0, "1D", "1D"],
            )
        } else {
            (vec!["COMPRESSED_DATA"], vec![tform0])
        };
        self.create_binary_table(nrows, &ttype, &tform, &[], None)?;
        {
            let h = self.header_mut()?;
            h.write_logical("ZIMAGE", true, Some("extension contains compressed image"))?;
            h.write_long(
                "ZBITPIX",
                i64::from(bitpix),
                Some("data type of original image"),
            )?;
            h.write_long(
                "ZNAXIS",
                naxes.len() as i64,
                Some("dimension of original image"),
            )?;
            for (i, &n) in naxes.iter().enumerate() {
                h.write_long(
                    &format!("ZNAXIS{}", i + 1),
                    n,
                    Some("length of original image axis"),
                )?;
            }
            for (i, &t) in tiles.iter().enumerate() {
                h.write_long(
                    &format!("ZTILE{}", i + 1),
                    t,
                    Some("size of tiles to be compressed"),
                )?;
            }
            if bitpix < 0 {
                if req.quantize_level == NO_QUANTIZE {
                    h.write_string(
                        "ZQUANTIZ",
                        "NONE",
                        Some("Lossless compression without quantization"),
                    )?;
                } else {
                    let method = if req.quantize_method == 0 {
                        SUBTRACTIVE_DITHER_1
                    } else {
                        req.quantize_method
                    };
                    let method =
                        if kind == CompressionType::Hcompress1 && method == SUBTRACTIVE_DITHER_2 {
                            SUBTRACTIVE_DITHER_1
                        } else {
                            method
                        };
                    let name = match method {
                        SUBTRACTIVE_DITHER_2 => "SUBTRACTIVE_DITHER_2",
                        NO_DITHER => "NO_DITHER",
                        _ => "SUBTRACTIVE_DITHER_1",
                    };
                    h.write_string("ZQUANTIZ", name, Some("Pixel Quantization Algorithm"))?;
                    if method != NO_DITHER {
                        h.write_long(
                            "ZDITHER0",
                            i64::from(req.dither_seed.max(1)),
                            Some("dithering offset when quantizing floats"),
                        )?;
                    }
                }
            }
            h.write_string("ZCMPTYPE", kind.zcmptype(), Some("compression algorithm"))?;
            if kind == CompressionType::Rice1 {
                h.write_string("ZNAME1", "BLOCKSIZE", Some("compression block size"))?;
                h.write_long("ZVAL1", 32, Some("pixels per block"))?;
                h.write_string("ZNAME2", "BYTEPIX", Some("bytes per pixel (1, 2, 4, or 8)"))?;
                let bp = match bitpix {
                    8 => 1,
                    16 => 2,
                    _ => 4,
                };
                h.write_long("ZVAL2", bp, Some("bytes per pixel (1, 2, 4, or 8)"))?;
            } else if kind == CompressionType::Hcompress1 {
                h.write_string("ZNAME1", "SCALE", Some("HCOMPRESS scale factor"))?;
                h.write_float_exp(
                    "ZVAL1",
                    f64::from(req.hcomp_scale),
                    7,
                    Some("HCOMPRESS scale factor"),
                )?;
                h.write_string("ZNAME2", "SMOOTH", Some("HCOMPRESS smooth option"))?;
                h.write_long(
                    "ZVAL2",
                    i64::from(req.hcomp_smooth),
                    Some("HCOMPRESS smooth option"),
                )?;
            }
            match ty {
                ImageType::U16 => {
                    h.write_fixed_double(
                        "BZERO",
                        32768.0,
                        0,
                        Some("offset data range to that of unsigned short"),
                    )?;
                    h.write_fixed_double("BSCALE", 1.0, 0, Some("default scaling factor"))?;
                }
                ImageType::I8 => {
                    h.write_fixed_double(
                        "BZERO",
                        -128.0,
                        0,
                        Some("offset data range to that of signed byte"),
                    )?;
                    h.write_fixed_double("BSCALE", 1.0, 0, Some("default scaling factor"))?;
                }
                ImageType::U32 => {
                    h.write_fixed_double(
                        "BZERO",
                        2_147_483_648.0,
                        0,
                        Some("offset data range to that of unsigned long"),
                    )?;
                    h.write_fixed_double("BSCALE", 1.0, 0, Some("default scaling factor"))?;
                }
                _ => {}
            }
        }
        self.flush()?;
        Ok(())
    }

    pub(crate) fn read_compressed_pixels<T: crate::image::Pixel>(
        &mut self,
        firstelem: i64,
        nelem: usize,
    ) -> Result<Vec<T>> {
        let info = {
            let inner = self.inner()?;
            let hdu = &inner.hdus[inner.current];
            let nrows = hdu.header.get_i64("NAXIS2").unwrap_or(0);
            hdu.header.zimage_info(nrows)?
        };
        let npix = info.npix();
        if firstelem < 1 || firstelem - 1 + nelem as i64 > npix {
            return Err(FitsError::new(crate::status::BAD_PIX_NUM));
        }
        let all = self.decompress_all(&info)?;
        let start = (firstelem - 1) as usize;
        let mut out = Vec::with_capacity(nelem);
        for chunk in all[start * info.bytes_per_pixel()..]
            .chunks_exact(info.bytes_per_pixel())
            .take(nelem)
        {
            out.push(crate::image::decode_pixel(
                chunk,
                info.image_type()?,
                info.bscale,
                info.bzero,
            )?);
        }
        Ok(out)
    }

    pub(crate) fn write_compressed_pixels<T: crate::image::Pixel>(
        &mut self,
        firstelem: i64,
        data: &[T],
    ) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        let info = {
            let inner = self.inner()?;
            let hdu = &inner.hdus[inner.current];
            let nrows = hdu.header.get_i64("NAXIS2").unwrap_or(0);
            hdu.header.zimage_info(nrows)?
        };
        let npix = info.npix() as usize;
        let bpp = info.bytes_per_pixel();
        let ty = info.image_type()?;
        if firstelem < 1 || firstelem as usize - 1 + data.len() > npix {
            return Err(FitsError::new(crate::status::BAD_PIX_NUM));
        }
        let mut raw = if firstelem == 1 && data.len() == npix {
            vec![0u8; npix * bpp]
        } else {
            self.decompress_all(&info)?
        };
        if raw.len() < npix * bpp {
            raw.resize(npix * bpp, 0);
        }
        let start = (firstelem - 1) as usize;
        for (i, &px) in data.iter().enumerate() {
            let enc = crate::image::encode_pixel(px, ty, info.bscale, info.bzero)?;
            let off = (start + i) * bpp;
            raw[off..off + bpp].copy_from_slice(&enc);
        }
        self.compress_all(&info, &raw)?;
        Ok(())
    }

    fn decompress_all(&mut self, info: &ZImageInfo) -> Result<Vec<u8>> {
        let npix = info.npix() as usize;
        let bpp = info.bytes_per_pixel();
        let mut out = vec![0u8; npix * bpp];
        let col = self
            .get_colnum(false, "COMPRESSED_DATA")
            .map_err(|_| FitsError::new(DATA_DECOMPRESSION_ERR))?;
        let zscale_col = self.get_colnum(false, "ZSCALE").ok();
        let zzero_col = self.get_colnum(false, "ZZERO").ok();
        let gzip_col = self.get_colnum(false, "GZIP_COMPRESSED_DATA").ok();
        let tiles = enumerate_tiles(&info.znaxis, &info.tilesize);
        let rand = if !info.no_quantize && info.zbitpix < 0 {
            Some(dither_table())
        } else {
            None
        };
        for (row, origin, shape) in tiles {
            let tile_npix: usize = shape.iter().map(|&d| d as usize).product();
            let bytes = self.read_vla_bytes(col, row)?;
            let (len, _) = self.read_descriptor(col, row)?;
            let tile = if len == 0 {
                if let Some(gc) = gzip_col {
                    let gz = self.read_vla_bytes(gc, row)?;
                    if gz.is_empty() {
                        vec![0u8; tile_npix * bpp]
                    } else {
                        decompress_gzip_tile(&gz, tile_npix, bpp, false)?
                    }
                } else {
                    vec![0u8; tile_npix * bpp]
                }
            } else {
                self.decompress_tile(
                    info,
                    row,
                    tile_npix,
                    &bytes,
                    &shape,
                    zscale_col,
                    zzero_col,
                    rand.as_deref(),
                )?
            };
            scatter_tile(&mut out, &info.znaxis, &origin, &shape, &tile, bpp);
        }
        Ok(out)
    }

    fn decompress_tile(
        &mut self,
        info: &ZImageInfo,
        row: i64,
        tile_npix: usize,
        bytes: &[u8],
        shape: &[i64],
        zscale_col: Option<i32>,
        zzero_col: Option<i32>,
        rand: Option<&[f32]>,
    ) -> Result<Vec<u8>> {
        let bpp = info.bytes_per_pixel();
        let mut scale = 1.0f64;
        let mut zero = 0.0f64;
        if let (Some(sc), Some(zc)) = (zscale_col, zzero_col) {
            scale = self
                .read_bin_col_f64(sc, row, 1, None)
                .ok()
                .and_then(|(v, _)| v.first().copied())
                .unwrap_or(1.0);
            zero = self
                .read_bin_col_f64(zc, row, 1, None)
                .ok()
                .and_then(|(v, _)| v.first().copied())
                .unwrap_or(0.0);
        }
        match info.kind {
            CompressionType::Rice1 => {
                let ints =
                    rice::decompress(bytes, tile_npix, info.rice_bytepix, info.rice_blocksize)?;
                ints_to_stored(&ints, info, scale, zero, row, rand)
            }
            CompressionType::Plio1 => {
                let words = be_i16_words(bytes);
                let ints = plio::decompress(&words, tile_npix)?;
                ints_to_stored(&ints, info, scale, zero, row, rand)
            }
            CompressionType::Hcompress1 => {
                let ny = shape.first().copied().unwrap_or(1) as i32;
                let nx = if shape.len() > 1 { shape[1] as i32 } else { 1 };
                let smooth = info.hcomp_smooth != 0;
                let (ints, _, _, _) = if info.zbitpix == 8 || info.zbitpix == 16 {
                    hcompress::decompress(bytes, tile_npix.max((nx * ny) as usize), smooth)?
                } else {
                    hcompress::decompress64(bytes, tile_npix.max((nx * ny) as usize), smooth)?
                };
                let take = ints.into_iter().take(tile_npix).collect::<Vec<_>>();
                ints_to_stored(&take, info, scale, zero, row, rand)
            }
            CompressionType::Gzip1 | CompressionType::Gzip2 => {
                let shuffle = info.kind == CompressionType::Gzip2;
                let raw = decompress_gzip_tile(bytes, tile_npix, bpp, shuffle)?;
                if info.zbitpix < 0 && !info.no_quantize {
                    let ints = be_i32_from_bytes(&raw);
                    ints_to_stored(&ints, info, scale, zero, row, rand)
                } else {
                    Ok(raw)
                }
            }
            CompressionType::None => Ok(bytes.to_vec()),
        }
    }

    fn compress_all(&mut self, info: &ZImageInfo, raw: &[u8]) -> Result<()> {
        let col = self.get_colnum(false, "COMPRESSED_DATA")?;
        let zscale_col = self.get_colnum(false, "ZSCALE").ok();
        let zzero_col = self.get_colnum(false, "ZZERO").ok();
        let bpp = info.bytes_per_pixel();
        let rand = if info.zbitpix < 0 && !info.no_quantize {
            Some(dither_table())
        } else {
            None
        };
        let qlevel = {
            let q = self.inner()?.compression.quantize_level;
            if q == 0.0 { 4.0 } else { q }
        };
        for (row, origin, shape) in enumerate_tiles(&info.znaxis, &info.tilesize) {
            let tile = gather_tile(raw, &info.znaxis, &origin, &shape, bpp);
            let tile_npix = tile.len() / bpp.max(1);
            let (payload, nelem, maybe_scale) =
                self.compress_tile(info, row, &tile, tile_npix, &shape, qlevel, rand.as_deref())?;
            if info.kind == CompressionType::Plio1 {
                self.write_vla_bytes(col, row, &payload, nelem)?;
            } else {
                self.write_vla_bytes(col, row, &payload, payload.len() as i64)?;
            }
            if let (Some(sc), Some(zc), Some((bscale, bzero))) =
                (zscale_col, zzero_col, maybe_scale)
            {
                self.write_bin_col_f64(sc, row, &[bscale])?;
                self.write_bin_col_f64(zc, row, &[bzero])?;
            }
        }
        Ok(())
    }

    fn compress_tile(
        &mut self,
        info: &ZImageInfo,
        row: i64,
        tile: &[u8],
        tile_npix: usize,
        shape: &[i64],
        qlevel: f32,
        rand: Option<&[f32]>,
    ) -> Result<(Vec<u8>, i64, Option<(f64, f64)>)> {
        let bpp = info.bytes_per_pixel();
        if info.zbitpix < 0 && info.no_quantize {
            let shuffle = info.kind == CompressionType::Gzip2;
            let payload = compress_gzip_tile(tile, bpp, shuffle)?;
            return Ok((payload, 0, None));
        }
        if info.zbitpix < 0 {
            let floats = match info.zbitpix {
                -32 => tile
                    .chunks_exact(4)
                    .map(|c| f64::from(f32::from_be_bytes(c.try_into().unwrap())))
                    .collect::<Vec<_>>(),
                _ => tile
                    .chunks_exact(8)
                    .map(|c| f64::from_be_bytes(c.try_into().unwrap()))
                    .collect::<Vec<_>>(),
            };
            let nx = shape.first().copied().unwrap_or(tile_npix as i64) as usize;
            let ny = tile_npix / nx.max(1);
            let dither_row = if info.quantize_method == NO_DITHER {
                0
            } else {
                row + i64::from(info.dither_seed.saturating_sub(1))
            };
            let q = quantize::quantize(
                &floats,
                nx,
                ny,
                qlevel,
                info.quantize_method,
                dither_row,
                rand.unwrap_or(&[]),
            );
            if let Some(q) = q {
                let payload = compress_ints(info, &q.idata, shape)?;
                let nelem = if info.kind == CompressionType::Plio1 {
                    (payload.len() / 2) as i64
                } else {
                    payload.len() as i64
                };
                return Ok((payload, nelem, Some((q.bscale, q.bzero))));
            }
            let payload = compress_gzip_tile(tile, bpp, info.kind == CompressionType::Gzip2)?;
            let n = payload.len() as i64;
            return Ok((payload, n, None));
        }
        let ints = stored_to_ints(tile, info.zbitpix);
        let payload = compress_ints(info, &ints, shape)?;
        let nelem = if info.kind == CompressionType::Plio1 {
            (payload.len() / 2) as i64
        } else {
            payload.len() as i64
        };
        Ok((payload, nelem, None))
    }
}

fn compress_ints(info: &ZImageInfo, ints: &[i32], shape: &[i64]) -> Result<Vec<u8>> {
    match info.kind {
        CompressionType::Rice1 => rice::compress(ints, info.rice_bytepix, info.rice_blocksize),
        CompressionType::Plio1 => {
            let words = plio::compress(ints)?;
            Ok(words.iter().flat_map(|w| w.to_be_bytes()).collect())
        }
        CompressionType::Hcompress1 => {
            let ny = shape.first().copied().unwrap_or(1) as i32;
            let nx = if shape.len() > 1 { shape[1] as i32 } else { 1 };
            let scale = if info.hcomp_scale < 0.0 {
                (-info.hcomp_scale + 0.5) as i32
            } else {
                0
            };
            if info.zbitpix == 8 || info.zbitpix == 16 {
                let mut a = ints.to_vec();
                a.resize((nx as usize) * (ny as usize).max(1), 0);
                hcompress::compress(&mut a, ny, nx, scale)
            } else {
                let mut a: Vec<i64> = ints.iter().copied().map(i64::from).collect();
                a.resize((nx as usize) * (ny as usize).max(1), 0);
                hcompress::compress64(&mut a, ny, nx, scale)
            }
        }
        CompressionType::Gzip1 | CompressionType::Gzip2 => {
            let bpp = info.bytes_per_pixel().min(4).max(if info.zbitpix == 8 {
                1
            } else if info.zbitpix == 16 {
                2
            } else {
                4
            });
            let bytes = ints_to_be(ints, bpp);
            compress_gzip_tile(&bytes, bpp, info.kind == CompressionType::Gzip2)
        }
        CompressionType::None => Ok(ints_to_be(ints, info.bytes_per_pixel())),
    }
}

fn ints_to_stored(
    ints: &[i32],
    info: &ZImageInfo,
    scale: f64,
    zero: f64,
    row: i64,
    rand: Option<&[f32]>,
) -> Result<Vec<u8>> {
    if info.zbitpix < 0 && (scale != 1.0 || zero != 0.0 || rand.is_some()) {
        let dither_row = if info.quantize_method == NO_DITHER {
            0
        } else {
            row + i64::from(info.dither_seed.saturating_sub(1))
        };
        let floats = quantize::dequantize(
            ints,
            scale,
            zero,
            info.quantize_method,
            dither_row,
            rand.unwrap_or(&[]),
        );
        let mut out = Vec::with_capacity(floats.len() * info.bytes_per_pixel());
        for f in floats {
            if info.zbitpix == -32 {
                out.extend_from_slice(&(f as f32).to_be_bytes());
            } else {
                out.extend_from_slice(&f.to_be_bytes());
            }
        }
        return Ok(out);
    }
    Ok(ints_to_be(ints, info.bytes_per_pixel()))
}

fn ints_to_be(ints: &[i32], bpp: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(ints.len() * bpp);
    for &v in ints {
        match bpp {
            1 => out.push(v as u8),
            2 => out.extend_from_slice(&(v as i16).to_be_bytes()),
            8 => out.extend_from_slice(&i64::from(v).to_be_bytes()),
            _ => out.extend_from_slice(&v.to_be_bytes()),
        }
    }
    out
}

fn stored_to_ints(tile: &[u8], zbitpix: i32) -> Vec<i32> {
    match zbitpix {
        8 => tile.iter().map(|&b| i32::from(b)).collect(),
        16 => tile
            .chunks_exact(2)
            .map(|c| i32::from(i16::from_be_bytes(c.try_into().unwrap())))
            .collect(),
        _ => tile
            .chunks_exact(4)
            .map(|c| i32::from_be_bytes(c.try_into().unwrap()))
            .collect(),
    }
}

fn be_i16_words(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_be_bytes(c.try_into().unwrap()))
        .collect()
}

fn be_i32_from_bytes(bytes: &[u8]) -> Vec<i32> {
    bytes
        .chunks_exact(4)
        .map(|c| i32::from_be_bytes(c.try_into().unwrap()))
        .collect()
}

fn compress_gzip_tile(tile: &[u8], width: usize, shuffle: bool) -> Result<Vec<u8>> {
    #[cfg(feature = "gzip")]
    {
        gzip_tile::compress_tile(tile, width, shuffle)
    }
    #[cfg(not(feature = "gzip"))]
    {
        let _ = (tile, width, shuffle);
        Err(FitsError::new(DATA_COMPRESSION_ERR))
    }
}

fn decompress_gzip_tile(bytes: &[u8], npix: usize, width: usize, shuffle: bool) -> Result<Vec<u8>> {
    #[cfg(feature = "gzip")]
    {
        gzip_tile::decompress_tile(bytes, npix, width, shuffle)
    }
    #[cfg(not(feature = "gzip"))]
    {
        let _ = (bytes, npix, width, shuffle);
        Err(FitsError::new(DATA_DECOMPRESSION_ERR))
    }
}

fn actual_tilesize(
    kind: CompressionType,
    naxes: &[i64],
    request: &[i64; MAX_COMPRESS_DIM],
) -> Result<Vec<i64>> {
    let naxis = naxes.len();
    if kind == CompressionType::Hcompress1 {
        if naxis < 2 {
            return Err(FitsError::with_message(
                DATA_COMPRESSION_ERR,
                "Hcompress cannot be used with 1-dimensional images (imcomp_init_table)",
            ));
        }
        let mut t = vec![0i64; naxis];
        t[0] = if request[0] <= 0 {
            naxes[0]
        } else {
            request[0]
        };
        if naxes[1] <= 30 {
            t[1] = naxes[1];
        } else {
            t[1] = 16;
        }
        for i in 2..naxis {
            t[i] = 1;
        }
        if t[0] < 4 || t[1] < 4 {
            t[0] = naxes[0].max(4);
            t[1] = naxes[1].max(4);
        }
        return Ok(t);
    }
    let mut t = Vec::with_capacity(naxis);
    for i in 0..naxis {
        let r = request.get(i).copied().unwrap_or(0);
        if i == 0 {
            t.push(if r <= 0 { naxes[i] } else { r });
        } else if r < 0 {
            t.push(naxes[i]);
        } else if r == 0 {
            t.push(1);
        } else {
            t.push(r);
        }
    }
    Ok(t)
}

fn tile_count(naxes: &[i64], tiles: &[i64]) -> i64 {
    naxes
        .iter()
        .zip(tiles)
        .map(|(&n, &t)| (n - 1) / t + 1)
        .product()
}

/// `(row, origin, shape)` for each tile, row-major with axis 0 fastest.
fn enumerate_tiles(naxes: &[i64], tiles: &[i64]) -> Vec<(i64, Vec<i64>, Vec<i64>)> {
    let naxis = naxes.len();
    let ntiles: Vec<i64> = naxes
        .iter()
        .zip(tiles)
        .map(|(&n, &t)| (n - 1) / t + 1)
        .collect();
    let total: i64 = ntiles.iter().product();
    let mut out = Vec::with_capacity(total as usize);
    for row0 in 0..total {
        let mut rest = row0;
        let mut idx = vec![0i64; naxis];
        // tiles are nested with axis 0 innermost (fastest), matching CFITSIO
        for i in 0..naxis {
            idx[i] = rest % ntiles[i];
            rest /= ntiles[i];
        }
        let mut origin = vec![0i64; naxis];
        let mut shape = vec![0i64; naxis];
        for i in 0..naxis {
            origin[i] = idx[i] * tiles[i];
            shape[i] = (origin[i] + tiles[i]).min(naxes[i]) - origin[i];
        }
        out.push((row0 + 1, origin, shape));
    }
    out
}

fn scatter_tile(
    dest: &mut [u8],
    naxes: &[i64],
    origin: &[i64],
    shape: &[i64],
    tile: &[u8],
    bpp: usize,
) {
    if naxes.len() == 1 {
        let off = origin[0] as usize * bpp;
        dest[off..off + tile.len()].copy_from_slice(tile);
        return;
    }
    let nx = naxes[0] as usize;
    let tx = shape[0] as usize;
    let n_rest: usize = shape.iter().skip(1).map(|&d| d as usize).product();
    for r in 0..n_rest {
        let mut coords = vec![0i64; naxes.len()];
        let mut rem = r as i64;
        for i in 1..naxes.len() {
            coords[i] = rem % shape[i];
            rem /= shape[i];
        }
        let mut dest_off = origin[0] as usize;
        let mut stride = nx;
        dest_off += (origin[1] as usize + coords[1] as usize) * stride;
        for i in 2..naxes.len() {
            stride *= naxes[i - 1] as usize;
            dest_off += (origin[i] as usize + coords[i] as usize) * stride;
        }
        dest_off *= bpp;
        let src_off = r * tx * bpp;
        dest[dest_off..dest_off + tx * bpp].copy_from_slice(&tile[src_off..src_off + tx * bpp]);
    }
}

/// `fits_set_compression_type`.
pub fn fits_set_compression_type(f: &mut FitsFile, ctype: i32) -> Result<()> {
    f.set_compression_type(ctype)
}

/// `fits_is_compressed_image`.
pub fn fits_is_compressed_image(f: &FitsFile) -> Result<bool> {
    f.is_compressed_image()
}

fn gather_tile(src: &[u8], naxes: &[i64], origin: &[i64], shape: &[i64], bpp: usize) -> Vec<u8> {
    let tile_npix: usize = shape.iter().map(|&d| d as usize).product();
    let mut tile = vec![0u8; tile_npix * bpp];
    if naxes.len() == 1 {
        let off = origin[0] as usize * bpp;
        let n = tile.len();
        tile.copy_from_slice(&src[off..off + n]);
        return tile;
    }
    let nx = naxes[0] as usize;
    let tx = shape[0] as usize;
    let n_rest: usize = shape.iter().skip(1).map(|&d| d as usize).product();
    for r in 0..n_rest {
        let mut coords = vec![0i64; naxes.len()];
        let mut rem = r as i64;
        for i in 1..naxes.len() {
            coords[i] = rem % shape[i];
            rem /= shape[i];
        }
        let mut src_off = origin[0] as usize;
        let mut stride = nx;
        src_off += (origin[1] as usize + coords[1] as usize) * stride;
        for i in 2..naxes.len() {
            stride *= naxes[i - 1] as usize;
            src_off += (origin[i] as usize + coords[i] as usize) * stride;
        }
        src_off *= bpp;
        let dest_off = r * tx * bpp;
        tile[dest_off..dest_off + tx * bpp].copy_from_slice(&src[src_off..src_off + tx * bpp]);
    }
    tile
}
