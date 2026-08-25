# rfitsio

Pure-Rust FITS I/O library. Behavioral oracle: **CFITSIO 4.7.0**.

This is a from-scratch implementation of the [FITS Standard 4.0](https://fits.gsfc.nasa.gov/standard40/fits_standard40aa-le.pdf) and of CFITSIO's documented interface. It does not link `libcfitsio` in the published crate. Tests compile a pinned copy of CFITSIO from `vendor/cfitsio` and compare status codes, `ffgerr` text, and (for writers) on-disk bytes.

License: Apache-2.0 OR MIT. Error-message wording reproduced from CFITSIO is covered by the NASA notice in `NOTICE`.

## Status

Layer A (core I/O) is under construction. Currently implemented:

- CFITSIO status codes and `ffgerr` text
- Header card format / parse / classify (`ffmkky`, `ffpsvc`, `ffgkcl`, `fftkey`)
- Disk and memory I/O, `FitsFile` create/open/close, byte-identical empty primary HDU

## Development

```bash
git submodule update --init --recursive
cargo test
```

Requires CMake, a C compiler, and zlib (to build the test-only oracle).
