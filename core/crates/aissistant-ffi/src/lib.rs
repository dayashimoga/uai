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
    bridge_automation_clear_history, bridge_automation_delete, bridge_automation_get,
    bridge_automation_history, bridge_automation_list, bridge_automation_save,
    bridge_automation_toggle, bridge_automation_trigger, bridge_delete_persona,
    bridge_delete_preference, bridge_device_cancel_pairing, bridge_device_confirm_pairing,
    bridge_device_discovered_peers, bridge_device_paired_list, bridge_device_revoke,
    bridge_device_set_permission, bridge_device_simulate_peer, bridge_device_start_pairing,
    bridge_device_unpair, bridge_emergency_lockdown, bridge_enforce_memory_budget,
    bridge_get_active_persona, bridge_get_capabilities, bridge_get_health,
    bridge_get_lifecycle_state, bridge_get_performance_snapshot, bridge_get_preference,
    bridge_get_setting, bridge_home_entity_control, bridge_home_entity_delete,
    bridge_home_entity_get, bridge_home_entity_list, bridge_home_entity_upsert,
    bridge_interpreter_clear_session, bridge_interpreter_get_turns,
    bridge_interpreter_start_session, bridge_interpreter_submit_turn,
    bridge_interpreter_swap_languages, bridge_is_lockdown_active, bridge_knowledge_delete_document,
    bridge_knowledge_index_document, bridge_knowledge_list_documents, bridge_knowledge_search,
    bridge_lift_emergency_lockdown, bridge_list_personas, bridge_list_preferences,
    bridge_model_benchmark_run, bridge_model_load, bridge_model_snapshot, bridge_model_unload,
    bridge_notify_memory_pressure, bridge_notify_power_transition, bridge_query_audit,
    bridge_request_shutdown, bridge_rotate_master_key, bridge_run_pipeline_benchmark,
    bridge_save_persona, bridge_set_active_persona, bridge_set_preference, bridge_set_setting,
    bridge_translate_text, bridge_update_permission, bridge_verify_audit,
    bridge_vision_ocr_extract, bridge_voice_inject_audio, bridge_voice_set_keywords,
    bridge_voice_set_mode, get_bridge, init_bridge, shutdown_bridge, AudioDeviceInfoItem,
    AudioDevicesSnapshot, AuditIntegrityReport, AuditItem, AutomationHistoryItem,
    AutomationRuleItem, CapabilityItem, CoreBridge, DocumentRecordItem, HealthReport, HealthStatus,
    InterpreterSessionStatusItem, InterpreterTurnItem, KnowledgeCitationItem, OcrBoundingBoxItem,
    OcrDocumentItem, OcrTextLineItem, OcrWordItem, PerformanceSnapshot, PipelineBenchmarkReport,
    SmartHomeCommandItem, SmartHomeCommandResultItem, SmartHomeEntityItem, SpeechModelBundleStatus,
    TranslationResultItem, UserPersonaItem, UserPreferenceItem, WorkflowActionItem,
    WorkflowExecutionReportItem,
};
pub use device_bridge::{
    DeviceSupervisor, DiscoveredPeerItem, PairedDeviceItem, PairingSessionInfo,
    PairingStateSnapshot, PeerPermissionItem,
};
pub use error::{FfiError, FfiResult};
pub use events::{BridgeEvent, EventStreamSink};
pub use lifecycle::{
    CancellationToken, CoreRuntime, LifecycleState, MemoryPressureLevel, PowerTransition,
    ShutdownReason,
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

        // Hardware audio devices enumeration via C-ABI
        let dev_ptr = c_abi::bridge_c_list_audio_devices();
        assert!(!dev_ptr.is_null());
        let dev_json = unsafe { CStr::from_ptr(dev_ptr) }.to_str().unwrap();
        let snapshot: AudioDevicesSnapshot = serde_json::from_str(dev_json).unwrap();
        assert!(!snapshot.input_devices.is_empty());
        assert!(!snapshot.output_devices.is_empty());
        unsafe { c_abi::bridge_c_free_string(dev_ptr) };

        // Audio device & language selection via C-ABI
        let mic_name = CString::new("Default System Microphone").unwrap();
        let dev_select_res = unsafe { c_abi::bridge_c_voice_set_device(mic_name.as_ptr()) };
        assert_eq!(dev_select_res, 0);

        let lang_tag = CString::new("es").unwrap();
        let lang_select_res = unsafe { c_abi::bridge_c_voice_set_language(lang_tag.as_ptr()) };
        assert_eq!(lang_select_res, 0);

        // Speech models status & download bootstrapping via C-ABI
        let status_ptr = c_abi::bridge_c_speech_models_status();
        assert!(!status_ptr.is_null());
        unsafe { c_abi::bridge_c_free_string(status_ptr) };

        let dl_ptr = c_abi::bridge_c_download_speech_models();
        assert!(!dl_ptr.is_null());
        let dl_json = unsafe { CStr::from_ptr(dl_ptr) }.to_str().unwrap();
        let bundle: SpeechModelBundleStatus = serde_json::from_str(dl_json).unwrap();
        assert!(bundle.all_ready);
        assert!(bundle.vad_ready && bundle.stt_ready && bundle.tts_ready);
        unsafe { c_abi::bridge_c_free_string(dl_ptr) };

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
        // Benchmark without loaded model must return error
        assert!(bridge.model_benchmark_run(32, 16).is_err());

        let model_id = "qwen2.5-0.5b-instruct-q4_k_m";
        bridge.model_load(model_id).unwrap();

        let report = bridge.model_benchmark_run(32, 16).unwrap();

        assert!(report.prompt_tokens >= 1);
        assert!(report.generated_tokens >= 1);
        assert!(report.total_elapsed_ms >= 1);
        assert!(report.tokens_per_second >= 0.0);
        assert!(report.memory_used_bytes > 0);
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

        // Test C-ABI model download and cancel
        let c_model_id = std::ffi::CString::new("qwen2.5-0.5b-instruct-q4_k_m").unwrap();
        let dl_res = unsafe { c_abi::bridge_c_model_download(c_model_id.as_ptr()) };
        assert_eq!(dl_res, 0);
        let cancel_res = unsafe { c_abi::bridge_c_model_cancel_download(c_model_id.as_ptr()) };
        assert_eq!(cancel_res, 0);

        // Test C-ABI import local gguf
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"GGUF_TEST_DATA").unwrap();
        let c_path = std::ffi::CString::new(tmp.path().to_str().unwrap()).unwrap();
        let import_res = unsafe { c_abi::bridge_c_model_import_local_gguf(c_path.as_ptr()) };
        assert_eq!(import_res, 0);

        // Test C-ABI delete model
        let delete_res = unsafe { c_abi::bridge_c_model_delete(c_model_id.as_ptr()) };
        assert_eq!(delete_res, 0);

        // Load model via C-ABI
        let load_res = unsafe { c_abi::bridge_c_model_load(c_model_id.as_ptr()) };
        assert_eq!(load_res, 0);

        // Run benchmark via C-ABI
        let bench_ptr = c_abi::bridge_c_model_benchmark_run(16, 8);
        assert!(!bench_ptr.is_null());
        let bench_str = unsafe { CStr::from_ptr(bench_ptr) }.to_str().unwrap();
        assert!(bench_str.contains("tokens_per_second"));
        unsafe { c_abi::bridge_c_free_string(bench_ptr) };

        // Unload model via C-ABI
        let unload_res = c_abi::bridge_c_model_unload();
        assert_eq!(unload_res, 0);

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
        let payload =
            "Invoice #1042\nTotal Amount: $42.50\nPayment Method: Offline Cash".as_bytes();
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

        // Test screen capture via C-ABI
        let screen_ptr = c_abi::bridge_c_capture_screen();
        assert!(!screen_ptr.is_null());
        let screen_json = unsafe { CStr::from_ptr(screen_ptr) }.to_str().unwrap();
        let screen_doc: OcrDocumentItem = serde_json::from_str(screen_json).unwrap();
        assert!(screen_doc.lines.len() >= 2);
        assert!(screen_doc.full_text.contains("Universal AI Assistant"));
        unsafe { c_abi::bridge_c_free_string(screen_ptr) };

        // Test file-based OCR via C-ABI
        let tmp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp_file.path(), payload).unwrap();
        let path_c = CString::new(tmp_file.path().to_str().unwrap()).unwrap();

        let file_ptr = unsafe { c_abi::bridge_c_vision_extract_file(path_c.as_ptr()) };
        assert!(!file_ptr.is_null());
        let file_json = unsafe { CStr::from_ptr(file_ptr) }.to_str().unwrap();
        let file_doc: OcrDocumentItem = serde_json::from_str(file_json).unwrap();
        assert_eq!(file_doc.lines.len(), 3);
        assert!(file_doc.full_text.contains("Invoice #1042"));
        unsafe { c_abi::bridge_c_free_string(file_ptr) };

        // Test bridge_c_analyze_image with path
        let analyze_ptr = unsafe { c_abi::bridge_c_analyze_image(path_c.as_ptr()) };
        assert!(!analyze_ptr.is_null());
        let analyze_json = unsafe { CStr::from_ptr(analyze_ptr) }.to_str().unwrap();
        let analyze_doc: OcrDocumentItem = serde_json::from_str(analyze_json).unwrap();
        assert_eq!(analyze_doc.lines.len(), 3);
        unsafe { c_abi::bridge_c_free_string(analyze_ptr) };

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_translation_and_interpreter_c_abi() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();
        let _ = c_abi::bridge_c_shutdown();

        let init = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(init, 0);

        // Load active model into memory for neural translation
        let model_id_c = CString::new("qwen2.5-0.5b-instruct-q4_k_m").unwrap();
        let load_res = unsafe { c_abi::bridge_c_model_load(model_id_c.as_ptr()) };
        assert_eq!(load_res, 0);

        // 1. Test Text Translation C-ABI
        let text_c = CString::new("Hello").unwrap();
        let src_c = CString::new("en").unwrap();
        let tgt_c = CString::new("es").unwrap();

        let trans_ptr = unsafe {
            c_abi::bridge_c_translate_text(text_c.as_ptr(), src_c.as_ptr(), tgt_c.as_ptr())
        };
        assert!(!trans_ptr.is_null());
        let trans_json = unsafe { CStr::from_ptr(trans_ptr) }.to_str().unwrap();
        let trans_res: TranslationResultItem = serde_json::from_str(trans_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(trans_ptr) };

        assert_eq!(trans_res.source_text, "Hello");
        assert_eq!(trans_res.translated_text, "Hola");
        assert_eq!(trans_res.source_lang, "en");
        assert_eq!(trans_res.target_lang, "es");
        assert!(trans_res.confidence >= 0.85);

        // Test Hindi and Kannada translation support (12 languages)
        let hi_c = CString::new("hi").unwrap();
        let kn_c = CString::new("kn").unwrap();
        let trans_hi_ptr = unsafe {
            c_abi::bridge_c_translate_text(text_c.as_ptr(), src_c.as_ptr(), hi_c.as_ptr())
        };
        assert!(!trans_hi_ptr.is_null());
        let trans_hi_json = unsafe { CStr::from_ptr(trans_hi_ptr) }.to_str().unwrap();
        let trans_hi_res: TranslationResultItem = serde_json::from_str(trans_hi_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(trans_hi_ptr) };
        assert_eq!(trans_hi_res.translated_text, "नमस्ते");

        let trans_kn_ptr = unsafe {
            c_abi::bridge_c_translate_text(text_c.as_ptr(), src_c.as_ptr(), kn_c.as_ptr())
        };
        assert!(!trans_kn_ptr.is_null());
        let trans_kn_json = unsafe { CStr::from_ptr(trans_kn_ptr) }.to_str().unwrap();
        let trans_kn_res: TranslationResultItem = serde_json::from_str(trans_kn_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(trans_kn_ptr) };
        assert_eq!(trans_kn_res.translated_text, "ನಮಸ್ಕಾರ");

        // 2. Test Interpreter Start C-ABI
        let lang_a_c = CString::new("en").unwrap();
        let lang_b_c = CString::new("fr").unwrap();
        let start_ptr =
            unsafe { c_abi::bridge_c_interpreter_start(lang_a_c.as_ptr(), lang_b_c.as_ptr()) };
        assert!(!start_ptr.is_null());
        let start_json = unsafe { CStr::from_ptr(start_ptr) }.to_str().unwrap();
        let status: InterpreterSessionStatusItem = serde_json::from_str(start_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(start_ptr) };

        assert_eq!(status.party_a_lang, "en");
        assert_eq!(status.party_b_lang, "fr");
        assert_eq!(status.active_speaker, "party_a");
        assert_eq!(status.turn_count, 0);

        // 3. Test Interpreter Submit Turn (Party A speaks)
        let turn1_c = CString::new("Thank you").unwrap();
        let turn1_ptr = unsafe { c_abi::bridge_c_interpreter_submit_turn(turn1_c.as_ptr()) };
        assert!(!turn1_ptr.is_null());
        let turn1_json = unsafe { CStr::from_ptr(turn1_ptr) }.to_str().unwrap();
        let turn1: InterpreterTurnItem = serde_json::from_str(turn1_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(turn1_ptr) };

        assert_eq!(turn1.turn_id, 1);
        assert_eq!(turn1.speaker, "party_a");
        assert_eq!(turn1.source_lang, "en");
        assert_eq!(turn1.target_lang, "fr");
        assert_eq!(turn1.translated_text, "Merci");

        // 4. Test Auto-Swapped Turn (Party B speaks)
        let turn2_c = CString::new("De rien").unwrap();
        let turn2_ptr = unsafe { c_abi::bridge_c_interpreter_submit_turn(turn2_c.as_ptr()) };
        assert!(!turn2_ptr.is_null());
        let turn2_json = unsafe { CStr::from_ptr(turn2_ptr) }.to_str().unwrap();
        let turn2: InterpreterTurnItem = serde_json::from_str(turn2_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(turn2_ptr) };

        assert_eq!(turn2.turn_id, 2);
        assert_eq!(turn2.speaker, "party_b");
        assert_eq!(turn2.source_lang, "fr");
        assert_eq!(turn2.target_lang, "en");
        assert_eq!(turn2.translated_text, "You are welcome");

        // 4b. Test Interpreter TTS Audio Replay C-ABI
        let turn1_id_c = CString::new("1").unwrap();
        let replay_res = unsafe { c_abi::bridge_c_interpreter_replay_tts(turn1_id_c.as_ptr()) };
        assert_eq!(replay_res, 0);

        let invalid_id_c = CString::new("9999").unwrap();
        let invalid_replay =
            unsafe { c_abi::bridge_c_interpreter_replay_tts(invalid_id_c.as_ptr()) };
        assert_eq!(invalid_replay, -1);

        // 5. Test Get Turns History C-ABI
        let history_ptr = c_abi::bridge_c_interpreter_get_turns();
        assert!(!history_ptr.is_null());
        let history_json = unsafe { CStr::from_ptr(history_ptr) }.to_str().unwrap();
        let history: Vec<InterpreterTurnItem> = serde_json::from_str(history_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(history_ptr) };
        assert_eq!(history.len(), 2);

        // 6. Test Language Swap C-ABI
        let swap_ptr = c_abi::bridge_c_interpreter_swap_languages();
        assert!(!swap_ptr.is_null());
        let swap_json = unsafe { CStr::from_ptr(swap_ptr) }.to_str().unwrap();
        let swapped: InterpreterSessionStatusItem = serde_json::from_str(swap_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(swap_ptr) };
        assert_eq!(swapped.party_a_lang, "fr");
        assert_eq!(swapped.party_b_lang, "en");

        // 7. Test Clear History C-ABI
        let clear_res = c_abi::bridge_c_interpreter_clear();
        assert_eq!(clear_res, 0);

        let history_after_ptr = c_abi::bridge_c_interpreter_get_turns();
        let history_after_json = unsafe { CStr::from_ptr(history_after_ptr) }
            .to_str()
            .unwrap();
        let history_after: Vec<InterpreterTurnItem> =
            serde_json::from_str(history_after_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(history_after_ptr) };
        assert!(history_after.is_empty());

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_persona_and_preference_c_abi() {
        let _lock = TEST_GLOBAL_LOCK.lock().unwrap();

        // 1. Initialize in-memory bridge
        let init_res = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(init_res, 0);

        // 2. Query default active persona
        let active_ptr = c_abi::bridge_c_persona_get_active();
        assert!(!active_ptr.is_null());
        let active_json = unsafe { CStr::from_ptr(active_ptr) }.to_str().unwrap();
        let default_active: Option<UserPersonaItem> = serde_json::from_str(active_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(active_ptr) };

        assert!(default_active.is_some());
        let def = default_active.unwrap();
        assert_eq!(def.id, "persona_default");
        assert_eq!(def.name, "Default Assistant");
        assert!(def.is_active);

        // 3. Save new custom persona
        let custom = UserPersonaItem {
            id: "persona_custom_tutor".to_string(),
            name: "Code Tutor".to_string(),
            tone: "technical".to_string(),
            verbosity: "detailed".to_string(),
            voice_id: "vits_piper_en_ryan".to_string(),
            speaking_rate: 1.15,
            pitch: 0.95,
            system_prompt_override: Some(
                "Always write idiomatic Rust with doc comments.".to_string(),
            ),
            is_active: true,
            created_at_ms: 0,
            updated_at_ms: 0,
        };
        let custom_json = serde_json::to_string(&custom).unwrap();
        let custom_c = CString::new(custom_json).unwrap();
        let save_ptr = unsafe { c_abi::bridge_c_persona_save(custom_c.as_ptr()) };
        assert!(!save_ptr.is_null());
        let save_str = unsafe { CStr::from_ptr(save_ptr) }.to_str().unwrap();
        assert_eq!(save_str, "null");
        unsafe { c_abi::bridge_c_free_string(save_ptr) };

        // 4. Verify custom is now active
        let active_ptr2 = c_abi::bridge_c_persona_get_active();
        let active2_json = unsafe { CStr::from_ptr(active_ptr2) }.to_str().unwrap();
        let active2: Option<UserPersonaItem> = serde_json::from_str(active2_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(active_ptr2) };
        assert_eq!(active2.unwrap().id, "persona_custom_tutor");

        // 5. List personas
        let list_ptr = c_abi::bridge_c_persona_list();
        let list_json = unsafe { CStr::from_ptr(list_ptr) }.to_str().unwrap();
        let personas: Vec<UserPersonaItem> = serde_json::from_str(list_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(list_ptr) };
        assert_eq!(personas.len(), 2);

        // 6. Set active back to default
        let def_id_c = CString::new("persona_default").unwrap();
        let set_active_ptr = unsafe { c_abi::bridge_c_persona_set_active(def_id_c.as_ptr()) };
        let set_str = unsafe { CStr::from_ptr(set_active_ptr) }.to_str().unwrap();
        assert_eq!(set_str, "null");
        unsafe { c_abi::bridge_c_free_string(set_active_ptr) };

        // 7. Delete custom persona
        let custom_id_c = CString::new("persona_custom_tutor").unwrap();
        let del_ptr = unsafe { c_abi::bridge_c_persona_delete(custom_id_c.as_ptr()) };
        let del_str = unsafe { CStr::from_ptr(del_ptr) }.to_str().unwrap();
        assert_eq!(del_str, "true");
        unsafe { c_abi::bridge_c_free_string(del_ptr) };

        // 8. Cannot delete default persona (returns error JSON)
        let del_def_ptr = unsafe { c_abi::bridge_c_persona_delete(def_id_c.as_ptr()) };
        let del_def_str = unsafe { CStr::from_ptr(del_def_ptr) }.to_str().unwrap();
        assert!(del_def_str.contains("code"));
        unsafe { c_abi::bridge_c_free_string(del_def_ptr) };

        // 9. Preferences: Set
        let pref_key = CString::new("editor.font_family").unwrap();
        let pref_val = CString::new("\"JetBrains Mono\"").unwrap();
        let pref_cat = CString::new("ui").unwrap();
        let set_pref_ptr = unsafe {
            c_abi::bridge_c_preference_set(pref_key.as_ptr(), pref_val.as_ptr(), pref_cat.as_ptr())
        };
        let set_pref_str = unsafe { CStr::from_ptr(set_pref_ptr) }.to_str().unwrap();
        assert_eq!(set_pref_str, "null");
        unsafe { c_abi::bridge_c_free_string(set_pref_ptr) };

        // 10. Preferences: Get
        let get_pref_ptr = unsafe { c_abi::bridge_c_preference_get(pref_key.as_ptr()) };
        let get_pref_json = unsafe { CStr::from_ptr(get_pref_ptr) }.to_str().unwrap();
        let get_pref: Option<UserPreferenceItem> = serde_json::from_str(get_pref_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(get_pref_ptr) };
        assert!(get_pref.is_some());
        assert_eq!(get_pref.unwrap().value_json, "\"JetBrains Mono\"");

        // 11. Preferences: List
        let list_prefs_ptr = unsafe { c_abi::bridge_c_preference_list(pref_cat.as_ptr()) };
        let list_prefs_json = unsafe { CStr::from_ptr(list_prefs_ptr) }.to_str().unwrap();
        let prefs: Vec<UserPreferenceItem> = serde_json::from_str(list_prefs_json).unwrap();
        unsafe { c_abi::bridge_c_free_string(list_prefs_ptr) };
        assert_eq!(prefs.len(), 1);

        // 12. Preferences: Delete
        let del_pref_ptr = unsafe { c_abi::bridge_c_preference_delete(pref_key.as_ptr()) };
        let del_pref_str = unsafe { CStr::from_ptr(del_pref_ptr) }.to_str().unwrap();
        assert_eq!(del_pref_str, "true");
        unsafe { c_abi::bridge_c_free_string(del_pref_ptr) };

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_automation_workflow_c_abi() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();
        let _ = c_abi::bridge_c_shutdown();
        let res = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(res, 0);

        // 1. Initial list of automations (empty)
        let list_ptr = unsafe { c_abi::bridge_c_automation_list(false) };
        let list_str = unsafe { CStr::from_ptr(list_ptr) }.to_str().unwrap();
        let list: Vec<AutomationRuleItem> = serde_json::from_str(list_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(list_ptr) };
        assert!(list.is_empty());

        // 2. Save new automation rule
        let rule = AutomationRuleItem {
            id: "auto_c_test_1".to_string(),
            name: "Morning News & Audio".to_string(),
            enabled: true,
            trigger_type: "manual".to_string(),
            trigger_config_json: "{\"type\":\"manual\"}".to_string(),
            actions_json: serde_json::json!([
                {
                    "action_type": "notification",
                    "title": "Good Morning",
                    "message": "Starting routine"
                }
            ])
            .to_string(),
            last_executed_ms: None,
            created_at_ms: 1000,
            updated_at_ms: 1000,
        };
        let rule_json = serde_json::to_string(&rule).unwrap();
        let rule_c = CString::new(rule_json).unwrap();
        let save_ptr = unsafe { c_abi::bridge_c_automation_save(rule_c.as_ptr()) };
        let save_str = unsafe { CStr::from_ptr(save_ptr) }.to_str().unwrap();
        assert_eq!(save_str, "null");
        unsafe { c_abi::bridge_c_free_string(save_ptr) };

        // 3. Get automation rule
        let id_c = CString::new("auto_c_test_1").unwrap();
        let get_ptr = unsafe { c_abi::bridge_c_automation_get(id_c.as_ptr()) };
        let get_str = unsafe { CStr::from_ptr(get_ptr) }.to_str().unwrap();
        let fetched: Option<AutomationRuleItem> = serde_json::from_str(get_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(get_ptr) };
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "Morning News & Audio");

        // 4. Trigger automation rule manually
        let trig_ptr = unsafe { c_abi::bridge_c_automation_trigger(id_c.as_ptr()) };
        let trig_str = unsafe { CStr::from_ptr(trig_ptr) }.to_str().unwrap();
        let report: WorkflowExecutionReportItem = serde_json::from_str(trig_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(trig_ptr) };
        assert_eq!(report.automation_id, "auto_c_test_1");
        assert_eq!(report.status, "success");

        // 5. Query execution history
        let hist_ptr = unsafe { c_abi::bridge_c_automation_history(id_c.as_ptr(), 10) };
        let hist_str = unsafe { CStr::from_ptr(hist_ptr) }.to_str().unwrap();
        let history: Vec<AutomationHistoryItem> = serde_json::from_str(hist_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(hist_ptr) };
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status, "success");

        // 6. Toggle automation rule
        let toggle_ptr = unsafe { c_abi::bridge_c_automation_toggle(id_c.as_ptr(), false) };
        let toggle_str = unsafe { CStr::from_ptr(toggle_ptr) }.to_str().unwrap();
        assert_eq!(toggle_str, "true");
        unsafe { c_abi::bridge_c_free_string(toggle_ptr) };

        // Check enabled is false
        let get_ptr2 = unsafe { c_abi::bridge_c_automation_get(id_c.as_ptr()) };
        let get_str2 = unsafe { CStr::from_ptr(get_ptr2) }.to_str().unwrap();
        let fetched2: Option<AutomationRuleItem> = serde_json::from_str(get_str2).unwrap();
        unsafe { c_abi::bridge_c_free_string(get_ptr2) };
        assert!(!fetched2.unwrap().enabled);

        // 7. Delete automation rule
        let del_ptr = unsafe { c_abi::bridge_c_automation_delete(id_c.as_ptr()) };
        let del_str = unsafe { CStr::from_ptr(del_ptr) }.to_str().unwrap();
        assert_eq!(del_str, "true");
        unsafe { c_abi::bridge_c_free_string(del_ptr) };

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_smart_home_c_abi() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();
        let _ = c_abi::bridge_c_shutdown();
        let res = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(res, 0);

        // 1. Initial list (empty)
        let list_ptr = unsafe { c_abi::bridge_c_home_entity_list(std::ptr::null()) };
        let list_str = unsafe { CStr::from_ptr(list_ptr) }.to_str().unwrap();
        let list: Vec<SmartHomeEntityItem> = serde_json::from_str(list_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(list_ptr) };
        assert!(list.is_empty());

        // 2. Upsert light entity
        let light = SmartHomeEntityItem {
            id: "light.living_room".to_string(),
            domain: "light".to_string(),
            name: "Living Room Light".to_string(),
            state_json: serde_json::json!({"state": "off", "brightness": 0}).to_string(),
            attributes_json: serde_json::json!({"room": "living"}).to_string(),
            protocol: "home_assistant".to_string(),
            is_available: true,
            last_updated_ms: 1000,
        };
        let light_json = serde_json::to_string(&light).unwrap();
        let light_c = CString::new(light_json).unwrap();
        let up_ptr = unsafe { c_abi::bridge_c_home_entity_upsert(light_c.as_ptr()) };
        let up_str = unsafe { CStr::from_ptr(up_ptr) }.to_str().unwrap();
        assert_eq!(up_str, "null");
        unsafe { c_abi::bridge_c_free_string(up_ptr) };

        // 3. Upsert thermostat entity
        let thermostat = SmartHomeEntityItem {
            id: "climate.hall".to_string(),
            domain: "climate".to_string(),
            name: "Hall Thermostat".to_string(),
            state_json: serde_json::json!({"state": "heat", "temperature": 20.0}).to_string(),
            attributes_json: serde_json::json!({}).to_string(),
            protocol: "mqtt".to_string(),
            is_available: true,
            last_updated_ms: 1000,
        };
        let thermo_json = serde_json::to_string(&thermostat).unwrap();
        let thermo_c = CString::new(thermo_json).unwrap();
        let up_ptr2 = unsafe { c_abi::bridge_c_home_entity_upsert(thermo_c.as_ptr()) };
        unsafe { c_abi::bridge_c_free_string(up_ptr2) };

        // 4. List all entities (2 total)
        let list_all_ptr = unsafe { c_abi::bridge_c_home_entity_list(std::ptr::null()) };
        let list_all_str = unsafe { CStr::from_ptr(list_all_ptr) }.to_str().unwrap();
        let all_entities: Vec<SmartHomeEntityItem> = serde_json::from_str(list_all_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(list_all_ptr) };
        assert_eq!(all_entities.len(), 2);

        // 5. List with domain filter "climate" (1 total)
        let filter_c = CString::new("climate").unwrap();
        let list_clim_ptr = unsafe { c_abi::bridge_c_home_entity_list(filter_c.as_ptr()) };
        let list_clim_str = unsafe { CStr::from_ptr(list_clim_ptr) }.to_str().unwrap();
        let clim_entities: Vec<SmartHomeEntityItem> = serde_json::from_str(list_clim_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(list_clim_ptr) };
        assert_eq!(clim_entities.len(), 1);
        assert_eq!(clim_entities[0].id, "climate.hall");

        // 6. Get single entity by ID
        let id_c = CString::new("light.living_room").unwrap();
        let get_ptr = unsafe { c_abi::bridge_c_home_entity_get(id_c.as_ptr()) };
        let get_str = unsafe { CStr::from_ptr(get_ptr) }.to_str().unwrap();
        let fetched: Option<SmartHomeEntityItem> = serde_json::from_str(get_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(get_ptr) };
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "Living Room Light");

        // 7. Control light: turn_on with brightness
        let cmd_light = SmartHomeCommandItem {
            entity_id: "light.living_room".to_string(),
            command: "turn_on".to_string(),
            brightness: Some(220),
            color_hex: Some("#FFCC00".to_string()),
            target_celsius: None,
            position_percent: None,
        };
        let cmd_light_json = serde_json::to_string(&cmd_light).unwrap();
        let cmd_light_c = CString::new(cmd_light_json).unwrap();
        let ctrl_ptr = unsafe { c_abi::bridge_c_home_entity_control(cmd_light_c.as_ptr()) };
        let ctrl_str = unsafe { CStr::from_ptr(ctrl_ptr) }.to_str().unwrap();
        let ctrl_res: SmartHomeCommandResultItem = serde_json::from_str(ctrl_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(ctrl_ptr) };
        assert!(ctrl_res.success);
        assert_eq!(ctrl_res.dispatched_via, "home_assistant");
        assert!(ctrl_res.new_state_json.contains("\"brightness\":220"));

        // 8. Control thermostat: set_temperature to 23.0
        let cmd_thermo = SmartHomeCommandItem {
            entity_id: "climate.hall".to_string(),
            command: "set_temperature".to_string(),
            brightness: None,
            color_hex: None,
            target_celsius: Some(23.0),
            position_percent: None,
        };
        let cmd_thermo_json = serde_json::to_string(&cmd_thermo).unwrap();
        let cmd_thermo_c = CString::new(cmd_thermo_json).unwrap();
        let ctrl_ptr2 = unsafe { c_abi::bridge_c_home_entity_control(cmd_thermo_c.as_ptr()) };
        let ctrl_str2 = unsafe { CStr::from_ptr(ctrl_ptr2) }.to_str().unwrap();
        let ctrl_res2: SmartHomeCommandResultItem = serde_json::from_str(ctrl_str2).unwrap();
        unsafe { c_abi::bridge_c_free_string(ctrl_ptr2) };
        assert!(ctrl_res2.success);
        assert_eq!(ctrl_res2.dispatched_via, "mqtt");
        assert!(ctrl_res2.new_state_json.contains("23.0"));

        // 9. Delete entity
        let del_ptr = unsafe { c_abi::bridge_c_home_entity_delete(id_c.as_ptr()) };
        let del_str = unsafe { CStr::from_ptr(del_ptr) }.to_str().unwrap();
        assert_eq!(del_str, "true");
        unsafe { c_abi::bridge_c_free_string(del_ptr) };

        // Verify entity deleted
        let get_del_ptr = unsafe { c_abi::bridge_c_home_entity_get(id_c.as_ptr()) };
        let get_del_str = unsafe { CStr::from_ptr(get_del_ptr) }.to_str().unwrap();
        let fetched_del: Option<SmartHomeEntityItem> = serde_json::from_str(get_del_str).unwrap();
        unsafe { c_abi::bridge_c_free_string(get_del_ptr) };
        assert!(fetched_del.is_none());

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_key_rotation_and_emergency_lockdown_lifecycle() {
        let bridge = CoreBridge::new_in_memory().unwrap();
        let sink = bridge.event_sink();
        let (_sub_id, rx) = sink.subscribe(20);

        // 1. Secret storage & master key rotation
        bridge
            .set_secret("wifi_password", "wpa3_super_secret")
            .unwrap();
        assert_eq!(
            bridge.get_secret("wifi_password").unwrap(),
            Some("wpa3_super_secret".to_string())
        );

        bridge.rotate_master_key().unwrap();

        // Secrets must remain seamlessly accessible after rotation
        assert_eq!(
            bridge.get_secret("wifi_password").unwrap(),
            Some("wpa3_super_secret".to_string())
        );

        // 2. Device simulation and revocation
        let item = DiscoveredPeerItem {
            instance_name: "rogue-laptop".to_string(),
            platform: "linux".to_string(),
            port: 5353,
            addresses: vec!["192.168.1.105".to_string()],
            fingerprint: "0102030405060708".to_string(),
            last_seen_ms: 1000,
        };
        bridge.device_simulate_peer(item).unwrap();
        let pairing_info = bridge.device_start_pairing("rogue-laptop").unwrap();
        let paired = bridge
            .device_confirm_pairing(&pairing_info.sas_code)
            .unwrap();
        assert_eq!(paired.trust_state, "trusted");

        // Revoke device
        bridge.device_revoke(&paired.device_id).unwrap();
        let updated_list = bridge.device_paired_list().unwrap();
        let revoked_device = updated_list
            .iter()
            .find(|d| d.device_id == paired.device_id)
            .unwrap();
        assert_eq!(revoked_device.trust_state, "revoked");

        // 3. Emergency lockdown & lift
        assert!(!bridge.is_lockdown_active());
        bridge
            .emergency_lockdown("anomalous mesh activity")
            .unwrap();
        assert!(bridge.is_lockdown_active());

        bridge.lift_emergency_lockdown().unwrap();
        assert!(!bridge.is_lockdown_active());

        // 4. Verify unbroken BLAKE3 audit integrity
        let audit_report = bridge.verify_audit_integrity().unwrap();
        assert!(audit_report.is_valid);
        assert!(audit_report.verified_count >= 4);

        // 5. Verify events received
        let mut key_rotated_seen = false;
        let mut device_revoked_seen = false;
        let mut lockdown_engaged_seen = false;
        let mut lockdown_lifted_seen = false;

        while let Ok(evt) = rx.try_recv() {
            match evt {
                BridgeEvent::KeyRotated { .. } => key_rotated_seen = true,
                BridgeEvent::DeviceRevoked { device_id } if device_id == paired.device_id => {
                    device_revoked_seen = true;
                }
                BridgeEvent::EmergencyLockdown {
                    active: true,
                    reason,
                } if reason == "anomalous mesh activity" => {
                    lockdown_engaged_seen = true;
                }
                BridgeEvent::EmergencyLockdown {
                    active: false,
                    reason,
                } if reason == "lifted_by_user" => {
                    lockdown_lifted_seen = true;
                }
                _ => {}
            }
        }

        assert!(key_rotated_seen);
        assert!(device_revoked_seen);
        assert!(lockdown_engaged_seen);
        assert!(lockdown_lifted_seen);
    }

    #[test]
    fn test_key_rotation_and_lockdown_c_abi() {
        let _lock = TEST_GLOBAL_LOCK.lock().unwrap();
        let init = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(init, 0);

        // Check initial lockdown state
        let lock_ptr = unsafe { c_abi::bridge_c_is_lockdown_active() };
        let lock_str = unsafe { CStr::from_ptr(lock_ptr) }.to_str().unwrap();
        assert_eq!(lock_str, "false");
        unsafe { c_abi::bridge_c_free_string(lock_ptr) };

        // Rotate master key
        let rot_ptr = unsafe { c_abi::bridge_c_rotate_master_key() };
        let rot_str = unsafe { CStr::from_ptr(rot_ptr) }.to_str().unwrap();
        assert_eq!(rot_str, "null"); // Ok(()) serializes to null in serialize_to_c_json
        unsafe { c_abi::bridge_c_free_string(rot_ptr) };

        // Engage emergency lockdown
        let reason_c = CString::new("panic_button_pressed").unwrap();
        let engage_ptr = unsafe { c_abi::bridge_c_emergency_lockdown(reason_c.as_ptr()) };
        let engage_str = unsafe { CStr::from_ptr(engage_ptr) }.to_str().unwrap();
        assert_eq!(engage_str, "null");
        unsafe { c_abi::bridge_c_free_string(engage_ptr) };

        // Verify lockdown active
        let lock_ptr2 = unsafe { c_abi::bridge_c_is_lockdown_active() };
        let lock_str2 = unsafe { CStr::from_ptr(lock_ptr2) }.to_str().unwrap();
        assert_eq!(lock_str2, "true");
        unsafe { c_abi::bridge_c_free_string(lock_ptr2) };

        // Lift lockdown
        let lift_ptr = unsafe { c_abi::bridge_c_lift_emergency_lockdown() };
        let lift_str = unsafe { CStr::from_ptr(lift_ptr) }.to_str().unwrap();
        assert_eq!(lift_str, "null");
        unsafe { c_abi::bridge_c_free_string(lift_ptr) };

        // Verify lockdown lifted
        let lock_ptr3 = unsafe { c_abi::bridge_c_is_lockdown_active() };
        let lock_str3 = unsafe { CStr::from_ptr(lock_ptr3) }.to_str().unwrap();
        assert_eq!(lock_str3, "false");
        unsafe { c_abi::bridge_c_free_string(lock_ptr3) };

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_performance_snapshot_and_memory_budget_enforcement() {
        let bridge = CoreBridge::new_in_memory().unwrap();
        let sink = bridge.event_sink();
        let (_sub_id, rx) = sink.subscribe(32);

        // 1. Initial performance snapshot
        let snapshot = bridge.get_performance_snapshot().unwrap();
        assert!(snapshot.cold_start_ms <= 250);
        assert!(snapshot.wake_word_frame_micros > 0);
        assert!(snapshot.stt_latency_ms <= 350);
        assert!(snapshot.tts_first_audio_ms <= 250);
        assert!(snapshot.resident_memory_bytes > 0);
        assert!(snapshot.idle_budget_bytes >= 60 * 1024 * 1024);
        assert!(!snapshot.platform.is_empty());

        // 2. Enforce memory budget
        let within_budget = bridge.enforce_memory_budget().unwrap();
        let snapshot_after = bridge.get_performance_snapshot().unwrap();
        assert_eq!(within_budget, !snapshot_after.memory_budget_exceeded);

        // 3. Event received
        let mut perf_event_seen = false;
        while let Ok(evt) = rx.try_recv() {
            if let BridgeEvent::PerformanceMetricsUpdated {
                cold_start_ms,
                wake_word_frame_micros,
                ..
            } = evt
            {
                perf_event_seen = true;
                assert!(cold_start_ms <= 250);
                assert!(wake_word_frame_micros > 0);
            }
        }
        assert!(perf_event_seen);
    }

    #[test]
    fn test_pipeline_benchmark_and_c_abi() {
        let _lock = TEST_GLOBAL_LOCK.lock().unwrap();
        let init = unsafe { c_abi::bridge_c_init(std::ptr::null()) };
        assert_eq!(init, 0);

        // 1. Run pipeline benchmark via Rust CoreBridge
        let bridge = get_bridge().unwrap();
        let report = bridge.run_pipeline_benchmark().unwrap();
        assert_eq!(report.wake_word_frames_tested, 100);
        assert!(report.wake_word_avg_frame_micros < 1000);
        assert!(report.wake_word_rtf < 0.1); // Sub-0.1 real-time factor (>10x real-time)
        assert!(report.ring_buffer_throughput_msps > 1.0);
        assert!(report.vad_window_micros > 0);
        assert!(report.sqlite_query_micros > 0);
        assert!(report.all_budgets_passed);

        // 2. Query C-ABI snapshot
        let snap_ptr = unsafe { c_abi::bridge_c_get_performance_snapshot() };
        assert!(!snap_ptr.is_null());
        let snap_str = unsafe { CStr::from_ptr(snap_ptr) }.to_str().unwrap();
        assert!(snap_str.contains("cold_start_ms"));
        assert!(snap_str.contains("resident_memory_bytes"));
        unsafe { c_abi::bridge_c_free_string(snap_ptr) };

        // 3. Enforce memory budget via C-ABI
        let mem_ptr = unsafe { c_abi::bridge_c_enforce_memory_budget() };
        assert!(!mem_ptr.is_null());
        let mem_str = unsafe { CStr::from_ptr(mem_ptr) }.to_str().unwrap();
        assert!(mem_str == "true" || mem_str == "false");
        unsafe { c_abi::bridge_c_free_string(mem_ptr) };

        // 4. Run pipeline benchmark via C-ABI
        let bench_ptr = unsafe { c_abi::bridge_c_run_pipeline_benchmark() };
        assert!(!bench_ptr.is_null());
        let bench_str = unsafe { CStr::from_ptr(bench_ptr) }.to_str().unwrap();
        assert!(bench_str.contains("wake_word_frames_tested"));
        assert!(bench_str.contains("all_budgets_passed"));
        unsafe { c_abi::bridge_c_free_string(bench_ptr) };

        let cleanup = c_abi::bridge_c_shutdown();
        assert_eq!(cleanup, 0);
    }

    #[test]
    fn test_power_transition_lifecycle() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();
        use std::time::Duration;
        let _ = c_abi::bridge_c_shutdown();

        let bridge = Arc::new(CoreBridge::new_in_memory().unwrap());
        let (_sub_id, rx) = bridge.event_sink().subscribe(16);

        // Notify suspend
        assert!(bridge
            .notify_power_transition(PowerTransition::Suspend)
            .is_ok());

        // Event stream should receive PowerTransition { event: "suspend" }
        let event_suspend = rx.recv_timeout(Duration::from_millis(500)).unwrap();
        match event_suspend {
            BridgeEvent::PowerTransition { event } => assert_eq!(event, "suspend"),
            other => panic!("Expected PowerTransition suspend, got: {other:?}"),
        }

        // Notify resume
        assert!(bridge
            .notify_power_transition(PowerTransition::Resume)
            .is_ok());
        let event_resume = rx.recv_timeout(Duration::from_millis(500)).unwrap();
        match event_resume {
            BridgeEvent::PowerTransition { event } => assert_eq!(event, "resume"),
            other => panic!("Expected PowerTransition resume, got: {other:?}"),
        }

        // Verify C-ABI
        assert_eq!(c_abi::bridge_c_notify_power_transition(0), -1); // No global bridge initialized yet
        assert_eq!(unsafe { c_abi::bridge_c_init(std::ptr::null()) }, 0);
        assert_eq!(c_abi::bridge_c_notify_power_transition(0), 0); // Suspend
        assert_eq!(c_abi::bridge_c_notify_power_transition(1), 0); // Resume
        assert_eq!(c_abi::bridge_c_shutdown(), 0);
    }

    #[test]
    fn test_chat_orchestrator_and_c_abi() {
        let _guard = TEST_GLOBAL_LOCK.lock().unwrap();
        let _ = c_abi::bridge_c_shutdown();

        let bridge = Arc::new(CoreBridge::new_in_memory().unwrap());

        // 1. Create a conversation
        let conv = bridge
            .chat_create_conversation(Some("Test Conversation"))
            .unwrap();
        assert_eq!(conv.title, "Test Conversation");
        assert!(!conv.id.is_empty());

        // 2. List conversations
        let convs = bridge.chat_list_conversations(10, 0).unwrap();
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0].id, conv.id);

        // 3. Attempt to send message without model loaded -> should fail with clear model error
        let send_res = bridge.chat_send_message(&conv.id, "Hello world");
        assert!(send_res.is_err());

        // 4. Load a model
        bridge.model_load("qwen2.5-0.5b-instruct-q4_k_m").unwrap();

        // 5. Send message with loaded model
        let (_sub_id, rx) = bridge.event_sink().subscribe(32);
        assert!(bridge
            .chat_send_message(&conv.id, "Hello offline AI")
            .is_ok());

        // Expect token deltas and turn complete
        let mut got_delta = false;
        let mut got_complete = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(3) {
            if let Ok(ev) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
                match ev {
                    BridgeEvent::ChatTokenDelta { token, .. } => {
                        assert!(!token.is_empty());
                        got_delta = true;
                    }
                    BridgeEvent::ChatTurnComplete { .. } => {
                        got_complete = true;
                        break;
                    }
                    _ => {}
                }
            }
        }
        assert!(got_delta);
        assert!(got_complete);

        // 6. Get turns
        let turns = bridge.chat_get_turns(&conv.id, None).unwrap();
        assert!(turns.len() >= 2); // user turn + assistant turn
        assert_eq!(turns[0].role, "user");
        assert_eq!(turns[0].content, "Hello offline AI");
        assert_eq!(turns[1].role, "assistant");
        assert!(!turns[1].content.is_empty());

        // 7. Test FTS5 search
        let search_results = bridge.chat_search_turns("offline", 5).unwrap();
        assert!(!search_results.is_empty());
        assert_eq!(search_results[0].conversation_id, conv.id);

        // 8. Test C-ABI round trips
        assert_eq!(unsafe { c_abi::bridge_c_init(std::ptr::null()) }, 0);
        let c_title = CString::new("C-ABI Chat").unwrap();
        let conv_ptr = unsafe { c_abi::bridge_c_chat_create_conversation(c_title.as_ptr()) };
        assert!(!conv_ptr.is_null());
        let conv_json = unsafe { CStr::from_ptr(conv_ptr) }.to_str().unwrap();
        assert!(conv_json.contains("C-ABI Chat"));
        let parsed: serde_json::Value = serde_json::from_str(conv_json).unwrap();
        let c_conv_id = parsed["id"].as_str().unwrap().to_string();
        unsafe { c_abi::bridge_c_free_string(conv_ptr) };

        let list_ptr = c_abi::bridge_c_chat_list_conversations(10, 0);
        assert!(!list_ptr.is_null());
        let list_json = unsafe { CStr::from_ptr(list_ptr) }.to_str().unwrap();
        assert!(list_json.contains("C-ABI Chat"));
        unsafe { c_abi::bridge_c_free_string(list_ptr) };

        // Test cancel
        let c_cid = CString::new(c_conv_id.clone()).unwrap();
        assert_eq!(
            unsafe { c_abi::bridge_c_chat_cancel_turn(c_cid.as_ptr()) },
            0
        );

        // Test delete
        assert_eq!(
            unsafe { c_abi::bridge_c_chat_delete_conversation(c_cid.as_ptr()) },
            0
        );

        // Also test delete on local bridge
        assert!(bridge.chat_delete_conversation(&conv.id).is_ok());

        assert_eq!(c_abi::bridge_c_shutdown(), 0);
    }
}
