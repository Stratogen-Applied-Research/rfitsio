# rfitsio

Pure-Rust FITS I/O library. Behavioral oracle: **CFITSIO 4.7.0**.

This is a from-scratch implementation of the [FITS Standard 4.0](https://fits.gsfc.nasa.gov/standard40/fits_standard40aa-le.pdf) and of CFITSIO's documented interface. It does not link `libcfitsio` in the published crate. Tests compile a pinned copy of CFITSIO from `vendor/cfitsio` and compare status codes, `ffgerr` text, and (for writers) on-disk bytes.

License: Apache-2.0 OR MIT. Error-message wording reproduced from CFITSIO is covered by the NASA notice in `NOTICE`.

## Status

Layer A (core I/O) and Layer B (tiled image compression) are implemented:

- CFITSIO status codes and `ffgerr` text
- Header card format / parse / classify (`ffmkky`, `ffpsvc`, `ffgkcl`, `fftkey`)
- Disk and memory I/O, `FitsFile` create/open/close, byte-identical empty primary HDU
- Image HDUs: all BITPIX (including unsigned-via-BZERO), write/read, implicit conversion, subsets
- Header keyword CRUD (write/read/update/modify/insert/delete), DATE, units, `ffghpr`
- ASCII table HDUs: `TFORMn` `Aw`/`Iw`/`Fw.d`/`Ew.d`/`Dw.d`, `TBCOL` spacing, column I/O, `TNULL`, row/column insert-delete
- Binary table HDUs: `TFORMn` `L/X/B/I/J/K/A/E/D/C/M` plus unsigned `S/U/V/W`, column I/O, `P`/`Q` variable-length arrays and heap/`PCOUNT`
- HDU surgery: copy / delete / insert image and table HDUs (`ffcopy`, `ffcpfl`, `ffdhdu`, `ffiimg`, `ffitab`, `ffibin`)
- `CHECKSUM` / `DATASUM` (`ffpcks`, `ffvcks`, `ffgcks`, `ffesum`)
- gzip `.gz` read/write (pure-Rust `flate2`), stdin/stdout and `write_hdu_to` streams
- Cookbook example (`cargo run --example cookbook`) matching CFITSIO `cookbook.out`
- `testprog` gate: checksum encode/decode, WCS `-TAN`, missing-file and HDU-move status codes locked to `testprog.out`
- Tiled compressed images (`ZIMAGE`): `RICE_1`, `GZIP_1`, `GZIP_2`, `PLIO_1`, `HCOMPRESS_1`, lossless float gzip (`ZQUANTIZ=NONE`), and quantized floats with subtractive dither. Pixel identity after funpack / CFITSIO `fits_read_img` is the gate.

## Development

```bash
git submodule update --init --recursive
cargo test
```

Requires CMake, a C compiler, and zlib (to build the test-only oracle).

The published library forbids `unsafe`. Lib unit tests (`cargo +nightly miri test --lib`) are Miri-clean once the test-only `cfitsio-sys` oracle is not pulled in; integration tests that link the C oracle are not run under Miri.
