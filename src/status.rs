//! CFITSIO 4.7.0 error status codes and `ffgerr` text.
//!
//! Numeric values and short messages match `fitsio.h` / `ffgerr` in
//! CFITSIO 4.7.0. Codes with no `ffgerr` case return
//! `"unknown error status"`, including several named constants that
//! CFITSIO defines but does not describe (e.g. `NO_COMPRESSED_TILE`).

/// Max length of an `ffgerr` message, excluding the trailing NUL (`FLEN_STATUS`).
pub const FLEN_STATUS: usize = 31;

// Control / non-error codes used as flags when opening files.
pub const CREATE_DISK_FILE: i32 = -106;
pub const OPEN_DISK_FILE: i32 = -105;
pub const SKIP_TABLE: i32 = -104;
pub const SKIP_IMAGE: i32 = -103;
pub const SKIP_NULL_PRIMARY: i32 = -102;
pub const USE_MEM_BUFF: i32 = -101;
pub const OVERFLOW_ERR: i32 = -11;
pub const PREPEND_PRIMARY: i32 = -9;

pub const SAME_FILE: i32 = 101;
pub const TOO_MANY_FILES: i32 = 103;
pub const FILE_NOT_OPENED: i32 = 104;
pub const FILE_NOT_CREATED: i32 = 105;
pub const WRITE_ERROR: i32 = 106;
pub const END_OF_FILE: i32 = 107;
pub const READ_ERROR: i32 = 108;
pub const FILE_NOT_CLOSED: i32 = 110;
pub const ARRAY_TOO_BIG: i32 = 111;
pub const READONLY_FILE: i32 = 112;
pub const MEMORY_ALLOCATION: i32 = 113;
pub const BAD_FILEPTR: i32 = 114;
pub const NULL_INPUT_PTR: i32 = 115;
pub const SEEK_ERROR: i32 = 116;
pub const BAD_NETTIMEOUT: i32 = 117;

pub const BAD_URL_PREFIX: i32 = 121;
pub const TOO_MANY_DRIVERS: i32 = 122;
pub const DRIVER_INIT_FAILED: i32 = 123;
pub const NO_MATCHING_DRIVER: i32 = 124;
pub const URL_PARSE_ERROR: i32 = 125;
pub const RANGE_PARSE_ERROR: i32 = 126;

pub const SHARED_ERRBASE: i32 = 150;
pub const SHARED_BADARG: i32 = 151;
pub const SHARED_NULPTR: i32 = 152;
pub const SHARED_TABFULL: i32 = 153;
pub const SHARED_NOTINIT: i32 = 154;
pub const SHARED_IPCERR: i32 = 155;
pub const SHARED_NOMEM: i32 = 156;
pub const SHARED_AGAIN: i32 = 157;
pub const SHARED_NOFILE: i32 = 158;
pub const SHARED_NORESIZE: i32 = 159;

pub const HEADER_NOT_EMPTY: i32 = 201;
pub const KEY_NO_EXIST: i32 = 202;
pub const KEY_OUT_BOUNDS: i32 = 203;
pub const VALUE_UNDEFINED: i32 = 204;
pub const NO_QUOTE: i32 = 205;
pub const BAD_INDEX_KEY: i32 = 206;
pub const BAD_KEYCHAR: i32 = 207;
pub const BAD_ORDER: i32 = 208;
pub const NOT_POS_INT: i32 = 209;
pub const NO_END: i32 = 210;
pub const BAD_BITPIX: i32 = 211;
pub const BAD_NAXIS: i32 = 212;
pub const BAD_NAXES: i32 = 213;
pub const BAD_PCOUNT: i32 = 214;
pub const BAD_GCOUNT: i32 = 215;
pub const BAD_TFIELDS: i32 = 216;
pub const NEG_WIDTH: i32 = 217;
pub const NEG_ROWS: i32 = 218;
pub const COL_NOT_FOUND: i32 = 219;
pub const BAD_SIMPLE: i32 = 220;
pub const NO_SIMPLE: i32 = 221;
pub const NO_BITPIX: i32 = 222;
pub const NO_NAXIS: i32 = 223;
pub const NO_NAXES: i32 = 224;
pub const NO_XTENSION: i32 = 225;
pub const NOT_ATABLE: i32 = 226;
pub const NOT_BTABLE: i32 = 227;
pub const NO_PCOUNT: i32 = 228;
pub const NO_GCOUNT: i32 = 229;
pub const NO_TFIELDS: i32 = 230;
pub const NO_TBCOL: i32 = 231;
pub const NO_TFORM: i32 = 232;
pub const NOT_IMAGE: i32 = 233;
pub const BAD_TBCOL: i32 = 234;
pub const NOT_TABLE: i32 = 235;
pub const COL_TOO_WIDE: i32 = 236;
pub const COL_NOT_UNIQUE: i32 = 237;
pub const BAD_ROW_WIDTH: i32 = 241;
pub const UNKNOWN_EXT: i32 = 251;
pub const UNKNOWN_REC: i32 = 252;
pub const END_JUNK: i32 = 253;
pub const BAD_HEADER_FILL: i32 = 254;
pub const BAD_DATA_FILL: i32 = 255;
pub const BAD_TFORM: i32 = 261;
pub const BAD_TFORM_DTYPE: i32 = 262;
pub const BAD_TDIM: i32 = 263;
pub const BAD_HEAP_PTR: i32 = 264;

pub const BAD_HDU_NUM: i32 = 301;
pub const BAD_COL_NUM: i32 = 302;
pub const NEG_FILE_POS: i32 = 304;
pub const NEG_BYTES: i32 = 306;
pub const BAD_ROW_NUM: i32 = 307;
pub const BAD_ELEM_NUM: i32 = 308;
pub const NOT_ASCII_COL: i32 = 309;
pub const NOT_LOGICAL_COL: i32 = 310;
pub const BAD_ATABLE_FORMAT: i32 = 311;
pub const BAD_BTABLE_FORMAT: i32 = 312;
pub const NO_NULL: i32 = 314;
pub const NOT_VARI_LEN: i32 = 317;
pub const BAD_DIMEN: i32 = 320;
pub const BAD_PIX_NUM: i32 = 321;
pub const ZERO_SCALE: i32 = 322;
pub const NEG_AXIS: i32 = 323;

pub const NOT_GROUP_TABLE: i32 = 340;
pub const HDU_ALREADY_MEMBER: i32 = 341;
pub const MEMBER_NOT_FOUND: i32 = 342;
pub const GROUP_NOT_FOUND: i32 = 343;
pub const BAD_GROUP_ID: i32 = 344;
pub const TOO_MANY_HDUS_TRACKED: i32 = 345;
pub const HDU_ALREADY_TRACKED: i32 = 346;
pub const BAD_OPTION: i32 = 347;
pub const IDENTICAL_POINTERS: i32 = 348;
pub const BAD_GROUP_ATTACH: i32 = 349;
pub const BAD_GROUP_DETACH: i32 = 350;

pub const NGP_ERRBASE: i32 = 360;
pub const NGP_NO_MEMORY: i32 = 360;
pub const NGP_READ_ERR: i32 = 361;
pub const NGP_NUL_PTR: i32 = 362;
pub const NGP_EMPTY_CURLINE: i32 = 363;
pub const NGP_UNREAD_QUEUE_FULL: i32 = 364;
pub const NGP_INC_NESTING: i32 = 365;
pub const NGP_ERR_FOPEN: i32 = 366;
pub const NGP_EOF: i32 = 367;
pub const NGP_BAD_ARG: i32 = 368;
pub const NGP_TOKEN_NOT_EXPECT: i32 = 369;

pub const BAD_I2C: i32 = 401;
pub const BAD_F2C: i32 = 402;
pub const BAD_INTKEY: i32 = 403;
pub const BAD_LOGICALKEY: i32 = 404;
pub const BAD_FLOATKEY: i32 = 405;
pub const BAD_DOUBLEKEY: i32 = 406;
pub const BAD_C2I: i32 = 407;
pub const BAD_C2F: i32 = 408;
pub const BAD_C2D: i32 = 409;
pub const BAD_DATATYPE: i32 = 410;
pub const BAD_DECIM: i32 = 411;
pub const NUM_OVERFLOW: i32 = 412;
pub const DATA_COMPRESSION_ERR: i32 = 413;
pub const DATA_DECOMPRESSION_ERR: i32 = 414;
pub const NO_COMPRESSED_TILE: i32 = 415;

pub const BAD_DATE: i32 = 420;

pub const PARSE_SYNTAX_ERR: i32 = 431;
pub const PARSE_BAD_TYPE: i32 = 432;
pub const PARSE_LRG_VECTOR: i32 = 433;
pub const PARSE_NO_OUTPUT: i32 = 434;
pub const PARSE_BAD_COL: i32 = 435;
pub const PARSE_BAD_OUTPUT: i32 = 436;

pub const ANGLE_TOO_BIG: i32 = 501;
pub const BAD_WCS_VAL: i32 = 502;
pub const WCS_ERROR: i32 = 503;
pub const BAD_WCS_PROJ: i32 = 504;
pub const NO_WCS_KEY: i32 = 505;
pub const APPROX_WCS_KEY: i32 = 506;

pub const NO_CLOSE_ERROR: i32 = 999;

const UNKNOWN: &str = "unknown error status";

/// Return the CFITSIO `ffgerr` message for `status`.
///
/// Matches CFITSIO 4.7.0, including `"unknown error status"` for codes
/// that the C `switch` does not name.
#[must_use]
pub fn status_text(status: i32) -> &'static str {
    // Mirrors fitscore.c ffgerr: [0, 300) in the first switch, else
    // status < 600 in the second, else unknown. Negative codes (except
    // those listed in the second switch, of which there are none) are
    // unknown because they fail the first range test and hit default
    // in the second.
    if (0..300).contains(&status) {
        match status {
            0 => "OK - no error",
            1 => "non-CFITSIO program error",
            101 => "same input and output files",
            103 => "attempt to open too many files",
            104 => "could not open the named file",
            105 => "couldn't create the named file",
            106 => "error writing to FITS file",
            107 => "tried to move past end of file",
            108 => "error reading from FITS file",
            110 => "could not close the file",
            111 => "array dimensions too big",
            112 => "cannot write to readonly file",
            113 => "could not allocate memory",
            114 => "invalid fitsfile pointer",
            115 => "NULL input pointer",
            116 => "error seeking file position",
            117 => "bad value for file download timeout setting",
            121 => "invalid URL prefix",
            122 => "too many I/O drivers",
            123 => "I/O driver init failed",
            124 => "no I/O driver for this URLtype",
            125 => "parse error in input file URL",
            126 => "parse error in range list",
            151 => "bad argument (shared mem drvr)",
            152 => "null ptr arg (shared mem drvr)",
            153 => "no free shared memory handles",
            154 => "share mem drvr not initialized",
            155 => "IPC system error (shared mem)",
            156 => "no memory (shared mem drvr)",
            157 => "share mem resource deadlock",
            158 => "lock file open/create failed",
            159 => "can't resize share mem block",
            201 => "header already has keywords",
            202 => "keyword not found in header",
            203 => "keyword number out of bounds",
            204 => "keyword value is undefined",
            205 => "string missing closing quote",
            206 => "error in indexed keyword name",
            207 => "illegal character in keyword",
            208 => "required keywords out of order",
            209 => "keyword value not positive int",
            210 => "END keyword not found",
            211 => "illegal BITPIX keyword value",
            212 => "illegal NAXIS keyword value",
            213 => "illegal NAXISn keyword value",
            214 => "illegal PCOUNT keyword value",
            215 => "illegal GCOUNT keyword value",
            216 => "illegal TFIELDS keyword value",
            217 => "negative table row size",
            218 => "negative number of rows",
            219 => "named column not found",
            220 => "illegal SIMPLE keyword value",
            221 => "first keyword not SIMPLE",
            222 => "second keyword not BITPIX",
            223 => "third keyword not NAXIS",
            224 => "missing NAXISn keywords",
            225 => "first keyword not XTENSION",
            226 => "CHDU not an ASCII table",
            227 => "CHDU not a binary table",
            228 => "PCOUNT keyword not found",
            229 => "GCOUNT keyword not found",
            230 => "TFIELDS keyword not found",
            231 => "missing TBCOLn keyword",
            232 => "missing TFORMn keyword",
            233 => "CHDU not an IMAGE extension",
            234 => "illegal TBCOLn keyword value",
            235 => "CHDU not a table extension",
            236 => "column exceeds width of table",
            237 => "more than 1 matching col. name",
            241 => "row width not = field widths",
            251 => "unknown FITS extension type",
            252 => "1st key not SIMPLE or XTENSION",
            253 => "END keyword is not blank",
            254 => "Header fill area not blank",
            255 => "Data fill area invalid",
            261 => "illegal TFORM format code",
            262 => "unknown TFORM datatype code",
            263 => "illegal TDIMn keyword value",
            264 => "invalid BINTABLE heap pointer",
            _ => UNKNOWN,
        }
    } else if status < 600 {
        match status {
            301 => "illegal HDU number",
            302 => "column number < 1 or > tfields",
            304 => "negative byte address",
            306 => "negative number of elements",
            307 => "bad first row number",
            308 => "bad first element number",
            309 => "not an ASCII (A) column",
            310 => "not a logical (L) column",
            311 => "bad ASCII table datatype",
            312 => "bad binary table datatype",
            314 => "null value not defined",
            317 => "not a variable length column",
            320 => "illegal number of dimensions",
            321 => "1st pixel no. > last pixel no.",
            322 => "BSCALE or TSCALn = 0.",
            323 => "illegal axis length < 1",
            340 => "not group table",
            341 => "HDU already member of group",
            342 => "group member not found",
            343 => "group not found",
            344 => "bad group id",
            345 => "too many HDUs tracked",
            346 => "HDU alread tracked",
            347 => "bad Grouping option",
            348 => "identical pointers (groups)",
            360 => "malloc failed in parser",
            361 => "file read error in parser",
            362 => "null pointer arg (parser)",
            363 => "empty line (parser)",
            364 => "cannot unread > 1 line",
            365 => "parser too deeply nested",
            366 => "file open failed (parser)",
            367 => "hit EOF (parser)",
            368 => "bad argument (parser)",
            369 => "unexpected token (parser)",
            401 => "bad int to string conversion",
            402 => "bad float to string conversion",
            403 => "keyword value not integer",
            404 => "keyword value not logical",
            405 => "keyword value not floating pt",
            406 => "keyword value not double",
            407 => "bad string to int conversion",
            408 => "bad string to float conversion",
            409 => "bad string to double convert",
            410 => "illegal datatype code value",
            411 => "illegal no. of decimals",
            412 => "datatype conversion overflow",
            413 => "error compressing image",
            414 => "error uncompressing image",
            420 => "bad date or time conversion",
            431 => "syntax error in expression",
            432 => "expression result wrong type",
            433 => "vector result too large",
            434 => "missing output column",
            435 => "bad data in parsed column",
            436 => "output extension of wrong type",
            501 => "WCS angle too large",
            502 => "bad WCS coordinate",
            503 => "error in WCS calculation",
            504 => "bad WCS projection type",
            505 => "WCS keywords not found",
            _ => UNKNOWN,
        }
    } else {
        UNKNOWN
    }
}

/// CFITSIO-named wrapper: `fits_get_errstatus`.
#[must_use]
pub fn fits_get_errstatus(status: i32) -> &'static str {
    status_text(status)
}
