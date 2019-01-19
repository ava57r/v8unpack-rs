extern crate v8unpack4rs;

use clap::{crate_authors, crate_version, App, Arg};
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

    basic_config = basic_config.level(level);

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

fn parse(app_m: &clap::ArgMatches, single_threaded: bool) {
    if let Some(v) = app_m.values_of("parse") {
        let args: Vec<&str> = v.collect();
        if single_threaded {
            parser::unpack_to_directory_no_load(&args[0], &args[1], true, true).unwrap();
        } else {
            parser::parse_to_folder(&args[0], &args[1], true).unwrap();
        }
    }
}

fn unpack(app_m: &clap::ArgMatches, single_threaded: bool) {
    if let Some(v) = app_m.values_of("unpack") {
        let args: Vec<&str> = v.collect();
        if single_threaded {
            parser::unpack_to_folder(&args[0], &args[1]).unwrap();
        } else {
            parser::unpack_pipeline(&args[0], &args[1]).unwrap();
        }
    }
}

fn pack(app_m: &clap::ArgMatches, _single_threaded: bool) {
    if let Some(v) = app_m.values_of("pack") {
        let args: Vec<&str> = v.collect();
        builder::pack_from_folder(&args[0], &args[1]).unwrap();
    }
}

fn build(app_m: &clap::ArgMatches, no_deflate: bool) {
    if let Some(v) = app_m.values_of("build") {
        let args: Vec<&str> = v.collect();
        builder::build_cf_file(&args[0], &args[1], no_deflate).unwrap();
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
            Arg::with_name("nopack")
                .help("Not deflate")
                .requires("build")
                .index(1),
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

    parse(&app_m, single_threaded);

    unpack(&app_m, single_threaded);

    pack(&app_m, single_threaded);

    build(&app_m, no_deflate);
}
