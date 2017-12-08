extern crate byteorder;
#[macro_use]
extern crate clap;
extern crate deflate;
extern crate hex;

mod container;

use clap::{App, Arg};

fn parse(args: Vec<&str>) -> std::io::Result<()> {
    container::V8File::unpack_to_directory_no_load(&args[0], &args[1], true, true)?;

    Ok(())
}

fn main() {
    let app_m = App::new("v8unpack-rs")
        .version(crate_version!())
        .author(crate_authors!())
        .about(
            "\n\t2008 Denis Demidov 2008-03-30\n\t2017 Alexander Andreev\n\
             Unpack, pack, deflate and inflate 1C v8 file (*.cf)",
        )
        .arg(
            Arg::with_name("parse")
                .short("p")
                .long("parse")
                .help("распаковать файлы в каталог")
                .takes_value(true)
                .value_names(&["INPUTFILE", "OUTDIR"]),
        )
        .get_matches();
    if let Some(vals) = app_m.values_of("parse") {
        let v: Vec<&str> = vals.collect();
        parse(v).unwrap();
    } else {
        println!(
            "{}",
            "Используйте ключ -h для получения справки"
        );
    }
}
