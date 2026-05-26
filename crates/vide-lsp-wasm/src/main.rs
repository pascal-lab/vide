use std::{cell::RefCell, ffi::CString, os::raw::c_char, panic, path::PathBuf};

use vide::browser::BrowserServer;

fn main() {}

#[cfg(target_os = "emscripten")]
unsafe extern "C" {
    fn emscripten_get_now() -> f64;
}

#[cfg(target_os = "emscripten")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _emscripten_get_now() -> f64 {
    unsafe { emscripten_get_now() }
}

thread_local! {
    static SERVER: RefCell<BrowserServer> = RefCell::new(BrowserServer::new());
}

#[unsafe(no_mangle)]
pub extern "C" fn vide_lsp_message(json_ptr: *const u8, json_len: usize) -> *mut c_char {
    run_json(|| {
        let json = read_utf8(json_ptr, json_len)?;
        SERVER.with(|server| server.borrow_mut().handle_message_json(&json))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vide_lsp_poll(_json_ptr: *const u8, _json_len: usize) -> *mut c_char {
    run_json(|| SERVER.with(|server| server.borrow_mut().poll_json()))
}

#[unsafe(no_mangle)]
pub extern "C" fn vide_lsp_reset() {
    SERVER.with(|server| {
        server.borrow_mut().reset();
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn vide_lsp_write_file(
    path_ptr: *const u8,
    path_len: usize,
    text_ptr: *const u8,
    text_len: usize,
) -> *mut c_char {
    run_json(|| {
        let path = read_utf8(path_ptr, path_len)?;
        let text = read_utf8(text_ptr, text_len)?;
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(path, text).map_err(|error| error.to_string())?;
        Ok("null".to_owned())
    })
}

/// # Safety
///
/// `ptr` must be a pointer previously returned by a Vide WASM FFI function
/// that transfers ownership of a NUL-terminated string to the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vide_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}

fn run_json(f: impl FnOnce() -> Result<String, String> + panic::UnwindSafe) -> *mut c_char {
    let result = panic::catch_unwind(f)
        .unwrap_or_else(|_| Err("Vide LSP session panicked".to_owned()))
        .unwrap_or_else(|error| {
            serde_json::to_string(&serde_json::json!({ "error": error }))
                .unwrap_or_else(|_| "{\"error\":\"Vide LSP session failed\"}".to_owned())
        });

    CString::new(result).expect("JSON output must not contain interior NUL bytes").into_raw()
}

fn read_utf8(ptr: *const u8, len: usize) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null input pointer".to_owned());
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(bytes).map(|value| value.to_owned()).map_err(|error| error.to_string())
}
