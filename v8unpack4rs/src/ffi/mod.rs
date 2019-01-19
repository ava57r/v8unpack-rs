use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic::catch_unwind;
use std::str::Utf8Error;

use crate::parser::unpack_to_directory_no_load;

unsafe fn get_string(ptr: *const c_char) -> Result<String, Utf8Error> {
    Ok(CStr::from_ptr(ptr).to_str()?.to_owned())
}

/// External interface to call the decompression of the file container from
/// other languages.
#[no_mangle]
pub unsafe extern "C" fn parse_cf(
    pfile_name: *const c_char,
    pdir_name: *const c_char,
) -> bool {
    let result = catch_unwind(|| {
        let file_name = get_string(pfile_name).unwrap();
        let dir_name = get_string(pdir_name).unwrap();

        unpack_to_directory_no_load(&file_name, &dir_name, true, true).unwrap()
    });

    if result.is_err() {
        eprintln!("Error parse!");
        return false;
    }

    true
}
