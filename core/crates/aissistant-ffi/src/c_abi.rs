//! Safe C-ABI exports for native runners and foreign language bindings.
//!
//! All C-ABI entry points catch panics, handle null pointers gracefully,
//! and enforce strict ownership rules for returned C strings.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};

use aissistant_core::ErrorDescriptor;

use crate::api::{
    bridge_get_capabilities, bridge_get_health, bridge_get_lifecycle_state, bridge_get_setting,
    bridge_notify_memory_pressure, bridge_query_audit, bridge_request_shutdown, bridge_set_setting,
    bridge_update_permission, bridge_verify_audit, init_bridge, shutdown_bridge,
};
use crate::error::FfiError;
use crate::lifecycle::{MemoryPressureLevel, ShutdownReason};

/// Converts a Rust `String` into an owned heap-allocated C string.
/// Caller is responsible for freeing via `bridge_c_free_string`.
fn string_to_c_char(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Serializes any `T: Serialize` into a C JSON string, or returns a serialized error descriptor on failure.
fn serialize_to_c_json<T: serde::Serialize>(result: Result<T, FfiError>) -> *mut c_char {
    match result {
        Ok(val) => match serde_json::to_string(&val) {
            Ok(json) => string_to_c_char(json),
            Err(e) => {
                let desc = FfiError::Serialization(e.to_string());
                let ed: ErrorDescriptor = desc.into();
                string_to_c_char(
                    serde_json::to_string(&ed).unwrap_or_else(|_| "{\"error\":\"fatal\"}".into()),
                )
            }
        },
        Err(err) => {
            let ed: ErrorDescriptor = err.into();
            string_to_c_char(
                serde_json::to_string(&ed).unwrap_or_else(|_| "{\"error\":\"fatal\"}".into()),
            )
        }
    }
}

/// Safely inspects a raw C string pointer.
///
/// # Safety
/// The pointer must be either null or point to a valid, null-terminated C string.
unsafe fn c_char_to_str<'a>(ptr: *const c_char) -> Result<&'a str, FfiError> {
    if ptr.is_null() {
        return Err(FfiError::InvalidParameter(
            "Null pointer provided to C-ABI function".to_string(),
        ));
    }
    // SAFETY: The pointer was checked for null above and caller contract requires
    // a valid null-terminated C string for the lifetime 'a.
    let c_str = unsafe { CStr::from_ptr(ptr) };
    c_str
        .to_str()
        .map_err(|e| FfiError::InvalidParameter(format!("Invalid UTF-8: {e}")))
}

/// Initializes the global core bridge singleton with an optional data directory.
/// Returns 0 on success, negative on error.
///
/// # Safety
/// If `data_dir` is non-null, it must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_init(data_dir: *const c_char) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let dir_str = if data_dir.is_null() {
            None
        } else {
            // SAFETY: data_dir is non-null and caller guarantees valid null-terminated string.
            match unsafe { c_char_to_str(data_dir) } {
                Ok(s) => Some(s),
                Err(_) => return -2,
            }
        };

        match init_bridge(dir_str) {
            Ok(()) => 0,
            Err(_) => -1,
        }
    }));

    result.unwrap_or(-3)
}

/// Shuts down the global core bridge singleton.
/// Returns 0 on success.
#[no_mangle]
pub extern "C" fn bridge_c_shutdown() -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| match shutdown_bridge() {
        Ok(()) => 0,
        Err(_) => -1,
    }));

    result.unwrap_or(-3)
}

/// Retrieves health diagnostics formatted as a JSON string.
/// The returned string MUST be freed with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_get_health() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_get_health));
    match result {
        Ok(res) => serialize_to_c_json(res),
        Err(_) => {
            serialize_to_c_json::<()>(Err(FfiError::PanicCaught("Panic in get_health".into())))
        }
    }
}

/// Retrieves the platform capability matrix formatted as a JSON string.
/// The returned string MUST be freed with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_get_capabilities() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_get_capabilities));
    match result {
        Ok(res) => serialize_to_c_json(res),
        Err(_) => serialize_to_c_json::<()>(Err(FfiError::PanicCaught(
            "Panic in get_capabilities".into(),
        ))),
    }
}

/// Updates permission state for a capability.
/// Returns updated `CapabilityItem` JSON. The returned string MUST be freed with `bridge_c_free_string`.
///
/// # Safety
/// `capability` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_update_permission(
    capability: *const c_char,
    granted: i32,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: capability must be a valid null-terminated C string per function contract.
        let cap_str = unsafe { c_char_to_str(capability) }?;
        bridge_update_permission(cap_str, granted != 0)
    }));

    match result {
        Ok(res) => serialize_to_c_json(res),
        Err(_) => serialize_to_c_json::<()>(Err(FfiError::PanicCaught(
            "Panic in update_permission".into(),
        ))),
    }
}

/// Queries historical audit records.
/// Returns an array of `AuditItem` as JSON. The returned string MUST be freed with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_query_audit(limit: u32, offset: u32) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        bridge_query_audit(limit as usize, offset as usize)
    }));

    match result {
        Ok(res) => serialize_to_c_json(res),
        Err(_) => {
            serialize_to_c_json::<()>(Err(FfiError::PanicCaught("Panic in query_audit".into())))
        }
    }
}

/// Cryptographically verifies the audit log chain.
/// Returns `AuditIntegrityReport` JSON. The returned string MUST be freed with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_verify_audit() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_verify_audit));
    match result {
        Ok(res) => serialize_to_c_json(res),
        Err(_) => {
            serialize_to_c_json::<()>(Err(FfiError::PanicCaught("Panic in verify_audit".into())))
        }
    }
}

/// Retrieves a persistent configuration setting formatted as JSON string.
/// Returns JSON string on success, NULL if not found, or error descriptor JSON on failure.
/// The returned string (if non-null) MUST be freed with `bridge_c_free_string`.
///
/// # Safety
/// `key` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_get_setting(key: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: key must be a valid null-terminated C string per function contract.
        let key_str = unsafe { c_char_to_str(key) }?;
        bridge_get_setting(key_str)
    }));

    match result {
        Ok(Ok(Some(json_str))) => string_to_c_char(json_str),
        Ok(Ok(None)) => std::ptr::null_mut(),
        Ok(Err(err)) => {
            let ed: ErrorDescriptor = err.into();
            string_to_c_char(
                serde_json::to_string(&ed).unwrap_or_else(|_| "{\"error\":\"fatal\"}".into()),
            )
        }
        Err(_) => {
            serialize_to_c_json::<()>(Err(FfiError::PanicCaught("Panic in get_setting".into())))
        }
    }
}

/// Stores a configuration setting as JSON.
/// Returns 0 on success, negative on error.
///
/// # Safety
/// `key` and `value_json` must point to valid null-terminated UTF-8 C strings.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_set_setting(
    key: *const c_char,
    value_json: *const c_char,
) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: key and value_json must be valid null-terminated C strings per contract.
        let key_str = unsafe { c_char_to_str(key) }?;
        let val_str = unsafe { c_char_to_str(value_json) }?;
        bridge_set_setting(key_str, val_str)
    }));

    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Retrieves the current lifecycle state formatted as a JSON string.
/// The returned string MUST be freed with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_get_lifecycle_state() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_get_lifecycle_state));
    match result {
        Ok(res) => serialize_to_c_json(res),
        Err(_) => serialize_to_c_json::<()>(Err(FfiError::PanicCaught(
            "Panic in get_lifecycle_state".into(),
        ))),
    }
}

/// Requests graceful shutdown with an optional reason string.
/// Returns 0 on success, negative on error.
///
/// # Safety
/// If `reason` is non-null, it must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_request_shutdown(reason: *const c_char) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let shutdown_reason = if reason.is_null() {
            ShutdownReason::UserRequested
        } else {
            let r_str = unsafe { c_char_to_str(reason) }?;
            match r_str.to_ascii_lowercase().as_str() {
                "os_signal" => ShutdownReason::OsSignal,
                "memory_pressure" => ShutdownReason::MemoryPressure,
                "fatal_error" => ShutdownReason::FatalError,
                "testing" => ShutdownReason::Testing,
                _ => ShutdownReason::UserRequested,
            }
        };

        bridge_request_shutdown(shutdown_reason)
    }));

    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Delivers a memory pressure notification (level: 0 = moderate, 1 = critical).
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn bridge_c_notify_memory_pressure(level: i32) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let p_level = if level >= 1 {
            MemoryPressureLevel::Critical
        } else {
            MemoryPressureLevel::Moderate
        };
        bridge_notify_memory_pressure(p_level)
    }));

    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Frees a C string previously allocated by `aissistant-ffi`.
///
/// # Safety
/// The pointer must either be null or have been returned by an `aissistant-ffi` C-ABI function.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        // SAFETY: ptr is non-null and was allocated via CString::into_raw in this crate.
        // Dropping the reconstructed CString reclaims the memory safely.
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}
