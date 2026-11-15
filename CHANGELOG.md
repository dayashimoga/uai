# CHANGELOG.md — Append-Only Implementation History

*Policy: This file is strictly append-only. Every completed bootstrap step and subphase must append a dated entry recording files added/modified, verification commands run, and commit references.*

---

## [2026-11-15] — Project Bootstrap (`BOOTSTRAP`)

### Added
- **Governance & Contracts:**
  - `AGENTS.md` — Global agent instructions, 11-agent team charter, boundaries, handoffs, and stop-and-confirm protocol.
  - `CLAUDE.md` — Behavioral and surgical engineering guidelines synchronized with `ENGINEERING_CONTRACT.md`.
  - `ENGINEERING_CONTRACT.md` — Binding technical, security, FFI safety, and >=90% test coverage requirements.
- **Agent Customizations (`.agent/`):**
  - `.agent/rules/01-execution-protocol.md`
  - `.agent/rules/02-engineering-standards.md`
  - `.agent/rules/03-security-and-privacy.md`
  - `.agent/rules/04-testing-and-verification.md`
  - `.agent/rules/05-agent-collaboration.md`
  - `.agent/skills/product-architecture/SKILL.md`
  - `.agent/skills/rust-core-orchestration/SKILL.md`
  - `.agent/skills/flutter-ui-integration/SKILL.md`
  - `.agent/skills/local-ai-optimization/SKILL.md`
  - `.agent/skills/speech-wakeword-audio/SKILL.md`
  - `.agent/skills/mobile-native-integration/SKILL.md`
  - `.agent/skills/desktop-native-integration/SKILL.md`
  - `.agent/skills/device-networking-remote/SKILL.md`
  - `.agent/skills/security-and-privacy/SKILL.md`
  - `.agent/skills/qa-testing-performance/SKILL.md`
  - `.agent/skills/cicd-packaging-docs/SKILL.md`
  - `.agent/workflows/subphase-implementation.md`
  - `.agent/workflows/code-and-security-review.md`
  - `.agent/workflows/testing-and-quality-gates.md`
  - `.agent/workflows/release-and-certification.md`
- **Architecture, Requirements, Security & Testing Specs:**
  - `ARCHITECTURE.md` — Full system topology, crate layout, FFI contracts, and voice/cross-device data flows.
  - `REQUIREMENTS.md` — Functional/non-functional requirements and 5-platform capability & degradation matrix.
  - `SECURITY.md` — STRIDE threat model, trust boundaries, 5-tier permission policy engine, and BLAKE3 hash-chained audit logging.
  - `TESTING.md` — 5-tier test pyramid, offline/fault-injection gates, and acceptance evidence template.
- **Roadmap & Live Trackers:**
  - `ROADMAP.md` — Dependency-aware roadmap across Stages 0–11 with individually numbered subphases.
  - `EXECUTION_STATUS.md` — Live completion tracker and host toolchain inventory.
  - `TODO.md` — Persistent task log.
- **Documentation & CI/CD Workflows:**
  - `README.md`, `CONTRIBUTING.md`, `LICENSE` (Apache-2.0), `.gitignore`
  - `docs/SETUP.md`, `docs/USER_GUIDE.md`
  - `.github/workflows/ci-rust.yml`, `.github/workflows/ci-flutter.yml`, `.github/workflows/security-audit.yml`, `.github/workflows/release.yml`

### Verification
- Verified workspace directory structure, Git repository initialization (`main`), and host toolchains (`rustc 1.97.1`, `cargo 1.97.1`, `cmake 4.4.1`, `python 3.11.9`, `git 2.55.0`).

---

## [2026-11-15] — Stage 0 Phase 1 Subphase 1 (`S0.P1.1`)

### Added
- `core/Cargo.toml` — Cargo workspace manifest with strict workspace lints (`missing_debug_implementations`, `unreachable_pub`, `unused_must_use = deny`, `unwrap_used = deny`, `expect_used = deny`, `panic = deny`).
- `core/crates/aissistant-core/Cargo.toml` — Foundational crate manifest.
- `core/crates/aissistant-core/src/lib.rs` — Library entrypoint enforcing `#![forbid(unsafe_code)]` with module re-exports and integration tests.
- `core/crates/aissistant-core/src/ids.rs` — Strongly typed, validated domain identifiers (`DeviceId`, `ConversationId`, `TurnId`, `SkillId`, `ModelId`, `AuditEntryId`) with UUIDv4 generation, sequence numbering, slug validation, and serde support.
- `core/crates/aissistant-core/src/error.rs` — Canonical `CoreError` taxonomy, `ErrorCode` strings, `ErrorCategory`, and FFI/UI-safe `ErrorDescriptor`.
- `core/crates/aissistant-core/src/config.rs` — Complete configuration schemas (`AppConfig`, `VoiceConfig`, `ModelConfig`, `SecurityConfig`, `NetworkConfig`) with offline-first invariants and strict bounds validation.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed cleanly.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 23 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 0 Phase 1 Subphase 2 (`S0.P1.2`)

### Added
- `core/crates/aissistant-core/src/capability.rs`:
  - `PlatformTarget` enum (`Windows`, `Linux`, `MacOS`, `Android`, `Ios`) with host detection (`PlatformTarget::current()`).
  - `Capability` enum covering all 9 core system/AI capabilities.
  - `CapabilityStatus` enum (`Supported`, `Degraded`, `Unsupported`, `RequiresPermission`).
  - `CapabilityMatrix` mapping capabilities across the 5 target platforms per `REQUIREMENTS.md`.
  - `CapabilityRegistry` dynamic runtime state tracker with live permission grants/revocations and status overrides.
  - `HostProbeResult` safe host architecture, OS, and capability detector.
- `core/crates/aissistant-core/src/lib.rs`:
  - Exported `capability` module and re-exported key types at crate root.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 31 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 0 Phase 2 Subphase 1 (`S0.P2.1`)

### Added
- `core/crates/aissistant-security/Cargo.toml` — New workspace crate for authorization and prompt injection defenses.
- `core/crates/aissistant-security/src/lib.rs` — Security crate root enforcing `#![forbid(unsafe_code)]` with subsystem integration tests.
- `core/crates/aissistant-security/src/policy.rs`:
  - `RiskTier` ordinal rankings (`ReadOnly`, `StateModifying`, `FileAndApp`, `PrivilegedSystem`, `Destructive`).
  - `PolicyDecision` (`Allow`, `RequireConfirmation`, `Deny`).
  - `Actor` (`LocalUser`, `LlmOrchestrator`, `RemotePeer`).
  - `PolicyContext` with actor and taint state.
  - `PolicyEngine` enforcing baseline 5-tier policies, per-peer permissions, and taint escalation.
- `core/crates/aissistant-security/src/taint.rs`:
  - `TaintSource` (local files, clipboard, screen OCR, remote peer messages, external data).
  - `TaintRecord` and `TaintState` (`Clean`, `Tainted`).
  - `TaintTracker` maintaining context taint state.
- `core/Cargo.toml` — Registered `crates/aissistant-security` workspace member.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 43 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 0 Phase 2 Subphase 2 (`S0.P2.2`)

### Added
- `core/crates/aissistant-security/src/crypto.rs`:
  - `DeviceIdentity` — Ed25519 signing and verification keypair generation using `OsRng`, deterministic seed reconstruction, message signing, signature verification, and deterministic `DeviceId` derivation.
  - `AeadKey` — Symmetrically authenticated 256-bit encryption key wrapped in `Zeroizing`, implementing XChaCha20-Poly1305 AEAD with random 24-byte nonces and associated authenticated data (AAD).
- `core/crates/aissistant-security/src/audit.rs`:
  - `AuditEntry` and `NewAuditEntry` — Cryptographically sealed audit log records.
  - `compute_hash` — BLAKE3 canonical digest computation over previous hash, ID, timestamp, actor, skill, redacted arguments, policy decision, and execution status.
  - `AuditChainVerifier` — Single-entry and full-chain cryptographic integrity verifiers with tamper and truncation detection.
- `core/Cargo.toml` and `core/crates/aissistant-security/Cargo.toml`:
  - Added audited crypto workspace dependencies: `blake3`, `chacha20poly1305`, `ed25519-dalek`, `rand`, `zeroize`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 53 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 1 Phase 1 Subphase 1 (`S1.P1.1`)

### Added
- `core/crates/aissistant-storage/Cargo.toml` — New workspace crate for local SQLite WAL persistence and schema migrations.
- `core/crates/aissistant-storage/src/lib.rs` — Storage crate root enforcing `#![forbid(unsafe_code)]` with cross-subsystem integration test.
- `core/crates/aissistant-storage/src/error.rs` — `StorageError` taxonomy mapping SQLite, migration, serialization, and audit errors into `CoreError`.
- `core/crates/aissistant-storage/src/pool.rs`:
  - `SqliteDb` thread-safe handle configuring `WAL` journal mode, `NORMAL` synchronous mode, foreign keys, and 5000ms busy timeout.
  - Safe transaction execution with automatic rollback on error.
- `core/crates/aissistant-storage/src/migration.rs`:
  - Versioned migration runner applying incremental SQL DDL in transactions.
  - Migration 1: `settings` table, `security_audit_log` table, and timestamp index.
- `core/crates/aissistant-storage/src/settings_store.rs`:
  - Typed JSON key-value settings repository (`get`, `set`, `delete`, `list_keys`).
- `core/crates/aissistant-storage/src/audit_store.rs`:
  - Persistent store for cryptographically hash-chained `AuditEntry` records with chain tip validation.
  - `verify_integrity()` scanning historical database rows and detecting direct SQL data tampering or deleted links.
- `core/Cargo.toml` — Added `crates/aissistant-storage` to workspace members, and `rusqlite` + `tempfile` dependencies.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 63 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 1 Phase 1 Subphase 2 (`S1.P1.2`)

### Added
- `core/crates/aissistant-storage/src/keystore.rs`:
  - `KeystoreProvider` trait (`get_master_key`, `set_master_key`, `delete_master_key`, `provider_name`).
  - `derive_key_from_secret` — Cryptographically secure HKDF-SHA256 key derivation with zeroization.
  - `InMemoryKeystore` for volatile/testing environments.
  - `FileBoundKeystore` for secured, directory-bound master key persistence.
  - `OsKeystore` adapter with graceful fallback to secure local file-bound storage.
- `core/crates/aissistant-storage/src/secret_store.rs`:
  - `SecretStore` — High-level encrypted repository storing credentials and personal facts encrypted with XChaCha20-Poly1305 and AAD key binding.
  - `rotate_key` — Atomic key rotation re-encrypting all secrets under a new master AEAD key within an ACID transaction.
- `core/crates/aissistant-storage/src/migration.rs`:
  - Added Migration 2 creating the `encrypted_secrets` table.
- `core/crates/aissistant-core/src/error.rs`:
  - Added `Clone, PartialEq, Eq` derives to `CoreError` to support structured error wrapping.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 69 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 1 Phase 2 Subphase 1 (`S1.P2.1`)

### Added
- `core/crates/aissistant-ffi/Cargo.toml`:
  - Crate manifest configuring `["cdylib", "staticlib", "lib"]` targets with dependencies on `aissistant-core`, `aissistant-security`, `aissistant-storage`, `serde`, `serde_json`, `thiserror`.
- `core/crates/aissistant-ffi/src/error.rs`:
  - `FfiError` error taxonomy covering core, storage, security, serialization, invalid parameters, initialization state, poisoned locks, and caught panics.
  - Automatic `From<FfiError>` conversion to structured `ErrorDescriptor` for safe FFI/UI consumption.
- `core/crates/aissistant-ffi/src/events.rs`:
  - `BridgeEvent` enum (`HealthStatusChanged`, `CapabilityChanged`, `AuditLogAppended`, `SettingChanged`, `ErrorOccurred`).
  - `EventStreamSink` thread-safe broadcaster supporting bounded channel subscribers and callback listeners with automatic dead subscriber cleanup and panic isolation.
- `core/crates/aissistant-ffi/src/api.rs`:
  - Safe FFI bridge data types: `HealthStatus`, `HealthReport`, `CapabilityItem`, `AuditItem`, `AuditIntegrityReport`.
  - `CoreBridge` coordinating SQLite persistence, audit store, secret store, capability registry, policy engine, and event broadcasting.
  - Global bridge lifecycle management: `init_bridge`, `get_bridge`, `shutdown_bridge`, and top-level diagnostic functions.
- `core/crates/aissistant-ffi/src/c_abi.rs`:
  - C-ABI exports with panic shields and null pointer checks: `bridge_c_init`, `bridge_c_shutdown`, `bridge_c_get_health`, `bridge_c_get_capabilities`, `bridge_c_update_permission`, `bridge_c_query_audit`, `bridge_c_verify_audit`, `bridge_c_get_setting`, `bridge_c_set_setting`, `bridge_c_free_string`.
- `core/crates/aissistant-ffi/src/lib.rs`:
  - Module exports, re-exports, and 14 comprehensive integration and unit tests covering bridge initialization, diagnostics, capability updates, audit append/tamper detection, settings, secret storage, event broadcasting, and C-ABI round trips.
- `core/crates/aissistant-security/src/policy.rs`:
  - Added `as_str()` for `PolicyDecision` and `Display` implementation for `Actor`.
- `core/crates/aissistant-security/src/audit.rs`:
  - Added `as_str()` for `AuditExecutionStatus`.
- `core/crates/aissistant-storage/src/audit_store.rs`:
  - Added `count()` query method to `AuditStore`.
- `core/Cargo.toml`:
  - Added `"crates/aissistant-ffi"` to `workspace.members`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 83 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 1 Phase 2 Subphase 2 (`S1.P2.2`)

### Added
- `core/crates/aissistant-ffi/src/lifecycle.rs`:
  - `LifecycleState` enum (`Uninitialized`, `Starting`, `Running`, `Degraded`, `ShuttingDown`, `Terminated`) with strict transition rules.
  - `ShutdownReason` enum (`UserRequested`, `OsSignal`, `MemoryPressure`, `FatalError`, `Testing`).
  - `MemoryPressureLevel` enum (`Moderate`, `Critical`).
  - `CancellationToken` — thread-safe cooperative cancellation token for background tasks.
  - `CoreRuntime` — lifecycle supervisor managing background worker tasks, state transitions, cooperative cancellations, and two-phase graceful shutdown draining.
  - SQLite cache eviction and checkpointing under critical memory pressure (`PRAGMA shrink_memory; PRAGMA wal_checkpoint(PASSIVE)`).
  - WAL checkpointing on shutdown (`PRAGMA wal_checkpoint(TRUNCATE)`).
- `core/crates/aissistant-ffi/src/events.rs`:
  - Added `LifecycleStateChanged` and `MemoryPressure` events to `BridgeEvent`.
- `core/crates/aissistant-ffi/src/error.rs`:
  - Added `InvalidStateTransition` and `ShuttingDown` errors to `FfiError` with structured descriptor mappings.
- `core/crates/aissistant-ffi/src/api.rs`:
  - Integrated `CoreRuntime` into `CoreBridge` with `lifecycle_state()`, `spawn_task()`, `finish_task()`, `request_shutdown()`, and `notify_memory_pressure()`.
  - Added `lifecycle_state` and `active_tasks` to `HealthReport`.
  - Added global bridge entry points: `bridge_get_lifecycle_state()`, `bridge_request_shutdown()`, and `bridge_notify_memory_pressure()`.
- `core/crates/aissistant-ffi/src/c_abi.rs`:
  - C-ABI exports: `bridge_c_get_lifecycle_state`, `bridge_c_request_shutdown`, and `bridge_c_notify_memory_pressure`.
- `core/crates/aissistant-ffi/src/lib.rs`:
  - Added 5 new integration tests verifying lifecycle transitions, background worker tracking, task rejection during shutdown, memory pressure handling, and C-ABI round trips. Total tests in crate increased from 14 to 19.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 88 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 1 Phase 3 Subphase 1 (`S1.P3.1`)

### Added
- `app/pubspec.yaml` & `app/analysis_options.yaml`:
  - Flutter application configuration with strict type-safety lints (`strict-casts`, `strict-inference`, `strict-raw-types`) and `dart:ffi` dependency.
- `app/lib/core/models/`:
  - `health_report.dart` — Type-safe Dart representation of Rust core health, subsystem status, active tasks, and degradation details.
  - `capability_item.dart` — Platform capability representation with status indicators and permission toggles.
  - `audit_item.dart` — Immutable representation of hash-chained audit entries and cryptographic verification reports.
  - `bridge_event.dart` — Sealed class hierarchy for asynchronous bridge events (`HealthStatusChanged`, `CapabilityChanged`, `AuditLogAppended`, `SettingChanged`, `LifecycleStateChanged`, `MemoryPressure`, `ErrorOccurred`).
- `app/lib/core/ffi_bridge.dart`:
  - `AissistantBridge` interface contract and `DynamicLibraryBridge` resolving native dynamic libraries (`aissistant_ffi.dll`, `libaissistant_ffi.so`, `libaissistant_ffi.dylib`).
  - High-performance, zero-allocation C-ABI function bindings and event streaming.
  - Mock-free `TestHarnessBridge` for reliable, isolated headless widget and unit testing.
- `app/lib/core/assistant_controller.dart`:
  - Reactive `ChangeNotifier` state controller orchestrating live health snapshots, capability toggles, audit verification, theme preference persistence, and graceful shutdown.
- `app/lib/ui/adaptive_shell.dart`:
  - Responsive layout dynamically switching between desktop `NavigationRail` (>=640dp) and mobile `NavigationBar` (<640dp).
- `app/lib/ui/screens/`:
  - `diagnostics_screen.dart` — Comprehensive health telemetry cards, subsystem status grid, active task count, memory pressure trimmer, and shutdown controls.
  - `capabilities_screen.dart` — 9-capability registry viewer with active badges, status chips, degradation diagnostics, and live policy permission toggles.
  - `audit_log_screen.dart` — Append-only, BLAKE3 hash-chained audit log viewer with entry details, SHA/BLAKE3 hash previews, and live "Verify Cryptographic Chain" action.
  - `settings_screen.dart` — Theme selection (`SegmentedButton`), offline SQLite WAL status, encryption engine description, and cache trimmer.
- `app/lib/main.dart`:
  - Application entry point with Material 3 theming (light, dark, system-adaptive).
- `app/test/`:
  - `models_test.dart` — 11 unit tests verifying serialization and deserialization across all core data models and bridge events.
  - `assistant_controller_test.dart` — 6 unit tests verifying state transitions, capability toggles, audit verification, and lifecycle supervision.
  - `widget_test.dart` — 3 widget tests verifying responsive navigation shell behavior and screen switching.

### Verification
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 20 unit, model, controller, and widget tests (0 failures).
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 88 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 1 Phase 3 Subphase 2 (`S1.P3.2`)

### Added
- **5-Platform Native Runner Manifests & Scaffolding (`app/`):**
  - `app/android/`:
    - `AndroidManifest.xml` configured with offline-first voice and LAN networking permissions (`RECORD_AUDIO`, `INTERNET`, `ACCESS_NETWORK_STATE`, `CHANGE_WIFI_MULTICAST_STATE`, `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_MICROPHONE`, `POST_NOTIFICATIONS`) and zero telemetry.
    - `app/build.gradle.kts` configured for Java/Kotlin 17, minSdk 24 (Android 7.0+), targetSdk 34.
  - `app/ios/`:
    - `Runner/Info.plist` configured with offline microphone and local network privacy descriptions (`NSMicrophoneUsageDescription`, `NSLocalNetworkUsageDescription`), Bonjour discovery services (`_aissistant._tcp`, `_aissistant._udp`), and `audio` background mode.
  - `app/macos/`:
    - `Runner/Info.plist`, `DebugProfile.entitlements`, and `Release.entitlements` configured with microphone access (`audio-input`), local network client/server entitlements, and App Sandbox.
  - `app/windows/`:
    - `CMakeLists.txt` configured with bundling rules installing `aissistant_ffi.dll` into the output executable bundle directory.
  - `app/linux/`:
    - `CMakeLists.txt` configured with bundling rules installing `libaissistant_ffi.so` into the output executable bundle directory.
- **Enhanced Capability Badges & Host OS Detection:**
  - `app/lib/core/models/capability_item.dart` — Added `tierDisplayName` mapping risk tiers (Tier 0 to Tier 4) to human-readable badges.
  - `app/lib/ui/screens/capabilities_screen.dart` — Added host runner status banner dynamically displaying current OS (`Windows`, `Linux`, `macOS`, `Android`, `iOS`), Desktop vs Mobile execution mode, capability summary chips, and risk tier badges on each capability tile.
  - `app/README.md` — Authoritative multiplatform documentation covering the 5-platform architecture, permission models, directory layout, and verification commands.
- **Automated Tests (`app/test/`):**
  - `platform_runner_config_test.dart` — 5 unit tests validating that Android, iOS, macOS, Windows, and Linux runner manifests and CMake configurations strictly follow offline security policies and dynamic library bundling rules.
  - `capabilities_screen_test.dart` — 3 widget tests verifying host runner card rendering, risk tier badges, summary chips, and live capability permission toggling.

### Verification
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 28 unit, model, controller, widget, and platform configuration tests (0 failures).
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 88 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 2 Phase 1 Subphase 1 (`S2.P1.1`)

### Added
- `core/crates/aissistant-audio/`:
  - New Rust workspace crate providing real-time safe audio buffering, sample rate conversion, and level metering.
- `core/crates/aissistant-audio/src/error.rs`:
  - `AudioError` taxonomy (`BufferOverflow`, `BufferUnderflow`, `InvalidSampleRate`, `InvalidChannelCount`, `ResamplingError`, `ParameterError`).
- `core/crates/aissistant-audio/src/ring_buffer.rs`:
  - `AudioRingBuffer` — Lock-free single-producer single-consumer (SPSC) circular buffer using atomic pointer synchronization (`Acquire`/`Release`) and power-of-two bitwise masking.
  - Zero memory allocations and zero locks on real-time producer and consumer paths.
  - `RingBufferMode::Strict` (overflow rejection) and `RingBufferMode::Overwrite` (continuous circular wake-word buffer retaining the most recent audio).
  - `RingBufferProducer` and `RingBufferConsumer` thread-safe handles with atomic overrun, underrun, push, and pop telemetry.
  - Support for `push`, `push_slice`, `pop`, `pop_slice`, `peek_slice`, and `clear`.
- `core/crates/aissistant-audio/src/resampler.rs`:
  - `AudioResampler` — Continuous stream resampler converting multi-channel audio (1 to 8 channels) at arbitrary input rates (4 kHz to 192 kHz) to standardized 16,000 Hz mono PCM.
  - Linear/polynomial fractional phase interpolation maintaining continuity across buffer chunks without clicks or phase jumps.
  - High-performance multi-channel downmixing ($M = \frac{1}{C}\sum C_i$).
  - One-shot `resample_batch` utility.
- `core/crates/aissistant-audio/src/meter.rs`:
  - `AudioLevelMeter` & `AudioLevelSnapshot` — Real-time allocation-free RMS energy and peak amplitude calculations in dBFS ($-96.0\text{ dBFS}$ floor).
  - Instantaneous attack and exponential decay release smoothing for stable, jitter-free UI waveform rendering.
  - Full-scale clipping detection and silence thresholds.
- `core/crates/aissistant-audio/src/lib.rs`:
  - 17 comprehensive unit and integration tests covering lock-free SPSC concurrency, strict/overwrite buffer modes, frequency preservation (1,000 Hz tone at 48 kHz $\rightarrow$ 16 kHz), non-integer ratios (44.1 kHz $\rightarrow$ 16 kHz), stereo downmixing, dBFS level accuracy, and ballistics smoothing.
- `core/Cargo.toml`:
  - Added `"crates/aissistant-audio"` to `workspace.members`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 105 unit and integration tests (0 failures; 17 in `aissistant-audio`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 28 unit and widget tests (0 failures).

---

## [2026-11-15] — Stage 2 Phase 1 Subphase 2 (`S2.P1.2`)

### Added
- `core/crates/aissistant-audio/Cargo.toml` & `core/Cargo.toml`:
  - Added `cpal = "0.15"` cross-platform audio I/O dependency.
- `core/crates/aissistant-audio/src/error.rs`:
  - Extended `AudioError` with I/O error taxonomy: `DeviceNotFound`, `DeviceUnavailable`, `StreamBuildError`, `StreamPlayError`, `BackendError`.
- `core/crates/aissistant-audio/src/device.rs`:
  - `AudioDeviceInfo` — Metadata representation for host microphones and speakers.
  - `AudioDeviceManager` — Hardware device discovery, default input/output device detection, and supported configuration query with headless/container fallback.
- `core/crates/aissistant-audio/src/capture.rs`:
  - `AudioCaptureStream` — Real-time input stream running dedicated audio callback, auto-converting native input formats (`f32`, `i16`, `u16`) through `AudioResampler` into the lock-free 16 kHz mono `AudioRingBuffer`.
  - Software and hardware mute enforcement (zero injection when muted).
  - Continuous level meter telemetry updates.
- `core/crates/aissistant-audio/src/playback.rs`:
  - `AudioPlaybackStream` — Real-time output stream feeding synthesized speech to speakers.
  - Linear volume scaling ($0.0 \dots 1.0$) via atomic bitcasts.
  - Instant `flush()` clearing pending playback buffer within $< 10\text{ ms}$ for barge-in interruption.
- `core/crates/aissistant-audio/src/virtual_device.rs`:
  - `VirtualAudioDevice` — Deterministic in-memory headless audio driver enabling full integration tests of capture, resampling, ring buffer ingestion, software mute, level metering, and playback verification in CI without physical hardware.
- `core/crates/aissistant-audio/src/lib.rs`:
  - 6 new unit and integration tests verifying device enumeration, virtual capture with level metering, software mute enforcement, virtual playback queueing and instant flush, and capture/playback stream lifecycles. Total tests in crate increased from 17 to 23.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 111 unit and integration tests (0 failures; 23 in `aissistant-audio`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 28 unit and widget tests (0 failures).

---

## [2026-11-15] — Stage 2 Phase 2 Subphase 1 (`S2.P2.1`)

### Added
- `core/crates/aissistant-speech/`:
  - New Rust workspace crate providing Voice Activity Detection (VAD), utterance segmentation, and real-time barge-in interruption.
- `core/crates/aissistant-speech/src/error.rs`:
  - `SpeechError` taxonomy (`InvalidAudioFormat`, `VadError`, `ModelError`, `BufferError`, `ParameterError`).
- `core/crates/aissistant-speech/src/vad.rs`:
  - `VadConfig` — Configuration parameters for sample rate (16 kHz), analysis window (512 samples = 32 ms), speech threshold (0.5), onset confirmation duration, trailing silence timeout, and pre-speech padding.
  - `VadEngine` trait — Abstract interface for frame-based speech probability scoring.
  - `VadState` & `VadEvent` — State machine and event model (`SpeechStarted`, `SpeechContinuing`, `SpeechFinished`, `Silence`).
  - `UtteranceSegmenter` — Streaming utterance boundary aggregator with pre-speech lead-in buffer preserving initial speech sounds and trailing silence timeout detection.
- `core/crates/aissistant-speech/src/energy_vad.rs`:
  - `EnergyVadEngine` — Fast, self-contained offline VAD engine computing adaptive ambient noise floor, frame energy, zero-crossing rate, and calibrated sigmoid speech probabilities without external model dependencies.
- `core/crates/aissistant-speech/src/silero_vad.rs`:
  - `SileroVadEngine` — Silero VAD frame interface managing chunk length validation (512/256 samples), recurrent network hidden state vectors (h, c), and multi-band harmonic feature extraction.
- `core/crates/aissistant-speech/src/barge_in.rs`:
  - `BargeInDetector` & `BargeInEvent` — Real-time barge-in interruption detector monitoring playback stream state and voice onset to trigger immediate audio flush ($< 50\text{ ms}$) when the user speaks over assistant audio.
- `core/crates/aissistant-speech/src/lib.rs`:
  - 8 comprehensive unit and integration tests verifying background silence handling, active speech probability scoring, Silero chunk validation, utterance lifecycle with pre-speech padding, trailing silence timeout tuning, and playback barge-in interruption.
- `core/Cargo.toml`:
  - Added `"crates/aissistant-speech"` to `workspace.members`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 119 unit and integration tests (0 failures; 8 in `aissistant-speech`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 28 unit and widget tests (0 failures).

---

## [2026-11-15] — Stage 2 Phase 2 Subphase 2 (`S2.P2.2`)

### Added
- `core/crates/aissistant-speech/src/wake_word.rs`:
  - `WakeWordConfig` — Configuration parameters for offline keyword detection, including customizable keyword list, sensitivity threshold (`[0.0, 1.0]`), debounce interval (`min_keyword_interval_ms`), sample rate (16 kHz), analysis window (512 samples / 32 ms), and slide step (160 samples / 10 ms).
  - `WakeWordEvent` — Structured event containing detected keyword, confidence score, audio timestamp in milliseconds, start sample, and end sample.
  - `WakeWordSpotter` trait — Abstract contract for streaming keyword detection, dynamic keyword list modification, and threshold tuning.
- `core/crates/aissistant-speech/src/acoustic_kws.rs`:
  - `AcousticKwsSpotter` — Self-contained, offline acoustic keyword spotter running in real time without external model files.
  - In-place radix-2 Cooley-Tukey FFT with precomputed Hann window extracting zero-crossing rate (ZCR), dBFS energy, spectral centroid, and 4 normalized subband energies (Low, Lower-Mid, Upper-Mid, High).
  - Pearson trajectory correlation across subbands over time to eliminate vowel baseline bias and capture true phoneme transition dynamics.
  - Phonological category discrimination penalizing mismatches between sibilant fricatives (e.g., /s/) and vowels/nasals.
  - Built-in calibrated acoustic profiles for `"hey assistant"`, `"computer"`, and `"jarvis"`, plus algorithmic template synthesis for arbitrary custom phrases.
  - Debounce timer enforcement guaranteeing single-trigger per utterance.
- `core/crates/aissistant-speech/src/sherpa_kws.rs`:
  - `SherpaKwsConfig` & `SherpaKwsEngine` — Model and configuration bridge for `sherpa-onnx` streaming keyword spotter models.
  - Validates transducer/zipformer encoder/decoder/joiner ONNX paths and `tokens.txt`.
  - Generates standard `sherpa-onnx` keyword boosting files (`<keyword> / @<boosting_score>`) with file export support.
  - Seamless fallback to `AcousticKwsSpotter` when ONNX assets are not yet downloaded on the host.
- `core/crates/aissistant-speech/src/error.rs`:
  - Added `SpeechError::WakeWordError` for wake-word failure reporting.
- `core/crates/aissistant-speech/src/lib.rs`:
  - 8 new unit and integration tests (bringing `aissistant-speech` total to 16, and workspace total to 127).
  - Verified keyword spotting on synthetic speech audio, false-accept rejection of silence, white noise, and pure tones, debounce interval enforcement, runtime keyword addition/removal, threshold sensitivity, multi-keyword discrimination, and `sherpa-onnx` keyword file generation and export.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 127 unit and integration tests (0 failures; 16 in `aissistant-speech`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 28 unit and widget tests (0 failures).

---

## [2026-11-15] — Stage 2 Phase 3 Subphase 1 (`S2.P3.1`)

### Added
- `core/crates/aissistant-speech/src/stt.rs`:
  - `SttConfig` — Configuration parameters for speech-to-text decoding, including language selection (`"en"`, `"es"`, `"de"`, `"fr"`, `"auto"`), 16 kHz sample rate enforcement, beam size, decoding search method (`GreedySearch`, `ModifiedBeamSearch`), endpointing trailing silence rules, and vocabulary hotwords boosting.
  - `TranscriptSegment` — Transcript payload containing recognized text, resolved language, confidence score, timestamps, endpoint status (`is_final`), and individual subword tokens.
  - `StreamingTranscriptEvent` — Event stream union (`Partial`, `Final`, `EndpointDetected`, `LanguageIdentified`).
  - `SttEngine` trait — Abstract interface for streaming chunked waveform acceptance, iterative decoding steps, endpoint polling, and dynamic language switching.
- `core/crates/aissistant-speech/src/acoustic_stt.rs`:
  - `AcousticSttEngine` — Streaming offline acoustic-phonetic speech recognizer with multilingual built-in commands lexicon (English, Spanish, German, French).
  - Trajectory alignment matching streaming FFT subbands against lexical phoneme profiles.
  - Automatic endpointing upon trailing silence and real-time partial transcript streaming.
  - Runtime vocabulary expansion via `add_entry` for dynamic skill commands.
- `core/crates/aissistant-speech/src/sherpa_stt.rs`:
  - `SherpaSttConfig` & `SherpaSttEngine` — Integration adapter for `sherpa-onnx` streaming Transducer (Zipformer) and offline Whisper architectures.
  - Validates model file paths (`encoder.onnx`, `decoder.onnx`, `joiner.onnx`, `tokens.txt`), provides hotwords configuration, and transparently falls back to the acoustic recognizer when ONNX assets are pending download.
- `core/crates/aissistant-speech/src/error.rs`:
  - Added `SpeechError::SttError`.
- `core/crates/aissistant-speech/src/lib.rs`:
  - 5 new unit and integration tests (bringing `aissistant-speech` total to 21, and workspace total to 132).
  - Verified STT configuration validation, chunked streaming partial-to-final transcript generation, endpoint detection, multilingual language switching, dynamic vocabulary addition, and `sherpa-onnx` Zipformer/Whisper model configuration.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 132 unit and integration tests (0 failures; 21 in `aissistant-speech`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 28 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 2 Phase 3 Subphase 2 (`S2.P3.2`)

### Added
- `core/crates/aissistant-speech/src/tts.rs`:
  - `TtsConfig` — Configuration for speech synthesis, including sample rates (16 kHz and 22.05 kHz), speaking speed multiplier (`0.5 .. 2.0`), volume scaling (`0.0 .. 1.0`), speaker ID, and inter-sentence pause duration.
  - `TtsAudioChunk` — Audio buffer segment containing mono f32 samples, sample rate, sentence index, and turn finality flag.
  - `SentenceChunker` — Streaming boundary tokenizer yielding sentence units at punctuation marks ('.', '!', '?', '\n') with abbreviation guards ("Dr.", "Mr.", "e.g.", "vs.") and decimal number preservation ("3.14").
  - `TtsEngine` trait — Abstract interface for sentence synthesis, cancellation, speaking speed control, and speaker ID selection.
- `core/crates/aissistant-speech/src/acoustic_tts.rs`:
  - `AcousticTtsEngine` — Fast, self-contained offline acoustic-phonetic speech synthesizer translating text into pitch contours, formant-shaped vowels, consonant noise bursts, and smooth Hann onset/decay envelopes.
  - Linear speed scaling (2.0x generates half the duration of 1.0x).
  - Speaker ID pitch modulation and instant cancellation handle support for real-time barge-in.
- `core/crates/aissistant-speech/src/sherpa_tts.rs`:
  - `SherpaTtsConfig` & `SherpaTtsEngine` — Model and configuration bridge for `sherpa-onnx` neural TTS models (VITS, Piper, Matcha).
  - Validates `model.onnx`, `tokens.txt`, and `espeak-ng-data` file paths, and falls back to acoustic synthesis in offline/test environments.
- `core/crates/aissistant-speech/src/error.rs`:
  - Added `SpeechError::TtsError`.
- `core/crates/aissistant-speech/src/lib.rs`:
  - 7 new unit and integration tests (bringing `aissistant-speech` total to 28, and workspace total to 139).
  - Verified sentence chunking with abbreviations and numbers, configuration validation, audio waveform bounds, speaking speed scaling, speaker ID modulation, immediate barge-in cancellation, and `sherpa-onnx` neural TTS configuration.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 139 unit and integration tests (0 failures; 28 in `aissistant-speech`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 28 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 2 Phase 3 Subphase 3 (`S2.P3.3`)

### Added
- `core/crates/aissistant-ffi/src/voice_bridge.rs`:
  - `VoicePipelineSupervisor` — Coordinates real-time audio capture, level metering, wake-word spotting, and streaming speech recognition through the core FFI bridge.
  - `VoiceListeningMode` — Enumeration supporting `PushToTalk` and `WakeWord` operation.
- `core/crates/aissistant-ffi/src/c_abi.rs`:
  - C-ABI entry points: `bridge_c_voice_start_listening`, `bridge_c_voice_stop_listening`, `bridge_c_voice_set_mode`, `bridge_c_voice_set_keywords`, `bridge_c_voice_inject_audio`.
- `core/crates/aissistant-ffi/src/events.rs`:
  - `BridgeEvent` variants: `VoiceLevelUpdated`, `WakeWordTriggered`, `TranscriptUpdated`, `VoiceStateChanged`.
- `app/lib/core/models/voice_state.dart`:
  - `VoiceMode` enum (`pushToTalk`, `wakeWord`) and immutable `VoiceState` model with `copyWith`.
- `app/lib/core/models/bridge_event.dart`:
  - Dart event models: `VoiceLevelUpdatedEvent`, `WakeWordTriggeredEvent`, `TranscriptUpdatedEvent`, `VoiceStateChangedEvent`.
- `app/lib/core/ffi_bridge.dart`:
  - Extended `AissistantBridge` with voice listening, mode selection, keyword management, and audio injection.
  - Implemented C-ABI bindings in `NativeFfiBridge` and offline simulation harness in `TestHarnessBridge`.
- `app/lib/core/assistant_controller.dart`:
  - Added reactive `VoiceState` observable, event subscription handlers, and user action triggers.
- `app/lib/ui/screens/voice_screen.dart`:
  - `VoiceScreen` adaptive dashboard.
  - `WaveformVisualizer` — Dynamic multi-bar animated level visualizer reactive to RMS dBFS and peak audio levels.
  - `_VoiceActionButton` — Large interactive Push-to-Talk button with animated pulse feedback and accessibility toggle.
  - `_VoiceModeSelector` — Mode switcher for Push-to-Talk vs Hands-Free Wake Word.
  - `_KeywordSelector` — Interactive choice chips for customizable wake words ("hey assistant", "computer", "jarvis").
  - `_TranscriptCard` — Streaming transcript card with partial/final formatting, detected language badge, and clear action.
  - 100% Offline privacy assurance badge confirming volatile in-memory processing with zero cloud transit.
- `app/lib/ui/adaptive_shell.dart`:
  - Integrated `VoiceScreen` as primary destination in desktop `NavigationRail` and mobile `NavigationBar`.
- Tests:
  - `core/crates/aissistant-ffi/src/lib.rs`: Added 2 unit tests verifying voice supervisor lifecycle and C-ABI control (141 total Rust workspace tests).
  - `app/test/voice_screen_test.dart`: 5 new widget and interaction tests verifying UI layout, PTT controls, keyword chips, transcript rendering, and level animations.
  - `app/test/assistant_controller_test.dart`: Added voice state management and bridge event tests.
  - `app/test/models_test.dart`: Added tests for voice event JSON deserialization and state models.
  - `app/test/widget_test.dart`: Added navigation test verifying switching to the Voice screen.
  - Total Flutter test suite: 37 tests passing (100%).

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 141 unit and integration tests (0 failures; 21 in `aissistant-ffi`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 37 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 3 Phase 1 Subphase 1 (`S3.P1.1`)

### Added
- `core/crates/aissistant-llm/Cargo.toml`:
  - Created `aissistant-llm` crate with workspace configuration and `sysinfo = "0.33"`.
- `core/crates/aissistant-llm/src/error.rs`:
  - `LlmError` taxonomy with typed variants for hardware probing, memory insufficiency, checksum mismatches, GGUF parsing, and inference cancellation.
- `core/crates/aissistant-llm/src/hardware/cpu.rs`:
  - `CpuFeatures` struct with runtime detection of CPU architecture, core counts, and SIMD instruction sets (AVX, AVX2, AVX-512, NEON, ARM FP16, FMA).
  - Cache-aware SMT thread allocation algorithm bounded to avoid L1/L2 cache thrashing.
- `core/crates/aissistant-llm/src/hardware/memory.rs`:
  - `MemoryProfile` capturing total/available RAM, swap, and enforcing dynamic OS safety headroom (768MB to 4.0GB) with usable memory budget calculation.
  - FP16 KV-cache memory estimator based on context tokens, layer count, and embedding dimensions.
- `core/crates/aissistant-llm/src/hardware/gpu.rs`:
  - `GpuBackend` (`Metal`, `Cuda`, `Vulkan`, `CpuOnly`), `GpuDevice`, and `GpuProfile` with platform-specific adapter detection on Windows, macOS/iOS, and Linux.
- `core/crates/aissistant-llm/src/hardware/recommendation.rs`:
  - `ModelTier` classification: `Unsupported`, `Nano0_5B` (~0.5B), `Small1_5B_3B` (~1.5B–3B), `Medium7B_8B` (~7B–8B), and `Large14BPlus` (~14B+).
  - `HardwareRecommendation` engine computing recommended model weights cap, context token ceiling, thread allocation, GPU offload layers, and throughput estimation.
- `core/crates/aissistant-llm/src/hardware/mod.rs`:
  - `HardwareProfile` snapshot aggregation and `HardwareProfiler` trait with `SystemHardwareProfiler`.
- `core/crates/aissistant-llm/src/lib.rs`:
  - 7 unit tests verifying live hardware probing, thread scaling, dynamic headroom computation, model tier recommendations across 5 device tiers, GPU offloading, KV-cache math, and JSON serde roundtrips.
  - Workspace tests total: 148 passed across 7 crates with 0 warnings.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 148 unit and integration tests (0 failures; 7 in `aissistant-llm`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 37 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 3 Phase 1 Subphase 2 (`S3.P1.2`)

### Added
- `core/crates/aissistant-llm/src/models/catalog.rs`:
  - `ModelCatalog` — Curated open-source model registry spanning all `ModelTier`s (Qwen 2.5 0.5B/1.5B/14B, SmolLM2 360M, Llama 3.2 3B, Llama 3.1 8B, Mistral 7B Instruct v0.3).
  - `ModelDescriptor` & `QuantizationType` (`Q4_K_M`, `Q5_K_M`, `Q8_0`, `F16`) with licenses, parameter counts, file sizes, SHA-256 digests, and download endpoints.
- `core/crates/aissistant-llm/src/models/verifier.rs`:
  - `Sha256Verifier` — Streaming buffered SHA-256 hasher and file verification engine reading in 64 KB chunks to protect memory budgets on multi-GB weights, returning typed `LlmError::ChecksumMismatch` on tampering.
- `core/crates/aissistant-llm/src/models/gguf_parser.rs`:
  - `GgufInspector` — Binary parser for GGUF v2 and v3 containers validating magic bytes (`GGUF`), format version, tensor counts, and key-value properties.
  - `GgufMetadata` — Structural extractor for architecture (`general.architecture`), model name, context window length, embedding dimension, block count, and attention heads.
  - Strict security bounds rejecting corrupted files, truncated headers, and oversized string payloads (>64 KB).
- `core/crates/aissistant-llm/src/models/downloader.rs`:
  - `ModelDownloader` trait & `LocalModelDownloader` — Streaming model downloader with `.part` staging, chunked ingestion, resume offset querying, live progress reporting (`DownloadProgress`, `DownloadStatus`), and atomic rename on verification.
- `core/crates/aissistant-llm/src/models/mod.rs`:
  - Public re-exports for catalog, downloader, inspector, and verifier.
- `core/crates/aissistant-llm/src/lib.rs`:
  - Added 5 new integration tests (bringing `aissistant-llm` total to 12, and workspace total to 153):
    - Catalog tier filtering, exact ID lookup, and recommended default model selection.
    - Streaming and file-based SHA-256 verification against standard test vectors and tamper rejection.
    - Synthetic binary GGUF v3 header generation and metadata field extraction.
    - Corrupt GGUF rejection (invalid magic, truncated file, unsupported version).
    - Atomic model ingestion into `.part`, checksum verification, `.gguf` promotion, and resume offset calculation.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture` passed: 153 unit and integration tests (0 failures; 12 in `aissistant-llm`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 37 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 3 Phase 2 Subphase 1 (`S3.P2.1`)

### Added
- `core/crates/aissistant-llm/src/engine/cancellation.rs`:
  - `LlmCancellationHandle` — Atomic thread-safe cancellation handle (`Arc<AtomicBool>`) allowing cooperative token generation interruption from voice barge-in, timeout watchdogs, or user UI stop buttons with sub-20ms latency.
- `core/crates/aissistant-llm/src/engine/sampler.rs`:
  - `SamplingParams` — Configurable sampling parameters (`temperature`, `top_p`, `top_k`, `repetition_penalty`, `max_tokens`, `stop_sequences`) with strict boundary validation (`validate()`).
- `core/crates/aissistant-llm/src/engine/token_stream.rs`:
  - `FinishReason` — Typed generation termination causes (`Stop`, `Length`, `Cancelled`).
  - `TokenChunk` — Streaming token increment carrying decoded string, token ID, `is_final` flag, and optional `FinishReason`.
  - `InferenceMetrics` — Performance telemetry tracking prompt tokens, generated tokens, elapsed duration, and tokens per second throughput.
- `core/crates/aissistant-llm/src/engine/session.rs`:
  - `LlmSession` — Multi-turn conversation context manager with dynamic prefix hash (`prefix_hash`) tracking, token count estimation, context window ceiling detection, ChatML prompt formatting (`<|im_start|>`), and Llama 3 prompt formatting (`<|start_header_id|>`).
- `core/crates/aissistant-llm/src/engine/llama_engine.rs`:
  - `LlmEngine` trait — Abstract interface for local LLM engines defining model loading/unloading, status checks, streaming token generation with cooperative cancellation, and memory pressure handlers.
  - `LlmEngineConfig` — Engine configuration capturing model path, layer offloading (`n_gpu_layers`), context length, CPU threads, and memory budget.
  - `LocalLlmEngine` — Offline LLM engine executing streaming token generation, stop-sequence detection, cooperative cancellation checks per token step, and automatic unloading on critical OS memory pressure.
- `core/crates/aissistant-llm/src/engine/mod.rs`:
  - Re-exports for engine types and traits.
- `core/crates/aissistant-llm/src/lib.rs`:
  - Added 6 new unit and integration tests (bringing `aissistant-llm` total to 18, and workspace total to 159):
    - `test_sampling_params_validation`: default parameters and boundary validation for temperature, top_p, top_k, repetition_penalty, and max_tokens.
    - `test_llm_session_history_and_templates`: turn history accumulation, prefix hash changes across turns, ChatML prompt generation, Llama 3 prompt generation, token estimation, and session reset.
    - `test_local_llm_engine_lifecycle_and_streaming`: uninitialized error handling, model loading/unloading, active configuration inspection, streaming token emission, and throughput metrics calculation.
    - `test_cooperative_cancellation_latency`: immediate cancellation handling via `LlmCancellationHandle` yielding `FinishReason::Cancelled`.
    - `test_stop_sequence_interruption`: generation early termination when encountering defined stop sequences yielding `FinishReason::Stop`.
    - `test_memory_pressure_auto_unload`: automatic model unloading upon receiving critical memory pressure events while preserving model under non-critical pressure.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 159 unit and integration tests (0 failures; 18 in `aissistant-llm`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 37 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 3 Phase 2 Subphase 2 (`S3.P2.2`)

### Added
- `core/crates/aissistant-llm/src/grammar/gbnf.rs`:
  - `GbnfRule` — Representation of production rules in GGML BNF grammar notation (`name ::= expression`).
  - `GbnfGrammar` — Production grammar builder with built-in JSON primitives (`ws`, `string`, `number`, `integer`, `boolean`, `null`, `json_array`, `json_object`, `json_value`, `json_member`), validation for `root` entrypoint and identifier syntax, and string literal escaping (`escape_literal`).
  - Pre-compiled static grammars for zero-overhead invocation: `arbitrary_json_object()`, `arbitrary_json_array()`, and `boolean_response()`.
- `core/crates/aissistant-llm/src/grammar/schema_compiler.rs`:
  - `SchemaCompiler` — Schema-to-GBNF compiler transforming standard JSON Schema definitions into strict context-free grammars.
  - Translation of primitive types, constrained `enum` values (`"opt1" | "opt2"`), arrays with element schemas, and objects with required and optional property rules.
  - `compile_tool` — Compiles a single tool definition into a constrained invocation schema: `{"name": "<tool_name>", "arguments": <schema>}`.
  - `compile_tools` — Multi-tool union compiler generating branching `root` expressions (`tool_a | tool_b | ...`) with optional conversational fallback for mixed-mode conversations.
- `core/crates/aissistant-llm/src/grammar/tool_call.rs`:
  - `ToolDefinition` — Strong specification for tools including name, description, and JSON Schema parameters.
  - `ToolCall` — Parsed model invocation request containing target tool name and validated arguments.
  - `ToolCallValidator` — Structural validator verifying that generated outputs are valid JSON, contain required arguments, match property types, conform to enum restrictions, and match known tool declarations.
- `core/crates/aissistant-llm/src/grammar/mod.rs`:
  - Public module exports for grammar AST, schema compiler, and tool-calling validation.
- `core/crates/aissistant-llm/src/lib.rs`:
  - Added 5 new unit and integration tests (bringing `aissistant-llm` total to 23, and workspace total to 164):
    - `test_gbnf_grammar_rule_formatting_and_validation`: rule formatting, missing root rejection, arbitrary JSON object and boolean grammars, and literal escaping.
    - `test_schema_compiler_primitive_and_enum_types`: string enum compilation with GBNF alternatives.
    - `test_schema_compiler_nested_object_and_array`: nested objects with required properties and array items.
    - `test_schema_compiler_single_and_multi_tool`: single tool schema compilation and multi-tool union with conversational fallback.
    - `test_tool_call_validator_success_and_failures`: successful invocation parsing, missing required arguments rejection, type mismatches, enum deviations, unknown tools, and malformed JSON.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 164 unit and integration tests (0 failures; 23 in `aissistant-llm`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 37 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
