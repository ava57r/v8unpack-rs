#[macro_use]
extern crate clap;
extern crate v8unpack4rs;

use clap::{App, Arg};

use v8unpack4rs::parser;

fn parse(args: Vec<&str>) -> bool {
    match parser::Parser::unpack_to_directory_no_load(&args[0], &args[1], true, true) {
        Ok(b) => b,
        Err(e) => panic!(e.to_string()),
    }
}

fn main() {
    let app_m = App::new("v8unpack")
        .version(crate_version!())
        .author(crate_authors!())
        .setting(clap::AppSettings::ArgRequiredElseHelp)
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
        if parse(v) {
            std::process::exit(0);
        }
    }
}
