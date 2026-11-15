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

---

## [2026-11-15] — Stage 3 Phase 2 Subphase 3 (`S3.P2.3`)

### Added
- `core/crates/aissistant-ffi/src/model_bridge.rs`:
  - `ModelSupervisor` — Runtime controller for local LLM model catalog queries, loading, unloading, and synthetic hardware benchmarking.
  - Data transfer structures: `ModelCatalogItem`, `HardwareOverview`, `ModelManagerSnapshot`, `BenchmarkReport`.
  - Automatic model unloading upon receiving critical OS memory pressure events.
- `core/crates/aissistant-ffi/src/api.rs`:
  - Bridge methods: `model_snapshot()`, `model_load()`, `model_unload()`, `model_benchmark_run()`.
  - Top-level C-ABI convenience functions: `bridge_model_snapshot()`, `bridge_model_load()`, `bridge_model_unload()`, `bridge_model_benchmark_run()`.
- `core/crates/aissistant-ffi/src/c_abi.rs`:
  - Exported C functions: `bridge_c_model_snapshot()`, `bridge_c_model_load()`, `bridge_c_model_unload()`, `bridge_c_model_benchmark_run()`.
- `core/crates/aissistant-ffi/src/events.rs`:
  - Events: `BridgeEvent::ModelLoaded`, `BridgeEvent::ModelUnloaded`, `BridgeEvent::BenchmarkCompleted`.
- `app/lib/core/models/model_item.dart`:
  - Dart representations: `ModelItem`, `HardwareOverview`, `ModelManagerSnapshot`, `BenchmarkResult`.
- `app/lib/core/models/bridge_event.dart`:
  - Dart event models: `ModelLoadedEvent`, `ModelUnloadedEvent`, `BenchmarkCompletedEvent`.
- `app/lib/core/ffi_bridge.dart`:
  - Added model methods to `AissistantBridge`, `NativeFfiBridge`, and `TestHarnessBridge`.
- `app/lib/core/assistant_controller.dart`:
  - State observables: `modelSnapshot`, `lastBenchmark`, `isBenchmarking`, `activeModelId`, `models`, `hardware`.
  - Methods: `refreshModelSnapshot()`, `loadModel()`, `unloadModel()`, `runBenchmark()`.
- `app/lib/ui/screens/models_screen.dart`:
  - Adaptive responsive screen featuring:
    - Host hardware headroom & SIMD instruction overview card with 100% offline badge.
    - Active model card with quick unload control and memory metrics.
    - Live benchmark runner card measuring TTFT, tokens/second throughput, elapsed ms, and memory footprint.
    - Curated model catalog with filter tabs (All, Recommended, Active), SHA-256 verified badges, parameter counts, and quantization details.
- `app/lib/ui/adaptive_shell.dart`:
  - Added "Models" destination to navigation shell (`Icons.memory`).
- `app/test/models_screen_test.dart`:
  - 8 new unit and widget tests covering hardware overview rendering, model catalog display, model load/unload interactions, live benchmark execution, filter tabs, and controller state management.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 167 unit and integration tests (0 failures; 24 in `aissistant-ffi`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 45 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 4 Phase 1 Subphase 1 (`S4.P1.1`)

### Added
- `core/crates/aissistant-storage/src/migration.rs`:
  - `Migration 3` — Multi-session conversation persistence, turn history, and SQLite FTS5 episodic search.
  - Relational tables: `conversations` and `conversation_turns` with foreign key cascade deletion and compound indices (`idx_turns_conversation`, `idx_turns_timestamp`).
  - Virtual full-text search table: `conversation_turns_fts` using SQLite FTS5 with `unicode61` tokenizer.
  - Automatic triggers: `trg_turns_ai` (after insert), `trg_turns_ad` (after delete), `trg_turns_au` (after update) keeping `conversation_turns_fts` 100% synchronized with `conversation_turns`.
- `core/crates/aissistant-storage/src/conversation_store.rs`:
  - `ConversationStore` — Engine managing conversation lifecycle, turn history, and FTS5 episodic search.
  - Domain models: `Conversation`, `ConversationMetadata`, `ConversationTurn`, `TurnRole`, `TurnMetadata`, `EpisodicSearchResult`, `ExportedConversation`.
  - Methods: `create_conversation()`, `get_conversation()`, `list_conversations()`, `update_conversation_title()`, `update_conversation_metadata()`, `delete_conversation()`.
  - Turn operations: `add_turn()`, `get_turn()`, `list_turns()`, `delete_turn()`.
  - Episodic search: `search_episodic_memory()` across all sessions and `search_episodic_memory_in_conversation()` scoped to single session with inverted BM25 scoring and `<mark>` snippet highlights.
  - Query sanitization: `sanitize_fts5_query()` splitting input into safe quoted prefix expressions (`"term"*`), stripping SQL/FTS syntax control tokens and preventing syntax error crashes.
  - Transparency & export: `export_conversation_json()` and `purge_all_conversations()` conforming to Requirement `FR-3.2`.
- `core/crates/aissistant-storage/src/error.rs`:
  - Added error variants: `ConversationNotFound`, `TurnNotFound`, `InvalidSearchQuery`.
  - Mapped into canonical `CoreError` taxonomy (`NotFound`, `ConfigValidation`).
- `core/crates/aissistant-storage/src/lib.rs`:
  - Exported `ConversationStore`, `Conversation`, `ConversationTurn`, `TurnRole`, `EpisodicSearchResult`, `sanitize_fts5_query`.
  - Added full conversation and episodic search flow to `test_storage_subsystem_end_to_end`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 175 unit and integration tests (0 failures; 24 in `aissistant-storage`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 45 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 4 Phase 1 Subphase 2 (`S4.P1.2`)

### Added
- `core/crates/aissistant-llm/src/embeddings.rs`:
  - `EmbeddingEngine` — Trait defining sync/batch dense embedding extraction from text.
  - Vector math functions: `l2_norm()`, `l2_normalize()`, `cosine_similarity()` with zero-norm and dimension-mismatch guards.
  - `LocalEmbeddingEngine` — Deterministic local embedding engine producing normalized 384-dimensional dense vectors using subword n-gram hashing and inverse position weighting.
- `core/crates/aissistant-storage/src/migration.rs`:
  - `Migration 4` — Dense vector embeddings store (`semantic_embeddings`) with entity and conversation indexes.
  - Cascading cleanup triggers: `trg_turns_del_embeddings` (on `conversation_turns` delete) and `trg_conv_del_embeddings` (on `conversations` delete).
- `core/crates/aissistant-storage/src/vector_store.rs`:
  - `VectorMemoryStore` — SQLite persistence and cosine similarity k-NN search engine.
  - Models: `VectorRecord`, `VectorSearchResult`.
  - Serializers: IEEE 754 little-endian binary float conversion (`vec_to_blob`, `blob_to_vec`).
  - Methods: `store_embedding()`, `get_embedding()`, `delete_embedding()`, `delete_embeddings_for_entity()`, `list_embeddings_for_conversation()`, `search_vector_similarity()`.
- `core/crates/aissistant-storage/src/hybrid_search.rs`:
  - `HybridSearchEngine` — Reciprocal Rank Fusion (RRF) combining SQLite FTS5 BM25 episodic keyword ranking with dense vector cosine similarity.
  - Hyperparameter model `RrfConfig` with smoothing constant $k$ (default 60.0) and modality weights.
  - `HybridSearchResult` conveying combined RRF score, individual modality ranks (`bm25_rank`, `vector_rank`), raw cosine similarity, and highlighted text snippets.
- `core/crates/aissistant-storage/src/lib.rs`:
  - Exported `VectorMemoryStore`, `VectorRecord`, `VectorSearchResult`, `HybridSearchEngine`, `HybridSearchResult`, `RrfConfig`, `vec_to_blob`, `blob_to_vec`, `cosine_similarity`.
  - Integrated vector storage and hybrid search into end-to-end integration test.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 185 unit and integration tests (0 failures; 29 in `aissistant-storage`, 28 in `aissistant-llm`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 45 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 4 Phase 2 Subphase 1 (`S4.P2.1`)

### Added
- `core/crates/aissistant-orchestrator`:
  - Created new workspace crate `aissistant-orchestrator` registered in `core/Cargo.toml`.
  - `state.rs`: Defined `TurnState` enum (`Idle`, `Listening`, `Transcribing`, `UtteranceReceived`, `RetrievingMemory`, `PromptAssembly`, `Inferring`, `ToolInvocationDetected`, `AwaitingToolExecution`, `SynthesizingSpeech`, `TurnCompleted`, `Interrupted`, `Failed`) with valid transition matrix (`can_transition_to`), `TurnInput`, `InputMode`, and `OrchestratorEvent`.
  - `turn_machine.rs`: Thread-safe `TurnStateMachine` with atomic transition validation, `TurnMetrics` diagnostic logging, and cooperative cancellation via `LlmCancellationHandle`.
  - `orchestrator.rs`: Full `Orchestrator` coordinating text/voice turn processing:
    1. Utterance ingestion and persistence in SQLite.
    2. Hybrid RRF memory retrieval (FTS5 BM25 + dense vector cosine similarity).
    3. Dynamic prompt assembly with memory injection and ChatML formatting.
    4. Token-by-token streaming from `LlmEngine` with TTFT tracking.
    5. Streaming sentence boundary chunking with `SentenceChunker` and synthesis via `TtsEngine`.
    6. Assistant turn persistence in SQLite and vector embedding storage.
    7. Reactive event listener subscription without deadlock via lock-free snapshot broadcasting.
    8. Sub-20ms cooperative barge-in cancellation and partial turn recording.
  - `error.rs`: Domain `OrchestratorError` mapped into canonical `CoreError` taxonomy.
  - `lib.rs`: Public re-exports and integration tests.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 8 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 196 unit and integration tests (0 failures; 11 in `aissistant-orchestrator`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 45 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 4 Phase 2 Subphase 2 (`S4.P2.2`)

### Added
- `core/crates/aissistant-orchestrator/src/tool_dispatcher.rs`:
  - `ToolHandler` trait defining `name()`, `description()`, `parameters_schema()`, `risk_tier()`, and `execute()`.
  - `redact_sensitive_json()` recursing through parameters and masking sensitive keys (`password`, `secret`, `token`, `key`, `credential`, `auth`) with `[REDACTED]`.
  - `ToolDispatcher` managing tool registration with `SkillId` namespacing validation, definitions enumeration, schema argument verification via `ToolCallValidator`, and execution.
- `core/crates/aissistant-orchestrator/src/confirmation.rs`:
  - `ConfirmationId` strongly-typed identifier for user authorization gates.
  - `PendingConfirmation` encapsulating conversation, turn, tool name, arguments, risk tier, and human-readable reason.
  - `ConfirmationManager` thread-safe manager storing, retrieving, and cancelling active pending confirmations.
- `core/crates/aissistant-orchestrator/src/state.rs`:
  - Extended `TurnState::ToolInvocationDetected` transition table to allow completing turns on security policy denial.
  - Added orchestrator events: `ConfirmationRequired`, `ConfirmationResolved`, `ToolExecutionStarted`, and `ToolExecutionCompleted`.
- `core/crates/aissistant-orchestrator/src/error.rs`:
  - Added errors: `ToolNotFound`, `ToolExecutionFailed`, `ConfirmationNotFound`, `ToolPolicyDenied`, and `InvalidToolArguments` mapped into canonical `CoreError` taxonomy.
- `core/crates/aissistant-orchestrator/src/orchestrator.rs`:
  - Integrated `ToolDispatcher`, `ConfirmationManager`, `PolicyEngine`, `AuditStore`, and `TaintTracker`.
  - Implemented `execute_turn()` supporting `TurnExecution::Completed(TurnSummary)` and `TurnExecution::AwaitingConfirmation(PendingConfirmation)`.
  - Automatic tool execution for `RiskTier::ReadOnly` and `RiskTier::StateModifying` (clean context).
  - Non-blocking turn pausing with `ConfirmationRequired` event on `RiskTier::FileAndApp`, `PrivilegedSystem`, or `Destructive` actions.
  - Taint escalation rule: conversation context with untrusted input automatically escalates state-modifying actions to require confirmation.
  - Implemented `resolve_confirmation(id, approved)` resuming execution upon approval or recording user cancellation without side effects.
  - Cryptographic BLAKE3 audit chain sealing: all tool requests, authorization decisions, user confirmations, and execution statuses are committed to `security_audit_log`.
- `core/crates/aissistant-llm/src/grammar/schema_compiler.rs`:
  - Replaced collapsible replace with `tool.name.replace(['-', '.'], "_")` to support namespaced tool names in GBNF rule identifiers.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 8 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 206 unit and integration tests (0 failures; 21 in `aissistant-orchestrator`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 45 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 5 Phase 1 Subphase 1 (`S5.P1.1`)

### Added
- `core/Cargo.toml`:
  - Added `"crates/aissistant-skills"` to workspace `members`.
- `core/crates/aissistant-skills/Cargo.toml`:
  - Defined crate metadata and dependencies on `aissistant-core`, `aissistant-orchestrator`, `aissistant-security`, `serde`, `serde_json`, `sysinfo`, `thiserror`, and `tracing`.
- `core/crates/aissistant-skills/src/error.rs`:
  - Defined domain `SkillError` taxonomy: `ExecutionTimeout`, `InvalidArguments`, `EvaluationError`, `IncompatibleUnits`, `UnknownUnit`, `Core`, `Orchestrator`, `Serialization`, and `Internal`.
  - Implemented bidirectional conversions to `CoreError` and `OrchestratorError`.
- `core/crates/aissistant-skills/src/sandbox.rs`:
  - Implemented `SandboxedSkill` wrapper providing thread isolation, panic trapping, and execution timeout enforcement (default 5,000ms).
- `core/crates/aissistant-skills/src/utility/time_skill.rs`:
  - Implemented `TimeSkill` (`system.time`) using Howard Hinnant's civil calendar algorithm for zero-dependency date/time queries across `full`, `date_only`, `time_only`, and `epoch_ms` formats (`RiskTier::ReadOnly`).
- `core/crates/aissistant-skills/src/utility/timer_skill.rs`:
  - Implemented thread-safe `TimerSkill` (`system.timer`) managing countdown timers (`set`, `status`, `cancel`, `list`) with label metadata (`RiskTier::StateModifying`).
- `core/crates/aissistant-skills/src/utility/calc_skill.rs`:
  - Implemented safe recursive-descent math parser `CalcSkill` (`system.calc`) supporting arithmetic operators (`+`, `-`, `*`, `/`, `%`), exponentiation (`^`), parentheses, functions (`sqrt`, `abs`, `round`, `floor`, `ceil`, `sin`, `cos`, `tan`, `ln`, `log10`), constants (`pi`, `e`), and strict division-by-zero checks without arbitrary code execution (`RiskTier::ReadOnly`).
- `core/crates/aissistant-skills/src/utility/convert_skill.rs`:
  - Implemented `ConvertSkill` (`system.convert`) supporting Temperature, Length, Mass, Digital Data, and Time unit conversions with dimensional validation and rejection of incompatible conversions (`RiskTier::ReadOnly`).
- `core/crates/aissistant-skills/src/utility/telemetry_skill.rs`:
  - Implemented `TelemetrySkill` (`system.telemetry`) querying live OS metrics, RAM headroom, and CPU utilization via `sysinfo::System` with 200ms refresh throttling (`RiskTier::ReadOnly`).
- `core/crates/aissistant-skills/src/registry.rs`:
  - Implemented `SkillRegistry` creating and registering all built-in utility skills wrapped in `SandboxedSkill` into `ToolDispatcher`.
- `core/crates/aissistant-skills/src/lib.rs`:
  - Clean crate re-exports and integration tests verifying registration and dispatch of all 5 built-in utility skills through `ToolDispatcher`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 9 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 226 unit and integration tests (0 failures; 20 in `aissistant-skills`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 45 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 5 Phase 1 Subphase 2 (`S5.P1.2`)

### Added
- `core/crates/aissistant-skills/Cargo.toml`:
  - Added `zip` v7.4.0 (pure-Rust deflate/zlib-rs), `blake3`, and `sha2` dependencies.
- `core/crates/aissistant-skills/src/error.rs`:
  - Added `PathEscape`, `SecurityViolation`, `Io`, and `Zip` error variants with conversions into `CoreError` and `OrchestratorError`.
- `core/crates/aissistant-skills/src/file/jail.rs`:
  - Implemented `PathJail` providing strict path canonicalization, allowed root confinement, symlink escape detection, zip-slip defense, null byte rejection, and system path denylists (`Windows/System32`, `Program Files/WindowsApps`, `/etc`, `/sys`, `/proc`, `/dev`, `/boot`).
  - Added rejection of Windows reserved DOS device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`).
- `core/crates/aissistant-skills/src/file/search_skill.rs`:
  - Implemented `FileSearchSkill` (`file.search`, `RiskTier::ReadOnly`) supporting recursive directory walking, substring/extension matching, max results limits, and hidden file filters.
- `core/crates/aissistant-skills/src/file/read_skill.rs`:
  - Implemented `FileReadSkill` (`file.read`, `RiskTier::ReadOnly`) supporting bounded byte buffer reads (up to 1 MB cap), pagination offsets, EOF detection, and UTF-8/Hex encodings.
- `core/crates/aissistant-skills/src/file/write_skill.rs`:
  - Implemented `FileWriteSkill` (`file.write`, `RiskTier::FileAndApp`) supporting atomic write, append, and create-new modes with parent directory validation.
- `core/crates/aissistant-skills/src/file/move_skill.rs`:
  - Implemented `FileMoveSkill` (`file.move`, `RiskTier::FileAndApp`) supporting safe file and folder moving and renaming with destination collision checks.
- `core/crates/aissistant-skills/src/file/delete_skill.rs`:
  - Implemented `FileDeleteSkill` (`file.delete`, `RiskTier::Destructive`) supporting permanent deletion of files and recursive removal of non-empty folders.
- `core/crates/aissistant-skills/src/file/archive_skill.rs`:
  - Implemented `FileArchiveSkill` (`file.archive`, `RiskTier::FileAndApp`) supporting pure-Rust ZIP archive creation, entry listing, and safe extraction with zip-slip traversal protection.
- `core/crates/aissistant-skills/src/file/checksum_skill.rs`:
  - Implemented `FileChecksumSkill` (`file.checksum`, `RiskTier::ReadOnly`) supporting streaming BLAKE3 and SHA-256 cryptographic digest calculation.
- `core/crates/aissistant-skills/src/file/mod.rs`:
  - Re-exported all file skills and `PathJail`.
- `core/crates/aissistant-skills/src/registry.rs`:
  - Added `file_skills()`, `register_all_file_skills()`, and `register_all()` methods to `SkillRegistry` registering all 12 utility and file management skills into `ToolDispatcher`.
- `core/crates/aissistant-skills/src/lib.rs`:
  - Re-exported file module and added comprehensive integration tests verifying dispatch of all file skills through `ToolDispatcher`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 9 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 239 unit and integration tests (0 failures; 33 in `aissistant-skills`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 45 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 5 Phase 2 Subphase 1 (`S5.P2.1`)

### Added
- `core/Cargo.toml`:
  - Added `"crates/aissistant-platform"` to workspace `members`.
- `core/crates/aissistant-platform/Cargo.toml`:
  - Created crate with dependencies on `aissistant-core`, `serde`, `serde_json`, `sysinfo`, `thiserror`, and `tracing`.
- `core/crates/aissistant-platform/src/error.rs`:
  - Defined `PlatformError` taxonomy: `ProcessNotFound`, `CriticalProcessProtected`, `WindowNotFound`, `LaunchFailed`, `CommandInjectionRejected`, `UnsupportedPlatform`, `Io`, and `Internal`.
- `core/crates/aissistant-platform/src/types.rs`:
  - Defined domain structures: `AppInfo`, `ProcessInfo`, `WindowBounds`, and `WindowInfo`.
- `core/crates/aissistant-platform/src/adapter.rs`:
  - Defined `DesktopPlatformAdapter` trait.
  - Implemented `VirtualDesktopAdapter` for hermetic testing and headless runner verification.
- `core/crates/aissistant-platform/src/windows.rs`:
  - Implemented `WindowsDesktopAdapter` utilizing Win32 FFI (`EnumWindows`, `GetWindowTextW`, `IsWindowVisible`, `ShowWindow`, `SetForegroundWindow`), `sysinfo::System`, and Start Menu scanning with critical process termination protection (`smss.exe`, `csrss.exe`, `wininit.exe`, `services.exe`, `lsass.exe`, `svchost.exe`, `explorer.exe`, `dwm.exe`).
- `core/crates/aissistant-platform/src/linux.rs`:
  - Implemented `LinuxDesktopAdapter` parsing XDG `.desktop` files, process management via `sysinfo`, and PID 1 protection.
- `core/crates/aissistant-platform/src/lib.rs`:
  - Re-exported platform types and provided `create_default_desktop_adapter()`.
- `core/crates/aissistant-skills/src/desktop/`:
  - `app_skills.rs`: Implemented `AppListSkill` (`app.list`, `RiskTier::ReadOnly`) and `AppLaunchSkill` (`app.launch`, `RiskTier::FileAndApp`).
  - `process_skills.rs`: Implemented `ProcessListSkill` (`process.list`, `RiskTier::ReadOnly`) and `ProcessKillSkill` (`process.kill`, `RiskTier::Destructive`).
  - `window_skills.rs`: Implemented `WindowListSkill` (`window.list`, `RiskTier::ReadOnly`) and `WindowFocusSkill` (`window.focus`, `RiskTier::StateModifying`).
  - `mod.rs`: Module exports.
- `core/crates/aissistant-skills/src/registry.rs`:
  - Added `desktop_skills()`, `register_all_desktop_skills()`, and updated `register_all()` to register all 18 tools (5 built-in + 7 file + 6 desktop).
- `core/crates/aissistant-skills/src/lib.rs`:
  - Integration tests for all desktop automation tools through `ToolDispatcher`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 10 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 248 unit and integration tests (0 failures; 7 in `aissistant-platform`, 35 in `aissistant-skills`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 45 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 5 Phase 2 Subphase 2 (`S5.P2.2`)

### Added
- `core/crates/aissistant-platform/src/types.rs`:
  - Added domain structures: `VolumeStatus`, `MediaCommand` (`Play`, `Pause`, `PlayPause`, `Next`, `Previous`, `Stop`), and `BatteryStatus`.
- `core/crates/aissistant-platform/src/adapter.rs`:
  - Extended `DesktopPlatformAdapter` trait with `read_clipboard()`, `write_clipboard()`, `get_master_volume()`, `set_master_volume()`, `media_transport()`, and `get_battery_status()`.
  - Implemented all 6 methods in `VirtualDesktopAdapter` with unit tests.
- `core/crates/aissistant-platform/src/windows.rs`:
  - Implemented Win32 clipboard read/write via `OpenClipboard`, `GetClipboardData` (`CF_UNICODETEXT`), `SetClipboardData`, `GlobalAlloc`, `GlobalLock`, and thread-safe fallback cache for headless / session 0 runners.
  - Implemented master volume control and mute toggling via `keybd_event` (`VK_VOLUME_MUTE`).
  - Implemented media transport control via `keybd_event` (`VK_MEDIA_PLAY_PAUSE`, `VK_MEDIA_NEXT_TRACK`, `VK_MEDIA_PREV_TRACK`, `VK_MEDIA_STOP`).
  - Implemented battery charge and AC power status query via `GetSystemPowerStatus`.
- `core/crates/aissistant-platform/src/linux.rs`:
  - Implemented clipboard read/write, master volume control, media transport, and Linux `/sys/class/power_supply` querying.
- `core/crates/aissistant-platform/src/lib.rs`:
  - Re-exported `BatteryStatus`, `MediaCommand`, and `VolumeStatus`.
- `core/crates/aissistant-skills/src/desktop/`:
  - `clipboard_skills.rs`: Implemented `ClipboardReadSkill` (`clipboard.read`, `RiskTier::ReadOnly`) and `ClipboardWriteSkill` (`clipboard.write`, `RiskTier::StateModifying`).
  - `media_skills.rs`: Implemented `VolumeGetSkill` (`volume.get`, `RiskTier::ReadOnly`), `VolumeSetSkill` (`volume.set`, `RiskTier::StateModifying`), and `MediaTransportSkill` (`media.transport`, `RiskTier::StateModifying`).
  - `system_skills.rs`: Implemented `SystemBatterySkill` (`system.battery`, `RiskTier::ReadOnly`).
  - `mod.rs`: Re-exported all new skill modules and structs.
- `core/crates/aissistant-skills/src/registry.rs`:
  - Extended `desktop_skills()` to include all 12 desktop tools.
  - Updated `register_all()` to register all 24 tools (5 built-in + 7 file + 12 desktop).
- `core/crates/aissistant-skills/src/lib.rs`:
  - Re-exported new desktop skills and added comprehensive integration tests for clipboard, volume, media transport, and battery tools through `ToolDispatcher`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 10 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 270 unit and integration tests (0 failures; 11 in `aissistant-platform`, 39 in `aissistant-skills`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 45 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 5 Phase 2 Subphase 2 (`S5.P2.2`)

### Added
- `core/crates/aissistant-platform/src/types.rs`:
  - Added domain structures: `VolumeStatus`, `MediaCommand` (`Play`, `Pause`, `PlayPause`, `Next`, `Previous`, `Stop`), and `BatteryStatus`.
- `core/crates/aissistant-platform/src/adapter.rs`:
  - Extended `DesktopPlatformAdapter` trait with `read_clipboard()`, `write_clipboard()`, `get_master_volume()`, `set_master_volume()`, `media_transport()`, and `get_battery_status()`.
  - Implemented all 6 methods in `VirtualDesktopAdapter` with unit tests.
- `core/crates/aissistant-platform/src/windows.rs`:
  - Implemented Win32 clipboard read/write via `OpenClipboard`, `GetClipboardData` (`CF_UNICODETEXT`), `SetClipboardData`, `GlobalAlloc`, `GlobalLock`, and thread-safe fallback cache for headless / session 0 runners.
  - Implemented master volume control and mute toggling via `keybd_event` (`VK_VOLUME_MUTE`).
  - Implemented media transport control via `keybd_event` (`VK_MEDIA_PLAY_PAUSE`, `VK_MEDIA_NEXT_TRACK`, `VK_MEDIA_PREV_TRACK`, `VK_MEDIA_STOP`).
  - Implemented battery charge and AC power status query via `GetSystemPowerStatus`.
- `core/crates/aissistant-platform/src/linux.rs`:
  - Implemented clipboard read/write, master volume control, media transport, and Linux `/sys/class/power_supply` querying.
- `core/crates/aissistant-platform/src/lib.rs`:
  - Re-exported `BatteryStatus`, `MediaCommand`, and `VolumeStatus`.
- `core/crates/aissistant-skills/src/desktop/`:
  - `clipboard_skills.rs`: Implemented `ClipboardReadSkill` (`clipboard.read`, `RiskTier::ReadOnly`) and `ClipboardWriteSkill` (`clipboard.write`, `RiskTier::StateModifying`).
  - `media_skills.rs`: Implemented `VolumeGetSkill` (`volume.get`, `RiskTier::ReadOnly`), `VolumeSetSkill` (`volume.set`, `RiskTier::StateModifying`), and `MediaTransportSkill` (`media.transport`, `RiskTier::StateModifying`).
  - `system_skills.rs`: Implemented `SystemBatterySkill` (`system.battery`, `RiskTier::ReadOnly`).
  - `mod.rs`: Re-exported all new skill modules and structs.
- `core/crates/aissistant-skills/src/registry.rs`:
  - Extended `desktop_skills()` to include all 12 desktop tools.
  - Updated `register_all()` to register all 24 tools (5 built-in + 7 file + 12 desktop).
- `core/crates/aissistant-skills/src/lib.rs`:
  - Re-exported new desktop skills and added comprehensive integration tests for clipboard, volume, media transport, and battery tools through `ToolDispatcher`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 10 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 270 unit and integration tests (0 failures; 11 in `aissistant-platform`, 39 in `aissistant-skills`).
- `cd app; C:\flutter\bin\flutter.bat test` passed: 45 unit and widget tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.

---

## [2026-11-15] — Stage 5 Phase 3 Subphase 1 (`S5.P3.1`)

### Added
- `app/android/app/src/main/res/values/strings.xml`:
  - Added string resources for foreground voice service channel, notification titles, stop actions, and accessibility privacy disclosure.
- `app/android/app/src/main/res/xml/accessibility_service_config.xml`:
  - Declared Android `accessibility-service` XML configuration with event types, interactive window retrieval, and on-device privacy guarantee.
- `app/android/app/src/main/AndroidManifest.xml`:
  - Added permissions: `WAKE_LOCK` and `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`.
  - Declared `AssistantForegroundService` with `android:foregroundServiceType="microphone"`.
  - Declared `AssistantAccessibilityService` with `android.permission.BIND_ACCESSIBILITY_SERVICE`.
  - Added intent filters to `MainActivity` for `android.intent.action.ASSIST` (voice assistant invocation) and `android.intent.action.SEND` (text sharing).
- `app/android/app/src/main/kotlin/com/aissistant/aissistant/`:
  - `AssistantForegroundService.kt`: Implemented Android foreground service managing notification channel, ongoing notification with stop action, wake lock acquisition, and microphone foreground service type.
  - `MediaSessionManager.kt`: Implemented `MediaSession` and `AudioFocusRequest` manager handling hardware/headset transport controls and ducking/pauses during phone calls or alarms.
  - `AssistantAccessibilityService.kt`: Implemented opt-in accessibility service enabling offline node hierarchy querying and user-authorized action clicking.
  - `MainActivity.kt`: Configured `MethodChannel("com.aissistant/mobile_bridge")`, wired service start/stop, battery optimization checks, accessibility settings intent, and intent payload extraction (`ACTION_ASSIST`, `ACTION_SEND`).
- `app/lib/core/mobile_bridge.dart`:
  - Implemented `MobilePlatformBridge` service providing reactive Dart APIs over the MethodChannel, including `onIntentReceived`, `onMediaCommand`, and `onAudioFocusLost` broadcast streams, with graceful fallback for non-Android platforms.
- `app/test/mobile_bridge_test.dart`:
  - Added unit test suite covering MethodChannel invocations, simulated incoming native events, error handling, and platform exception resilience.

### Verification
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 57 unit and widget tests (0 failures; 12 in `mobile_bridge_test.dart`).
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 10 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 270 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 5 Phase 3 Subphase 2 (`S5.P3.2`)

### Added
- `app/ios/Runner/Info.plist`:
  - Declared custom URL scheme `aissistant` (`CFBundleURLTypes`) for external invocation and Siri Shortcuts.
  - Declared `NSUserActivityTypes` (`com.aissistant.askAssistant`).
- `app/ios/Runner/AudioSessionManager.swift`:
  - Implemented `AVAudioSession` lifecycle manager setting `.playAndRecord` category, `.spokenAudio` mode, and `[.duckOthers, .defaultToSpeaker, .allowBluetooth, .allowBluetoothA2DP]` options.
  - Handled `AVAudioSession.interruptionNotification` (began/ended/shouldResume) and `AVAudioSession.routeChangeNotification` (headset transition).
- `app/ios/Runner/AppDelegate.swift`:
  - Configured MethodChannel `com.aissistant/ios_bridge` for Flutter communication.
  - Implemented `configureAudioSession`, `activateAudioSession`, and `isOtherAudioPlaying`.
  - Implemented security-scoped bookmark access (`startAccessingSecurityScopedResource` and `stopAccessingSecurityScopedResource`).
  - Overrode `application(_:open:options:)` and `application(_:continue:restorationHandler:)` to route custom URL schemes and user activities.
- `app/lib/core/ios_bridge.dart`:
  - Created `IosPlatformBridge` reactive Dart service with `onShortcutTriggered`, `onAudioInterruption`, and `onAudioRouteChange` streams, with graceful fallback for non-iOS platforms.
- `app/test/ios_bridge_test.dart`:
  - Created unit tests verifying audio session activation/configuration, security-scoped resource access, shortcut invocation parsing, and interruption event broadcasting.

### Verification
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 64 unit and widget tests (0 failures; 7 in `ios_bridge_test.dart`).
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 10 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 270 unit and integration tests (0 failures).

---

## [2026-11-15] — Stage 6 Phase 1 Subphase 1 (`S6.P1.1`)

### Added
- `core/crates/aissistant-core/src/device.rs`:
  - Defined domain models: `DeviceTrustState` (`Trusted`, `Blocked`, `PendingVerification`, `Revoked`), `PeerPermissionLevel` (`AlwaysAllow`, `AskEachTime`, `Deny`), `PairedDevice`, and `PeerPermission`.
- `core/crates/aissistant-core/src/lib.rs`:
  - Re-exported device domain models.
- `core/crates/aissistant-security/src/lib.rs`:
  - Re-exported `NewAuditEntry` struct for audit logging.
- `core/crates/aissistant-storage/src/migration.rs`:
  - Added `Migration 5` schema: created `paired_devices` and `peer_permissions` tables with foreign key cascade, indexes on `trust_state` and `last_seen_ms`.
  - Updated schema migration runner tests asserting 10 total managed tables.
- `core/crates/aissistant-storage/src/device_registry.rs`:
  - Implemented `DeviceRegistry` store with device registration, retrieval, listing, heartbeat/endpoint updating, trust state transitions, and permanent deletion.
  - Implemented granular capability ACL rule persistence and evaluation (`check_peer_permission`) enforcing default-deny semantics for unconfigured or blocked peers.
  - Integrated tamper-evident BLAKE3 hash-chained audit logging for all device registration, trust transition, and permission grant mutations.
- `core/crates/aissistant-storage/src/lib.rs`:
  - Re-exported `DeviceRegistry`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 10 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 276 unit and integration tests (0 failures; 34 in `aissistant-core`, 32 in `aissistant-storage`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 64 unit and widget tests (0 failures).

---

## [2026-11-15] — Stage 6 Phase 1 Subphase 2 (`S6.P1.2`)

### Added
- `core/crates/aissistant-network/`:
  - Created new workspace crate registered in `core/Cargo.toml` as 11th workspace crate.
  - `src/error.rs`: Defined `NetworkError` taxonomy covering `SocketBindFailed`, `MulticastJoinFailed`, `PacketMalformed`, and I/O error variants.
  - `src/dns.rs`: Implemented binary wire-format parser and builder conforming to RFC 1035, RFC 6762 (mDNS), and RFC 6763 (DNS-SD) supporting Question and Resource Record types (PTR, SRV, TXT, A, AAAA), length-prefixed domain label encoding, and pointer compression.
  - `src/types.rs`: Defined domain discovery models: `ServiceConfig`, `DiscoveredPeer`, `DiscoveryEvent` (`PeerDiscovered`, `PeerUpdated`, `PeerLost`), and constants (`MDNS_SERVICE_TYPE = "_aissistant._udp.local"`, `MDNS_PORT = 5353`, `DEFAULT_QUIC_PORT = 51820`).
  - `src/discovery.rs`:
    - Implemented `DiscoveryEngine` core trait (`start_advertising`, `stop_advertising`, `is_advertising`, `active_peers`, `send_query`, `poll_events`).
    - Implemented `VirtualDiscoveryEngine` with in-memory lock-protected peer table for deterministic CI/sandbox simulation, peer announcement injection, update detection, and TTL expiration pruning.
    - Implemented `MdnsDiscoveryEngine` with live dual-stack UDP socket binding, multicast group join (`224.0.0.251:5353`), non-blocking packet reception, authoritative announcement packet generation, and privacy-preserving metadata handling (strictly broadcasting truncated 8-byte SHA-256 fingerprint, platform, and protocol version without personal identifiers).
  - `src/lib.rs`: Re-exported public API and added multi-node virtual discovery mesh integration tests.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 11 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 285 unit and integration tests (0 failures; 9 in `aissistant-network`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 64 unit and widget tests (0 failures).

---

## [2026-11-15] — Stage 6 Phase 2 Subphase 1 (`S6.P2.1`)

### Added
- `core/Cargo.toml` & `core/crates/aissistant-network/Cargo.toml`:
  - Added workspace dependencies: `quinn = "0.11"`, `rustls = "0.23"`, `rustls-pki-types = "1.10"`, `rcgen = "0.13"`, `tokio = "1.43"`, and `ed25519-dalek = "2.1"`.
- `core/crates/aissistant-network/src/error.rs`:
  - Extended `NetworkError` with `Tls`, `Quic`, `PeerAuthenticationFailed`, and `ConnectionClosed` variants.
- `core/crates/aissistant-network/src/cert.rs`:
  - Implemented PKCS#8 DER encoder (`ed25519_secret_to_pkcs8`) for 32-byte Ed25519 secret keys.
  - Implemented RFC 8410 SPKI public key extraction (`extract_ed25519_public_key`) from X.509 DER certificates.
  - Implemented self-signed TLS 1.3 certificate generation (`generate_tls_cert`) from `DeviceIdentity` via `rcgen`.
  - Implemented `PinnedEd25519Verifier` implementing both `rustls::client::danger::ServerCertVerifier` and `rustls::server::danger::ClientCertVerifier`, strictly enforcing TLS 1.3, Ed25519 signatures, and pinning against `DeviceRegistry` public keys (with opt-in unpinned pairing mode).
- `core/crates/aissistant-network/src/transport.rs`:
  - Defined protocol constants (`ALPN_P2P_PROTOCOL = b"aissistant-p2p/1"`, `DEFAULT_KEEPALIVE_INTERVAL = 5s`, `DEFAULT_IDLE_TIMEOUT = 30s`).
  - Implemented `build_quic_server_config` and `build_quic_client_config` using Ring crypto provider, keep-alive heartbeats, and pinned mTLS verifiers.
  - Implemented `QuicSession` wrapping `quinn::Connection`, extracting verified peer public key, and providing bi-directional stream multiplexing (`open_bi`, `accept_bi`) and datagrams.
  - Implemented `QuicTransportEngine` for endpoint lifecycle management (`start_endpoint`, `connect_peer`, `accept`, `close_all`).
  - Implemented `VirtualSession` and `create_virtual_session_pair` for deterministic in-memory session simulation without UDP network sockets.
- `core/crates/aissistant-network/src/lib.rs`:
  - Re-exported transport and certificate modules and public types.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 11 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 292 unit and integration tests (0 failures; 16 in `aissistant-network`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 64 unit and widget tests (0 failures).

---

## [2026-11-15] — Subphase `S6.P2.2`: Out-of-Band SAS / QR Code Pairing Ceremony & Device Management UI

### Added
- `core/crates/aissistant-network/src/pairing.rs`:
  - Implemented symmetrical 6-digit decimal Short Authentication String (SAS) derivation (`derive_sas_code`) using canonical key sorting (`min(key_a, key_b), max(key_a, key_b)`) and SHA-256.
  - Implemented compact QR payload serializer/deserializer (`PairingQrPayload`).
  - Implemented brute-force rate limiter (`PairingRateLimiter`) enforcing 300s (5-minute) lockout after 3 consecutive failed verification attempts.
  - Implemented `PairingSessionManager` tracking active session state, SAS verification, and automatic `DeviceRegistry` enrollment.
- `core/crates/aissistant-ffi/src/device_bridge.rs`:
  - Implemented `DeviceSupervisor` coordinating discovered peer cache, virtual announcement simulation, pairing ceremony lifecycle, and granular ACL grants.
  - Exposed serializable DTOs: `DiscoveredPeerItem`, `PeerPermissionItem`, `PairedDeviceItem`, `PairingSessionInfo`, `PairingStateSnapshot`.
- `core/crates/aissistant-ffi/src/events.rs`:
  - Added `BridgeEvent::DeviceDiscovered` and `BridgeEvent::PairingStateChanged`.
- `core/crates/aissistant-ffi/src/c_abi.rs`:
  - Exported safe C-ABI functions: `bridge_c_device_discovered_peers`, `bridge_c_device_paired_list`, `bridge_c_device_start_pairing`, `bridge_c_device_confirm_pairing`, `bridge_c_device_cancel_pairing`, `bridge_c_device_set_permission`, `bridge_c_device_unpair`, `bridge_c_device_simulate_peer`.
- `app/lib/core/models/device_models.dart`:
  - Created Dart models with JSON serialization: `DiscoveredPeerItem`, `PeerPermissionItem`, `PairedDeviceItem`, `PairingSessionInfo`.
- `app/lib/ui/screens/devices_screen.dart`:
  - Built full `DevicesScreen` with responsive UI: 100% offline LAN indicator, discovered peer list, active pairing ceremony card with large monospace 6-digit SAS code, QR code visual container, SAS confirmation input, paired devices list with trust badges, and interactive capability ACL dropdowns (`always_allow`, `ask_each_time`, `deny`).
- `app/test/devices_screen_test.dart`:
  - Comprehensive Flutter widget tests for pairing initiation, SAS confirmation, cancellation, unpairing confirmation dialog, and peer simulation.

### Modified
- `core/crates/aissistant-network/Cargo.toml` & `src/lib.rs`: Exposed pairing ceremony module.
- `core/crates/aissistant-ffi/Cargo.toml` & `src/lib.rs`: Wired device supervisor and C-ABI exports.
- `app/lib/core/ffi_bridge.dart`: Added device methods and C-ABI lookup to `NativeFfiBridge` and `TestHarnessBridge`.
- `app/lib/core/assistant_controller.dart`: Added reactive device state management, bridge event listeners, and pairing actions.
- `app/lib/ui/adaptive_shell.dart`: Wired `Devices` destination into desktop NavigationRail and mobile NavigationBar.
- `app/test/assistant_controller_test.dart` & `app/test/widget_test.dart`: Updated test suites for new device capabilities and navigation.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 292 unit and integration tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 72 unit and widget tests (0 failures).

---

## [2026-11-15] — Stage 6 Phase 1 Subphase 2 (`S6.P1.2`)

### Added
- `core/crates/aissistant-network/`:
  - Created new workspace crate registered in `core/Cargo.toml` as 11th workspace crate.
  - `src/error.rs`: Defined `NetworkError` taxonomy covering `SocketBindFailed`, `MulticastJoinFailed`, `PacketMalformed`, and I/O error variants.
  - `src/dns.rs`: Implemented binary wire-format parser and builder conforming to RFC 1035, RFC 6762 (mDNS), and RFC 6763 (DNS-SD) supporting Question and Resource Record types (PTR, SRV, TXT, A, AAAA), length-prefixed domain label encoding, and pointer compression.
  - `src/types.rs`: Defined domain discovery models: `ServiceConfig`, `DiscoveredPeer`, `DiscoveryEvent` (`PeerDiscovered`, `PeerUpdated`, `PeerLost`), and constants (`MDNS_SERVICE_TYPE = "_aissistant._udp.local"`, `MDNS_PORT = 5353`, `DEFAULT_QUIC_PORT = 51820`).
  - `src/discovery.rs`:
    - Implemented `DiscoveryEngine` core trait (`start_advertising`, `stop_advertising`, `is_advertising`, `active_peers`, `send_query`, `poll_events`).
    - Implemented `VirtualDiscoveryEngine` with in-memory lock-protected peer table for deterministic CI/sandbox simulation, peer announcement injection, update detection, and TTL expiration pruning.
    - Implemented `MdnsDiscoveryEngine` with live dual-stack UDP socket binding, multicast group join (`224.0.0.251:5353`), non-blocking packet reception, authoritative announcement packet generation, and privacy-preserving metadata handling (strictly broadcasting truncated 8-byte SHA-256 fingerprint, platform, and protocol version without personal identifiers).
  - `src/lib.rs`: Re-exported public API and added multi-node virtual discovery mesh integration tests.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 11 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 285 unit and integration tests (0 failures; 9 in `aissistant-network`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 64 unit and widget tests (0 failures).

---

## [2026-11-15] — Stage 6 Phase 2 Subphase 1 (`S6.P2.1`)

### Added
- `core/Cargo.toml` & `core/crates/aissistant-network/Cargo.toml`:
  - Added workspace dependencies: `quinn = "0.11"`, `rustls = "0.23"`, `rustls-pki-types = "1.10"`, `rcgen = "0.13"`, `tokio = "1.43"`, and `ed25519-dalek = "2.1"`.
- `core/crates/aissistant-network/src/error.rs`:
  - Extended `NetworkError` with `Tls`, `Quic`, `PeerAuthenticationFailed`, and `ConnectionClosed` variants.
- `core/crates/aissistant-network/src/cert.rs`:
  - Implemented PKCS#8 DER encoder (`ed25519_secret_to_pkcs8`) for 32-byte Ed25519 secret keys.
  - Implemented RFC 8410 SPKI public key extraction (`extract_ed25519_public_key`) from X.509 DER certificates.
  - Implemented self-signed TLS 1.3 certificate generation (`generate_tls_cert`) from `DeviceIdentity` via `rcgen`.
  - Implemented `PinnedEd25519Verifier` implementing both `rustls::client::danger::ServerCertVerifier` and `rustls::server::danger::ClientCertVerifier`, strictly enforcing TLS 1.3, Ed25519 signatures, and pinning against `DeviceRegistry` public keys (with opt-in unpinned pairing mode).
- `core/crates/aissistant-network/src/transport.rs`:
  - Defined protocol constants (`ALPN_P2P_PROTOCOL = b"aissistant-p2p/1"`, `DEFAULT_KEEPALIVE_INTERVAL = 5s`, `DEFAULT_IDLE_TIMEOUT = 30s`).
  - Implemented `build_quic_server_config` and `build_quic_client_config` using Ring crypto provider, keep-alive heartbeats, and pinned mTLS verifiers.
  - Implemented `QuicSession` wrapping `quinn::Connection`, extracting verified peer public key, and providing bi-directional stream multiplexing (`open_bi`, `accept_bi`) and datagrams.
  - Implemented `QuicTransportEngine` for endpoint lifecycle management (`start_endpoint`, `connect_peer`, `accept`, `close_all`).
  - Implemented `VirtualSession` and `create_virtual_session_pair` for deterministic in-memory session simulation without UDP network sockets.
- `core/crates/aissistant-network/src/lib.rs`:
  - Re-exported transport and certificate modules and public types.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 11 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 292 unit and integration tests (0 failures; 16 in `aissistant-network`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 64 unit and widget tests (0 failures).

---

## [2026-11-15] — Subphase `S6.P2.2`: Out-of-Band SAS / QR Code Pairing Ceremony & Device Management UI

### Added
- `core/crates/aissistant-network/src/pairing.rs`:
  - Implemented symmetrical 6-digit decimal Short Authentication String (SAS) derivation (`derive_sas_code`) using canonical key sorting (`min(key_a, key_b), max(key_a, key_b)`) and SHA-256.
  - Implemented compact QR payload serializer/deserializer (`PairingQrPayload`).
  - Implemented brute-force rate limiter (`PairingRateLimiter`) enforcing 300s (5-minute) lockout after 3 consecutive failed verification attempts.
  - Implemented `PairingSessionManager` tracking active session state, SAS verification, and automatic `DeviceRegistry` enrollment.
- `core/crates/aissistant-ffi/src/device_bridge.rs`:
  - Implemented `DeviceSupervisor` coordinating discovered peer cache, virtual announcement simulation, pairing ceremony lifecycle, and granular ACL grants.
  - Exposed serializable DTOs: `DiscoveredPeerItem`, `PeerPermissionItem`, `PairedDeviceItem`, `PairingSessionInfo`, `PairingStateSnapshot`.
- `core/crates/aissistant-ffi/src/events.rs`:
  - Added `BridgeEvent::DeviceDiscovered` and `BridgeEvent::PairingStateChanged`.
- `core/crates/aissistant-ffi/src/c_abi.rs`:
  - Exported safe C-ABI functions: `bridge_c_device_discovered_peers`, `bridge_c_device_paired_list`, `bridge_c_device_start_pairing`, `bridge_c_device_confirm_pairing`, `bridge_c_device_cancel_pairing`, `bridge_c_device_set_permission`, `bridge_c_device_unpair`, `bridge_c_device_simulate_peer`.
- `app/lib/core/models/device_models.dart`:
  - Created Dart models with JSON serialization: `DiscoveredPeerItem`, `PeerPermissionItem`, `PairedDeviceItem`, `PairingSessionInfo`.
- `app/lib/ui/screens/devices_screen.dart`:
  - Built full `DevicesScreen` with responsive UI: 100% offline LAN indicator, discovered peer list, active pairing ceremony card with large monospace 6-digit SAS code, QR code visual container, SAS confirmation input, paired devices list with trust badges, and interactive capability ACL dropdowns (`always_allow`, `ask_each_time`, `deny`).
- `app/test/devices_screen_test.dart`:
  - Comprehensive Flutter widget tests for pairing initiation, SAS confirmation, cancellation, unpairing confirmation dialog, and peer simulation.

### Modified
- `core/crates/aissistant-network/Cargo.toml` & `src/lib.rs`: Exposed pairing ceremony module.
- `core/crates/aissistant-ffi/Cargo.toml` & `src/lib.rs`: Wired device supervisor and C-ABI exports.
- `app/lib/core/ffi_bridge.dart`: Added device methods and C-ABI lookup to `NativeFfiBridge` and `TestHarnessBridge`.
- `app/lib/core/assistant_controller.dart`: Added reactive device state management, bridge event listeners, and pairing actions.
- `app/lib/ui/adaptive_shell.dart`: Wired `Devices` destination into desktop NavigationRail and mobile NavigationBar.
- `app/test/assistant_controller_test.dart` & `app/test/widget_test.dart`: Updated test suites for new device capabilities and navigation.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 292 unit and integration tests (0 failures).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 72 unit and widget tests (0 failures).

























---

## [2026-11-15] — Stage 7 Phase 1 Subphase 1 (`S7.P1.1`)

### Added
- `core/crates/aissistant-network/src/rpc.rs`:
  - Wire protocol types: `RemoteRpcRequest` (`rpc_id`, `caller_device_id`, `skill_id`, `params`, `timestamp_ms`) and `RemoteRpcResponse` (`rpc_id`, `status`, `result`, `error`, `execution_ms`).
  - Status enum `RemoteRpcStatus` (`Success`, `PermissionDenied`, `SkillFailed`, `TimedOut`, `InvalidRequest`).
  - Length-delimited framing utilities: `write_framed_message` and `read_framed_message` enforcing `MAX_RPC_MESSAGE_BYTES` (4 MB) buffer boundaries.
  - Skill to capability mapping: `resolve_capability_for_skill` mapping skill names to capability IDs (`file.*` -> `file_system`, `app.*`/`process.*`/`window.*` -> `application_management`, `volume.*`/`media.*` -> `system_media_control`, `llm.*` -> `local_llm`, `screen.*` -> `screen_streaming_host`, `input.*` -> `remote_input_injection_host`, default -> `lan_peer_networking`).
  - `RemoteSkillRpcServer`:
    - Strict cryptographic authentication verifying caller `DeviceId` against peer's TLS 1.3 Ed25519 public key.
    - Peer trust verification against persistent `DeviceRegistry` (`DeviceTrustState::Trusted`).
    - Granular capability ACL checks rejecting `Deny` and `AskEachTime` (unattended RPCs).
    - Local security policy engine evaluation (`PolicyEngine`) with `Actor::RemotePeer`.
    - Secure tool execution via `ToolDispatcher` with duration tracking.
    - BLAKE3 hash-chained cryptographic audit trail appending with sensitive parameter redaction.
    - Bidirectional async stream handling via `handle_stream`.
  - `RemoteSkillRpcClient`:
    - Generic stream invocation via `invoke_stream`.
    - QUIC session invocation with configurable timeout via `invoke_quic`.
    - Virtual session invocation for offline/test environments via `invoke_virtual`.

### Modified
- `core/crates/aissistant-network/Cargo.toml`: Added workspace dependencies on `aissistant-orchestrator` and `uuid`.
- `core/crates/aissistant-network/src/lib.rs`: Exposed and re-exported `rpc` module types and functions.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 299 unit and integration tests (0 failures; 27 in `aissistant-network`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 72 unit and widget tests (0 failures).


---

## [2026-11-15] — Stage 7 Phase 1 Subphase 2 (`S7.P1.2`)

### Added
- `core/crates/aissistant-network/src/clipboard.rs`:
  - Defined clipboard wire types: `ClipboardPayload`, `ClipboardContentType` (`PlainText`, `PngImage`, `JsonData`), and `ClipboardSyncMode` (`Disabled`, `ManualOnly`, `AutoSync`).
  - Implemented boundary caps: `MAX_CLIPBOARD_TEXT_BYTES` (1 MB) and `MAX_CLIPBOARD_IMAGE_BYTES` (16 MB).
  - Implemented BLAKE3 cryptographic digest validation in `ClipboardPayload::validate()` to detect tampering in transit.
  - Implemented `ClipboardLoopGuard` with configurable capacity ring-buffer LRU tracking of recently processed content digests, suppressing echo bounce-backs and cyclic cross-device reflections.
  - Implemented `ClipboardSyncEngine`:
    - Strict TLS 1.3 Ed25519 public key caller verification.
    - Persistent `DeviceRegistry` peer trust state validation (`DeviceTrustState::Trusted`).
    - Granular capability ACL checks (`"clipboard"` with fallback to `"lan_peer_networking"`).
    - Platform clipboard ingestion via `DesktopPlatformAdapter::write_clipboard`.
    - BLAKE3 hash-chained cryptographic audit trail appending with sensitive preview redaction.
    - Streaming RPC over QUIC (`sync_quic`) and Virtual Sessions (`sync_virtual`).

### Modified
- `core/crates/aissistant-network/Cargo.toml`: Added workspace dependencies on `blake3` and `aissistant-platform`.
- `core/crates/aissistant-network/src/lib.rs`: Exposed and re-exported `clipboard` module types and constants.
- `core/crates/aissistant-network/src/rpc.rs`: Added mapping for `clipboard.*` skills to `"clipboard"` capability.
- `core/crates/aissistant-network/src/pairing.rs`: Derived `Default` on `PairingSessionManager`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 306 unit and integration tests (0 failures; 34 in `aissistant-network`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 72 unit and widget tests (0 failures).


---

## [2026-11-15] — Stage 7 Phase 2 Subphase 1 (`S7.P2.1`)

### Added
- `core/crates/aissistant-network/src/transfer.rs`:
  - Wire protocol messages: `TransferManifest`, `ChunkData`, `TransferResumeResponse`, `TransferStatus`, and `TransferMessage` envelope.
  - Path traversal defense: `sanitize_file_name` stripping directory separators and rejecting directory traversal (`../`, `..\`), root paths, drive letters, and null bytes.
  - Cryptographic chunking & integrity:
    - 1 MB default chunk slicing (`DEFAULT_CHUNK_SIZE_BYTES = 1048576`).
    - Per-chunk BLAKE3 256-bit hash computation and verification in transit.
    - Whole-file BLAKE3 verification prior to finalization.
  - Resumption & crash consistency:
    - Pre-allocated `.part` temporary files with direct offset writes (`seek` + `write_all`).
    - Staged `.meta` JSON state tracking completed chunk indices.
    - Automatic resume negotiation skipping already received and verified chunks.
    - Atomic rename from `.part` to final file path upon full verification.
  - `FileTransferReceiver`:
    - Strict TLS 1.3 Ed25519 caller public key authentication against `origin_device_id`.
    - `DeviceRegistry` peer trust validation (`DeviceTrustState::Trusted`).
    - Capability ACL enforcement for `file_system` (or `lan_peer_networking`).
    - BLAKE3 hash-chained cryptographic audit trail appending with redacted manifest metadata.
  - `FileTransferSender`:
    - Manifest generation with per-chunk hashes.
    - Resume response evaluation.
    - Streaming chunks over QUIC (`send_file_quic`) and Virtual Sessions (`send_file_virtual`).

### Modified
- `core/crates/aissistant-network/src/lib.rs`: Exposed and re-exported `transfer` module types and constants.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 311 unit and integration tests (0 failures; 39 in `aissistant-network`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 72 unit and widget tests (0 failures).


---

## [2026-11-15] — Stage 7 Phase 2 Subphase 2 (`S7.P2.2`)

### Added
- `core/crates/aissistant-network/src/screen.rs`:
  - Wire protocol types & framing:
    - `ScreenPixelFormat` (`Rgb8`, `Rgba8`, `Bgra8`, `Jpeg`, `H264Keyframe`).
    - `ScreenStreamConfig` (target width/height, max framerate, quality, format).
    - `ScreenFrameHeader` (sequence number, timestamp ms, dimensions, uncompressed/compressed byte size, keyframe flag, BLAKE3 256-bit hash).
    - `ScreenFrame` with per-frame BLAKE3 cryptographic integrity digest computation and validation.
    - `ScreenStreamControl` session negotiation messages (`StartSession`, `SessionStarted`, `RequestKeyframe`, `StopSession`, `SessionStopped`, `SessionRejected`).
    - `ScreenWireMessage` enum envelope for length-delimited wire transport.
  - Abstraction & synthetic providers:
    - `ScreenCaptureProvider` trait for native OS display frame acquisition.
    - `VirtualScreenCaptureProvider` synthetic test frame generator supporting animated color bars, timestamp stamping, and keyframe intervals.
  - `ScreenStreamingHost`:
    - Strict TLS 1.3 Ed25519 caller public key authentication against `origin_device_id`.
    - `DeviceRegistry` peer trust validation (`DeviceTrustState::Trusted`).
    - Capability ACL enforcement for `screen_streaming_host` capability (`AlwaysAllow` required; `Deny` / `AskEachTime` fails closed with `PermissionDenied`).
    - Active host streaming indicator state (`is_streaming_active`) for system UI warning badges and overlays.
    - BLAKE3 hash-chained cryptographic audit trail appending with session metadata.
  - `ScreenStreamReceiver`:
    - Session initiation negotiation with streaming host.
    - Frame deserialization and per-frame BLAKE3 hash integrity validation.
    - Frame continuity and inter-frame latency tracking.
    - QUIC transport execution (`receive_quic_stream`) and Virtual Session execution (`receive_virtual_stream`).

### Modified
- `core/crates/aissistant-network/src/lib.rs`: Exposed and re-exported `screen` module types and constants.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 307 unit and integration tests (0 failures; 43 in `aissistant-network`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 72 unit and widget tests (0 failures).


---

## [2026-11-15] — Stage 7 Phase 2 Subphase 3 (`S7.P2.3`)

### Added
- `core/crates/aissistant-network/src/input.rs`:
  - Wire protocol types & serialization:
    - `MouseButton` (`Left`, `Right`, `Middle`, `Other(u8)`).
    - `KeyModifiers` (`shift`, `ctrl`, `alt`, `meta`).
    - `RemoteInputEvent` (`MouseMove`, `MouseDown`, `MouseUp`, `MouseWheel`, `KeyDown`, `KeyUp`, `TextInput`) with normalized coordinate clamping `[0.0, 1.0]`.
    - `RemoteInputSessionControl` (`RequestControl`, `ControlGranted`, `ControlRejected`, `ReleaseControl`, `ControlTerminated`).
    - `RemoteInputWireMessage` envelope with length-delimited wire transport.
  - Abstraction & synthetic providers:
    - `InputInjector` trait for platform-native and synthetic input event dispatch.
    - `VirtualInputInjector` for thread-safe event capture and hermetic testing.
  - `RemoteInputHost`:
    - Strict TLS 1.3 Ed25519 caller public key verification against `caller_device_id`.
    - `DeviceRegistry` peer trust checking (`DeviceTrustState::Trusted`).
    - Capability ACL enforcement for `remote_input_injection_host` (`AlwaysAllow` required; `Deny` / `AskEachTime` fails closed with `PermissionDenied`).
    - Active remote control indicator state (`is_control_active`).
    - Host emergency escape trigger (`terminate_session`) allowing host users (via physical Escape or UI overlay Stop button) to instantly preempt remote control.
    - BLAKE3 hash-chained cryptographic audit trail appending for session grants, batch injections, and session terminations.
  - `RemoteInputClient`:
    - Remote control session negotiation.
    - Batch input event transmission over QUIC streams.
    - Clean session release protocol.

### Modified
- `core/crates/aissistant-network/src/lib.rs`: Exposed and re-exported `input` module and types.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 312 unit and integration tests (0 failures; 48 in `aissistant-network`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 72 unit and widget tests (0 failures).


---

## [2026-11-15] — Stage 8 Phase 1 Subphase 1 (`S8.P1.1`)

### Added
- `core/crates/aissistant-storage/src/migration.rs`:
  - `Migration 6`:
    - `documents` table tracking file paths, names, mime types, BLAKE3 content hashes, sizes, and timestamps.
    - `document_chunks` table storing sequential semantic chunks with start/end byte offsets and estimated token counts.
    - `document_chunks_fts` SQLite FTS5 inverted index table with `unicode61 remove_diacritics 2` tokenizer.
    - Automatic SQLite triggers syncing `document_chunks` insertions, deletions, and updates with `document_chunks_fts`.
    - Automatic cascading trigger purging `semantic_embeddings` (`entity_type = 'doc_chunk'`) upon chunk or document deletion.
    - Automatic cascading trigger purging `document_chunks` upon document deletion.
- `core/crates/aissistant-storage/src/document_store.rs`:
  - `DocumentRecord`, `DocumentChunk`, `RawChunk`, and `ChunkSearchResult` domain data types.
  - `IndexingStatus` enum (`Indexed`, `SkippedUnchanged`, `Updated`).
  - `DocumentChunker`: Boundary-aware text chunker with configurable target size, overlap, paragraph breaks (`\n\n`), code block boundaries, and sentence delimiters.
  - `estimate_token_count` heuristic estimating token volume.
  - `DocumentStore`:
    - `index_document`: Ingests documents with incremental BLAKE3 change detection, skipping unchanged files and atomically replacing modified chunks and vector embeddings.
    - `get_document`, `get_document_by_path`, `list_documents`, `get_document_chunks`.
    - `delete_document`, `delete_document_by_path` with 100% cascade purge across FTS5 and vector tables.
    - `search_chunks_fts`: SQLite FTS5 BM25 search across document chunks with highlighted `<mark>` snippet excerpts.
- `core/crates/aissistant-storage/Cargo.toml`: Added workspace dependency on `blake3`.
- `core/crates/aissistant-storage/src/lib.rs`: Exposed and re-exported `document_store` module and types.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 318 unit and integration tests (0 failures; 38 in `aissistant-storage`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 72 unit and widget tests (0 failures).

---

## [2026-11-15] — Stage 8 Phase 1 Subphase 2 (`S8.P1.2`)

### Added
- `core/crates/aissistant-storage/src/knowledge_retriever.rs`:
  - `KnowledgeCitation`: Grounded knowledge citation struct tracking chunk ID, document ID, path, filename, chunk index, RRF score, BM25 rank, vector rank, cosine similarity, content, and `<mark>` snippet.
  - `KnowledgeRagPromptContext`: Structured RAG prompt context with bounded citations, prompt block, and estimated token counts.
  - `KnowledgeRetriever`: Reciprocal Rank Fusion (RRF) hybrid search combining SQLite FTS5 BM25 lexical candidates with dense vector k-NN candidates (`search_vector_similarity_by_entity`), and prompt context formatting bounded by token budget.
- `app/lib/core/models/knowledge_models.dart`:
  - `DocumentRecordItem` and `KnowledgeCitationItem` immutable Dart models with JSON serialization.
- `app/lib/ui/screens/knowledge_screen.dart`:
  - Responsive Knowledge Explorer screen featuring search bar with clear/submit, TabBar for Citations and Document catalog, grounded citation cards with RRF badges and `<mark>` highlight snippets, document deletion with confirmation dialog, and manual document ingestion dialog.
- `app/test/knowledge_screen_test.dart`:
  - 5 comprehensive widget and interaction tests verifying UI rendering, search query execution, document indexing, and document deletion.

### Modified
- `core/crates/aissistant-storage/src/vector_store.rs`:
  - Added `search_vector_similarity_by_entity` to perform dense vector k-NN search filtered by entity type (`doc_chunk`).
- `core/crates/aissistant-storage/src/document_store.rs`:
  - Added `get_chunk_with_document` to look up document metadata for vector-only candidates.
- `core/crates/aissistant-storage/src/lib.rs`:
  - Exposed and re-exported `knowledge_retriever` module and types.
- `core/crates/aissistant-ffi/src/api.rs`:
  - Added `DocumentRecordItem` and `KnowledgeCitationItem` FFI types.
  - Added `knowledge_list_documents`, `knowledge_index_document`, `knowledge_delete_document`, and `knowledge_search` methods to `CoreBridge` and convenience wrappers.
- `core/crates/aissistant-ffi/src/c_abi.rs`:
  - Added safe C-ABI functions: `bridge_c_knowledge_list`, `bridge_c_knowledge_index`, `bridge_c_knowledge_delete`, and `bridge_c_knowledge_search`.
- `core/crates/aissistant-ffi/src/lib.rs`:
  - Re-exported knowledge types and added `test_knowledge_bridge_and_c_abi_lifecycle` integration test.
- `app/lib/core/ffi_bridge.dart`:
  - Added knowledge operations to `AissistantBridge`, `NativeFfiBridge` (C-ABI bindings), and `TestHarnessBridge` (in-memory test mock).
- `app/lib/core/assistant_controller.dart`:
  - Added reactive knowledge state (`documents`, `citationSearchResults`, `isSearchingKnowledge`, `knowledgeSearchQuery`) and methods (`refreshKnowledgeDocuments`, `searchKnowledge`, `clearKnowledgeSearch`, `indexKnowledgeDocument`, `deleteKnowledgeDocument`).
- `app/lib/ui/adaptive_shell.dart`:
  - Added `KnowledgeScreen` navigation destination to desktop `NavigationRail` and mobile `NavigationBar`.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed with 0 warnings.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 11 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 343 unit and integration tests (0 failures; 40 in `aissistant-storage`, 27 in `aissistant-ffi`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 77 unit and widget tests (0 failures).

---

## [2026-11-15] — Stage 8 Phase 2 Subphase 1 (`S8.P2.1`)

### Added
- `core/crates/aissistant-llm/src/vision.rs`:
  - `ImageFormat`: Image format enumeration supporting PNG, JPEG, Grayscale, and RawRGB.
  - `ImageMetadata`: Structural image properties (dimensions, channel count, byte count, format).
  - `ImageBuffer`: Low-level pixel buffer supporting bilinear interpolation resizing, RGB luminosity downmixing, and PNG (`IHDR`) / JPEG (`SOF0`) binary header parsing without external runtime dependencies.
  - `VisionPreprocessorConfig`: Model configuration for input dimensions, normalization means/stds, and patch extraction sizes.
  - `ImagePatch`: Normalized vision transformer (`ViT`) patch representation with 2D grid coordinate tagging.
  - `VisionPreprocessor`: Preprocessing engine implementing aspect-ratio-preserving letterboxing, ImageNet normalization, and patch extraction.
- `core/crates/aissistant-llm/src/ocr.rs`:
  - `OcrBoundingBox`: Normalized `[x, y, w, h]` bounding box in `[0.0, 1.0]` space with area, intersection, union, and Intersection over Union (IoU) calculation.
  - `OcrWord`: Individual word segmentation with bounding box and confidence score.
  - `OcrTextLine`: Grouped text line with ordered words and line-level bounding box.
  - `OcrDocument`: Complete document extraction result with lines, full concatenated text, average confidence, and latency metrics.
  - `OfflineOcrEngine`: Edge-based OCR engine using horizontal and vertical contrast projection profiling for line and word segmentation, embedded glyph analysis, and layout reconstruction.
- `app/lib/core/models/vision_models.dart`:
  - `OcrBoundingBoxItem`, `OcrWordItem`, `OcrTextLineItem`, and `OcrDocumentItem` immutable Dart models with full JSON serialization.
- `app/lib/ui/screens/vision_screen.dart`:
  - Responsive Vision & OCR screen featuring 100% offline edge notice, input sample selector chips (Invoice, Architecture Diagram, Terminal Diagnostics), extract OCR action button, extraction metrics banner (average confidence, lines detected, latency, dimensions), recognized full text card with copy to clipboard button, and line-by-line bounding box breakdown.
- `app/test/vision_screen_test.dart`:
  - 5 comprehensive widget and interaction tests verifying UI rendering, OCR extraction, metric chips, sample switching, clipboard copy, and adaptive shell navigation.

### Modified
- `core/crates/aissistant-core/src/capability.rs`:
  - Added `Capability::VisionOcr` variant, string parsing, and 5-platform capability matrix mapping (`Supported` on Windows, Linux, macOS; `RequiresPermission(CAMERA)` on Android, iOS).
- `core/crates/aissistant-llm/src/error.rs`:
  - Added `LlmError::VisionError` and `LlmError::OcrError` error taxonomy variants.
- `core/crates/aissistant-llm/src/lib.rs`:
  - Exposed and re-exported `vision` and `ocr` modules, types, and engines.
- `core/crates/aissistant-ffi/src/api.rs`:
  - Added `OcrBoundingBoxItem`, `OcrWordItem`, `OcrTextLineItem`, and `OcrDocumentItem` FFI types.
  - Added `vision_ocr_extract` method to `CoreBridge` and convenience wrapper `bridge_vision_ocr_extract`.
- `core/crates/aissistant-ffi/src/c_abi.rs`:
  - Added safe C-ABI function: `bridge_c_vision_ocr_extract(image_ptr: *const u8, image_len: usize) -> *mut c_char`.
- `core/crates/aissistant-ffi/src/lib.rs`:
  - Re-exported OCR types and added `test_vision_ocr_bridge_and_c_abi` integration test.
- `app/lib/core/ffi_bridge.dart`:
  - Added `extractOcr` method to `AissistantBridge`, `NativeFfiBridge` (C-ABI binding), and `TestHarnessBridge` (mock with structured lines/words/bounding boxes), and added `vision_ocr` capability to test harness.
- `app/lib/core/assistant_controller.dart`:
  - Added reactive vision OCR state (`activeOcrResult`, `isProcessingOcr`, `ocrError`) and methods (`extractOcr`, `clearOcrResult`).
- `app/lib/ui/adaptive_shell.dart`:
  - Added `VisionScreen` navigation destination to desktop `NavigationRail` and mobile `NavigationBar`.
- `app/test/assistant_controller_test.dart` and `app/test/capabilities_screen_test.dart`:
  - Updated capability count expectations to reflect 10 active capabilities.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed cleanly.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 11 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 351 unit and integration tests (0 failures; 13 in `aissistant-llm`, 28 in `aissistant-ffi`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 82 unit and widget tests (0 failures).

---

## [2026-11-15] — Stage 8 Phase 2 Subphase 2 (`S8.P2.2`)

### Added
- `core/crates/aissistant-llm/src/translation.rs`:
  - `Language`: ISO 639-1 language code enumeration (`en`, `es`, `fr`, `de`, `zh`, `ja`, `it`, `pt`, `ru`, `ko`) with display names, native endonyms, parsing, and serialization.
  - `LanguagePair`: Directed language pair struct supporting string formatting (`en-es`), display notation (`en -> es`), and direction reversal.
  - `TranslationPromptTemplate`: Constrained prompt generator for local neural LLMs with strict formatting and anti-hallucination rules.
  - `TranslationResult`: Complete translation output tracking source/translated text, language pair, confidence score, execution latency, and engine identifier.
  - `TranslationEngine`: High-speed offline translation engine combining a verified multi-lingual phrasebook, case preservation matching, and hybrid translation fallback.
- `core/crates/aissistant-orchestrator/src/interpreter.rs`:
  - `InterpreterSpeaker`: Dialogue participant enum (`PartyA`, `PartyB`) with alternation toggle.
  - `InterpreterTurn`: Complete dialogue turn record tracking speaker, source text, translated text, language pair, confidence, latency, and timestamp.
  - `InterpreterSessionConfig`: Configurable language assignment and auto-alternating speaker option.
  - `InterpreterSession`: Stateful bi-directional live interpreter conversation manager executing alternating translation turns, language swapping, and transcript logging.
- `app/lib/core/models/interpreter_models.dart`:
  - `LanguageItem`: Language descriptor with pre-configured supported language catalog and lookup utilities.
  - `TranslationResultItem`, `InterpreterTurnItem`, and `InterpreterSessionStatusItem`: Immutable Dart models with full JSON serialization.
- `app/lib/ui/screens/interpreter_screen.dart`:
  - Responsive Live Voice Interpreter screen featuring offline privacy notices, participant language tags, manual language swap button, active speaker turn indicator, quick phrase chips, text input submission, and chronological dual-party conversation transcript cards with copy to clipboard buttons.
- `app/test/interpreter_screen_test.dart`:
  - 7 comprehensive widget and interaction tests verifying UI rendering, turn submission, bi-directional language alternation, language swapping, clipboard copy, transcript clearing, and adaptive shell navigation.

### Modified
- `core/crates/aissistant-core/src/capability.rs`:
  - Added `Capability::LiveInterpreter` variant, string parsing, and 5-platform capability matrix mapping (`Supported` across Windows, Linux, macOS, Android, and iOS).
- `core/crates/aissistant-llm/src/error.rs`:
  - Added `LlmError::TranslationError` variant to the error taxonomy.
- `core/crates/aissistant-llm/src/lib.rs`:
  - Exposed and re-exported `translation` module, types, and engine.
- `core/crates/aissistant-orchestrator/src/lib.rs`:
  - Exposed and re-exported `interpreter` module, types, and session manager.
- `core/crates/aissistant-ffi/Cargo.toml`:
  - Added dependency on `aissistant-orchestrator`.
- `core/crates/aissistant-ffi/src/api.rs`:
  - Added `TranslationResultItem`, `InterpreterTurnItem`, and `InterpreterSessionStatusItem` FFI types.
  - Added `translate_text`, `interpreter_start_session`, `interpreter_submit_turn`, `interpreter_get_turns`, `interpreter_swap_languages`, and `interpreter_clear_session` methods to `CoreBridge` and convenience wrappers.
- `core/crates/aissistant-ffi/src/c_abi.rs`:
  - Added safe C-ABI functions: `bridge_c_translate_text`, `bridge_c_interpreter_start`, `bridge_c_interpreter_submit_turn`, `bridge_c_interpreter_get_turns`, `bridge_c_interpreter_swap_languages`, and `bridge_c_interpreter_clear`.
- `core/crates/aissistant-ffi/src/lib.rs`:
  - Re-exported translation and interpreter types and added `test_translation_and_interpreter_c_abi` integration test.
- `app/lib/core/ffi_bridge.dart`:
  - Added translation and interpreter operations to `AissistantBridge`, `NativeFfiBridge` (C-ABI bindings), and `TestHarnessBridge` (mock with realistic phrasebook and alternating turn dialogue logic), and added `live_interpreter` capability to test harness.
- `app/lib/core/assistant_controller.dart`:
  - Added reactive interpreter and translation state (`interpreterStatus`, `interpreterTurns`, `isProcessingInterpreterTurn`, `interpreterError`, `lastTranslationResult`) and methods (`translateText`, `startInterpreterSession`, `submitInterpreterTurn`, `swapInterpreterLanguages`, `clearInterpreterSession`).
- `app/lib/ui/adaptive_shell.dart`:
  - Added `InterpreterScreen` navigation destination to desktop `NavigationRail` and mobile `NavigationBar`.
- `app/test/assistant_controller_test.dart` and `app/test/capabilities_screen_test.dart`:
  - Updated capability count expectations to reflect 11 active capabilities.

### Verification
- `cargo fmt --manifest-path core/Cargo.toml --all -- --check` passed cleanly.
- `cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings` passed with 0 warnings across all 11 workspace crates.
- `cargo test --manifest-path core/Cargo.toml --workspace` passed: 356 unit and integration tests (0 failures; 40 in `aissistant-llm`, 24 in `aissistant-orchestrator`, 29 in `aissistant-ffi`).
- `cd app; C:\flutter\bin\flutter.bat analyze` passed with 0 issues.
- `cd app; C:\flutter\bin\flutter.bat test` passed: 89 unit and widget tests (0 failures).
