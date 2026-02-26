use std::{
    ffi::{CStr, CString, NulError, c_char},
    ptr::NonNull,
};

#[derive(thiserror::Error, Debug)]
pub enum ParserErr {
    #[error("{0}")]
    NulErr(#[from] NulError),

    #[error("{0}")]
    FFICallErr(i32),

    #[error("")]
    NonNulIsNoneErr,
}

unsafe extern "C" {
    pub fn find_meta(html: *const c_char, result: *mut *mut c_char) -> i32;
    pub fn find_detail(html: *const c_char, result: *mut *mut c_char) -> i32;
    pub fn update_tag(html: *const c_char, result: *mut *mut c_char) -> i32;
    pub fn find_max_idx(html: *const c_char, result: *mut i32) -> i32;
    pub fn free_char(ptr: *mut c_char);
}

pub fn max_idx_finder(html: &str) -> Result<String, ParserErr> {
    let html_cchar = CString::new(html)?;
    let mut max_idx: i32 = -1;
    let ffi_result: i32;

    unsafe {
        ffi_result = find_max_idx(html_cchar.as_ptr(), &mut max_idx);
    };

    if ffi_result == 0 {
        Ok(format!("{}", max_idx))
    } else {
        Err(ParserErr::FFICallErr(ffi_result))
    }
}

pub fn ffi_parser_factory(
    ffi_func: unsafe extern "C" fn(*const c_char, *mut *mut c_char) -> i32,
) -> impl Fn(&str) -> Result<String, ParserErr> {
    move |html_str: &str| {
        let html_cchar = CString::new(html_str)?;
        let mut result_json = std::ptr::null_mut();

        unsafe {
            let ffi_result = ffi_func(html_cchar.as_ptr(), &mut result_json);
            match NonNull::new(result_json) {
                Some(n) => {
                    let result = CStr::from_ptr(n.as_ptr()).to_string_lossy().to_string();
                    free_char(result_json);

                    if ffi_result == 0 {
                        Ok(result)
                    } else {
                        Err(ParserErr::FFICallErr(ffi_result))
                    }
                }

                None => Err(ParserErr::NonNulIsNoneErr),
            }
        }
    }
}
