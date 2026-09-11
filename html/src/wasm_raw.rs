//! C-ABI exports for non-JS WASM hosts (Python wasmtime, Go wazero) — the
//! `html`-crate edition. Built from this crate (which depends on the engine),
//! so ONE wasm carries both the HTML input path (`forme_render_html`) and the
//! engine surface (`forme_render_pdf`, `forme_certify_pdf`, …).
//!
//! This is the single C-ABI for the html-crate wasm. The engine's own
//! `wasm_raw` (behind the engine's `wasm-raw` feature) is NOT enabled in this
//! build, so there are no duplicate symbols — that one still serves the
//! engine-only wasm (e.g. the Go/wazero path).
//!
//! Protocol (same as the engine's, so existing hosts are unchanged):
//! 1. `forme_alloc` an input buffer, 2. write bytes, 3. call an op, 4. read the
//! result via `forme_get_result_ptr`/`_len`, 5. `forme_free_result`,
//! 6. `forme_dealloc` the input.

use std::alloc::{alloc, dealloc, Layout};

// WASM is single-threaded, so static mut is safe here.
static mut RESULT_BUF: *mut u8 = std::ptr::null_mut();
static mut RESULT_LEN: usize = 0;
static mut ERROR_BUF: *mut u8 = std::ptr::null_mut();
static mut ERROR_LEN: usize = 0;

/// Allocate a buffer in WASM linear memory.
#[no_mangle]
pub extern "C" fn forme_alloc(size: usize, align: usize) -> *mut u8 {
    let layout = match Layout::from_size_align(size, align) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    unsafe { alloc(layout) }
}

/// Deallocate a buffer previously allocated with `forme_alloc`.
///
/// # Safety
/// `ptr` must have been allocated by `forme_alloc` with the same `size`/`align`.
#[no_mangle]
pub unsafe extern "C" fn forme_dealloc(ptr: *mut u8, size: usize, align: usize) {
    if ptr.is_null() || size == 0 {
        return;
    }
    let layout = match Layout::from_size_align(size, align) {
        Ok(l) => l,
        Err(_) => return,
    };
    dealloc(ptr, layout);
}

/// Render a JSON document (the component-model input) to PDF bytes — the same
/// path the engine wasm exposes, kept so this wasm is a superset.
///
/// # Safety
/// `ptr` must point to `len` valid UTF-8 bytes.
#[no_mangle]
pub unsafe extern "C" fn forme_render_pdf(ptr: *const u8, len: usize) -> i32 {
    free_result_buf();
    free_error_buf();

    let json_bytes = std::slice::from_raw_parts(ptr, len);
    let json_str = match std::str::from_utf8(json_bytes) {
        Ok(s) => s,
        Err(e) => {
            set_error(&format!("Invalid UTF-8: {e}"));
            return 1;
        }
    };

    match forme::render_json(json_str) {
        Ok(pdf_bytes) => {
            set_result(&pdf_bytes);
            0
        }
        Err(e) => {
            set_error(&e.to_string());
            1
        }
    }
}

/// Render HTML + print-CSS to PDF bytes with default options — the HTML input
/// path, matching `renderHtml(html, {})` in `@formepdf/html`.
///
/// # Safety
/// `ptr` must point to `len` valid UTF-8 bytes.
#[no_mangle]
pub unsafe extern "C" fn forme_render_html(ptr: *const u8, len: usize) -> i32 {
    free_result_buf();
    free_error_buf();

    let html_bytes = std::slice::from_raw_parts(ptr, len);
    let html_str = match std::str::from_utf8(html_bytes) {
        Ok(s) => s,
        Err(e) => {
            set_error(&format!("Invalid UTF-8: {e}"));
            return 1;
        }
    };

    match crate::render_html(html_str, &crate::HtmlOptions::default()) {
        Ok(out) => {
            set_result(&out.pdf);
            0
        }
        Err(e) => {
            set_error(&e.to_string());
            1
        }
    }
}

/// Certify PDF bytes with an X.509 certificate (config is JSON).
///
/// # Safety
/// `pdf_ptr` must point to `pdf_len` valid bytes; `config_ptr` to `config_len`
/// valid UTF-8 bytes.
#[no_mangle]
pub unsafe extern "C" fn forme_certify_pdf(
    pdf_ptr: *const u8,
    pdf_len: usize,
    config_ptr: *const u8,
    config_len: usize,
) -> i32 {
    free_result_buf();
    free_error_buf();

    let pdf_bytes = std::slice::from_raw_parts(pdf_ptr, pdf_len);
    let config_bytes = std::slice::from_raw_parts(config_ptr, config_len);
    let config_str = match std::str::from_utf8(config_bytes) {
        Ok(s) => s,
        Err(e) => {
            set_error(&format!("Invalid UTF-8 in config: {e}"));
            return 1;
        }
    };

    let config: forme::model::CertificationConfig = match serde_json::from_str(config_str) {
        Ok(c) => c,
        Err(e) => {
            set_error(&format!("Invalid certification config JSON: {e}"));
            return 1;
        }
    };

    match forme::certify_pdf(pdf_bytes, &config) {
        Ok(certified_bytes) => {
            set_result(&certified_bytes);
            0
        }
        Err(e) => {
            set_error(&e.to_string());
            1
        }
    }
}

/// Pointer to the result bytes (after a successful op).
#[no_mangle]
pub extern "C" fn forme_get_result_ptr() -> *const u8 {
    unsafe { RESULT_BUF }
}

/// Length of the result bytes.
#[no_mangle]
pub extern "C" fn forme_get_result_len() -> usize {
    unsafe { RESULT_LEN }
}

/// Pointer to the error message (after a failed op).
#[no_mangle]
pub extern "C" fn forme_get_error_ptr() -> *const u8 {
    unsafe { ERROR_BUF }
}

/// Length of the error message.
#[no_mangle]
pub extern "C" fn forme_get_error_len() -> usize {
    unsafe { ERROR_LEN }
}

/// Free the result buffer. Call after reading the result bytes.
#[no_mangle]
pub extern "C" fn forme_free_result() {
    unsafe { free_result_buf() }
}

// ── internal ────────────────────────────────────────────────────────────────

fn set_result(bytes: &[u8]) {
    let len = bytes.len();
    let layout = Layout::from_size_align(len, 1).unwrap();
    unsafe {
        let buf = alloc(layout);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, len);
        RESULT_BUF = buf;
        RESULT_LEN = len;
    }
}

fn set_error(msg: &str) {
    let bytes = msg.as_bytes();
    let len = bytes.len();
    let layout = Layout::from_size_align(len, 1).unwrap();
    unsafe {
        let buf = alloc(layout);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, len);
        ERROR_BUF = buf;
        ERROR_LEN = len;
    }
}

unsafe fn free_result_buf() {
    if !RESULT_BUF.is_null() && RESULT_LEN > 0 {
        let layout = Layout::from_size_align(RESULT_LEN, 1).unwrap();
        dealloc(RESULT_BUF, layout);
        RESULT_BUF = std::ptr::null_mut();
        RESULT_LEN = 0;
    }
}

unsafe fn free_error_buf() {
    if !ERROR_BUF.is_null() && ERROR_LEN > 0 {
        let layout = Layout::from_size_align(ERROR_LEN, 1).unwrap();
        dealloc(ERROR_BUF, layout);
        ERROR_BUF = std::ptr::null_mut();
        ERROR_LEN = 0;
    }
}
