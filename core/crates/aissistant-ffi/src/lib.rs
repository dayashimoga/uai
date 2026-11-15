//! FFI bridge, async event stream sinks, and diagnostics for Universal AI Assistant.

pub mod api;
pub mod c_abi;
pub mod error;
pub mod events;

pub use api::{
    bridge_get_capabilities, bridge_get_health, bridge_get_setting, bridge_query_audit,
    bridge_set_setting, bridge_update_permission, bridge_verify_audit, get_bridge, init_bridge,
    shutdown_bridge, AuditIntegrityReport, AuditItem, CapabilityItem, CoreBridge, HealthReport,
    HealthStatus,
};
pub use error::{FfiError, FfiResult};
pub use events::{BridgeEvent, EventStreamSink};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use aissistant_core::SkillId;
    use aissistant_security::{Actor, AuditExecutionStatus, PolicyDecision};
    use std::ffi::{CStr, CString};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    static TEST_GLOBAL_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_in_memory_bridge_health_and_capabilities() {
        let bridge = CoreBridge::new_in_memory().unwrap();
        let health = bridge.health();

        assert!(health.database_ok);
        assert!(health.audit_ok);
        assert!(health.keystore_ok);
        assert!(health.total_capabilities > 0);

        let caps = bridge.capabilities();
        assert!(!caps.is_empty());
        let cap_names: Vec<String> = caps.iter().map(|c| c.name.clone()).collect();
        assert!(cap_names.contains(&"local_llm".to_string()));
        assert!(cap_names.contains(&"file_system".to_string()));
    }

    #[test]
    fn test_persistent_bridge_keystore_and_secrets() {
        let dir = tempdir().unwrap();
        let bridge = CoreBridge::new_with_path(dir.path()).unwrap();

        // Store and retrieve secret
        bridge.set_secret("openai.key", "sk-proj-test1234").unwrap();
        let val = bridge.get_secret("openai.key").unwrap();
        assert_eq!(val, Some("sk-proj-test1234".to_string()));

        // Check non-existent secret
        let missing = bridge.get_secret("nonexistent").unwrap();
        assert_eq!(missing, None);
    }

    #[test]
    fn test_capability_permission_update_and_event_broadcast() {
        let bridge = CoreBridge::new_in_memory().unwrap();
        let sink = bridge.event_sink();
        let (_sub_id, rx) = sink.subscribe(10);

        // Update capability permission
        let item = bridge.update_permission("file_system", true).unwrap();
        assert!(item.is_active);

        // Verify broadcast event received
        let evt = rx.recv().unwrap();
        match evt {
            BridgeEvent::CapabilityChanged {
                capability, status, ..
            } => {
                assert_eq!(capability, "file_system");
                assert_eq!(status, "supported");
            }
            other => panic!("Unexpected event: {:?}", other),
        }
    }

    #[test]
    fn test_audit_log_append_query_and_integrity_verification() {
        let bridge = CoreBridge::new_in_memory().unwrap();
        let sink = bridge.event_sink();
        let (_sub_id, rx) = sink.subscribe(10);

        let skill = SkillId::new("device.reboot").unwrap();
        let item = bridge
            .append_audit_entry(
                Actor::LocalUser,
                skill,
                "{\"force\":false}",
                PolicyDecision::Allow,
                AuditExecutionStatus::Success,
            )
            .unwrap();

        assert_eq!(item.sequence, 1);
        assert_eq!(item.actor, "local_user");
        assert_eq!(item.decision, "allow");

        // Verify broadcast
        let evt = rx.recv().unwrap();
        match evt {
            BridgeEvent::AuditLogAppended {
                sequence, actor, ..
            } => {
                assert_eq!(sequence, 1);
                assert_eq!(actor, "local_user");
            }
            other => panic!("Unexpected event: {:?}", other),
        }

        // Query audit entries
        let entries = bridge.query_audit_log(10, 0).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sequence, 1);

        // Verify integrity
        let report = bridge.verify_audit_integrity().unwrap();
        assert!(report.is_valid);
        assert_eq!(report.verified_count, 1);
        assert_eq!(report.head_hash, item.entry_hash);
    }

    #[test]
    fn test_settings_store_roundtrip_and_events() {
        let bridge = CoreBridge::new_in_memory().unwrap();
        let sink = bridge.event_sink();
        let (_sub_id, rx) = sink.subscribe(10);

        bridge.set_setting("assistant.voice_rate", "1.25").unwrap();
        let val = bridge.get_setting("assistant.voice_rate").unwrap();
        assert_eq!(val, Some("1.25".to_string()));

        let evt = rx.recv().unwrap();
        match evt {
            BridgeEvent::SettingChanged { key, .. } => {
                assert_eq!(key, "assistant.voice_rate");
            }
            other => panic!("Unexpected event: {:?}", other),
        }
    }

    #[test]
    fn test_event_stream_sink_concurrent_subscribers_and_callbacks() {
        let sink = Arc::new(EventStreamSink::new());

        // Channel subscriber
        let (sub_id, rx) = sink.subscribe(32);

        // Callback subscriber
        let callback_count = Arc::new(AtomicUsize::new(0));
        let count_clone = callback_count.clone();
        let cb_id = sink.add_callback(move |_evt| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        assert_eq!(sink.total_subscribers(), 2);

        // Broadcast test event
        sink.broadcast(BridgeEvent::ErrorOccurred {
            code: "TEST_ERR".into(),
            message: "Test error broadcast".into(),
            recoverable: true,
        });

        // Verify channel received event
        let evt = rx.recv().unwrap();
        assert_eq!(
            evt,
            BridgeEvent::ErrorOccurred {
                code: "TEST_ERR".into(),
                message: "Test error broadcast".into(),
                recoverable: true,
            }
        );

        // Verify callback fired
        assert_eq!(callback_count.load(Ordering::SeqCst), 1);

        // Unsubscribe
        sink.unsubscribe(sub_id);
        sink.remove_callback(cb_id);
        assert_eq!(sink.total_subscribers(), 0);
    }

    #[test]
    fn test_event_stream_sink_purges_disconnected_receivers() {
        let sink = EventStreamSink::new();

        {
            let (_sub_id, _rx) = sink.subscribe(10);
            assert_eq!(sink.total_subscribers(), 1);
            // _rx dropped here
        }

        // Broadcaster should prune the disconnected receiver
        sink.broadcast(BridgeEvent::HealthStatusChanged {
            status: "healthy".into(),
            details: vec![],
        });

        assert_eq!(sink.total_subscribers(), 0);
    }

    #[test]
    fn test_event_stream_sink_callback_panic_isolation() {
        let sink = EventStreamSink::new();

        // Panicking callback
        sink.add_callback(|_| {
            panic!("Intentional callback panic");
        });

        // Well-behaved callback
        let normal_called = Arc::new(AtomicUsize::new(0));
        let normal_clone = normal_called.clone();
        sink.add_callback(move |_| {
            normal_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Broadcast should not panic and well-behaved callback must still execute
        sink.broadcast(BridgeEvent::HealthStatusChanged {
            status: "healthy".into(),
            details: vec![],
        });

        assert_eq!(normal_called.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_c_abi_round_trip() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();

        // Init bridge in-memory via C-ABI
        let init_res = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(init_res, 0);

        // Get Health JSON
        let health_ptr = c_abi::bridge_c_get_health();
        assert!(!health_ptr.is_null());
        let health_str = unsafe { CStr::from_ptr(health_ptr).to_str().unwrap() };
        assert!(health_str.contains("healthy"));
        unsafe { c_abi::bridge_c_free_string(health_ptr) };

        // Get Capabilities JSON
        let caps_ptr = c_abi::bridge_c_get_capabilities();
        assert!(!caps_ptr.is_null());
        let caps_str = unsafe { CStr::from_ptr(caps_ptr).to_str().unwrap() };
        assert!(caps_str.contains("local_llm"));
        unsafe { c_abi::bridge_c_free_string(caps_ptr) };

        // Set Setting via C-ABI
        let key_c = CString::new("theme.mode").unwrap();
        let val_c = CString::new("\"dark\"").unwrap();
        let set_res = unsafe { c_abi::bridge_c_set_setting(key_c.as_ptr(), val_c.as_ptr()) };
        assert_eq!(set_res, 0);

        // Get Setting via C-ABI
        let get_ptr = unsafe { c_abi::bridge_c_get_setting(key_c.as_ptr()) };
        assert!(!get_ptr.is_null());
        let get_str = unsafe { CStr::from_ptr(get_ptr).to_str().unwrap() };
        assert_eq!(get_str, "\"dark\"");
        unsafe { c_abi::bridge_c_free_string(get_ptr) };

        // Update Permission via C-ABI
        let cap_c = CString::new("file_system").unwrap();
        let update_ptr = unsafe { c_abi::bridge_c_update_permission(cap_c.as_ptr(), 1) };
        assert!(!update_ptr.is_null());
        let update_str = unsafe { CStr::from_ptr(update_ptr).to_str().unwrap() };
        assert!(update_str.contains("supported"));
        unsafe { c_abi::bridge_c_free_string(update_ptr) };

        // Verify Audit via C-ABI
        let audit_ptr = c_abi::bridge_c_verify_audit();
        assert!(!audit_ptr.is_null());
        let audit_str = unsafe { CStr::from_ptr(audit_ptr).to_str().unwrap() };
        assert!(audit_str.contains("is_valid"));
        unsafe { c_abi::bridge_c_free_string(audit_ptr) };

        // Shutdown
        let shut_res = c_abi::bridge_c_shutdown();
        assert_eq!(shut_res, 0);
    }

    #[test]
    fn test_c_abi_null_pointer_safety() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();

        let ptr = unsafe { c_abi::bridge_c_get_setting(std::ptr::null()) };
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert!(s.contains("Null pointer provided"));
        unsafe { c_abi::bridge_c_free_string(ptr) };

        let set_res = unsafe { c_abi::bridge_c_set_setting(std::ptr::null(), std::ptr::null()) };
        assert_eq!(set_res, -1);
    }

    #[test]
    fn test_error_descriptor_conversion() {
        use aissistant_core::{ErrorCategory, ErrorCode, ErrorDescriptor};

        let err1 = FfiError::Security("Denied by test policy".into());
        let desc1: ErrorDescriptor = err1.into();
        assert_eq!(desc1.code, ErrorCode::CorePermissionDenied);
        assert_eq!(desc1.category, ErrorCategory::Security);

        let err2 = FfiError::InvalidParameter("Missing argument".into());
        let desc2: ErrorDescriptor = err2.into();
        assert_eq!(desc2.code, ErrorCode::CoreConfigInvalid);
        assert_eq!(desc2.category, ErrorCategory::Validation);

        let err3 = FfiError::NotInitialized;
        let desc3: ErrorDescriptor = err3.into();
        assert_eq!(desc3.category, ErrorCategory::Configuration);
    }

    #[test]
    fn test_global_bridge_lifecycle_and_shutdown() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();

        // Ensure clean initial state
        let _ = shutdown_bridge();

        // Getting uninitialized bridge must fail
        assert!(matches!(get_bridge(), Err(FfiError::NotInitialized)));

        // Init bridge in-memory
        init_bridge(None).unwrap();
        let bridge = get_bridge().unwrap();
        assert!(bridge.health().database_ok);

        // Idempotent init
        init_bridge(None).unwrap();

        // Shutdown
        shutdown_bridge().unwrap();
        assert!(matches!(get_bridge(), Err(FfiError::NotInitialized)));
    }

    #[test]
    fn test_evaluate_policy_via_bridge() {
        use aissistant_security::{PolicyContext, RiskTier, TaintTracker};

        let bridge = CoreBridge::new_in_memory().unwrap();
        let tracker = TaintTracker::new();
        let ctx = PolicyContext::llm(tracker.state().clone());

        let skill_read = SkillId::new("system.time").unwrap();
        let dec_read = bridge
            .evaluate_policy(&skill_read, RiskTier::ReadOnly, &ctx)
            .unwrap();
        assert!(dec_read.is_allowed());

        let skill_file = SkillId::new("file.delete").unwrap();
        let dec_file = bridge
            .evaluate_policy(&skill_file, RiskTier::Destructive, &ctx)
            .unwrap();
        assert!(dec_file.requires_confirmation());
    }

    #[test]
    fn test_audit_chain_tamper_detection_via_bridge() {
        let bridge = CoreBridge::new_in_memory().unwrap();

        let skill = SkillId::new("device.reboot").unwrap();
        bridge
            .append_audit_entry(
                Actor::LocalUser,
                skill,
                "{}",
                PolicyDecision::Allow,
                AuditExecutionStatus::Success,
            )
            .unwrap();

        let report = bridge.verify_audit_integrity().unwrap();
        assert!(report.is_valid);
        assert_eq!(report.verified_count, 1);

        // Tamper directly via raw SQL on the underlying DB handle
        bridge
            .db()
            .with_conn(|conn| {
                conn.execute(
                    "UPDATE security_audit_log SET arguments_redacted_json = '{\"tampered\":true}' WHERE rowid = 1;",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        // Bridge audit verification must detect the tampering
        let tampered_report = bridge.verify_audit_integrity().unwrap();
        assert!(!tampered_report.is_valid);
        assert!(tampered_report.error_message.is_some());

        // Health report must reflect degraded/critical state
        let health = bridge.health();
        assert!(!health.audit_ok);
        assert_eq!(health.status, HealthStatus::Critical);
    }
}
