use std::ffi::CStr;
use std::os::raw::c_char;
use std::str::Utf8Error;
use std::panic::catch_unwind;

use parser;

unsafe fn get_string(ptr: *const c_char) -> Result<String, Utf8Error> {
    Ok(CStr::from_ptr(ptr).to_str()?.to_owned())
}

#[no_mangle]
pub unsafe extern "C" fn parse_cf(pfile_name: *const c_char, pdir_name: *const c_char) -> bool {
    let result = catch_unwind(|| {
        let file_name = get_string(pfile_name).unwrap();
        let dir_name = get_string(pdir_name).unwrap();

        return parser::Parser::unpack_to_directory_no_load(&file_name, &dir_name, true, true)
            .unwrap();
    });

    if result.is_err() {
        eprintln!("Error parse!");
        return false;
    }

    return true;
}
