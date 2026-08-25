# PSRFITS and pulsar I/O

`srfits` / `rfitsio` is a general FITS library (Layer A core I/O + Layer B
tiled `ZIMAGE` compression). PSRFITS is a FITS *convention* (primary header
plus named binary-table extensions), not a separate file format. This note
is the plan for making the crate usable on pulsar data.

Published crate: [`srfits`](https://crates.io/crates/srfits). Library name in
code remains `rfitsio`. Oracle: CFITSIO 4.7.0. Pixel/table identity after
CFITSIO read/write is the gate unless noted.

## PSRFITS layout (what pulsar code actually needs)

Typical HDUs:

| HDU | Role |
|---|---|
| Primary, `NAXIS=0` | Telescope, source, `STT_IMJD` / `STT_SMJD` / `STT_OFFS`, `OBSFREQ`, … |
| `HISTORY`, `PSRPARAM`, `POLYCO` / `T2PREDICT` | History, ephemeris, predictors |
| `SUBINT` | One row per subint; `DATA` is a large vector cell |

`DATA` is the load-bearing column:

- Fold / PSR / CAL: 16-bit `I`, `TDIM = (NBIN, NCHAN, NPOL)`
- Search: 8-bit `B` (or 1/2/4-bit packed *inside* those bytes),
  `TDIM = (NBITS-or-NBIN, NCHAN, NPOL, NSBLK)`

Channel metadata is also vector cells: `DAT_FREQ` `nchanE`, `DAT_WTS`,
`DAT_SCL`, `DAT_OFFS`. Physical sample is `raw * DAT_SCL + DAT_OFFS`.

FITS `TDIM` is Fortran order: the first axis is fastest.

Search-mode packing (`NBIT < 8`) is **not** FITS `X` bit columns. Samples
are packed into `B` bytes with earlier samples in the high-order bits
(Parkes / PSRFITS / SKA PST):

| NBIT | Bit7 (MSB) … Bit0 (LSB) |
|---:|---|
| 1 | val1 … val8 |
| 2 | val1, val2, val3, val4 |
| 4 | val1, val2 |
| 8 | val1 |

## Already fine

- Open PSRFITS, walk HDUs, `movnam("SUBINT")`
- Primary + extension keywords
- Scalar `SUBINT` columns (`TSUBINT` `1D`, `OFFS_SUB`, …)
- Copy/delete HDUs, gzip of modest files, `CHECKSUM`
- ASCII `T2PREDICT` / binary `HISTORY` / `POLYCO` as tables (vector cells
  need the I/O below)

## Caveats (work items)

### 1. Vector columns — CFITSIO `(firstrow, firstelem, nelem)` *(done)*

`write_col_*` / `read_col_*` used to index **rows**, not elements inside a
cell. CFITSIO flattens a column with repeat `R` as a stream of elements:
element `i` lives in row `i/R + 1` at offset `i % R` (1-based
`firstelem`). Writing 2048 floats to `2048E` must fill **one** row.

Scalar columns (`R = 1`) are unchanged. VLAs still write the whole heap
array for the given row.

### 2. `TDIM` stored, not applied *(done)*

`TDIMn = '(n1,n2,...)'` must parse/write like `ffgtdm` / `ffptdm`. Product
of axes equals the `TFORM` repeat. Helpers convert 1-based Fortran
coordinates to a 1-based vector element (and back) so `DATA` cubes can be
indexed as `(nbin, nchan, npol)` / `(nbit, nchan, npol, nsblk)`.

### 3. Packed 1/2/4-bit search data *(done)*

Application-level pack/unpack of `NBIT` samples in `DATA` bytes, MSB-first
as in the table above. Not FITS `X` columns.

### 4. Gzip fully inflated into RAM *(later)*

`.fits.gz` is decompressed into a memory backend. Search-mode files can be
tens of GB uncompressed. Uncompressed disk files seek; streaming gzip is
future work.

### 5. No row/column filters *(Layer C, later)*

No `fits_select` / iterator. Walk `NAXIS2` yourself.

### 6. Header card order / comments *(later)*

Keyword CRUD works. Byte-identical PSRCHIVE/DSPSR templates are not the
gate; “PSRCHIVE can open the file” is.

### 7. Not a CFITSIO C ABI *(out of scope for v1)*

Cannot `LD_PRELOAD` under PRESTO. New Rust (or a later FFI) only.

## Out of this PR (still planned)

- `DAT_SCL` / `DAT_OFFS` apply helper (uses items 1–3)
- Streaming gzip
- Layer C filters
- C ABI

## Verdict after items 1–3

Fold-mode and search-mode `SUBINT` cubes can be read and written as vector
cells with `TDIM` and optional `NBIT` unpack (`write_col_*_elem` /
`read_col_*_elem`, `read_tdim` / `write_tdim`, `pack_samples` /
`unpack_samples`). Large `.fits.gz` search files and PRESTO drop-in remain
later.

API sketch:

```rust
use rfitsio::{pack_samples, unpack_samples, tdim_elem};

f.write_col_f32(col_freq, row, &dat_freq)?;           // 2048E in one row
f.write_tdim(col_data, &[nbin, nchan, npol])?;
f.write_col_i64_elem(col_data, row, 1, &samples)?;    // full cell
let i = tdim_elem(&[nbin, nchan, npol], &[ibin, ichan, ipol])?;
let packed = pack_samples(&samples, nbit)?;
let samples = unpack_samples(&packed, nbit, nsamp)?;
```
