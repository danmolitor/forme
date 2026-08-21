//! C-ABI exports for non-JS WASM hosts (e.g. Python wasmtime, Go wazero).
//!
//! Provides a simple memory-based protocol:
//! 1. Host calls `forme_alloc` to allocate input buffer in WASM memory
//! 2. Host writes document JSON, or template and data JSON, into allocated buffers
//! 3. Host calls `forme_render_pdf` or `forme_render_template`
//! 4. Host reads result via `forme_get_result_ptr`/`forme_get_result_len`
//! 5. Host calls `forme_free_result` to release the output buffer
//! 6. Host calls `forme_dealloc` to release the input buffer

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
/// `ptr` must have been allocated by `forme_alloc` with the same `size` and `align`.
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

/// Render a JSON document to PDF bytes.
///
/// Returns 0 on success (result available via `forme_get_result_ptr`/`forme_get_result_len`).
/// Returns 1 on error (error message via `forme_get_error_ptr`/`forme_get_error_len`).
///
/// # Safety
/// `ptr` must point to `len` valid UTF-8 bytes.
#[no_mangle]
pub unsafe extern "C" fn forme_render_pdf(ptr: *const u8, len: usize) -> i32 {
    // Free any previous result/error
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

    finish_render(crate::render_json(json_str))
}

/// Render a compiled template with data to PDF bytes.
///
/// Returns 0 on success (result available via `forme_get_result_ptr`/`forme_get_result_len`).
/// Returns 1 on error (error message via `forme_get_error_ptr`/`forme_get_error_len`).
///
/// # Safety
/// `template_ptr` must point to `template_len` valid UTF-8 bytes.
/// `data_ptr` must point to `data_len` valid UTF-8 bytes.
#[no_mangle]
pub unsafe extern "C" fn forme_render_template(
    template_ptr: *const u8,
    template_len: usize,
    data_ptr: *const u8,
    data_len: usize,
) -> i32 {
    free_result_buf();
    free_error_buf();

    let template_bytes = std::slice::from_raw_parts(template_ptr, template_len);
    let template_json = match std::str::from_utf8(template_bytes) {
        Ok(s) => s,
        Err(e) => {
            set_error(&format!("Invalid UTF-8 in template: {e}"));
            return 1;
        }
    };

    let data_bytes = std::slice::from_raw_parts(data_ptr, data_len);
    let data_json = match std::str::from_utf8(data_bytes) {
        Ok(s) => s,
        Err(e) => {
            set_error(&format!("Invalid UTF-8 in data: {e}"));
            return 1;
        }
    };

    finish_render(crate::render_template(template_json, data_json))
}

unsafe fn finish_render(result: Result<Vec<u8>, crate::FormeError>) -> i32 {
    match result {
        Ok(pdf_bytes) => {
            let len = pdf_bytes.len();
            let layout = Layout::from_size_align(len, 1).unwrap();
            let buf = alloc(layout);
            std::ptr::copy_nonoverlapping(pdf_bytes.as_ptr(), buf, len);
            RESULT_BUF = buf;
            RESULT_LEN = len;
            0
        }
        Err(e) => {
            set_error(&e.to_string());
            1
        }
    }
}

/// Certify PDF bytes with an X.509 certificate.
///
/// Returns 0 on success (result available via `forme_get_result_ptr`/`forme_get_result_len`).
/// Returns 1 on error (error message via `forme_get_error_ptr`/`forme_get_error_len`).
///
/// # Safety
/// `pdf_ptr` must point to `pdf_len` valid bytes.
/// `config_ptr` must point to `config_len` valid UTF-8 bytes (JSON).
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

    let config: crate::model::CertificationConfig = match serde_json::from_str(config_str) {
        Ok(c) => c,
        Err(e) => {
            set_error(&format!("Invalid certification config JSON: {e}"));
            return 1;
        }
    };

    match crate::certify_pdf(pdf_bytes, &config) {
        Ok(certified_bytes) => {
            let len = certified_bytes.len();
            let layout = Layout::from_size_align(len, 1).unwrap();
            let buf = alloc(layout);
            std::ptr::copy_nonoverlapping(certified_bytes.as_ptr(), buf, len);
            RESULT_BUF = buf;
            RESULT_LEN = len;
            0
        }
        Err(e) => {
            set_error(&e.to_string());
            1
        }
    }
}

/// Get pointer to the result PDF bytes (after successful `forme_render_pdf`).
#[no_mangle]
pub extern "C" fn forme_get_result_ptr() -> *const u8 {
    unsafe { RESULT_BUF }
}

/// Get length of the result PDF bytes.
#[no_mangle]
pub extern "C" fn forme_get_result_len() -> usize {
    unsafe { RESULT_LEN }
}

/// Get pointer to the error message (after failed `forme_render_pdf`).
#[no_mangle]
pub extern "C" fn forme_get_error_ptr() -> *const u8 {
    unsafe { ERROR_BUF }
}

/// Get length of the error message.
#[no_mangle]
pub extern "C" fn forme_get_error_len() -> usize {
    unsafe { ERROR_LEN }
}

/// Free the result buffer. Call after reading the PDF bytes.
#[no_mangle]
pub extern "C" fn forme_free_result() {
    unsafe { free_result_buf() }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_compiled_template() {
        let template = br#"{
            "children": [{
                "kind": {"type": "Text", "content": {"$ref": "title"}},
                "style": {"fontSize": 24},
                "children": []
            }],
            "metadata": {"title": {"$ref": "title"}},
            "defaultPage": {
                "size": "A4",
                "margin": {"top": 54, "right": 54, "bottom": 54, "left": 54},
                "wrap": true
            }
        }"#;
        let data = br#"{"title":"Invoice #001"}"#;

        let status = unsafe {
            forme_render_template(template.as_ptr(), template.len(), data.as_ptr(), data.len())
        };

        assert_eq!(status, 0);
        let result = unsafe { std::slice::from_raw_parts(RESULT_BUF, RESULT_LEN) };
        assert!(result.starts_with(b"%PDF-"));
        forme_free_result();
    }
}
