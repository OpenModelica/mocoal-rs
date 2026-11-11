// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! The C interface of the shared library `libmocoal`, as INTERFACE.md gives it.
//! Mirrors `juliac/libmocoal.jl` of ModelicaComponentAutolayout.jl.

use std::ffi::{CStr, CString, c_char};
use std::panic::catch_unwind;
use std::sync::Mutex;

use crate::cli::layout_text;

/// The message of the last `mocoal_layout`.
static MESSAGE: Mutex<Option<CString>> = Mutex::new(None);

const EMPTY: &CStr = c"";
const VERSION: &CStr = match CStr::from_bytes_with_nul(concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes()) {
    Ok(v) => v,
    Err(_) => c"",
};

fn set_message(text: &str) {
    // A message with a NUL in it ends there.
    let text = text.split('\0').next().unwrap_or("");
    let mut message = MESSAGE.lock().unwrap_or_else(|e| e.into_inner());
    *message = CString::new(text).ok();
}

/// The text of a C string, "" for `NULL`. `Err` when it is not UTF-8.
///
/// # Safety
/// `p` is `NULL` or points to a NUL-terminated text.
unsafe fn c_text<'a>(p: *const c_char) -> Result<&'a str, String> {
    if p.is_null() {
        return Ok("");
    }
    // SAFETY: the caller gives a NUL-terminated text that lives for the call.
    let s = unsafe { CStr::from_ptr(p) };
    s.to_str().map_err(|_| "the text is not UTF-8".to_owned())
}

const INTERNAL_ERROR: &str = "internal error, no layout";

fn layout(text: &str, file_name: &str, options: &str) -> Result<(CString, String), String> {
    let (out, warnings) = layout_text(text, file_name, options)?;
    let out = CString::new(out).map_err(|_| INTERNAL_ERROR.to_owned())?;
    Ok((out, warnings))
}

/// The Modelica source `text` with a layout, or `NULL` when there is none; `mocoal_message` then
/// tells why. The result is released with `mocoal_free`.
///
/// # Safety
/// Each argument is `NULL` or points to a NUL-terminated text.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mocoal_layout(
    text: *const c_char,
    file_name: *const c_char,
    options: *const c_char,
) -> *mut c_char {
    set_message("");
    // SAFETY: the arguments are as the caller of this function has to give them.
    let args = unsafe { (c_text(text), c_text(file_name), c_text(options)) };
    let result = match args {
        (Ok(text), Ok(file_name), Ok(options)) => {
            // No panic may cross the interface.
            catch_unwind(|| layout(text, file_name, options)).unwrap_or_else(|_| Err(INTERNAL_ERROR.to_owned()))
        }
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
    };
    match result {
        Ok((out, warnings)) => {
            set_message(&warnings);
            out.into_raw()
        }
        Err(e) => {
            set_message(&e);
            std::ptr::null_mut()
        }
    }
}

/// The message of the last `mocoal_layout`: the error after `NULL`, else the warnings, one per
/// line, or "". Valid until the next `mocoal_layout`.
#[unsafe(no_mangle)]
pub extern "C" fn mocoal_message() -> *const c_char {
    let message = MESSAGE.lock().unwrap_or_else(|e| e.into_inner());
    match &*message {
        // The text is on the heap and stays until the next `mocoal_layout` replaces it.
        Some(m) => m.as_ptr(),
        None => EMPTY.as_ptr(),
    }
}

/// The version of the implementation. Valid while the library is loaded.
#[unsafe(no_mangle)]
pub extern "C" fn mocoal_version() -> *const c_char {
    VERSION.as_ptr()
}

/// Release a result of `mocoal_layout`. `NULL` is allowed.
///
/// # Safety
/// `text` is `NULL` or a result of `mocoal_layout` that is not yet released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mocoal_free(text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: `text` comes from `CString::into_raw` in `mocoal_layout`.
        drop(unsafe { CString::from_raw(text) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface() {
        // SAFETY: the arguments are NUL-terminated texts or NULL; results are released once.
        unsafe {
            assert_eq!(CStr::from_ptr(mocoal_version()).to_str(), Ok(env!("CARGO_PKG_VERSION")));
            let out = mocoal_layout(c"".as_ptr(), std::ptr::null(), c"--graphics move".as_ptr());
            assert!(!out.is_null());
            assert_eq!(CStr::from_ptr(out).to_bytes(), b"");
            assert_eq!(CStr::from_ptr(mocoal_message()).to_bytes(), b"");
            mocoal_free(out);
            let out = mocoal_layout(c"model A Real x end A;".as_ptr(), c"a.mo".as_ptr(), std::ptr::null());
            assert!(out.is_null());
            let message = CStr::from_ptr(mocoal_message()).to_string_lossy().into_owned();
            assert!(message.starts_with("a.mo:1:"), "{message}");
            let bad = [0xffu8, 0];
            assert!(mocoal_layout(bad.as_ptr().cast(), std::ptr::null(), std::ptr::null()).is_null());
            assert_eq!(CStr::from_ptr(mocoal_message()).to_bytes(), b"the text is not UTF-8");
            mocoal_free(std::ptr::null_mut());
        }
    }
}
