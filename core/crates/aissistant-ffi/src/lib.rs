//! FFI bridge, async event stream sinks, and diagnostics for Universal AI Assistant.

pub mod api;
pub mod c_abi;
pub mod device_bridge;
pub mod error;
pub mod events;
pub mod lifecycle;
pub mod model_bridge;
pub mod voice_bridge;

pub use api::{
    bridge_device_cancel_pairing, bridge_device_confirm_pairing, bridge_device_discovered_peers,
    bridge_device_paired_list, bridge_device_set_permission, bridge_device_simulate_peer,
    bridge_device_start_pairing, bridge_device_unpair, bridge_get_capabilities, bridge_get_health,
    bridge_get_lifecycle_state, bridge_get_setting, bridge_knowledge_delete_document,
    bridge_knowledge_index_document, bridge_knowledge_list_documents, bridge_knowledge_search,
    bridge_model_benchmark_run, bridge_model_load, bridge_model_snapshot, bridge_model_unload,
    bridge_notify_memory_pressure, bridge_query_audit, bridge_request_shutdown, bridge_set_setting,
    bridge_update_permission, bridge_verify_audit, bridge_vision_ocr_extract,
    bridge_voice_inject_audio, bridge_voice_set_keywords, bridge_voice_set_mode,
    bridge_voice_start_listening, bridge_voice_stop_listening, get_bridge, init_bridge,
    shutdown_bridge, AuditIntegrityReport, AuditItem, CapabilityItem, CoreBridge,
    DocumentRecordItem, HealthReport, HealthStatus, KnowledgeCitationItem, OcrBoundingBoxItem,
    OcrDocumentItem, OcrTextLineItem, OcrWordItem,
};
pub use device_bridge::{
    DeviceSupervisor, DiscoveredPeerItem, PairedDeviceItem, PairingSessionInfo,
    PairingStateSnapshot, PeerPermissionItem,
};
pub use error::{FfiError, FfiResult};
pub use events::{BridgeEvent, EventStreamSink};
pub use lifecycle::{
    CancellationToken, CoreRuntime, LifecycleState, MemoryPressureLevel, ShutdownReason,
};
pub use model_bridge::{
    BenchmarkReport, HardwareOverview, ModelCatalogItem, ModelManagerSnapshot, ModelSupervisor,
};
pub use voice_bridge::{VoiceListeningMode, VoicePipelineSupervisor};

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

    #[test]
    fn test_lifecycle_state_transitions_and_validation() {
        let sink = Arc::new(EventStreamSink::new());
        let (_sub, rx) = sink.subscribe(10);
        let runtime = CoreRuntime::new(sink);

        assert_eq!(runtime.state(), LifecycleState::Uninitialized);

        // Invalid jump Uninitialized -> Running must fail
        let invalid = runtime.transition_to(LifecycleState::Running, None);
        assert!(matches!(
            invalid,
            Err(FfiError::InvalidStateTransition { .. })
        ));

        // Valid transition Uninitialized -> Starting
        runtime
            .transition_to(LifecycleState::Starting, Some("boot"))
            .unwrap();
        assert_eq!(runtime.state(), LifecycleState::Starting);

        let evt1 = rx.recv().unwrap();
        match evt1 {
            BridgeEvent::LifecycleStateChanged {
                previous,
                current,
                reason,
            } => {
                assert_eq!(previous, "uninitialized");
                assert_eq!(current, "starting");
                assert_eq!(reason, Some("boot".to_string()));
            }
            other => panic!("Unexpected event: {:?}", other),
        }

        // Starting -> Running
        runtime
            .transition_to(LifecycleState::Running, Some("ready"))
            .unwrap();
        assert!(runtime.state().is_operational());

        // Running -> Degraded
        runtime
            .transition_to(LifecycleState::Degraded, Some("ram_low"))
            .unwrap();
        assert!(runtime.state().is_operational());

        // Degraded -> ShuttingDown
        runtime
            .transition_to(LifecycleState::ShuttingDown, Some("user_exit"))
            .unwrap();
        assert!(!runtime.state().is_operational());

        // ShuttingDown -> Terminated
        runtime
            .transition_to(LifecycleState::Terminated, Some("clean_exit"))
            .unwrap();
        assert!(runtime.state().is_terminal());
    }

    #[test]
    fn test_background_task_tracking_and_graceful_shutdown() {
        let bridge = CoreBridge::new_in_memory().unwrap();
        assert_eq!(bridge.lifecycle_state(), LifecycleState::Running);

        // Spawn two cooperative background tasks
        let (task1, cancel1) = bridge.spawn_task("sync_worker_1").unwrap();
        let (task2, cancel2) = bridge.spawn_task("sync_worker_2").unwrap();

        assert_eq!(bridge.runtime().active_task_count(), 2);
        assert!(!cancel1.is_cancelled());
        assert!(!cancel2.is_cancelled());

        // Finish task 1
        bridge.finish_task(task1);
        assert_eq!(bridge.runtime().active_task_count(), 1);

        // In background thread, finish task 2 after a small delay
        let bridge_clone = bridge.runtime().clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            bridge_clone.finish_task(task2);
        });

        // Request shutdown with timeout
        bridge
            .request_shutdown(
                ShutdownReason::UserRequested,
                std::time::Duration::from_millis(1000),
            )
            .unwrap();

        assert_eq!(bridge.lifecycle_state(), LifecycleState::Terminated);
        assert!(cancel1.is_cancelled());
        assert!(cancel2.is_cancelled());
        assert_eq!(bridge.runtime().active_task_count(), 0);
    }

    #[test]
    fn test_rejection_of_tasks_during_shutdown() {
        let bridge = CoreBridge::new_in_memory().unwrap();
        bridge
            .request_shutdown(
                ShutdownReason::Testing,
                std::time::Duration::from_millis(100),
            )
            .unwrap();

        assert_eq!(bridge.lifecycle_state(), LifecycleState::Terminated);

        // Spawning new tasks after shutdown must be rejected
        let res = bridge.spawn_task("late_worker");
        assert!(matches!(res, Err(FfiError::ShuttingDown)));
    }

    #[test]
    fn test_memory_pressure_handling_and_sqlite_checkpoint() {
        let bridge = CoreBridge::new_in_memory().unwrap();
        let sink = bridge.event_sink();
        let (_sub, rx) = sink.subscribe(10);

        // Moderate memory pressure
        bridge
            .notify_memory_pressure(MemoryPressureLevel::Moderate)
            .unwrap();

        let evt1 = rx.recv().unwrap();
        match evt1 {
            BridgeEvent::MemoryPressure { level } => {
                assert_eq!(level, "moderate");
            }
            other => panic!("Unexpected event: {:?}", other),
        }

        // Critical memory pressure
        bridge
            .notify_memory_pressure(MemoryPressureLevel::Critical)
            .unwrap();

        let evt2 = rx.recv().unwrap();
        match evt2 {
            BridgeEvent::MemoryPressure { level } => {
                assert_eq!(level, "critical");
            }
            other => panic!("Unexpected event: {:?}", other),
        }
    }

    #[test]
    fn test_c_abi_lifecycle_round_trip() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();

        // Initialize bridge via C-ABI
        let init_res = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(init_res, 0);

        // Query lifecycle state
        let state_ptr = c_abi::bridge_c_get_lifecycle_state();
        assert!(!state_ptr.is_null());
        let state_str = unsafe { CStr::from_ptr(state_ptr).to_str().unwrap() };
        assert!(state_str.contains("running"));
        unsafe { c_abi::bridge_c_free_string(state_ptr) };

        // Notify memory pressure via C-ABI
        let mem_res = c_abi::bridge_c_notify_memory_pressure(1); // 1 = Critical
        assert_eq!(mem_res, 0);

        // Request shutdown via C-ABI
        let reason_c = CString::new("user_requested").unwrap();
        let shut_res = unsafe { c_abi::bridge_c_request_shutdown(reason_c.as_ptr()) };
        assert_eq!(shut_res, 0);

        // Query state again -> must be terminated
        let post_state_ptr = c_abi::bridge_c_get_lifecycle_state();
        assert!(!post_state_ptr.is_null());
        let post_state_str = unsafe { CStr::from_ptr(post_state_ptr).to_str().unwrap() };
        assert!(post_state_str.contains("terminated"));
        unsafe { c_abi::bridge_c_free_string(post_state_ptr) };

        // Clean up global singleton
        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_voice_bridge_listening_and_level_metering() {
        use std::time::Duration;

        let bridge = CoreBridge::new_in_memory().unwrap();
        let (_id, receiver) = bridge.event_sink().subscribe(32);

        assert!(!bridge.voice_is_listening());
        assert_eq!(bridge.voice_mode(), VoiceListeningMode::PushToTalk);

        // Start listening in PushToTalk mode
        bridge
            .voice_start_listening(VoiceListeningMode::PushToTalk)
            .unwrap();
        assert!(bridge.voice_is_listening());

        let ev1 = receiver.recv_timeout(Duration::from_millis(500)).unwrap();
        assert!(matches!(
            ev1,
            BridgeEvent::VoiceStateChanged {
                is_listening: true,
                ..
            }
        ));

        // Inject audio samples (16 kHz sine wave)
        let sine_samples: Vec<f32> = (0..512)
            .map(|t| (2.0 * std::f32::consts::PI * 440.0 * (t as f32) / 16_000.0).sin() * 0.5)
            .collect();
        bridge.voice_inject_audio(&sine_samples).unwrap();

        // Level meter event must be broadcast
        let ev2 = receiver.recv_timeout(Duration::from_millis(500)).unwrap();
        assert!(matches!(ev2, BridgeEvent::VoiceLevelUpdated { .. }));

        // Stop listening
        bridge.voice_stop_listening().unwrap();
        assert!(!bridge.voice_is_listening());

        let ev3 = receiver.recv_timeout(Duration::from_millis(500)).unwrap();
        assert!(matches!(
            ev3,
            BridgeEvent::VoiceStateChanged {
                is_listening: false,
                ..
            }
        ));
    }

    #[test]
    fn test_c_abi_voice_control_round_trip() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();

        let init_res = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(init_res, 0);

        // Start listening
        let start_res = c_abi::bridge_c_voice_start_listening(1); // 1 = WakeWord
        assert_eq!(start_res, 0);

        // Switch mode to PushToTalk (0)
        let mode_res = c_abi::bridge_c_voice_set_mode(0);
        assert_eq!(mode_res, 0);

        // Set keywords
        let kw_json = CString::new("[\"hey assistant\", \"computer\"]").unwrap();
        let kw_res = unsafe { c_abi::bridge_c_voice_set_keywords(kw_json.as_ptr()) };
        assert_eq!(kw_res, 0);

        // Inject audio
        let samples = [0.1f32; 512];
        let inject_res =
            unsafe { c_abi::bridge_c_voice_inject_audio(samples.as_ptr(), samples.len()) };
        assert_eq!(inject_res, 0);

        // Stop listening
        let stop_res = c_abi::bridge_c_voice_stop_listening();
        assert_eq!(stop_res, 0);

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_model_bridge_snapshot_and_lifecycle() {
        let bridge = CoreBridge::new_in_memory().unwrap();
        let snapshot = bridge.model_snapshot();

        assert!(!snapshot.hardware.cpu_architecture.is_empty());
        assert!(snapshot.hardware.total_ram_bytes > 0);
        assert!(!snapshot.models.is_empty());
        assert_eq!(snapshot.active_model_id, None);

        let first_model = &snapshot.models[0];
        let model_id = first_model.id.clone();

        // Load model
        assert!(bridge.model_load(&model_id).is_ok());
        let updated_snapshot = bridge.model_snapshot();
        assert_eq!(updated_snapshot.active_model_id, Some(model_id.clone()));

        // Unload model
        assert!(bridge.model_unload().is_ok());
        let unloaded_snapshot = bridge.model_snapshot();
        assert_eq!(unloaded_snapshot.active_model_id, None);
    }

    #[test]
    fn test_model_bridge_benchmark_execution() {
        let bridge = CoreBridge::new_in_memory().unwrap();
        let report = bridge.model_benchmark_run(32, 16).unwrap();

        assert!(report.prompt_tokens >= 1);
        assert!(report.generated_tokens >= 1);
        assert!(report.total_elapsed_ms >= 1);
        assert!(report.tokens_per_second >= 0.0);
    }

    #[test]
    fn test_model_bridge_c_abi() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();

        let init_res = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(init_res, 0);

        // Get snapshot
        let snap_ptr = c_abi::bridge_c_model_snapshot();
        assert!(!snap_ptr.is_null());
        let snap_str = unsafe { CStr::from_ptr(snap_ptr) }.to_str().unwrap();
        assert!(snap_str.contains("hardware"));
        assert!(snap_str.contains("models"));
        unsafe { c_abi::bridge_c_free_string(snap_ptr) };

        // Run benchmark via C-ABI
        let bench_ptr = c_abi::bridge_c_model_benchmark_run(16, 8);
        assert!(!bench_ptr.is_null());
        let bench_str = unsafe { CStr::from_ptr(bench_ptr) }.to_str().unwrap();
        assert!(bench_str.contains("tokens_per_second"));
        unsafe { c_abi::bridge_c_free_string(bench_ptr) };

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_device_bridge_c_abi() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();

        let init_res = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(init_res, 0);

        // Initially no discovered peers
        let peers_ptr = c_abi::bridge_c_device_discovered_peers();
        assert!(!peers_ptr.is_null());
        let peers_str = unsafe { CStr::from_ptr(peers_ptr) }.to_str().unwrap();
        assert_eq!(peers_str, "[]");
        unsafe { c_abi::bridge_c_free_string(peers_ptr) };

        // Simulate a peer via C-ABI
        let peer_json = CString::new(
            r#"{"instance_name":"c-peer","platform":"linux","port":51820,"addresses":["192.168.1.12"],"fingerprint":"aabbccddeeff0011","last_seen_ms":5000}"#,
        )
        .unwrap();
        let sim_res = unsafe { c_abi::bridge_c_device_simulate_peer(peer_json.as_ptr()) };
        assert_eq!(sim_res, 0);

        // Check discovered peers again
        let peers_ptr2 = c_abi::bridge_c_device_discovered_peers();
        assert!(!peers_ptr2.is_null());
        let peers_str2 = unsafe { CStr::from_ptr(peers_ptr2) }.to_str().unwrap();
        assert!(peers_str2.contains("c-peer"));
        unsafe { c_abi::bridge_c_free_string(peers_ptr2) };

        // Start pairing via C-ABI
        let target_name = CString::new("c-peer").unwrap();
        let pair_ptr = unsafe { c_abi::bridge_c_device_start_pairing(target_name.as_ptr()) };
        assert!(!pair_ptr.is_null());
        let pair_str = unsafe { CStr::from_ptr(pair_ptr) }.to_str().unwrap();
        assert!(pair_str.contains("sas_code"));

        let info: PairingSessionInfo = serde_json::from_str(pair_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(pair_ptr) };

        // Confirm pairing with SAS code via C-ABI
        let sas_cstr = CString::new(info.sas_code).unwrap();
        let confirm_ptr = unsafe { c_abi::bridge_c_device_confirm_pairing(sas_cstr.as_ptr()) };
        assert!(!confirm_ptr.is_null());
        let confirm_str = unsafe { CStr::from_ptr(confirm_ptr) }.to_str().unwrap();
        assert!(confirm_str.contains("trusted"));
        let paired: PairedDeviceItem = serde_json::from_str(confirm_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(confirm_ptr) };

        // Paired list
        let list_ptr = c_abi::bridge_c_device_paired_list();
        assert!(!list_ptr.is_null());
        let list_str = unsafe { CStr::from_ptr(list_ptr) }.to_str().unwrap();
        assert!(list_str.contains("c-peer"));
        unsafe { c_abi::bridge_c_free_string(list_ptr) };

        // Update ACL permission via C-ABI
        let dev_id_cstr = CString::new(paired.device_id.clone()).unwrap();
        let cap_cstr = CString::new("file_system").unwrap();
        let lvl_cstr = CString::new("deny").unwrap();
        let perm_res = unsafe {
            c_abi::bridge_c_device_set_permission(
                dev_id_cstr.as_ptr(),
                cap_cstr.as_ptr(),
                lvl_cstr.as_ptr(),
            )
        };
        assert_eq!(perm_res, 0);

        // Unpair device via C-ABI
        let unpair_res = unsafe { c_abi::bridge_c_device_unpair(dev_id_cstr.as_ptr()) };
        assert_eq!(unpair_res, 0);

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_knowledge_bridge_and_c_abi_lifecycle() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();
        let _ = c_abi::bridge_c_shutdown();

        let init = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(init, 0);

        // 1. Index document via C-ABI
        let path = CString::new("notes/architecture.md").unwrap();
        let fname = CString::new("architecture.md").unwrap();
        let mime = CString::new("text/markdown").unwrap();
        let content = CString::new(
            "Universal AI Assistant is built with Flutter and Rust core for total offline privacy.",
        )
        .unwrap();

        let doc_id_ptr = unsafe {
            c_abi::bridge_c_knowledge_index(
                path.as_ptr(),
                fname.as_ptr(),
                mime.as_ptr(),
                content.as_ptr(),
            )
        };
        assert!(!doc_id_ptr.is_null());
        let doc_id_json = unsafe { CStr::from_ptr(doc_id_ptr) }.to_str().unwrap();
        let doc_id: String = serde_json::from_str(doc_id_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(doc_id_ptr) };
        assert!(doc_id.starts_with("doc_"));

        // 2. List documents via C-ABI
        let list_ptr = c_abi::bridge_c_knowledge_list();
        assert!(!list_ptr.is_null());
        let list_json = unsafe { CStr::from_ptr(list_ptr) }.to_str().unwrap();
        let docs: Vec<DocumentRecordItem> = serde_json::from_str(list_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(list_ptr) };
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].file_name, "architecture.md");
        assert_eq!(docs[0].chunk_count, 1);

        // 3. Search knowledge via C-ABI
        let query = CString::new("Flutter Rust offline").unwrap();
        let search_ptr = unsafe { c_abi::bridge_c_knowledge_search(query.as_ptr(), 5) };
        assert!(!search_ptr.is_null());
        let search_json = unsafe { CStr::from_ptr(search_ptr) }.to_str().unwrap();
        let citations: Vec<KnowledgeCitationItem> = serde_json::from_str(search_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(search_ptr) };
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].file_name, "architecture.md");
        assert!(citations[0].snippet.contains("<mark>"));

        // 4. Delete document via C-ABI
        let del_id = CString::new(doc_id).unwrap();
        let del_res = unsafe { c_abi::bridge_c_knowledge_delete(del_id.as_ptr()) };
        assert_eq!(del_res, 0);

        // 5. Verify empty list after deletion
        let list_after_ptr = c_abi::bridge_c_knowledge_list();
        assert!(!list_after_ptr.is_null());
        let list_after_json = unsafe { CStr::from_ptr(list_after_ptr) }.to_str().unwrap();
        let docs_after: Vec<DocumentRecordItem> = serde_json::from_str(list_after_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(list_after_ptr) };
        assert!(docs_after.is_empty());

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_vision_ocr_bridge_and_c_abi() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();
        let _ = c_abi::bridge_c_shutdown();

        let init = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(init, 0);

        // Synthetic image payload with embedded OCR text
        let payload = "Invoice #1042\nTotal Amount: $42.50\nPayment Method: Offline Cash".as_bytes();
        let ptr = unsafe { c_abi::bridge_c_vision_ocr_extract(payload.as_ptr(), payload.len()) };
        assert!(!ptr.is_null());

        let json_str = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        let doc: OcrDocumentItem = serde_json::from_str(json_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(ptr) };

        assert_eq!(doc.lines.len(), 3);
        assert!(doc.full_text.contains("Invoice #1042"));
        assert!(doc.full_text.contains("Total Amount: $42.50"));
        assert!(doc.average_confidence > 0.9);
        assert!(doc.lines[0].words.len() >= 2);

        // Null pointer safety check
        let err_ptr = unsafe { c_abi::bridge_c_vision_ocr_extract(std::ptr::null(), 0) };
        assert!(!err_ptr.is_null());
        let err_str = unsafe { CStr::from_ptr(err_ptr) }.to_str().unwrap();
        assert!(err_str.contains("message") || err_str.contains("code"));
        unsafe { c_abi::bridge_c_free_string(err_ptr) };

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }
}
