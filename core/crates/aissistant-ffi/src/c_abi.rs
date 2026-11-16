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
    bridge_automation_clear_history, bridge_automation_delete, bridge_automation_get,
    bridge_automation_history, bridge_automation_list, bridge_automation_save,
    bridge_automation_toggle, bridge_automation_trigger, bridge_delete_persona,
    bridge_delete_preference, bridge_device_cancel_pairing, bridge_device_confirm_pairing,
    bridge_device_discovered_peers, bridge_device_paired_list, bridge_device_revoke,
    bridge_device_set_permission, bridge_device_simulate_peer, bridge_device_start_pairing,
    bridge_device_unpair, bridge_emergency_lockdown, bridge_get_active_persona,
    bridge_get_capabilities, bridge_get_health, bridge_get_lifecycle_state, bridge_get_preference,
    bridge_get_setting, bridge_home_entity_control, bridge_home_entity_delete,
    bridge_home_entity_get, bridge_home_entity_list, bridge_home_entity_upsert,
    bridge_interpreter_clear_session, bridge_interpreter_get_turns,
    bridge_interpreter_start_session, bridge_interpreter_submit_turn,
    bridge_interpreter_swap_languages, bridge_is_lockdown_active, bridge_knowledge_delete_document,
    bridge_knowledge_index_document, bridge_knowledge_list_documents, bridge_knowledge_search,
    bridge_lift_emergency_lockdown, bridge_list_personas, bridge_list_preferences,
    bridge_model_benchmark_run, bridge_model_load, bridge_model_snapshot, bridge_model_unload,
    bridge_notify_memory_pressure, bridge_query_audit, bridge_request_shutdown,
    bridge_rotate_master_key, bridge_save_persona, bridge_set_active_persona,
    bridge_set_preference, bridge_set_setting, bridge_translate_text, bridge_update_permission,
    bridge_verify_audit, bridge_vision_ocr_extract, bridge_voice_inject_audio,
    bridge_voice_set_keywords, bridge_voice_set_mode, bridge_voice_start_listening,
    bridge_voice_stop_listening, init_bridge, shutdown_bridge, AutomationRuleItem,
    SmartHomeCommandItem, SmartHomeEntityItem, UserPersonaItem,
};
use crate::device_bridge::DiscoveredPeerItem;
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

/// Starts voice listening (mode: 0 = PushToTalk, 1 = WakeWord).
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn bridge_c_voice_start_listening(mode: u32) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| bridge_voice_start_listening(mode)));
    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Stops voice listening and returns to idle.
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn bridge_c_voice_stop_listening() -> i32 {
    let result = catch_unwind(AssertUnwindSafe(bridge_voice_stop_listening));
    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Sets active voice listening mode (mode: 0 = PushToTalk, 1 = WakeWord).
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn bridge_c_voice_set_mode(mode: u32) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| bridge_voice_set_mode(mode)));
    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Sets active wake-word keywords from a JSON array string.
/// Returns 0 on success, negative on error.
///
/// # Safety
/// `keywords_json` must be a valid, null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_voice_set_keywords(keywords_json: *const c_char) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let json_str = unsafe { c_char_to_str(keywords_json) }?;
        bridge_voice_set_keywords(json_str)
    }));

    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Injects audio samples into the voice pipeline for processing.
/// Returns 0 on success, negative on error.
///
/// # Safety
/// `samples` must point to a valid slice of `len` 32-bit floats.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_voice_inject_audio(samples: *const f32, len: usize) -> i32 {
    if samples.is_null() || len == 0 {
        return -1;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let slice = unsafe { std::slice::from_raw_parts(samples, len) };
        bridge_voice_inject_audio(slice)
    }));

    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Returns a JSON string representing the model catalog snapshot and detected hardware parameters.
/// Caller must free returned pointer with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_model_snapshot() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_model_snapshot));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_model_snapshot".into(),
        ))
    }))
}

/// Loads a model into memory by ID.
/// Returns 0 on success, negative on error.
///
/// # Safety
/// `model_id` must point to a valid, null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_model_load(model_id: *const c_char) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(model_id) }?;
        bridge_model_load(id_str)
    }));

    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Unloads the active model from memory.
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn bridge_c_model_unload() -> i32 {
    let result = catch_unwind(AssertUnwindSafe(bridge_model_unload));
    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Executes an on-device inference benchmark and returns JSON metrics report.
/// Caller must free returned pointer with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_model_benchmark_run(
    prompt_tokens: usize,
    generate_tokens: usize,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        bridge_model_benchmark_run(prompt_tokens, generate_tokens)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_model_benchmark_run".into(),
        ))
    }))
}

/// Returns discovered network peers as a JSON array.
/// Caller must free returned pointer with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_device_discovered_peers() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_device_discovered_peers));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_device_discovered_peers".into(),
        ))
    }))
}

/// Returns paired devices and their assigned permissions as a JSON array.
/// Caller must free returned pointer with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_device_paired_list() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_device_paired_list));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_device_paired_list".into(),
        ))
    }))
}

/// Initiates a pairing ceremony with a discovered peer.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `peer_instance` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_device_start_pairing(
    peer_instance: *const c_char,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let peer_str = unsafe { c_char_to_str(peer_instance) }?;
        bridge_device_start_pairing(peer_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_device_start_pairing".into(),
        ))
    }))
}

/// Confirms pairing with the 6-digit SAS code.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `sas_code` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_device_confirm_pairing(sas_code: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let code_str = unsafe { c_char_to_str(sas_code) }?;
        bridge_device_confirm_pairing(code_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_device_confirm_pairing".into(),
        ))
    }))
}

/// Cancels an active pairing ceremony.
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn bridge_c_device_cancel_pairing() -> i32 {
    let result = catch_unwind(AssertUnwindSafe(bridge_device_cancel_pairing));
    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Sets or updates a capability permission level for a paired device.
/// Returns 0 on success, negative on error.
///
/// # Safety
/// `device_id`, `capability`, and `level` must point to valid null-terminated UTF-8 C strings.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_device_set_permission(
    device_id: *const c_char,
    capability: *const c_char,
    level: *const c_char,
) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(device_id) }?;
        let cap_str = unsafe { c_char_to_str(capability) }?;
        let lvl_str = unsafe { c_char_to_str(level) }?;
        bridge_device_set_permission(id_str, cap_str, lvl_str)
    }));
    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Unpairs a device and revokes its permissions.
/// Returns 0 on success, negative on error.
///
/// # Safety
/// `device_id` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_device_unpair(device_id: *const c_char) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(device_id) }?;
        bridge_device_unpair(id_str)
    }));
    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Simulates a discovered peer from JSON input.
/// Returns 0 on success, negative on error.
///
/// # Safety
/// `peer_json` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_device_simulate_peer(peer_json: *const c_char) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let json_str = unsafe { c_char_to_str(peer_json) }?;
        let item: DiscoveredPeerItem =
            serde_json::from_str(json_str).map_err(|e| FfiError::Serialization(e.to_string()))?;
        bridge_device_simulate_peer(item)
    }));
    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Returns all indexed documents as a JSON array.
/// Caller must free returned pointer with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_knowledge_list() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_knowledge_list_documents));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_knowledge_list".into(),
        ))
    }))
}

/// Ingests and chunks a local document text. Returns the created document ID string.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `path`, `file_name`, `mime_type`, and `content` must point to valid null-terminated UTF-8 C strings.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_knowledge_index(
    path: *const c_char,
    file_name: *const c_char,
    mime_type: *const c_char,
    content: *const c_char,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let path_str = unsafe { c_char_to_str(path) }?;
        let name_str = unsafe { c_char_to_str(file_name) }?;
        let mime_str = unsafe { c_char_to_str(mime_type) }?;
        let content_str = unsafe { c_char_to_str(content) }?;
        bridge_knowledge_index_document(path_str, name_str, mime_str, content_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_knowledge_index".into(),
        ))
    }))
}

/// Deletes an indexed document by ID.
/// Returns 0 on success, negative on error.
///
/// # Safety
/// `doc_id` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_knowledge_delete(doc_id: *const c_char) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(doc_id) }?;
        bridge_knowledge_delete_document(id_str)
    }));
    match result {
        Ok(Ok(true)) => 0,
        _ => -1,
    }
}

/// Performs hybrid search over document chunks and returns grounded citations as a JSON array.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `query` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_knowledge_search(
    query: *const c_char,
    limit: usize,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let q_str = unsafe { c_char_to_str(query) }?;
        bridge_knowledge_search(q_str, limit)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_knowledge_search".into(),
        ))
    }))
}

/// Extracts recognized text, bounding boxes, and metrics from an image byte buffer.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `image_ptr` must point to a readable array of at least `image_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_vision_ocr_extract(
    image_ptr: *const u8,
    image_len: usize,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if image_ptr.is_null() || image_len == 0 {
            return Err(FfiError::InvalidParameter(
                "Image buffer pointer is null or length is 0".into(),
            ));
        }
        let slice = unsafe { std::slice::from_raw_parts(image_ptr, image_len) };
        bridge_vision_ocr_extract(slice)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_vision_ocr_extract".into(),
        ))
    }))
}

/// Translates text between two ISO 639-1 languages.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `text`, `source_lang`, and `target_lang` must point to valid null-terminated UTF-8 C strings.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_translate_text(
    text: *const c_char,
    source_lang: *const c_char,
    target_lang: *const c_char,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let t_str = unsafe { c_char_to_str(text) }?;
        let src_str = unsafe { c_char_to_str(source_lang) }?;
        let tgt_str = unsafe { c_char_to_str(target_lang) }?;
        bridge_translate_text(t_str, src_str, tgt_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_translate_text".into(),
        ))
    }))
}

/// Starts or reconfigures a live voice interpreter session.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `party_a_lang` and `party_b_lang` must point to valid null-terminated UTF-8 C strings.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_interpreter_start(
    party_a_lang: *const c_char,
    party_b_lang: *const c_char,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let a_str = unsafe { c_char_to_str(party_a_lang) }?;
        let b_str = unsafe { c_char_to_str(party_b_lang) }?;
        bridge_interpreter_start_session(a_str, b_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_interpreter_start".into(),
        ))
    }))
}

/// Submits an utterance turn to the active interpreter session.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `text` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_interpreter_submit_turn(text: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let t_str = unsafe { c_char_to_str(text) }?;
        bridge_interpreter_submit_turn(t_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_interpreter_submit_turn".into(),
        ))
    }))
}

/// Returns all completed turns in the active interpreter session.
/// Caller must free returned pointer with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_interpreter_get_turns() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_interpreter_get_turns));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_interpreter_get_turns".into(),
        ))
    }))
}

/// Swaps Party A and Party B languages in the active interpreter session.
/// Caller must free returned pointer with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_interpreter_swap_languages() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_interpreter_swap_languages));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_interpreter_swap_languages".into(),
        ))
    }))
}

/// Clears active interpreter session history.
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn bridge_c_interpreter_clear() -> i32 {
    let result = catch_unwind(AssertUnwindSafe(bridge_interpreter_clear_session));
    match result {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Returns active persona as JSON.
/// Caller must free returned pointer with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_persona_get_active() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_get_active_persona));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_persona_get_active".into(),
        ))
    }))
}

/// Returns list of all user personas as JSON array.
/// Caller must free returned pointer with `bridge_c_free_string`.
#[no_mangle]
pub extern "C" fn bridge_c_persona_list() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_list_personas));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_persona_list".into(),
        ))
    }))
}

/// Saves or updates a user persona from JSON.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `persona_json` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_persona_save(persona_json: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let json_str = unsafe { c_char_to_str(persona_json) }?;
        let persona: UserPersonaItem = serde_json::from_str(json_str)
            .map_err(|e| FfiError::Serialization(format!("Invalid persona JSON: {e}")))?;
        bridge_save_persona(persona)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_persona_save".into(),
        ))
    }))
}

/// Sets active persona by ID.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `id` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_persona_set_active(id: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(id) }?;
        bridge_set_active_persona(id_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_persona_set_active".into(),
        ))
    }))
}

/// Deletes a user persona by ID.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `id` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_persona_delete(id: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(id) }?;
        bridge_delete_persona(id_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_persona_delete".into(),
        ))
    }))
}

/// Retrieves an explicit user preference by key.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `key` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_preference_get(key: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let key_str = unsafe { c_char_to_str(key) }?;
        bridge_get_preference(key_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_preference_get".into(),
        ))
    }))
}

/// Sets or updates an explicit user preference.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `key`, `value_json`, and `category` must point to valid null-terminated UTF-8 C strings.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_preference_set(
    key: *const c_char,
    value_json: *const c_char,
    category: *const c_char,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let key_str = unsafe { c_char_to_str(key) }?;
        let value_str = unsafe { c_char_to_str(value_json) }?;
        let cat_str = unsafe { c_char_to_str(category) }?;
        bridge_set_preference(key_str, value_str, cat_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_preference_set".into(),
        ))
    }))
}

/// Lists explicit user preferences, optionally filtered by category.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `category` can be null or a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_preference_list(category: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let cat_str = if category.is_null() {
            None
        } else {
            Some(unsafe { c_char_to_str(category) }?)
        };
        bridge_list_preferences(cat_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_preference_list".into(),
        ))
    }))
}

/// Deletes an explicit user preference by key.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `key` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_preference_delete(key: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let key_str = unsafe { c_char_to_str(key) }?;
        bridge_delete_preference(key_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_preference_delete".into(),
        ))
    }))
}

/// Lists automated workflow rules.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// Caller must free returned pointer with `bridge_c_free_string`.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_automation_list(enabled_only: bool) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| bridge_automation_list(enabled_only)));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_automation_list".into(),
        ))
    }))
}

/// Retrieves an automated workflow rule by ID.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `id` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_automation_get(id: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(id) }?;
        bridge_automation_get(id_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_automation_get".into(),
        ))
    }))
}

/// Saves or updates an automated workflow rule from serialized JSON.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `rule_json` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_automation_save(rule_json: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let json_str = unsafe { c_char_to_str(rule_json) }?;
        let item: AutomationRuleItem =
            serde_json::from_str(json_str).map_err(|e| FfiError::Serialization(e.to_string()))?;
        bridge_automation_save(item)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_automation_save".into(),
        ))
    }))
}

/// Toggles the enabled state of an automated workflow rule.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `id` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_automation_toggle(
    id: *const c_char,
    enabled: bool,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(id) }?;
        bridge_automation_toggle(id_str, enabled)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_automation_toggle".into(),
        ))
    }))
}

/// Deletes an automated workflow rule.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `id` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_automation_delete(id: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(id) }?;
        bridge_automation_delete(id_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_automation_delete".into(),
        ))
    }))
}

/// Triggers manual execution of an automated workflow rule.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `id` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_automation_trigger(id: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(id) }?;
        bridge_automation_trigger(id_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_automation_trigger".into(),
        ))
    }))
}

/// Lists execution history for an automation rule.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `automation_id` can be null or a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_automation_history(
    automation_id: *const c_char,
    limit: usize,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let auto_id = if automation_id.is_null() {
            None
        } else {
            Some(unsafe { c_char_to_str(automation_id) }?)
        };
        bridge_automation_history(auto_id, limit)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_automation_history".into(),
        ))
    }))
}

/// Clears execution history for an automation rule.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `automation_id` can be null or a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_automation_clear_history(
    automation_id: *const c_char,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let auto_id = if automation_id.is_null() {
            None
        } else {
            Some(unsafe { c_char_to_str(automation_id) }?)
        };
        bridge_automation_clear_history(auto_id)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_automation_clear_history".into(),
        ))
    }))
}

/// Lists registered smart-home entities, optionally filtered by domain.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `domain` can be null or a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_home_entity_list(domain: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let dom = if domain.is_null() {
            None
        } else {
            Some(unsafe { c_char_to_str(domain) }?)
        };
        bridge_home_entity_list(dom)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_home_entity_list".into(),
        ))
    }))
}

/// Retrieves a registered smart-home entity by ID.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `id` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_home_entity_get(id: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(id) }?;
        bridge_home_entity_get(id_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_home_entity_get".into(),
        ))
    }))
}

/// Saves or updates a smart-home entity record.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `item_json` must point to a valid null-terminated UTF-8 JSON string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_home_entity_upsert(item_json: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let json_str = unsafe { c_char_to_str(item_json) }?;
        let item: SmartHomeEntityItem = serde_json::from_str(json_str).map_err(|e| {
            FfiError::Serialization(format!("Invalid SmartHomeEntityItem JSON: {e}"))
        })?;
        bridge_home_entity_upsert(item)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_home_entity_upsert".into(),
        ))
    }))
}

/// Deletes a registered smart-home entity by ID.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `id` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_home_entity_delete(id: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(id) }?;
        bridge_home_entity_delete(id_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_home_entity_delete".into(),
        ))
    }))
}

/// Controls a local smart-home device.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `cmd_json` must point to a valid null-terminated UTF-8 JSON string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_home_entity_control(cmd_json: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let json_str = unsafe { c_char_to_str(cmd_json) }?;
        let cmd: SmartHomeCommandItem = serde_json::from_str(json_str).map_err(|e| {
            FfiError::Serialization(format!("Invalid SmartHomeCommandItem JSON: {e}"))
        })?;
        bridge_home_entity_control(cmd)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_home_entity_control".into(),
        ))
    }))
}

/// Rotates the master AEAD encryption key.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// This function catches all panics internally. The returned pointer must be freed with `bridge_c_free_string`.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_rotate_master_key() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_rotate_master_key));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_rotate_master_key".into(),
        ))
    }))
}

/// Revokes cryptographic trust for a paired peer device.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `device_id` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_device_revoke(device_id: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let id_str = unsafe { c_char_to_str(device_id) }?;
        bridge_device_revoke(id_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_device_revoke".into(),
        ))
    }))
}

/// Triggers immediate offline emergency lockdown.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// `reason` must point to a valid null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_emergency_lockdown(reason: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let reason_str = unsafe { c_char_to_str(reason) }?;
        bridge_emergency_lockdown(reason_str)
    }));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_emergency_lockdown".into(),
        ))
    }))
}

/// Lifts active emergency lockdown state.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// This function catches all panics internally. The returned pointer must be freed with `bridge_c_free_string`.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_lift_emergency_lockdown() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_lift_emergency_lockdown));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_lift_emergency_lockdown".into(),
        ))
    }))
}

/// Returns whether emergency lockdown is currently active.
/// Caller must free returned pointer with `bridge_c_free_string`.
///
/// # Safety
/// This function catches all panics internally. The returned pointer must be freed with `bridge_c_free_string`.
#[no_mangle]
pub unsafe extern "C" fn bridge_c_is_lockdown_active() -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(bridge_is_lockdown_active));
    serialize_to_c_json(result.unwrap_or_else(|_| {
        Err(FfiError::PanicCaught(
            "Panic in bridge_c_is_lockdown_active".into(),
        ))
    }))
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
