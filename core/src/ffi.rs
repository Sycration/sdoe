//! C ABI over the JSON engine. See `include/sdoe.h`.
//!
//! Every function is safe to call from any thread; each engine handle
//! serialises access internally. Returned strings are owned by the caller
//! and released with `sdoe_string_free`.

use crate::api::Engine;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

pub struct SdoeEngine {
    inner: Mutex<Engine>,
}

fn to_c(s: String) -> *mut c_char {
    CString::new(s.replace('\0', "")).map(CString::into_raw).unwrap_or(std::ptr::null_mut())
}

/// Creates an engine with an empty project.
#[unsafe(no_mangle)]
pub extern "C" fn sdoe_engine_new() -> *mut SdoeEngine {
    Box::into_raw(Box::new(SdoeEngine { inner: Mutex::new(Engine::new()) }))
}

/// Destroys an engine. Passing NULL does nothing.
///
/// # Safety
/// `engine` must come from `sdoe_engine_new` and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sdoe_engine_free(engine: *mut SdoeEngine) {
    if !engine.is_null() {
        drop(unsafe { Box::from_raw(engine) });
    }
}

/// Runs a command. `method` is a command name such as "sheet.get" and
/// `params_json` a JSON object (NULL or "" for none). Returns a JSON
/// object: {"ok":true,"result":...,"changed":bool,"revision":n} or
/// {"ok":false,"error":"..."}.
///
/// # Safety
/// `engine` must be a live handle; the strings must be NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sdoe_call(engine: *mut SdoeEngine, method: *const c_char, params_json: *const c_char) -> *mut c_char {
    if engine.is_null() || method.is_null() {
        return to_c(r#"{"ok":false,"error":"null argument"}"#.into());
    }
    let eng = unsafe { &*engine };
    let method = unsafe { CStr::from_ptr(method) }.to_string_lossy().into_owned();
    let params = if params_json.is_null() { String::new() } else { unsafe { CStr::from_ptr(params_json) }.to_string_lossy().into_owned() };
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut guard = match eng.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.call(&method, &params)
    }));
    match result {
        Ok(s) => to_c(s),
        Err(_) => to_c(r#"{"ok":false,"error":"internal error"}"#.into()),
    }
}

/// Frees a string returned by `sdoe_call` or `sdoe_version`.
///
/// # Safety
/// `s` must come from this library and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sdoe_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Library version, e.g. "0.1.0". Free with `sdoe_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn sdoe_version() -> *mut c_char {
    to_c(env!("CARGO_PKG_VERSION").to_string())
}
