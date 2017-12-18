use std::{fmt, string};
use std::io;

#[derive(Debug)]
pub enum V8Error {
    NotV8File,
    IoError(io::Error),
    Utf8Error(string::FromUtf8Error),
}

impl From<io::Error> for V8Error {
    fn from(other: io::Error) -> V8Error {
        V8Error::IoError(other)
    }
}

impl From<string::FromUtf8Error> for V8Error {
    fn from(other: string::FromUtf8Error) -> V8Error {
        V8Error::Utf8Error(other)
    }
}

impl fmt::Display for V8Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            V8Error::IoError(ref e) => fmt::Display::fmt(e, f),
            V8Error::NotV8File => write!(f, "Not correct V8 file"),
            V8Error::Utf8Error(ref e) => fmt::Display::fmt(e, f),
        }
    }
}
