use container::*;
use std::{fs, path};

pub fn pack_from_folder(dirname: &str, filename_out: &str) -> Result<bool> {
    fs::copy(
        path::Path::new(dirname).join("FileHeader"),
        path::Path::new(filename_out),
    ).expect("SaveFile. Error in creating file!");

    Ok(true)
}
