use crate::{redis_communication::RedisResponse, scrape_window::err::ScrapeErr};
use std::{
    ffi::{CStr, CString, c_char},
    ptr::NonNull,
};
use std::{pin::Pin, sync::Arc};

unsafe extern "C" {
    pub fn find_meta(html: *const c_char, result: *mut *mut c_char) -> i32;
    pub fn find_detail(html: *const c_char, result: *mut *mut c_char) -> i32;
    pub fn update_tag(html: *const c_char, result: *mut *mut c_char) -> i32;
    pub fn find_max_idx(html: *const c_char, result: *mut i32) -> i32;
    pub fn free_char(ptr: *mut c_char);
}

pub fn max_idx_finder(html: &str) -> Result<String, ScrapeErr> {
    let html_cchar = CString::new(html)?;
    let mut max_idx: i32 = -1;
    let ffi_result: i32;

    unsafe {
        ffi_result = find_max_idx(html_cchar.as_ptr(), &mut max_idx);
    };

    if ffi_result == 0 {
        Ok(format!("{}", max_idx))
    } else {
        Err(ScrapeErr::FFICallErr(ffi_result))
    }
}

pub fn ffi_parser_factory(
    foreign_func: unsafe extern "C" fn(*const c_char, *mut *mut c_char) -> i32,
) -> impl Fn(&str) -> Result<String, ScrapeErr> {
    move |html_string: &str| {
        let html_cchar = CString::new(html_string)?;
        let mut result_json = std::ptr::null_mut();
        let ffi_result: i32;
        let result: String;

        unsafe {
            ffi_result = foreign_func(html_cchar.as_ptr(), &mut result_json);
            match NonNull::new(result_json) {
                Some(n) => {
                    result = CStr::from_ptr(n.as_ptr()).to_string_lossy().to_string();
                    free_char(result_json);
                }
                None => {
                    return Err(ScrapeErr::NonNulIsNoneErr);
                }
            };
        };

        if ffi_result == 0 {
            Ok(result)
        } else {
            Err(ScrapeErr::FFICallErr(ffi_result))
        }
    }
}

fn real_scraper_factory(
    parser: impl Fn(&str) -> Result<String, ScrapeErr> + 'static + Send + Sync,
    retry: i32,
) -> impl Fn(reqwest::Client, String) -> Pin<Box<dyn Future<Output = Result<String, ScrapeErr>> + Send>> // Fn(client, url)
{
    let parser = Arc::new(parser);
    move |client: reqwest::Client, url: String| {
        let moved_parser = parser.clone();
        Box::pin(async move {
            let resp;
            let mut tempt = 0;

            while tempt < retry {
                let req = client.get(&url);
                match req.send().await {
                    Ok(result) => {
                        resp = result;
                        let text = resp.text().await?;
                        return moved_parser(&text);
                    }
                    Err(e) => {
                        tracing::error!("{}", e);
                        tempt += 1;
                    }
                }
            }

            Err(ScrapeErr::LoopOutErr)
        })
    }
}

pub fn scraper_factory(
    parser: impl Fn(&str) -> Result<String, ScrapeErr> + 'static + Send + Sync,
    retry: i32,
) -> impl Fn(reqwest::Client, String) -> Pin<Box<dyn Future<Output = RedisResponse> + Send>> // Fn(client, url)
{
    let real_func = Arc::new(real_scraper_factory(parser, retry));
    move |client: reqwest::Client, url: String| {
        let real_func = real_func.clone();
        Box::pin(async move {
            match real_func(client, url).await {
                Ok(result) => RedisResponse {
                    payload: Some(result),
                    error: None,
                    index: -1,
                },
                Err(e) => RedisResponse {
                    payload: None,
                    error: Some(format!("{}", e)),
                    index: -1,
                },
            }
        })
    }
}
