#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;
extern crate chrono;
extern crate fern;

extern crate v8unpack4rs;

use clap::{App, Arg};
use std::io;
use v8unpack4rs::{builder, parser};

fn setup_logging(log_level: Option<&str>) -> Result<(), fern::InitError> {
    let mut basic_config = fern::Dispatch::new();

    let level = match log_level {
        None => log::LevelFilter::Info,
        Some(v) => match v {
            "info" => log::LevelFilter::Info,
            "debug" => log::LevelFilter::Debug,
            "warn" => log::LevelFilter::Warn,
            "trace" => log::LevelFilter::Trace,
            "error" => log::LevelFilter::Error,
            _ => panic!(
                "Bad value of the logging level. True variants: debug, info, \
                 trace, warn, error."
            ),
        },
    };

    let stdout_config = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.target(),
                record.level(),
                message
            ))
        })
        .chain(io::stdout());

    basic_config.chain(stdout_config).apply()?;

    Ok(())
}

fn parse(args: Vec<&str>, single_threaded: bool) -> bool {
    if single_threaded {
        match parser::unpack_to_directory_no_load(&args[0], &args[1], true, true) {
            Ok(b) => b,
            Err(e) => {
                println!("{:?}", e);
                panic!(e.to_string());
            }
        }
    } else {
        match parser::parse_to_folder(&args[0], &args[1], true) {
            Ok(b) => b,
            Err(e) => {
                println!("{:?}", e);
                panic!(e.to_string());
            }
        }
    }
}

fn unpack(args: Vec<&str>, single_threaded: bool) -> bool {
    if single_threaded {
        match parser::unpack_to_folder(&args[0], &args[1]) {
            Ok(b) => b,
            Err(e) => panic!(e.to_string()),
        }
    } else {
        match parser::unpack_pipeline(&args[0], &args[1]) {
            Ok(b) => b,
            Err(e) => panic!(e.to_string()),
        }
    }
}

fn pack(args: Vec<&str>, _single_threaded: bool) -> bool {
    match builder::pack_from_folder(&args[0], &args[1]) {
        Ok(b) => b,
        Err(e) => panic!(e.to_string()),
    }
}

fn build(args: Vec<&str>, no_deflate: bool) -> bool {
    match builder::build_cf_file(&args[0], &args[1], no_deflate) {
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
        .arg(
            Arg::with_name("pack")
                .long("pack")
                .help("Package the binaries in *.cf")
                .takes_value(true)
                .value_names(&["INPUTFILE", "OUTDIR"]),
        )
        .arg(
            Arg::with_name("build")
                .short("b")
                .long("build")
                .help("Build the binaries in *.cf from source files")
                .takes_value(true)
                .value_names(&["INPUTFILE", "OUTDIR"]),
        )
        .arg(
            Arg::with_name("no-deflate")
                .short("n")
                .long("no-deflate")
                .help("Not deflate"),
        )
        .arg(
            Arg::with_name("verbosity")
                .short("v")
                .long("verbosity")
                .help("Logging verbosity level")
                .takes_value(true)
                .value_name("LOG_LEVEL"),
        )
        .get_matches();

    let single_threaded = app_m.is_present("single-threaded");
    let no_deflate = app_m.is_present("no-deflate");

    if app_m.is_present("verbosity") {
        setup_logging(app_m.value_of("verbosity"))
            .expect("failed to initialize logging.");
    }

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

    if let Some(vals) = app_m.values_of("pack") {
        let v: Vec<&str> = vals.collect();
        if pack(v, true) {
            std::process::exit(0);
        }
    }

    if let Some(vals) = app_m.values_of("build") {
        let v: Vec<&str> = vals.collect();
        if build(v, no_deflate) {
            std::process::exit(0);
        }
    }
}
