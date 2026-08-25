//! Port of CFITSIO `utilities/cookbook.c`.
//!
//! Run with `cargo run --example cookbook` from a writable directory.

use rfitsio::status::END_OF_FILE;
use rfitsio::{AccessMode, FitsFile, HduType, ImageType};

fn main() {
    writeimage();
    writeascii();
    writebintable();
    copyhdu();
    selectrows();
    readheader();
    readimage();
    readtable();
    println!("\nAll the cfitsio cookbook routines ran successfully.");
}

fn writeimage() {
    let filename = "atestfil.fit";
    let naxes = [300i64, 200];
    let mut array = vec![0u16; (naxes[0] * naxes[1]) as usize];
    for jj in 0..naxes[1] {
        for ii in 0..naxes[0] {
            array[(jj * naxes[0] + ii) as usize] = (ii + jj) as u16;
        }
    }
    let _ = std::fs::remove_file(filename);
    let mut f = FitsFile::create(filename).unwrap();
    f.create_image(ImageType::U16, &naxes).unwrap();
    f.write_image(1, &array).unwrap();
    f.update_key_lng("EXPOSURE", 1500, Some("Total Exposure Time"))
        .unwrap();
    f.close().unwrap();
}

fn writeascii() {
    let mut f = FitsFile::open("atestfil.fit", AccessMode::ReadWrite).unwrap();
    let ttype = ["Planet", "Diameter", "Density"];
    let tform = ["a8", "I6", "F4.2"];
    let tunit = [None, Some("km"), Some("g/cm")];
    f.create_tbl(
        HduType::AsciiTable,
        6,
        &ttype,
        &tform,
        &tunit,
        Some("PLANETS_ASCII"),
    )
    .unwrap();
    let planet = ["Mercury", "Venus", "Earth", "Mars", "Jupiter", "Saturn"];
    let diameter = [4880i64, 12112, 12742, 6800, 143000, 121000];
    let density = [5.1f32, 5.3, 5.52, 3.94, 1.33, 0.69];
    f.write_col_str(1, 1, &planet).unwrap();
    f.write_col_i64(2, 1, &diameter).unwrap();
    f.write_col_f32(3, 1, &density).unwrap();
    f.close().unwrap();
}

fn writebintable() {
    let mut f = FitsFile::open("atestfil.fit", AccessMode::ReadWrite).unwrap();
    f.movabs_hdu(2).unwrap();
    let ttype = ["Planet", "Diameter", "Density"];
    let tform = ["8a", "1J", "1E"];
    let tunit = [None, Some("km"), Some("g/cm")];
    f.create_tbl(
        HduType::BinaryTable,
        6,
        &ttype,
        &tform,
        &tunit,
        Some("PLANETS_Binary"),
    )
    .unwrap();
    let planet = ["Mercury", "Venus", "Earth", "Mars", "Jupiter", "Saturn"];
    let diameter = [4880i64, 12112, 12742, 6800, 143000, 121000];
    let density = [5.1f32, 5.3, 5.52, 3.94, 1.33, 0.69];
    f.write_col_str(1, 1, &planet).unwrap();
    f.write_col_i64(2, 1, &diameter).unwrap();
    f.write_col_f32(3, 1, &density).unwrap();
    f.close().unwrap();
}

fn copyhdu() {
    let _ = std::fs::remove_file("btestfil.fit");
    let mut inf = FitsFile::open("atestfil.fit", AccessMode::ReadOnly).unwrap();
    let mut out = FitsFile::create("btestfil.fit").unwrap();
    inf.copy_hdu(&mut out, 0).unwrap();
    inf.movabs_hdu(3).unwrap();
    inf.copy_hdu(&mut out, 0).unwrap();
    out.close().unwrap();
    inf.close().unwrap();
}

fn selectrows() {
    let mut inf = FitsFile::open("atestfil.fit", AccessMode::ReadOnly).unwrap();
    let mut out = FitsFile::open("btestfil.fit", AccessMode::ReadWrite).unwrap();
    inf.movabs_hdu(3).unwrap();
    out.movabs_hdu(2).unwrap();
    out.create_hdu().unwrap();
    let (nkeys, _) = inf.hdrpos().unwrap();
    for ii in 1..=nkeys {
        let card = inf.read_record(ii).unwrap();
        out.write_record(card.as_str().unwrap_or("")).unwrap();
    }
    out.flush().unwrap();
    let (naxes, _) = inf.read_keys_lng("NAXIS", 1, 2).unwrap();
    let colnum = inf.get_colnum(false, "density").unwrap();
    let (density, _) = inf
        .read_col_f32(colnum, 1, naxes[1] as usize, Some(-99.0))
        .unwrap();
    let mut noutrows = 0i64;
    for irow in 1..=naxes[1] {
        if density[(irow - 1) as usize] < 3.0 {
            noutrows += 1;
            let buf = inf.read_tblbytes(irow, 1, naxes[0] as usize).unwrap();
            out.write_tblbytes(noutrows, 1, &buf).unwrap();
        }
    }
    out.update_key_lng("NAXIS2", noutrows, None).unwrap();
    out.close().unwrap();
    inf.close().unwrap();
}

fn readheader() {
    let mut f = FitsFile::open("atestfil.fit", AccessMode::ReadOnly).unwrap();
    let mut ii = 1usize;
    loop {
        match f.movabs_hdu(ii) {
            Ok(_) => {}
            Err(e) if e.status == END_OF_FILE => break,
            Err(e) => panic!("{e}"),
        }
        let (nkeys, _) = f.hdrpos().unwrap();
        println!("Header listing for HDU #{ii}:");
        for jj in 1..=nkeys {
            let card = f.read_record(jj).unwrap();
            println!("{}", card.as_str().unwrap_or("").trim_end());
        }
        println!("END\n");
        ii += 1;
    }
    f.close().unwrap();
}

fn readimage() {
    let mut f = FitsFile::open("atestfil.fit", AccessMode::ReadOnly).unwrap();
    f.movabs_hdu(1).unwrap();
    let (naxes, _) = f.read_keys_lng("NAXIS", 1, 2).unwrap();
    let npixels = (naxes[0] * naxes[1]) as usize;
    let buf: Vec<f32> = f.read_image(1, npixels).unwrap();
    let mut datamin = f32::MAX;
    let mut datamax = f32::MIN;
    for &v in &buf {
        datamin = datamin.min(v);
        datamax = datamax.max(v);
    }
    println!("\nMin and max image pixels =  {datamin:.0}, {datamax:.0}");
    f.close().unwrap();
}

fn readtable() {
    let mut f = FitsFile::open("atestfil.fit", AccessMode::ReadOnly).unwrap();
    for hdunum in 2..=3 {
        let hdutype = f.movabs_hdu(hdunum).unwrap();
        match hdutype {
            HduType::AsciiTable => println!("\nReading ASCII table in HDU {hdunum}:"),
            HduType::BinaryTable => println!("\nReading binary table in HDU {hdunum}:"),
            HduType::Image => panic!("Error: this HDU is not an ASCII or binary table"),
        }
        let (ttype, _) = f.read_keys_str("TTYPE", 1, 3).unwrap();
        println!(" Row  {:>10} {:>10} {:>10}", ttype[0], ttype[1], ttype[2]);
        let (name, _) = f.read_col_str(1, 1, 6, Some(" ")).unwrap();
        let (dia, _) = f.read_col_i64(2, 1, 6, Some(0)).unwrap();
        let (den, _) = f.read_col_f32(3, 1, 6, Some(0.0)).unwrap();
        for ii in 0..6 {
            println!(
                "{:5} {:>10} {:>10} {:10.2}",
                ii + 1,
                name[ii],
                dia[ii],
                den[ii]
            );
        }
    }
    f.close().unwrap();
}
