//! Build the vendored CFITSIO 4.7.0 static library for oracle tests.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let cfitsio_src = manifest_dir.join("../../vendor/cfitsio");
    let cfitsio_src = cfitsio_src.canonicalize().unwrap_or(cfitsio_src);

    println!("cargo:rerun-if-changed={}", cfitsio_src.display());
    println!("cargo:root={}", cfitsio_src.display());

    let dst = cmake::Config::new(&cfitsio_src)
        .define("USE_CURL", "OFF")
        .define("TESTS", "OFF")
        .define("UTILS", "OFF")
        .define("ITERPROGS", "OFF")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("USE_PTHREADS", "OFF")
        .define("USE_BZIP2", "OFF")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .build();

    let libdir = find_libdir(&dst);
    println!("cargo:rustc-link-search=native={}", libdir.display());
    println!("cargo:rustc-link-lib=static=cfitsio");
    println!("cargo:rustc-link-lib=z");

    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("apple") {
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    } else if !target.contains("windows") {
        println!("cargo:rustc-link-lib=m");
    }
}

fn find_libdir(dst: &Path) -> PathBuf {
    for candidate in ["lib", "lib64", "lib/x86_64-linux-gnu"] {
        let dir = dst.join(candidate);
        if dir.join("libcfitsio.a").exists() || dir.join("cfitsio.lib").exists() {
            return dir;
        }
    }
    dst.join("lib")
}
