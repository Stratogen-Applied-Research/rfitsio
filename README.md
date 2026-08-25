# rfitsio

Pure-Rust FITS I/O library. Behavioral oracle: **CFITSIO 4.7.0**.

This is a from-scratch implementation of the [FITS Standard 4.0](https://fits.gsfc.nasa.gov/standard40/fits_standard40aa-le.pdf) and of CFITSIO's documented interface. It does not link `libcfitsio` in the published crate. Tests compile a pinned copy of CFITSIO from `vendor/cfitsio` and compare status codes, `ffgerr` text, and (for writers) on-disk bytes.

License: Apache-2.0 OR MIT. Error-message wording reproduced from CFITSIO is covered by the NASA notice in `NOTICE`.

## Status

Layer A (core I/O) is under construction. Currently implemented:

- CFITSIO status codes and `ffgerr` text
- Header card format / parse / classify (`ffmkky`, `ffpsvc`, `ffgkcl`, `fftkey`)
- Disk and memory I/O, `FitsFile` create/open/close, byte-identical empty primary HDU
- Image HDUs: all BITPIX (including unsigned-via-BZERO), write/read, implicit conversion, subsets
- Header keyword CRUD (write/read/update/modify/insert/delete), DATE, units, `ffghpr`
- ASCII table HDUs: `TFORMn` `Aw`/`Iw`/`Fw.d`/`Ew.d`/`Dw.d`, `TBCOL` spacing, column I/O, `TNULL`, row/column insert-delete
- Binary table HDUs: `TFORMn` `L/X/B/I/J/K/A/E/D/C/M` plus unsigned `S/U/V/W`, column I/O, `P`/`Q` variable-length arrays and heap/`PCOUNT`

## Development

```bash
git submodule update --init --recursive
cargo test
```

Requires CMake, a C compiler, and zlib (to build the test-only oracle).
