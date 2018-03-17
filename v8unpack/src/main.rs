#[macro_use]
extern crate clap;
extern crate v8unpack4rs;

use clap::{App, Arg};
use v8unpack4rs::parser;

fn parse(args: Vec<&str>, single_threaded: bool) -> bool {
    if single_threaded {
        match parser::Parser::unpack_to_directory_no_load(&args[0], &args[1], true, true) {
            Ok(b) => b,
            Err(e) => panic!(e.to_string()),
        }
    } else {
        match parser::Parser::parse_to_folder(&args[0], &args[1], true) {
            Ok(b) => b,
            Err(e) => panic!(e.to_string()),
        }
    }
}

fn unpack(args: Vec<&str>, single_threaded: bool) -> bool {
    if single_threaded {
        match parser::Parser::unpack_to_folder(&args[0], &args[1]) {
            Ok(b) => b,
            Err(e) => panic!(e.to_string()),
        }
    } else {
        match parser::Parser::unpack_pipeline(&args[0], &args[1]) {
            Ok(b) => b,
            Err(e) => panic!(e.to_string()),
        }
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
                .help("unzip the files into a directory")
                .takes_value(true)
                .value_names(&["INPUTFILE", "OUTDIR"]),
        )
        .arg(
            Arg::with_name("unpack")
                .short("u")
                .long("unpack")
                .help("unzip the binaries into the directory")
                .takes_value(true)
                .value_names(&["INPUTFILE", "OUTDIR"]),
        )
        .arg(
            Arg::with_name("single-threaded")
                .short("s")
                .long("single-threaded")
                .help("Do all the work on a single thread."),
        )
        .get_matches();

    let single_threaded = app_m.is_present("single-threaded");

    if let Some(vals) = app_m.values_of("parse") {
        let v: Vec<&str> = vals.collect();
        if parse(v, single_threaded) {
            std::process::exit(0);
        }
    }

    if let Some(vals) = app_m.values_of("unpack") {
        let v: Vec<&str> = vals.collect();
        if unpack(v, single_threaded) {
            std::process::exit(0);
        }
    }
}
