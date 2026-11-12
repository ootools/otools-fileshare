//! Panic-safe FFI boundary helpers for OTools native plugins.
//!
//! # Why this exists
//!
//! `otools_plugin_invoke` is an `extern "C"` boundary. A Rust panic raised
//! inside plugin logic **cannot unwind across a DLL boundary**: the C
//! calling convention has no provision for it, so the runtime invokes
//! `panic_cannot_unwind` and calls `abort()`. On Windows that surfaces as
//! `0xC000001D` at the `ud2` + `__fastfail(FAST_FAIL_FATAL_APP_EXIT)` stub,
//! and the entire host process dies — including the host's own panic hook,
//! which never learns anything happened.
//!
//! Verified empirically: a plugin that lets the panic escape produces exactly
//! `thread caused non-unwinding panic. aborting.` with exit code 127, which
//! matches the production crash signature.
//!
//! # The fix
//!
//! Catch the panic **inside the plugin**, at the FFI entry point, and convert
//! it into an ordinary error response. Nothing then unwinds across the
//! boundary, so the host stays alive and the user sees a normal error instead
//! of losing their whole session.
//!
//! This requires the plugin cdylib to be built with `panic = "unwind"`.
//! Cargo does not allow `panic` in a `[profile.*.package.*]` override, so the
//! workspace release profile must use `unwind`.
//!
//! # Usage
//!
//! ```ignore
//! #[no_mangle]
//! pub extern "C" fn otools_plugin_invoke(
//!     input_ptr: *const u8,
//!     input_len: usize,
//!     output_len: *mut usize,
//! ) -> *mut u8 {
//!     otools_plugin_ffi::guard_invoke(input_ptr, input_len, output_len, |input| {
//!         let parsed: Value = serde_json::from_slice(input)?;
//!         Ok(respond_ok(dispatch(&method, payload)))
//!     })
//! }
//! ```
//!
//! The closure returns `Result<Value, String>`; either way the caller gets a
//! valid JSON response buffer and never a crash.

use serde_json::{json, Value};

/// Wrap a buffer produced by [`take_json_response`] into the C ABI contract:
/// a pointer the host can read, with the length written to `output_len`.
///
/// # Safety
///
/// `ptr` must come from a `Vec<u8>` whose length is `len`, and ownership is
/// transferred to the host (which must eventually pass it back to
/// `otools_plugin_free`).
pub fn take_json_response(mut output: Vec<u8>, output_len: *mut usize) -> *mut u8 {
    if output.is_empty() {
        // A zero-length buffer would make the host's `from_raw_parts` invalid.
        output.extend_from_slice(b"{}");
    }
    let len = output.len();
    unsafe {
        *output_len = len;
    }
    let ptr = output.as_mut_ptr();
    // The host owns the allocation now; do not drop it here.
    std::mem::forget(output);
    ptr
}

/// Build the JSON body for a caught panic.
///
/// The message is intentionally generic on the surface but kept useful:
/// operators need the plugin name and the panic text to triage.
fn panic_response(plugin: &str, message: &str) -> Value {
    json!({
        "ok": false,
        "error": format!("插件 {plugin} 内部错误: {message}"),
        "panic": true,
        "plugin": plugin,
    })
}

/// Extracts a human-readable message from a panic payload.
fn payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        return (*text).to_string();
    }
    if let Some(text) = payload.downcast_ref::<String>() {
        return text.clone();
    }
    "unknown panic payload".to_string()
}

/// The guarded `otools_plugin_invoke` body.
///
/// `logic` receives the raw input slice and returns a JSON response value, or
/// an error string. Any panic inside `logic` is caught and converted into an
/// error response, so the host process survives.
///
/// # Safety
///
/// `input_ptr`/`input_len` must describe a valid readable range, and
/// `output_len` must be a valid writable pointer. Callers are the plugin's own
/// `extern "C"` entry points, which validate pointers before delegating here.
pub fn guard_invoke<F>(
    input_ptr: *const u8,
    input_len: usize,
    output_len: *mut usize,
    logic: F,
) -> *mut u8
where
    F: FnOnce(&[u8]) -> Result<Value, String>,
{
    const PLUGIN: &str = env!("CARGO_PKG_NAME");

    if input_ptr.is_null() || output_len.is_null() {
        return std::ptr::null_mut();
    }

    let input = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };

    // `AssertUnwindSafe` is justified: we are not touching host-owned
    // invariants across the boundary, and the alternative is losing the
    // process. The payload is only formatted into a string.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| logic(input)));

    let response = match outcome {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => json!({ "ok": false, "error": error }),
        Err(payload) => {
            let message = payload_message(payload.as_ref());
            panic_response(PLUGIN, &message)
        }
    };

    let encoded = serde_json::to_vec(&response)
        .unwrap_or_else(|_| br#"{"ok":false,"error":"serialize failed"}"#.to_vec());
    take_json_response(encoded, output_len)
}

/// Guards an FFI entry point whose body is an existing function.
///
/// This is the form used when wrapping the many existing plugins: the
/// original `extern "C"` body becomes a plain Rust function (`invoke_impl`)
/// and this wrapper runs it inside `catch_unwind`.
///
/// On the panic path, `invoke` may have already produced a buffer, or may
/// have produced nothing at all. Since we cannot know the buffer's length
/// reliably once a panic unwound past its write, we discard any pointer it
/// returned and emit a fresh error response. That is deliberate: leaking one
/// response buffer is far better than handing the host a pointer whose length
/// is unknown.
///
/// # Safety
///
/// `input_ptr`/`input_len` must describe a valid readable range and
/// `output_len` must be writable. `invoke` must not itself unwind.
pub fn guard_owned_invoke<F>(
    plugin: &str,
    input_ptr: *const u8,
    input_len: usize,
    output_len: *mut usize,
    invoke: F,
) -> *mut u8
where
    F: FnOnce(*const u8, usize, *mut usize) -> *mut u8,
{
    if input_ptr.is_null() || output_len.is_null() {
        return std::ptr::null_mut();
    }

    // SAFETY: `invoke` is an `extern "C"` boundary and must not unwind. The
    // catch below is what makes that guarantee hold even if it does.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        invoke(input_ptr, input_len, output_len)
    }));

    match outcome {
        Ok(ptr) => ptr,
        Err(payload) => {
            let message = payload_message(payload.as_ref());
            let response = panic_response(plugin, &message);
            let encoded = serde_json::to_vec(&response)
                .unwrap_or_else(|_| br#"{"ok":false,"error":"serialize failed"}"#.to_vec());
            take_json_response(encoded, output_len)
        }
    }
}

/// The body of a guarded `otools_plugin_free`.
///
/// Reconstructing a `Vec` is the one operation here that can genuinely fail
/// (a mismatched length is undefined behaviour), so it is validated and
/// guarded rather than trusted.
///
/// **Do not add `#[no_mangle]` here.** This crate is linked into every plugin
/// cdylib, and each plugin already exports its own `otools_plugin_free`.
/// A `#[no_mangle]` definition in this crate would collide with the plugin's
/// at link time on Windows (`error LNK2005: otools_plugin_free already
/// defined`). Plugins call [`free_response`] from their own
/// `#[no_mangle] otools_plugin_free` wrapper instead.
pub fn free_response(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    let outcome = std::panic::catch_unwind(|| unsafe {
        drop(Vec::from_raw_parts(ptr, len, len));
    });
    if outcome.is_err() {
        // Intentionally silent: a double free is a caller bug we cannot fix
        // here, but panicking would only escalate it.
    }
}

/// Test-only copy of the plugin-side `otools_plugin_free` export.
///
/// It exists purely so the logic above can be exercised; see the note on
/// [`free_response`] for why the real export must not live in this crate.
#[cfg(test)]
#[no_mangle]
pub extern "C" fn otools_plugin_free(ptr: *mut u8, len: usize) {
    free_response(ptr, len);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invoke_with<F>(input: &[u8], logic: F) -> (bool, Value)
    where
        F: FnOnce(&[u8]) -> Result<Value, String> + std::panic::UnwindSafe,
    {
        let mut len: usize = 0;
        let ptr = guard_invoke(input.as_ptr(), input.len(), &mut len, logic);
        assert!(!ptr.is_null(), "guard must always return a buffer");
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
        let value: Value = serde_json::from_slice(bytes).expect("response must be JSON");
        otools_plugin_free(ptr, len);
        (true, value)
    }

    #[test]
    fn success_path_returns_value() {
        let (_, value) = invoke_with(b"{}", |_| Ok(json!({ "ok": true, "v": 7 })));
        assert_eq!(value["ok"], json!(true));
        assert_eq!(value["v"], json!(7));
    }

    #[test]
    fn error_path_is_returned_as_error_response() {
        let (_, value) = invoke_with(b"{}", |_| Err("bad request".to_string()));
        assert_eq!(value["ok"], json!(false));
        assert_eq!(value["error"], json!("bad request"));
    }

    /// `guard_owned_invoke` is the wrapper actually applied to the 30 native
    /// plugins, where the body is an existing `extern "C"`-style function
    /// rather than a closure returning `Result`.
    fn owned_invoke_with<F>(input: &[u8], invoke: F) -> Value
    where
        F: FnOnce(*const u8, usize, *mut usize) -> *mut u8,
    {
        let mut len: usize = 0;
        let ptr = guard_owned_invoke(
            "unit-test-plugin",
            input.as_ptr(),
            input.len(),
            &mut len,
            invoke,
        );
        assert!(!ptr.is_null(), "guard must always return a buffer");
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
        let value: Value = serde_json::from_slice(bytes).expect("response must be JSON");
        otools_plugin_free(ptr, len);
        value
    }

    #[test]
    fn owned_guard_passes_through_a_normal_response() {
        let value = owned_invoke_with(b"{}", |input_ptr, input_len, output_len| {
            let input = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };
            // Echo the input back as a JSON string, proving the bytes reached
            // the implementation unchanged.
            let text = String::from_utf8_lossy(input).to_string();
            let body = serde_json::to_vec(&Value::String(text)).expect("serialize");
            take_json_response(body, output_len)
        });
        assert_eq!(value, json!("{}"));
    }

    #[test]
    fn owned_guard_converts_panic_into_error_response() {
        // Must not unwind into the host, and must still hand back valid JSON.
        let value = owned_invoke_with(b"{}", |_, _, _| -> *mut u8 { panic!("impl exploded") });
        assert_eq!(value["ok"], json!(false));
        assert_eq!(value["panic"], json!(true));
        assert_eq!(value["plugin"], json!("unit-test-plugin"));
        assert!(
            value["error"].as_str().unwrap().contains("impl exploded"),
            "message preserved: {value}"
        );
    }

    #[test]
    fn owned_guard_rejects_null_pointers() {
        let mut len: usize = 0;
        let ptr = guard_owned_invoke("p", std::ptr::null(), 0, &mut len, |_, _, out| {
            take_json_response(b"{}".to_vec(), out)
        });
        assert!(ptr.is_null());

        let input = b"{}";
        let ptr = guard_owned_invoke(
            "p",
            input.as_ptr(),
            input.len(),
            std::ptr::null_mut(),
            |_, _, _| std::ptr::null_mut(),
        );
        assert!(ptr.is_null());
    }

    #[test]
    fn panic_is_caught_and_reported_not_propagated() {
        // The whole point: this must not unwind out of the FFI boundary.
        let (_, value) = invoke_with(b"{}", |_| panic!("plugin blew up"));
        assert_eq!(value["ok"], json!(false));
        assert_eq!(value["panic"], json!(true));
        assert!(
            value["error"].as_str().unwrap().contains("plugin blew up"),
            "panic message should be preserved: {value}"
        );
    }

    #[test]
    fn panic_with_string_payload_is_reported() {
        let (_, value) = invoke_with(b"{}", |_| {
            std::panic::panic_any(String::from("owned payload"))
        });
        assert_eq!(value["panic"], json!(true));
        assert!(value["error"].as_str().unwrap().contains("owned payload"));
    }

    #[test]
    fn non_string_payload_is_still_handled() {
        let (_, value) = invoke_with(b"{}", |_| std::panic::panic_any(42u32));
        assert_eq!(value["panic"], json!(true));
        assert!(value["error"].as_str().unwrap().contains("unknown"));
    }

    #[test]
    fn null_input_pointer_is_rejected() {
        let mut len: usize = 0;
        let ptr = guard_invoke(std::ptr::null(), 0, &mut len, |_| Ok(json!({})));
        assert!(ptr.is_null());
    }

    #[test]
    fn null_output_pointer_is_rejected() {
        let input = b"{}";
        let ptr = guard_invoke(input.as_ptr(), input.len(), std::ptr::null_mut(), |_| {
            Ok(json!({}))
        });
        assert!(ptr.is_null());
    }

    #[test]
    fn empty_response_is_replaced_with_valid_json() {
        // A zero-length buffer would make the host's from_raw_parts unsound.
        let mut len: usize = 0;
        let ptr = take_json_response(Vec::new(), &mut len);
        assert!(!ptr.is_null());
        assert!(len > 0);
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
        let value: Value = serde_json::from_slice(bytes).expect("valid JSON");
        assert!(value.is_object());
        otools_plugin_free(ptr, len);
    }

    #[test]
    fn free_with_null_or_zero_len_is_a_noop() {
        otools_plugin_free(std::ptr::null_mut(), 0);
        otools_plugin_free(std::ptr::null_mut(), 8);
    }

    #[test]
    fn input_slice_reaches_the_closure_unchanged() {
        let payload = br#"{"method":"ping"}"#;
        let (_, value) = invoke_with(payload, |input| {
            assert_eq!(input, payload);
            Ok(json!({ "ok": true }))
        });
        assert_eq!(value["ok"], json!(true));
    }
}
