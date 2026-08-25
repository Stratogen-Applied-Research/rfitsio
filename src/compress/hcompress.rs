//! H-transform image compression (`HCOMPRESS_1`).
//!
//! Port of CFITSIO `fits_hcompress` / `fits_hdecompress` (STScI Hcompress).
//! `ny` is the fastest-varying axis (FITS `NAXIS1` / tile width). `scale == 0`
//! is lossless.

#![allow(clippy::needless_range_loop, clippy::collapsible_if)]

use crate::error::{FitsError, Result};
use crate::status::{DATA_COMPRESSION_ERR, DATA_DECOMPRESSION_ERR};

const CODE_MAGIC: [u8; 2] = [0xDD, 0x99];
const HUFF_CODE: [i32; 16] = [
    0x3e, 0x00, 0x01, 0x08, 0x02, 0x09, 0x1a, 0x1b, 0x03, 0x1c, 0x0a, 0x1d, 0x0b, 0x1e, 0x3f, 0x0c,
];
const HUFF_NCODE: [i32; 16] = [6, 3, 3, 4, 3, 4, 5, 5, 3, 5, 4, 5, 4, 5, 6, 4];

fn log2n_of(nmax: i32) -> i32 {
    let mut log2n = ((nmax as f32).ln() / 2f32.ln() + 0.5) as i32;
    if nmax > (1 << log2n) {
        log2n += 1;
    }
    log2n
}

/// Compress a 2-D `i32` tile. `ny` is the fastest axis.
pub fn compress(a: &mut [i32], ny: i32, nx: i32, scale: i32) -> Result<Vec<u8>> {
    htrans(a, nx, ny)?;
    digitize(a, nx, ny, scale);
    encode(a, nx, ny, scale)
}

/// Compress a 2-D `i64` tile (used for 32-bit FITS images).
pub fn compress64(a: &mut [i64], ny: i32, nx: i32, scale: i32) -> Result<Vec<u8>> {
    htrans64(a, nx, ny)?;
    digitize64(a, nx, ny, scale);
    encode64(a, nx, ny, scale)
}

/// Decompress into `i32` pixels. Returns `(pixels, ny, nx, scale)` where `ny`
/// is the fastest axis.
pub fn decompress(input: &[u8], na: usize, smooth: bool) -> Result<(Vec<i32>, i32, i32, i32)> {
    let (mut a, nx, ny, scale) = decode(input, na)?;
    undigitize(&mut a, nx, ny, scale);
    hinv(&mut a, nx, ny, smooth, scale)?;
    Ok((a, ny, nx, scale))
}

/// Decompress a 64-bit Hcompress stream and pack back to `i32`.
pub fn decompress64(input: &[u8], na: usize, smooth: bool) -> Result<(Vec<i32>, i32, i32, i32)> {
    let (mut a, nx, ny, scale) = decode64(input, na)?;
    undigitize64(&mut a, nx, ny, scale);
    hinv64(&mut a, nx, ny, smooth, scale)?;
    let nval = (nx as usize).saturating_mul(ny as usize);
    let out: Vec<i32> = a.into_iter().take(nval).map(|v| v as i32).collect();
    Ok((out, ny, nx, scale))
}

fn htrans(a: &mut [i32], nx: i32, ny: i32) -> Result<()> {
    let nmax = nx.max(ny);
    let log2n = log2n_of(nmax);
    let mut tmp = vec![0i32; ((nmax + 1) / 2) as usize];
    let mut shift = 0i32;
    let mut mask = -2i32;
    let mut mask2 = mask << 1;
    let mut prnd = 1i32;
    let mut prnd2 = prnd << 1;
    let mut nrnd2 = prnd2 - 1;
    let mut nxtop = nx;
    let mut nytop = ny;
    for _ in 0..log2n {
        let oddx = nxtop % 2;
        let oddy = nytop % 2;
        let mut i = 0;
        while i < nxtop - oddx {
            let mut s00 = (i * ny) as usize;
            let mut s10 = s00 + ny as usize;
            let mut j = 0;
            while j < nytop - oddy {
                let h0 = (a[s10 + 1] + a[s10] + a[s00 + 1] + a[s00]) >> shift;
                let hx = (a[s10 + 1] + a[s10] - a[s00 + 1] - a[s00]) >> shift;
                let hy = (a[s10 + 1] - a[s10] + a[s00 + 1] - a[s00]) >> shift;
                let hc = (a[s10 + 1] - a[s10] - a[s00 + 1] + a[s00]) >> shift;
                a[s10 + 1] = hc;
                a[s10] = (if hx >= 0 { hx + prnd } else { hx }) & mask;
                a[s00 + 1] = (if hy >= 0 { hy + prnd } else { hy }) & mask;
                a[s00] = (if h0 >= 0 { h0 + prnd2 } else { h0 + nrnd2 }) & mask2;
                s00 += 2;
                s10 += 2;
                j += 2;
            }
            if oddy != 0 {
                let h0 = (a[s10] + a[s00]) << (1 - shift);
                let hx = (a[s10] - a[s00]) << (1 - shift);
                a[s10] = (if hx >= 0 { hx + prnd } else { hx }) & mask;
                a[s00] = (if h0 >= 0 { h0 + prnd2 } else { h0 + nrnd2 }) & mask2;
            }
            i += 2;
        }
        if oddx != 0 {
            let mut s00 = (i * ny) as usize;
            let mut j = 0;
            while j < nytop - oddy {
                let h0 = (a[s00 + 1] + a[s00]) << (1 - shift);
                let hy = (a[s00 + 1] - a[s00]) << (1 - shift);
                a[s00 + 1] = (if hy >= 0 { hy + prnd } else { hy }) & mask;
                a[s00] = (if h0 >= 0 { h0 + prnd2 } else { h0 + nrnd2 }) & mask2;
                s00 += 2;
                j += 2;
            }
            if oddy != 0 {
                let h0 = a[s00] << (2 - shift);
                a[s00] = (if h0 >= 0 { h0 + prnd2 } else { h0 + nrnd2 }) & mask2;
            }
        }
        for ii in 0..nxtop {
            shuffle(&mut a[(ny * ii) as usize..], nytop, 1, &mut tmp);
        }
        for j in 0..nytop {
            shuffle(&mut a[j as usize..], nxtop, ny, &mut tmp);
        }
        nxtop = (nxtop + 1) >> 1;
        nytop = (nytop + 1) >> 1;
        shift = 1;
        mask = mask2;
        prnd = prnd2;
        mask2 <<= 1;
        prnd2 <<= 1;
        nrnd2 = prnd2 - 1;
    }
    let _ = DATA_COMPRESSION_ERR;
    Ok(())
}

fn htrans64(a: &mut [i64], nx: i32, ny: i32) -> Result<()> {
    let nmax = nx.max(ny);
    let log2n = log2n_of(nmax);
    let mut tmp = vec![0i64; ((nmax + 1) / 2) as usize];
    let mut shift = 0i32;
    let mut mask: i64 = -2;
    let mut mask2 = mask << 1;
    let mut prnd: i64 = 1;
    let mut prnd2 = prnd << 1;
    let mut nrnd2 = prnd2 - 1;
    let mut nxtop = nx;
    let mut nytop = ny;
    for _ in 0..log2n {
        let oddx = nxtop % 2;
        let oddy = nytop % 2;
        let mut i = 0;
        while i < nxtop - oddx {
            let mut s00 = (i * ny) as usize;
            let mut s10 = s00 + ny as usize;
            let mut j = 0;
            while j < nytop - oddy {
                let h0 = (a[s10 + 1] + a[s10] + a[s00 + 1] + a[s00]) >> shift;
                let hx = (a[s10 + 1] + a[s10] - a[s00 + 1] - a[s00]) >> shift;
                let hy = (a[s10 + 1] - a[s10] + a[s00 + 1] - a[s00]) >> shift;
                let hc = (a[s10 + 1] - a[s10] - a[s00 + 1] + a[s00]) >> shift;
                a[s10 + 1] = hc;
                a[s10] = (if hx >= 0 { hx + prnd } else { hx }) & mask;
                a[s00 + 1] = (if hy >= 0 { hy + prnd } else { hy }) & mask;
                a[s00] = (if h0 >= 0 { h0 + prnd2 } else { h0 + nrnd2 }) & mask2;
                s00 += 2;
                s10 += 2;
                j += 2;
            }
            if oddy != 0 {
                let h0 = (a[s10] + a[s00]) << (1 - shift);
                let hx = (a[s10] - a[s00]) << (1 - shift);
                a[s10] = (if hx >= 0 { hx + prnd } else { hx }) & mask;
                a[s00] = (if h0 >= 0 { h0 + prnd2 } else { h0 + nrnd2 }) & mask2;
            }
            i += 2;
        }
        if oddx != 0 {
            let mut s00 = (i * ny) as usize;
            let mut j = 0;
            while j < nytop - oddy {
                let h0 = (a[s00 + 1] + a[s00]) << (1 - shift);
                let hy = (a[s00 + 1] - a[s00]) << (1 - shift);
                a[s00 + 1] = (if hy >= 0 { hy + prnd } else { hy }) & mask;
                a[s00] = (if h0 >= 0 { h0 + prnd2 } else { h0 + nrnd2 }) & mask2;
                s00 += 2;
                j += 2;
            }
            if oddy != 0 {
                let h0 = a[s00] << (2 - shift);
                a[s00] = (if h0 >= 0 { h0 + prnd2 } else { h0 + nrnd2 }) & mask2;
            }
        }
        for ii in 0..nxtop {
            shuffle64(&mut a[(ny * ii) as usize..], nytop, 1, &mut tmp);
        }
        for j in 0..nytop {
            shuffle64(&mut a[j as usize..], nxtop, ny, &mut tmp);
        }
        nxtop = (nxtop + 1) >> 1;
        nytop = (nytop + 1) >> 1;
        shift = 1;
        mask = mask2;
        prnd = prnd2;
        mask2 <<= 1;
        prnd2 <<= 1;
        nrnd2 = prnd2 - 1;
    }
    Ok(())
}

fn shuffle(a: &mut [i32], n: i32, n2: i32, tmp: &mut [i32]) {
    let n2 = n2 as usize;
    let mut pt = 0usize;
    let mut p1 = n2;
    let mut i = 1i32;
    while i < n {
        tmp[pt] = a[p1];
        pt += 1;
        p1 += n2 + n2;
        i += 2;
    }
    p1 = n2;
    let mut p2 = n2 + n2;
    i = 2;
    while i < n {
        a[p1] = a[p2];
        p1 += n2;
        p2 += n2 + n2;
        i += 2;
    }
    pt = 0;
    i = 1;
    while i < n {
        a[p1] = tmp[pt];
        p1 += n2;
        pt += 1;
        i += 2;
    }
}

fn shuffle64(a: &mut [i64], n: i32, n2: i32, tmp: &mut [i64]) {
    let n2 = n2 as usize;
    let mut pt = 0usize;
    let mut p1 = n2;
    let mut i = 1i32;
    while i < n {
        tmp[pt] = a[p1];
        pt += 1;
        p1 += n2 + n2;
        i += 2;
    }
    p1 = n2;
    let mut p2 = n2 + n2;
    i = 2;
    while i < n {
        a[p1] = a[p2];
        p1 += n2;
        p2 += n2 + n2;
        i += 2;
    }
    pt = 0;
    i = 1;
    while i < n {
        a[p1] = tmp[pt];
        p1 += n2;
        pt += 1;
        i += 2;
    }
}

fn digitize(a: &mut [i32], nx: i32, ny: i32, scale: i32) {
    if scale <= 1 {
        return;
    }
    let d = (scale + 1) / 2 - 1;
    for p in a.iter_mut().take((nx * ny) as usize) {
        *p = if *p > 0 {
            (*p + d) / scale
        } else {
            (*p - d) / scale
        };
    }
}

fn digitize64(a: &mut [i64], nx: i32, ny: i32, scale: i32) {
    if scale <= 1 {
        return;
    }
    let d = i64::from((scale + 1) / 2 - 1);
    let scale64 = i64::from(scale);
    for p in a.iter_mut().take((nx * ny) as usize) {
        *p = if *p > 0 {
            (*p + d) / scale64
        } else {
            (*p - d) / scale64
        };
    }
}

fn undigitize(a: &mut [i32], nx: i32, ny: i32, scale: i32) {
    if scale <= 1 {
        return;
    }
    for p in a.iter_mut().take((nx * ny) as usize) {
        *p *= scale;
    }
}

fn undigitize64(a: &mut [i64], nx: i32, ny: i32, scale: i32) {
    if scale <= 1 {
        return;
    }
    let s = i64::from(scale);
    for p in a.iter_mut().take((nx * ny) as usize) {
        *p *= s;
    }
}

struct BitOut {
    buf: Vec<u8>,
    max: usize,
    buffer2: i32,
    bits_to_go2: i32,
    bitbuffer: i32,
    bits_to_go3: i32,
}

impl BitOut {
    fn new(max: usize) -> Self {
        Self {
            buf: Vec::with_capacity(max.min(1 << 20)),
            max,
            buffer2: 0,
            bits_to_go2: 8,
            bitbuffer: 0,
            bits_to_go3: 0,
        }
    }
    fn qwrite(&mut self, bytes: &[u8]) -> Result<()> {
        if self.buf.len() + bytes.len() > self.max {
            return Err(FitsError::with_message(
                DATA_COMPRESSION_ERR,
                "encode: output buffer too small",
            ));
        }
        self.buf.extend_from_slice(bytes);
        Ok(())
    }
    fn writeint(&mut self, mut a: i32) -> Result<()> {
        let mut b = [0u8; 4];
        for i in (0..4).rev() {
            b[i] = (a as u32 & 0xff) as u8;
            a >>= 8;
        }
        self.qwrite(&b)
    }
    fn writelonglong(&mut self, mut a: i64) -> Result<()> {
        let mut b = [0u8; 8];
        for i in (0..8).rev() {
            b[i] = (a as u64 & 0xff) as u8;
            a >>= 8;
        }
        self.qwrite(&b)
    }
    fn start_bits(&mut self) {
        self.buffer2 = 0;
        self.bits_to_go2 = 8;
    }
    fn output_nbits(&mut self, bits: i32, n: i32) {
        const MASK: [i32; 9] = [0, 1, 3, 7, 15, 31, 63, 127, 255];
        self.buffer2 <<= n;
        self.buffer2 |= bits & MASK[n as usize];
        self.bits_to_go2 -= n;
        if self.bits_to_go2 <= 0 {
            if self.buf.len() < self.max {
                self.buf
                    .push(((self.buffer2 >> (-self.bits_to_go2)) & 0xff) as u8);
            }
            self.bits_to_go2 += 8;
        }
    }
    fn output_nybble(&mut self, bits: i32) {
        self.buffer2 = (self.buffer2 << 4) | (bits & 15);
        self.bits_to_go2 -= 4;
        if self.bits_to_go2 <= 0 {
            if self.buf.len() < self.max {
                self.buf
                    .push(((self.buffer2 >> (-self.bits_to_go2)) & 0xff) as u8);
            }
            self.bits_to_go2 += 8;
        }
    }
    fn output_nnybble(&mut self, n: i32, array: &[u8]) {
        if n == 1 {
            self.output_nybble(i32::from(array[0]));
            return;
        }
        let mut kk = 0usize;
        if self.bits_to_go2 <= 4 {
            self.output_nybble(i32::from(array[0]));
            kk = 1;
            if n == 2 {
                self.output_nybble(i32::from(array[1]));
                return;
            }
        }
        let shift = 8 - self.bits_to_go2;
        let jj = (n as usize - kk) / 2;
        if self.bits_to_go2 == 8 {
            self.buffer2 = 0;
            for _ in 0..jj {
                if self.buf.len() < self.max {
                    self.buf
                        .push(((array[kk] & 15) << 4) | (array[kk + 1] & 15));
                }
                kk += 2;
            }
        } else {
            for _ in 0..jj {
                self.buffer2 = (self.buffer2 << 8)
                    | i32::from((array[kk] & 15) << 4)
                    | i32::from(array[kk + 1] & 15);
                kk += 2;
                if self.buf.len() < self.max {
                    self.buf.push(((self.buffer2 >> shift) & 0xff) as u8);
                }
            }
        }
        if kk != n as usize {
            self.output_nybble(i32::from(array[n as usize - 1]));
        }
    }
    fn done_bits(&mut self) {
        if self.bits_to_go2 < 8 && self.buf.len() < self.max {
            self.buf.push((self.buffer2 << self.bits_to_go2) as u8);
        }
    }
    fn output_huffman(&mut self, c: i32) {
        self.output_nbits(HUFF_CODE[c as usize], HUFF_NCODE[c as usize]);
    }
}

fn sign_bits<T: Copy + PartialOrd + Default>(
    a: &mut [T],
    nel: usize,
    zero: T,
    neg: impl Fn(T) -> T,
) -> Vec<u8> {
    let mut signbits = vec![0u8; nel.div_ceil(8)];
    let mut nsign = 0usize;
    let mut bits_to_go = 8i32;
    for i in 0..nel {
        if a[i] > zero {
            signbits[nsign] <<= 1;
            bits_to_go -= 1;
        } else if a[i] < zero {
            signbits[nsign] <<= 1;
            signbits[nsign] |= 1;
            bits_to_go -= 1;
            a[i] = neg(a[i]);
        }
        if bits_to_go == 0 {
            bits_to_go = 8;
            nsign += 1;
        }
    }
    if bits_to_go != 8 {
        signbits[nsign] <<= bits_to_go;
        nsign += 1;
    }
    signbits.truncate(nsign);
    signbits
}

fn encode(a: &mut [i32], nx: i32, ny: i32, scale: i32) -> Result<Vec<u8>> {
    let nel = (nx * ny) as usize;
    let mut out = BitOut::new((nel * 8 + 64).max(256));
    out.qwrite(&CODE_MAGIC)?;
    out.writeint(nx)?;
    out.writeint(ny)?;
    out.writeint(scale)?;
    out.writelonglong(i64::from(a[0]))?;
    a[0] = 0;
    let signs = sign_bits(a, nel, 0, |v| -v);
    let nbitplanes = nbitplanes_i32(a, nx, ny, nel);
    out.qwrite(&nbitplanes)?;
    doencode(&mut out, a, nx, ny, nbitplanes)?;
    if !signs.is_empty() {
        out.qwrite(&signs)?;
    }
    Ok(out.buf)
}

fn encode64(a: &mut [i64], nx: i32, ny: i32, scale: i32) -> Result<Vec<u8>> {
    let nel = (nx * ny) as usize;
    let mut out = BitOut::new((nel * 8 + 64).max(256));
    out.qwrite(&CODE_MAGIC)?;
    out.writeint(nx)?;
    out.writeint(ny)?;
    out.writeint(scale)?;
    out.writelonglong(a[0])?;
    a[0] = 0;
    let signs = sign_bits(a, nel, 0, |v| -v);
    let nbitplanes = nbitplanes_i64(a, nx, ny, nel);
    out.qwrite(&nbitplanes)?;
    doencode64(&mut out, a, nx, ny, nbitplanes)?;
    if !signs.is_empty() {
        out.qwrite(&signs)?;
    }
    Ok(out.buf)
}

fn nbitplanes_i32(a: &[i32], nx: i32, ny: i32, nel: usize) -> [u8; 3] {
    let nx2 = (nx + 1) / 2;
    let ny2 = (ny + 1) / 2;
    let mut vmax = [0i32; 3];
    let mut j = 0i32;
    let mut k = 0i32;
    for &v in a.iter().take(nel) {
        let q = i32::from(j >= ny2) + i32::from(k >= nx2);
        if vmax[q as usize] < v {
            vmax[q as usize] = v;
        }
        j += 1;
        if j >= ny {
            j = 0;
            k += 1;
        }
    }
    let mut nbitplanes = [0u8; 3];
    for q in 0..3 {
        let mut v = vmax[q];
        while v > 0 {
            nbitplanes[q] += 1;
            v >>= 1;
        }
    }
    nbitplanes
}

fn nbitplanes_i64(a: &[i64], nx: i32, ny: i32, nel: usize) -> [u8; 3] {
    let nx2 = (nx + 1) / 2;
    let ny2 = (ny + 1) / 2;
    let mut vmax = [0i64; 3];
    let mut j = 0i32;
    let mut k = 0i32;
    for &v in a.iter().take(nel) {
        let q = i32::from(j >= ny2) + i32::from(k >= nx2);
        if vmax[q as usize] < v {
            vmax[q as usize] = v;
        }
        j += 1;
        if j >= ny {
            j = 0;
            k += 1;
        }
    }
    let mut nbitplanes = [0u8; 3];
    for q in 0..3 {
        let mut v = vmax[q];
        while v > 0 {
            nbitplanes[q] += 1;
            v >>= 1;
        }
    }
    nbitplanes
}

fn doencode(out: &mut BitOut, a: &[i32], nx: i32, ny: i32, nbitplanes: [u8; 3]) -> Result<()> {
    let nx2 = (nx + 1) / 2;
    let ny2 = (ny + 1) / 2;
    out.start_bits();
    qtree_encode(out, a, ny, nx2, ny2, i32::from(nbitplanes[0]))?;
    qtree_encode(
        out,
        &a[ny2 as usize..],
        ny,
        nx2,
        ny / 2,
        i32::from(nbitplanes[1]),
    )?;
    qtree_encode(
        out,
        &a[(ny * nx2) as usize..],
        ny,
        nx / 2,
        ny2,
        i32::from(nbitplanes[1]),
    )?;
    qtree_encode(
        out,
        &a[(ny * nx2 + ny2) as usize..],
        ny,
        nx / 2,
        ny / 2,
        i32::from(nbitplanes[2]),
    )?;
    out.output_nybble(0);
    out.done_bits();
    Ok(())
}

fn doencode64(out: &mut BitOut, a: &[i64], nx: i32, ny: i32, nbitplanes: [u8; 3]) -> Result<()> {
    let nx2 = (nx + 1) / 2;
    let ny2 = (ny + 1) / 2;
    out.start_bits();
    qtree_encode64(out, a, ny, nx2, ny2, i32::from(nbitplanes[0]))?;
    qtree_encode64(
        out,
        &a[ny2 as usize..],
        ny,
        nx2,
        ny / 2,
        i32::from(nbitplanes[1]),
    )?;
    qtree_encode64(
        out,
        &a[(ny * nx2) as usize..],
        ny,
        nx / 2,
        ny2,
        i32::from(nbitplanes[1]),
    )?;
    qtree_encode64(
        out,
        &a[(ny * nx2 + ny2) as usize..],
        ny,
        nx / 2,
        ny / 2,
        i32::from(nbitplanes[2]),
    )?;
    out.output_nybble(0);
    out.done_bits();
    Ok(())
}

fn qtree_onebit(a: &[i32], n: i32, nx: i32, ny: i32, bit: i32, b: &mut [u8]) {
    let b0 = 1 << bit;
    let b1 = b0 << 1;
    let b2 = b0 << 2;
    let b3 = b0 << 3;
    let mut k = 0usize;
    let mut i = 0i32;
    while i < nx - 1 {
        let mut s00 = (n * i) as usize;
        let mut s10 = s00 + n as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            b[k] = (((a[s10 + 1] & b0)
                | ((a[s10] << 1) & b1)
                | ((a[s00 + 1] << 2) & b2)
                | ((a[s00] << 3) & b3))
                >> bit) as u8;
            k += 1;
            s00 += 2;
            s10 += 2;
            j += 2;
        }
        if j < ny {
            b[k] = ((((a[s10] << 1) & b1) | ((a[s00] << 3) & b3)) >> bit) as u8;
            k += 1;
        }
        i += 2;
    }
    if i < nx {
        let mut s00 = (n * i) as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            b[k] = ((((a[s00 + 1] << 2) & b2) | ((a[s00] << 3) & b3)) >> bit) as u8;
            k += 1;
            s00 += 2;
            j += 2;
        }
        if j < ny {
            b[k] = (((a[s00] << 3) & b3) >> bit) as u8;
        }
    }
}

fn qtree_onebit64(a: &[i64], n: i32, nx: i32, ny: i32, bit: i32, b: &mut [u8]) {
    let b0: i64 = 1 << bit;
    let b1 = b0 << 1;
    let b2 = b0 << 2;
    let b3 = b0 << 3;
    let mut k = 0usize;
    let mut i = 0i32;
    while i < nx - 1 {
        let mut s00 = (n * i) as usize;
        let mut s10 = s00 + n as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            b[k] = (((a[s10 + 1] & b0)
                | ((a[s10] << 1) & b1)
                | ((a[s00 + 1] << 2) & b2)
                | ((a[s00] << 3) & b3))
                >> bit) as u8;
            k += 1;
            s00 += 2;
            s10 += 2;
            j += 2;
        }
        if j < ny {
            b[k] = ((((a[s10] << 1) & b1) | ((a[s00] << 3) & b3)) >> bit) as u8;
            k += 1;
        }
        i += 2;
    }
    if i < nx {
        let mut s00 = (n * i) as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            b[k] = ((((a[s00 + 1] << 2) & b2) | ((a[s00] << 3) & b3)) >> bit) as u8;
            k += 1;
            s00 += 2;
            j += 2;
        }
        if j < ny {
            b[k] = (((a[s00] << 3) & b3) >> bit) as u8;
        }
    }
}

fn qtree_reduce(a: &[u8], n: i32, nx: i32, ny: i32, b: &mut [u8]) {
    let mut k = 0usize;
    let mut i = 0i32;
    while i < nx - 1 {
        let mut s00 = (n * i) as usize;
        let mut s10 = s00 + n as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            b[k] = u8::from(a[s10 + 1] != 0)
                | (u8::from(a[s10] != 0) << 1)
                | (u8::from(a[s00 + 1] != 0) << 2)
                | (u8::from(a[s00] != 0) << 3);
            k += 1;
            s00 += 2;
            s10 += 2;
            j += 2;
        }
        if j < ny {
            b[k] = (u8::from(a[s10] != 0) << 1) | (u8::from(a[s00] != 0) << 3);
            k += 1;
        }
        i += 2;
    }
    if i < nx {
        let mut s00 = (n * i) as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            b[k] = (u8::from(a[s00 + 1] != 0) << 2) | (u8::from(a[s00] != 0) << 3);
            k += 1;
            s00 += 2;
            j += 2;
        }
        if j < ny {
            b[k] = u8::from(a[s00] != 0) << 3;
        }
    }
}

fn bufcopy(
    out: &mut BitOut,
    a: &[u8],
    n: usize,
    buffer: &mut [u8],
    b: &mut usize,
    bmax: usize,
) -> bool {
    for &ai in a.iter().take(n) {
        if ai != 0 {
            out.bitbuffer |= HUFF_CODE[ai as usize] << out.bits_to_go3;
            out.bits_to_go3 += HUFF_NCODE[ai as usize];
            if out.bits_to_go3 >= 8 {
                buffer[*b] = (out.bitbuffer & 0xff) as u8;
                *b += 1;
                if *b >= bmax {
                    return true;
                }
                out.bitbuffer >>= 8;
                out.bits_to_go3 -= 8;
            }
        }
    }
    false
}

fn qtree_encode(
    out: &mut BitOut,
    a: &[i32],
    n: i32,
    nqx: i32,
    nqy: i32,
    nbitplanes: i32,
) -> Result<()> {
    if nqx <= 0 || nqy <= 0 {
        return Ok(());
    }
    let nqmax = nqx.max(nqy);
    let log2n = log2n_of(nqmax);
    let nqx2 = (nqx + 1) / 2;
    let nqy2 = (nqy + 1) / 2;
    let bmax = ((nqx2 * nqy2 + 1) / 2) as usize;
    let mut scratch = vec![0u8; (2 * bmax).max(1)];
    let mut buffer = vec![0u8; bmax.max(1)];
    for bit in (0..nbitplanes).rev() {
        let mut b = 0usize;
        out.bitbuffer = 0;
        out.bits_to_go3 = 0;
        qtree_onebit(a, n, nqx, nqy, bit, &mut scratch);
        let mut nx = (nqx + 1) >> 1;
        let mut ny = (nqy + 1) >> 1;
        let expand = bufcopy(out, &scratch, (nx * ny) as usize, &mut buffer, &mut b, bmax);
        let mut direct = expand;
        if !direct {
            for _ in 1..log2n {
                let copy = scratch.clone();
                qtree_reduce(&copy, ny, nx, ny, &mut scratch);
                nx = (nx + 1) >> 1;
                ny = (ny + 1) >> 1;
                if bufcopy(out, &scratch, (nx * ny) as usize, &mut buffer, &mut b, bmax) {
                    direct = true;
                    break;
                }
            }
        }
        if direct {
            out.output_nybble(0x0);
            qtree_onebit(a, n, nqx, nqy, bit, &mut scratch);
            out.output_nnybble(((nqx + 1) / 2) * ((nqy + 1) / 2), &scratch);
        } else {
            out.output_nybble(0xF);
            if b == 0 {
                if out.bits_to_go3 > 0 {
                    out.output_nbits(
                        out.bitbuffer & ((1 << out.bits_to_go3) - 1),
                        out.bits_to_go3,
                    );
                } else {
                    out.output_huffman(0);
                }
            } else {
                if out.bits_to_go3 > 0 {
                    out.output_nbits(
                        out.bitbuffer & ((1 << out.bits_to_go3) - 1),
                        out.bits_to_go3,
                    );
                }
                for i in (0..b).rev() {
                    out.output_nbits(i32::from(buffer[i]), 8);
                }
            }
        }
    }
    Ok(())
}

fn qtree_encode64(
    out: &mut BitOut,
    a: &[i64],
    n: i32,
    nqx: i32,
    nqy: i32,
    nbitplanes: i32,
) -> Result<()> {
    if nqx <= 0 || nqy <= 0 {
        return Ok(());
    }
    let nqmax = nqx.max(nqy);
    let log2n = log2n_of(nqmax);
    let nqx2 = (nqx + 1) / 2;
    let nqy2 = (nqy + 1) / 2;
    let bmax = ((nqx2 * nqy2 + 1) / 2) as usize;
    let mut scratch = vec![0u8; (2 * bmax).max(1)];
    let mut buffer = vec![0u8; bmax.max(1)];
    for bit in (0..nbitplanes).rev() {
        let mut b = 0usize;
        out.bitbuffer = 0;
        out.bits_to_go3 = 0;
        qtree_onebit64(a, n, nqx, nqy, bit, &mut scratch);
        let mut nx = (nqx + 1) >> 1;
        let mut ny = (nqy + 1) >> 1;
        let expand = bufcopy(out, &scratch, (nx * ny) as usize, &mut buffer, &mut b, bmax);
        let mut direct = expand;
        if !direct {
            for _ in 1..log2n {
                let copy = scratch.clone();
                qtree_reduce(&copy, ny, nx, ny, &mut scratch);
                nx = (nx + 1) >> 1;
                ny = (ny + 1) >> 1;
                if bufcopy(out, &scratch, (nx * ny) as usize, &mut buffer, &mut b, bmax) {
                    direct = true;
                    break;
                }
            }
        }
        if direct {
            out.output_nybble(0x0);
            qtree_onebit64(a, n, nqx, nqy, bit, &mut scratch);
            out.output_nnybble(((nqx + 1) / 2) * ((nqy + 1) / 2), &scratch);
        } else {
            out.output_nybble(0xF);
            if b == 0 {
                if out.bits_to_go3 > 0 {
                    out.output_nbits(
                        out.bitbuffer & ((1 << out.bits_to_go3) - 1),
                        out.bits_to_go3,
                    );
                } else {
                    out.output_huffman(0);
                }
            } else {
                if out.bits_to_go3 > 0 {
                    out.output_nbits(
                        out.bitbuffer & ((1 << out.bits_to_go3) - 1),
                        out.bits_to_go3,
                    );
                }
                for i in (0..b).rev() {
                    out.output_nbits(i32::from(buffer[i]), 8);
                }
            }
        }
    }
    Ok(())
}

struct BitIn<'a> {
    data: &'a [u8],
    next: usize,
    buffer2: i32,
    bits_to_go: i32,
}

impl<'a> BitIn<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            next: 0,
            buffer2: 0,
            bits_to_go: 0,
        }
    }
    fn qread(&mut self, n: usize) -> Result<Vec<u8>> {
        if self.next + n > self.data.len() {
            return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
        }
        let s = self.data[self.next..self.next + n].to_vec();
        self.next += n;
        Ok(s)
    }
    fn readint(&mut self) -> Result<i32> {
        let b = self.qread(4)?;
        Ok((i32::from(b[0]) << 24)
            | (i32::from(b[1]) << 16)
            | (i32::from(b[2]) << 8)
            | i32::from(b[3]))
    }
    fn readlonglong(&mut self) -> Result<i64> {
        let b = self.qread(8)?;
        let mut a = i64::from(b[0]);
        for i in 1..8 {
            a = (a << 8) + i64::from(b[i]);
        }
        Ok(a)
    }
    fn start_bits(&mut self) {
        self.bits_to_go = 0;
    }
    fn input_bit(&mut self) -> Result<i32> {
        if self.bits_to_go == 0 {
            if self.next >= self.data.len() {
                return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
            }
            self.buffer2 = i32::from(self.data[self.next]);
            self.next += 1;
            self.bits_to_go = 8;
        }
        self.bits_to_go -= 1;
        Ok((self.buffer2 >> self.bits_to_go) & 1)
    }
    fn input_nbits(&mut self, n: i32) -> Result<i32> {
        const MASK: [i32; 9] = [0, 1, 3, 7, 15, 31, 63, 127, 255];
        if self.bits_to_go < n {
            if self.next >= self.data.len() {
                return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
            }
            self.buffer2 = (self.buffer2 << 8) | i32::from(self.data[self.next]);
            self.next += 1;
            self.bits_to_go += 8;
        }
        self.bits_to_go -= n;
        Ok((self.buffer2 >> self.bits_to_go) & MASK[n as usize])
    }
    fn input_nybble(&mut self) -> Result<i32> {
        if self.bits_to_go < 4 {
            if self.next >= self.data.len() {
                return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
            }
            self.buffer2 = (self.buffer2 << 8) | i32::from(self.data[self.next]);
            self.next += 1;
            self.bits_to_go += 8;
        }
        self.bits_to_go -= 4;
        Ok((self.buffer2 >> self.bits_to_go) & 15)
    }
    fn input_nnybble(&mut self, n: i32, array: &mut [u8]) -> Result<()> {
        if n == 1 {
            array[0] = self.input_nybble()? as u8;
            return Ok(());
        }
        if self.bits_to_go == 8 {
            self.next -= 1;
            self.bits_to_go = 0;
        }
        let shift1 = self.bits_to_go + 4;
        let shift2 = self.bits_to_go;
        let mut kk = 0usize;
        if self.bits_to_go == 0 {
            for _ in 0..n / 2 {
                if self.next >= self.data.len() {
                    return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
                }
                self.buffer2 = (self.buffer2 << 8) | i32::from(self.data[self.next]);
                self.next += 1;
                array[kk] = ((self.buffer2 >> 4) & 15) as u8;
                array[kk + 1] = (self.buffer2 & 15) as u8;
                kk += 2;
            }
        } else {
            for _ in 0..n / 2 {
                if self.next >= self.data.len() {
                    return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
                }
                self.buffer2 = (self.buffer2 << 8) | i32::from(self.data[self.next]);
                self.next += 1;
                array[kk] = ((self.buffer2 >> shift1) & 15) as u8;
                array[kk + 1] = ((self.buffer2 >> shift2) & 15) as u8;
                kk += 2;
            }
        }
        if (n / 2) * 2 != n {
            array[n as usize - 1] = self.input_nybble()? as u8;
        }
        Ok(())
    }
    fn input_huffman(&mut self) -> Result<i32> {
        let mut c = self.input_nbits(3)?;
        if c < 4 {
            return Ok(1 << c);
        }
        c = self.input_bit()? | (c << 1);
        if c < 13 {
            return Ok(match c {
                8 => 3,
                9 => 5,
                10 => 10,
                11 => 12,
                12 => 15,
                _ => c,
            });
        }
        c = self.input_bit()? | (c << 1);
        if c < 31 {
            return Ok(match c {
                26 => 6,
                27 => 7,
                28 => 9,
                29 => 11,
                30 => 13,
                _ => c,
            });
        }
        c = self.input_bit()? | (c << 1);
        if c == 62 { Ok(0) } else { Ok(14) }
    }
}

fn decode(input: &[u8], na: usize) -> Result<(Vec<i32>, i32, i32, i32)> {
    let mut inp = BitIn::new(input);
    let magic = inp.qread(2)?;
    if magic != CODE_MAGIC {
        return Err(FitsError::with_message(
            DATA_DECOMPRESSION_ERR,
            "bad file format",
        ));
    }
    let nx = inp.readint()?;
    let ny = inp.readint()?;
    let scale = inp.readint()?;
    if nx <= 0 || ny <= 0 || (nx as usize).saturating_mul(ny as usize) > na {
        return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
    }
    let sumall = inp.readlonglong()?;
    let planes = inp.qread(3)?;
    let nbitplanes = [planes[0], planes[1], planes[2]];
    let mut a = vec![0i32; na.max((nx * ny) as usize)];
    dodecode(&mut inp, &mut a, nx, ny, nbitplanes)?;
    a[0] = sumall as i32;
    Ok((a, nx, ny, scale))
}

fn decode64(input: &[u8], na: usize) -> Result<(Vec<i64>, i32, i32, i32)> {
    let mut inp = BitIn::new(input);
    let magic = inp.qread(2)?;
    if magic != CODE_MAGIC {
        return Err(FitsError::with_message(
            DATA_DECOMPRESSION_ERR,
            "bad file format",
        ));
    }
    let nx = inp.readint()?;
    let ny = inp.readint()?;
    let scale = inp.readint()?;
    if nx <= 0 || ny <= 0 || (nx as usize).saturating_mul(ny as usize) > na {
        return Err(FitsError::new(DATA_DECOMPRESSION_ERR));
    }
    let sumall = inp.readlonglong()?;
    let planes = inp.qread(3)?;
    let nbitplanes = [planes[0], planes[1], planes[2]];
    let mut a = vec![0i64; na.max((nx * ny) as usize)];
    dodecode64(&mut inp, &mut a, nx, ny, nbitplanes)?;
    a[0] = sumall;
    Ok((a, nx, ny, scale))
}

fn dodecode(
    inp: &mut BitIn<'_>,
    a: &mut [i32],
    nx: i32,
    ny: i32,
    nbitplanes: [u8; 3],
) -> Result<()> {
    let nel = (nx * ny) as usize;
    a[..nel].fill(0);
    let nx2 = (nx + 1) / 2;
    let ny2 = (ny + 1) / 2;
    inp.start_bits();
    qtree_decode(inp, a, ny, nx2, ny2, i32::from(nbitplanes[0]))?;
    qtree_decode(
        inp,
        &mut a[ny2 as usize..],
        ny,
        nx2,
        ny / 2,
        i32::from(nbitplanes[1]),
    )?;
    qtree_decode(
        inp,
        &mut a[(ny * nx2) as usize..],
        ny,
        nx / 2,
        ny2,
        i32::from(nbitplanes[1]),
    )?;
    qtree_decode(
        inp,
        &mut a[(ny * nx2 + ny2) as usize..],
        ny,
        nx / 2,
        ny / 2,
        i32::from(nbitplanes[2]),
    )?;
    if inp.input_nybble()? != 0 {
        return Err(FitsError::with_message(
            DATA_DECOMPRESSION_ERR,
            "dodecode: bad bit plane values",
        ));
    }
    inp.start_bits();
    for v in a.iter_mut().take(nel) {
        if *v != 0 && inp.input_bit()? != 0 {
            *v = -*v;
        }
    }
    Ok(())
}

fn dodecode64(
    inp: &mut BitIn<'_>,
    a: &mut [i64],
    nx: i32,
    ny: i32,
    nbitplanes: [u8; 3],
) -> Result<()> {
    let nel = (nx * ny) as usize;
    a[..nel].fill(0);
    let nx2 = (nx + 1) / 2;
    let ny2 = (ny + 1) / 2;
    inp.start_bits();
    qtree_decode64(inp, a, ny, nx2, ny2, i32::from(nbitplanes[0]))?;
    qtree_decode64(
        inp,
        &mut a[ny2 as usize..],
        ny,
        nx2,
        ny / 2,
        i32::from(nbitplanes[1]),
    )?;
    qtree_decode64(
        inp,
        &mut a[(ny * nx2) as usize..],
        ny,
        nx / 2,
        ny2,
        i32::from(nbitplanes[1]),
    )?;
    qtree_decode64(
        inp,
        &mut a[(ny * nx2 + ny2) as usize..],
        ny,
        nx / 2,
        ny / 2,
        i32::from(nbitplanes[2]),
    )?;
    if inp.input_nybble()? != 0 {
        return Err(FitsError::with_message(
            DATA_DECOMPRESSION_ERR,
            "dodecode64: bad bit plane values",
        ));
    }
    inp.start_bits();
    for v in a.iter_mut().take(nel) {
        if *v != 0 && inp.input_bit()? != 0 {
            *v = -*v;
        }
    }
    Ok(())
}

fn qtree_bitins(a: &[u8], nx: i32, ny: i32, b: &mut [i32], n: i32, bit: i32) {
    let plane = 1 << bit;
    let mut k = 0usize;
    let mut i = 0i32;
    while i < nx - 1 {
        let mut s00 = (n * i) as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            let v = a[k];
            if v & 1 != 0 {
                b[s00 + n as usize + 1] |= plane;
            }
            if v & 2 != 0 {
                b[s00 + n as usize] |= plane;
            }
            if v & 4 != 0 {
                b[s00 + 1] |= plane;
            }
            if v & 8 != 0 {
                b[s00] |= plane;
            }
            s00 += 2;
            k += 1;
            j += 2;
        }
        if j < ny {
            let v = a[k];
            if v & 2 != 0 {
                b[s00 + n as usize] |= plane;
            }
            if v & 8 != 0 {
                b[s00] |= plane;
            }
            k += 1;
        }
        i += 2;
    }
    if i < nx {
        let mut s00 = (n * i) as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            let v = a[k];
            if v & 4 != 0 {
                b[s00 + 1] |= plane;
            }
            if v & 8 != 0 {
                b[s00] |= plane;
            }
            s00 += 2;
            k += 1;
            j += 2;
        }
        if j < ny {
            if a[k] & 8 != 0 {
                b[s00] |= plane;
            }
        }
    }
}

fn qtree_bitins64(a: &[u8], nx: i32, ny: i32, b: &mut [i64], n: i32, bit: i32) {
    let plane: i64 = 1 << bit;
    let mut k = 0usize;
    let mut i = 0i32;
    while i < nx - 1 {
        let mut s00 = (n * i) as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            let v = a[k];
            if v & 1 != 0 {
                b[s00 + n as usize + 1] |= plane;
            }
            if v & 2 != 0 {
                b[s00 + n as usize] |= plane;
            }
            if v & 4 != 0 {
                b[s00 + 1] |= plane;
            }
            if v & 8 != 0 {
                b[s00] |= plane;
            }
            s00 += 2;
            k += 1;
            j += 2;
        }
        if j < ny {
            let v = a[k];
            if v & 2 != 0 {
                b[s00 + n as usize] |= plane;
            }
            if v & 8 != 0 {
                b[s00] |= plane;
            }
            k += 1;
        }
        i += 2;
    }
    if i < nx {
        let mut s00 = (n * i) as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            let v = a[k];
            if v & 4 != 0 {
                b[s00 + 1] |= plane;
            }
            if v & 8 != 0 {
                b[s00] |= plane;
            }
            s00 += 2;
            k += 1;
            j += 2;
        }
        if j < ny && a[k] & 8 != 0 {
            b[s00] |= plane;
        }
    }
}

fn qtree_copy(a: &[u8], nx: i32, ny: i32, b: &mut [u8], n: i32) {
    let nx2 = (nx + 1) / 2;
    let ny2 = (ny + 1) / 2;
    let mut k = (ny2 * (nx2 - 1) + ny2 - 1) as usize;
    for i in (0..nx2).rev() {
        let mut s00 = (2 * (n * i + ny2 - 1)) as usize;
        for _ in (0..ny2).rev() {
            b[s00] = a[k];
            k = k.saturating_sub(1);
            s00 = s00.saturating_sub(2);
        }
    }
    let mut i = 0i32;
    while i < nx - 1 {
        let mut s00 = (n * i) as usize;
        let mut s10 = s00 + n as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            let v = b[s00];
            b[s10 + 1] = v & 1;
            b[s10] = (v >> 1) & 1;
            b[s00 + 1] = (v >> 2) & 1;
            b[s00] = (v >> 3) & 1;
            s00 += 2;
            s10 += 2;
            j += 2;
        }
        if j < ny {
            b[s10] = (b[s00] >> 1) & 1;
            b[s00] = (b[s00] >> 3) & 1;
        }
        i += 2;
    }
    if i < nx {
        let mut s00 = (n * i) as usize;
        let mut j = 0i32;
        while j < ny - 1 {
            b[s00 + 1] = (b[s00] >> 2) & 1;
            b[s00] = (b[s00] >> 3) & 1;
            s00 += 2;
            j += 2;
        }
        if j < ny {
            b[s00] = (b[s00] >> 3) & 1;
        }
    }
}

fn qtree_expand(inp: &mut BitIn<'_>, a: &[u8], nx: i32, ny: i32, b: &mut [u8]) -> Result<()> {
    qtree_copy(a, nx, ny, b, ny);
    for i in (0..(nx * ny) as usize).rev() {
        if b[i] != 0 {
            b[i] = inp.input_huffman()? as u8;
        }
    }
    Ok(())
}

fn qtree_decode(
    inp: &mut BitIn<'_>,
    a: &mut [i32],
    n: i32,
    nqx: i32,
    nqy: i32,
    nbitplanes: i32,
) -> Result<()> {
    if nqx <= 0 || nqy <= 0 {
        return Ok(());
    }
    let nqmax = nqx.max(nqy);
    let log2n = log2n_of(nqmax);
    let nqx2 = (nqx + 1) / 2;
    let nqy2 = (nqy + 1) / 2;
    let mut scratch = vec![0u8; (nqx2 * nqy2).max(1) as usize];
    for bit in (0..nbitplanes).rev() {
        let b = inp.input_nybble()?;
        if b == 0 {
            inp.input_nnybble(((nqx + 1) / 2) * ((nqy + 1) / 2), &mut scratch)?;
            qtree_bitins(&scratch, nqx, nqy, a, n, bit);
        } else if b != 0xf {
            return Err(FitsError::with_message(
                DATA_DECOMPRESSION_ERR,
                "qtree_decode: bad format code",
            ));
        } else {
            scratch[0] = inp.input_huffman()? as u8;
            let mut nx = 1i32;
            let mut ny = 1i32;
            let mut nfx = nqx;
            let mut nfy = nqy;
            let mut c = 1 << log2n;
            for _ in 1..log2n {
                c >>= 1;
                nx <<= 1;
                ny <<= 1;
                if nfx <= c {
                    nx -= 1;
                } else {
                    nfx -= c;
                }
                if nfy <= c {
                    ny -= 1;
                } else {
                    nfy -= c;
                }
                let copy = scratch.clone();
                qtree_expand(inp, &copy, nx, ny, &mut scratch)?;
            }
            qtree_bitins(&scratch, nqx, nqy, a, n, bit);
        }
    }
    Ok(())
}

fn qtree_decode64(
    inp: &mut BitIn<'_>,
    a: &mut [i64],
    n: i32,
    nqx: i32,
    nqy: i32,
    nbitplanes: i32,
) -> Result<()> {
    if nqx <= 0 || nqy <= 0 {
        return Ok(());
    }
    let nqmax = nqx.max(nqy);
    let log2n = log2n_of(nqmax);
    let nqx2 = (nqx + 1) / 2;
    let nqy2 = (nqy + 1) / 2;
    let mut scratch = vec![0u8; (nqx2 * nqy2).max(1) as usize];
    for bit in (0..nbitplanes).rev() {
        let b = inp.input_nybble()?;
        if b == 0 {
            inp.input_nnybble(((nqx + 1) / 2) * ((nqy + 1) / 2), &mut scratch)?;
            qtree_bitins64(&scratch, nqx, nqy, a, n, bit);
        } else if b != 0xf {
            return Err(FitsError::with_message(
                DATA_DECOMPRESSION_ERR,
                "qtree_decode64: bad format code",
            ));
        } else {
            scratch[0] = inp.input_huffman()? as u8;
            let mut nx = 1i32;
            let mut ny = 1i32;
            let mut nfx = nqx;
            let mut nfy = nqy;
            let mut c = 1 << log2n;
            for _ in 1..log2n {
                c >>= 1;
                nx <<= 1;
                ny <<= 1;
                if nfx <= c {
                    nx -= 1;
                } else {
                    nfx -= c;
                }
                if nfy <= c {
                    ny -= 1;
                } else {
                    nfy -= c;
                }
                let copy = scratch.clone();
                qtree_expand(inp, &copy, nx, ny, &mut scratch)?;
            }
            qtree_bitins64(&scratch, nqx, nqy, a, n, bit);
        }
    }
    Ok(())
}

fn unshuffle(a: &mut [i32], n: i32, n2: i32, tmp: &mut [i32]) {
    let n2u = n2 as usize;
    let nhalf = (n + 1) >> 1;
    let mut pt = 0usize;
    let mut p1 = n2u * nhalf as usize;
    for _ in nhalf..n {
        tmp[pt] = a[p1];
        p1 += n2u;
        pt += 1;
    }
    let mut p2 = n2u * (nhalf as usize - 1);
    p1 = (n2u * (nhalf as usize - 1)) << 1;
    for _ in (0..nhalf).rev() {
        a[p1] = a[p2];
        p2 = p2.saturating_sub(n2u);
        p1 = p1.saturating_sub(n2u + n2u);
    }
    pt = 0;
    p1 = n2u;
    let mut i = 1i32;
    while i < n {
        a[p1] = tmp[pt];
        p1 += n2u + n2u;
        pt += 1;
        i += 2;
    }
}

fn unshuffle64(a: &mut [i64], n: i32, n2: i32, tmp: &mut [i64]) {
    let n2u = n2 as usize;
    let nhalf = (n + 1) >> 1;
    let mut pt = 0usize;
    let mut p1 = n2u * nhalf as usize;
    for _ in nhalf..n {
        tmp[pt] = a[p1];
        p1 += n2u;
        pt += 1;
    }
    let mut p2 = n2u * (nhalf as usize - 1);
    p1 = (n2u * (nhalf as usize - 1)) << 1;
    for _ in (0..nhalf).rev() {
        a[p1] = a[p2];
        p2 = p2.saturating_sub(n2u);
        p1 = p1.saturating_sub(n2u + n2u);
    }
    pt = 0;
    p1 = n2u;
    let mut i = 1i32;
    while i < n {
        a[p1] = tmp[pt];
        p1 += n2u + n2u;
        pt += 1;
        i += 2;
    }
}

fn hinv(a: &mut [i32], nx: i32, ny: i32, _smooth: bool, _scale: i32) -> Result<()> {
    let nmax = nx.max(ny);
    let log2n = log2n_of(nmax);
    let mut tmp = vec![0i32; ((nmax + 1) / 2) as usize];
    let mut shift = 1i32;
    let mut bit0 = 1 << (log2n - 1);
    let mut bit1 = bit0 << 1;
    let bit2_init = bit0 << 2;
    let mut mask0 = -bit0;
    let mut mask1 = mask0 << 1;
    let mask2 = mask0 << 2;
    let mut prnd0 = bit0 >> 1;
    let mut prnd1 = bit1 >> 1;
    let prnd2 = bit2_init >> 1;
    let mut nrnd0 = prnd0 - 1;
    let mut nrnd1 = prnd1 - 1;
    let nrnd2 = prnd2 - 1;
    a[0] = (a[0] + if a[0] >= 0 { prnd2 } else { nrnd2 }) & mask2;
    let mut nxtop = 1i32;
    let mut nytop = 1i32;
    let mut nxf = nx;
    let mut nyf = ny;
    let mut c = 1 << log2n;
    for k in (0..log2n).rev() {
        c >>= 1;
        nxtop <<= 1;
        nytop <<= 1;
        if nxf <= c {
            nxtop -= 1;
        } else {
            nxf -= c;
        }
        if nyf <= c {
            nytop -= 1;
        } else {
            nyf -= c;
        }
        if k == 0 {
            nrnd0 = 0;
            shift = 2;
        }
        for i in 0..nxtop {
            unshuffle(&mut a[(ny * i) as usize..], nytop, 1, &mut tmp);
        }
        for j in 0..nytop {
            unshuffle(&mut a[j as usize..], nxtop, ny, &mut tmp);
        }
        let oddx = nxtop % 2;
        let oddy = nytop % 2;
        let mut i = 0i32;
        while i < nxtop - oddx {
            let mut s00 = (ny * i) as usize;
            let mut s10 = s00 + ny as usize;
            let mut j = 0i32;
            while j < nytop - oddy {
                let mut h0 = a[s00];
                let mut hx = a[s10];
                let mut hy = a[s00 + 1];
                let mut hc = a[s10 + 1];
                hx = (hx + if hx >= 0 { prnd1 } else { nrnd1 }) & mask1;
                hy = (hy + if hy >= 0 { prnd1 } else { nrnd1 }) & mask1;
                hc = (hc + if hc >= 0 { prnd0 } else { nrnd0 }) & mask0;
                let lowbit0 = hc & bit0;
                hx = if hx >= 0 { hx - lowbit0 } else { hx + lowbit0 };
                hy = if hy >= 0 { hy - lowbit0 } else { hy + lowbit0 };
                let lowbit1 = (hc ^ hx ^ hy) & bit1;
                h0 = if h0 >= 0 {
                    h0 + lowbit0 - lowbit1
                } else {
                    h0 + if lowbit0 == 0 {
                        lowbit1
                    } else {
                        lowbit0 - lowbit1
                    }
                };
                a[s10 + 1] = (h0 + hx + hy + hc) >> shift;
                a[s10] = (h0 + hx - hy - hc) >> shift;
                a[s00 + 1] = (h0 - hx + hy - hc) >> shift;
                a[s00] = (h0 - hx - hy + hc) >> shift;
                s00 += 2;
                s10 += 2;
                j += 2;
            }
            if oddy != 0 {
                let mut h0 = a[s00];
                let mut hx = a[s10];
                hx = (if hx >= 0 { hx + prnd1 } else { hx + nrnd1 }) & mask1;
                let lowbit1 = hx & bit1;
                h0 = if h0 >= 0 { h0 - lowbit1 } else { h0 + lowbit1 };
                a[s10] = (h0 + hx) >> shift;
                a[s00] = (h0 - hx) >> shift;
            }
            i += 2;
        }
        if oddx != 0 {
            let mut s00 = (ny * i) as usize;
            let mut j = 0i32;
            while j < nytop - oddy {
                let mut h0 = a[s00];
                let mut hy = a[s00 + 1];
                hy = (if hy >= 0 { hy + prnd1 } else { hy + nrnd1 }) & mask1;
                let lowbit1 = hy & bit1;
                h0 = if h0 >= 0 { h0 - lowbit1 } else { h0 + lowbit1 };
                a[s00 + 1] = (h0 + hy) >> shift;
                a[s00] = (h0 - hy) >> shift;
                s00 += 2;
                j += 2;
            }
            if oddy != 0 {
                a[s00] >>= shift;
            }
        }
        bit1 = bit0;
        bit0 >>= 1;
        mask1 = mask0;
        mask0 >>= 1;
        prnd1 = prnd0;
        prnd0 >>= 1;
        nrnd1 = nrnd0;
        nrnd0 = prnd0 - 1;
    }
    Ok(())
}

fn hinv64(a: &mut [i64], nx: i32, ny: i32, _smooth: bool, _scale: i32) -> Result<()> {
    let nmax = nx.max(ny);
    let log2n = log2n_of(nmax);
    let mut tmp = vec![0i64; ((nmax + 1) / 2) as usize];
    let mut shift = 1i32;
    let mut bit0: i64 = 1 << (log2n - 1);
    let mut bit1 = bit0 << 1;
    let bit2_init = bit0 << 2;
    let mut mask0 = -bit0;
    let mut mask1 = mask0 << 1;
    let mask2 = mask0 << 2;
    let mut prnd0 = bit0 >> 1;
    let mut prnd1 = bit1 >> 1;
    let prnd2 = bit2_init >> 1;
    let mut nrnd0 = prnd0 - 1;
    let mut nrnd1 = prnd1 - 1;
    let nrnd2 = prnd2 - 1;
    a[0] = (a[0] + if a[0] >= 0 { prnd2 } else { nrnd2 }) & mask2;
    let mut nxtop = 1i32;
    let mut nytop = 1i32;
    let mut nxf = nx;
    let mut nyf = ny;
    let mut c = 1 << log2n;
    for k in (0..log2n).rev() {
        c >>= 1;
        nxtop <<= 1;
        nytop <<= 1;
        if nxf <= c {
            nxtop -= 1;
        } else {
            nxf -= c;
        }
        if nyf <= c {
            nytop -= 1;
        } else {
            nyf -= c;
        }
        if k == 0 {
            nrnd0 = 0;
            shift = 2;
        }
        for i in 0..nxtop {
            unshuffle64(&mut a[(ny * i) as usize..], nytop, 1, &mut tmp);
        }
        for j in 0..nytop {
            unshuffle64(&mut a[j as usize..], nxtop, ny, &mut tmp);
        }
        let oddx = nxtop % 2;
        let oddy = nytop % 2;
        let mut i = 0i32;
        while i < nxtop - oddx {
            let mut s00 = (ny * i) as usize;
            let mut s10 = s00 + ny as usize;
            let mut j = 0i32;
            while j < nytop - oddy {
                let mut h0 = a[s00];
                let mut hx = a[s10];
                let mut hy = a[s00 + 1];
                let mut hc = a[s10 + 1];
                hx = (hx + if hx >= 0 { prnd1 } else { nrnd1 }) & mask1;
                hy = (hy + if hy >= 0 { prnd1 } else { nrnd1 }) & mask1;
                hc = (hc + if hc >= 0 { prnd0 } else { nrnd0 }) & mask0;
                let lowbit0 = hc & bit0;
                hx = if hx >= 0 { hx - lowbit0 } else { hx + lowbit0 };
                hy = if hy >= 0 { hy - lowbit0 } else { hy + lowbit0 };
                let lowbit1 = (hc ^ hx ^ hy) & bit1;
                h0 = if h0 >= 0 {
                    h0 + lowbit0 - lowbit1
                } else {
                    h0 + if lowbit0 == 0 {
                        lowbit1
                    } else {
                        lowbit0 - lowbit1
                    }
                };
                a[s10 + 1] = (h0 + hx + hy + hc) >> shift;
                a[s10] = (h0 + hx - hy - hc) >> shift;
                a[s00 + 1] = (h0 - hx + hy - hc) >> shift;
                a[s00] = (h0 - hx - hy + hc) >> shift;
                s00 += 2;
                s10 += 2;
                j += 2;
            }
            if oddy != 0 {
                let mut h0 = a[s00];
                let mut hx = a[s10];
                hx = (if hx >= 0 { hx + prnd1 } else { hx + nrnd1 }) & mask1;
                let lowbit1 = hx & bit1;
                h0 = if h0 >= 0 { h0 - lowbit1 } else { h0 + lowbit1 };
                a[s10] = (h0 + hx) >> shift;
                a[s00] = (h0 - hx) >> shift;
            }
            i += 2;
        }
        if oddx != 0 {
            let mut s00 = (ny * i) as usize;
            let mut j = 0i32;
            while j < nytop - oddy {
                let mut h0 = a[s00];
                let mut hy = a[s00 + 1];
                hy = (if hy >= 0 { hy + prnd1 } else { hy + nrnd1 }) & mask1;
                let lowbit1 = hy & bit1;
                h0 = if h0 >= 0 { h0 - lowbit1 } else { h0 + lowbit1 };
                a[s00 + 1] = (h0 + hy) >> shift;
                a[s00] = (h0 - hy) >> shift;
                s00 += 2;
                j += 2;
            }
            if oddy != 0 {
                a[s00] >>= shift;
            }
        }
        bit1 = bit0;
        bit0 >>= 1;
        mask1 = mask0;
        mask0 >>= 1;
        prnd1 = prnd0;
        prnd0 >>= 1;
        nrnd1 = nrnd0;
        nrnd0 = prnd0 - 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hcompress_lossless_roundtrip() {
        let nx = 8i32;
        let ny = 8i32;
        let orig: Vec<i32> = (0..64).map(|i| (i % 17) - 4).collect();
        let mut a = orig.clone();
        let bytes = compress(&mut a, ny, nx, 0).unwrap();
        let (out, _, _, _) = decompress(&bytes, 64, false).unwrap();
        assert_eq!(&out[..64], orig.as_slice());
    }
}
