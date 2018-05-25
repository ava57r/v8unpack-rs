#[cfg(test)]
extern crate tempdir;
extern crate v8unpack4rs;

use tempdir::TempDir;
use v8unpack4rs::{builder, parser};

use std::fs::File;
use std::io::prelude::*;

pub static TEST_FILE1: &'static [u8] = include_bytes!("test1.cf");

#[test]
fn test_parse_and_build() {
    let dir = TempDir::new("test_parse_build").unwrap();
    let unpack = dir.path().join("unpack");
    let unpack = unpack.to_str().unwrap();
    let test1 = dir.path().join("test1.cf");
    let mut test_file = File::create(test1.clone()).unwrap();
    test_file.write_all(TEST_FILE1).unwrap();

    let parse_ok = match parser::unpack_to_directory_no_load(
        test1.as_path().to_str().unwrap(),
        unpack,
        true,
        true,
    ) {
        Ok(b) => b,
        Err(e) => {
            panic!(e.to_string());
        }
    };

    assert!(parse_ok);

    const BUILD_FILE: &'static str = "build.cf";
    let build_file = dir.path().join(BUILD_FILE);
    let build_file = build_file.as_path().to_str().unwrap();

    let build_ok = match builder::build_cf_file(unpack, build_file, false) {
        Ok(b) => b,
        Err(e) => panic!(e.to_string()),
    };

    assert!(build_ok);
    let unpack2 = dir.path().join("unpack2");
    let unpack2 = unpack2.to_str().unwrap();

    let parse_ok2 =
        match parser::unpack_to_directory_no_load(build_file, unpack2, true, true) {
            Ok(b) => b,
            Err(e) => {
                panic!(e.to_string());
            }
        };

    assert!(parse_ok2);

    dir.close().unwrap();
}
