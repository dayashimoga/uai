# ROADMAP.md — Dependency-Aware Master Execution Roadmap

Every stage (`Stage 0` – `Stage 11`) is divided into multiple phases (`P1`, `P2`, ...), and each phase is divided into individually numbered, independently verifiable subphases (`S{stage}.P{phase}.{subphase}`).

**Mandatory Execution Rule:** Only **one subphase** may be implemented per user approval cycle. Every subphase delivers a working, tested increment.

---

## Stage 0: Requirements, Architecture & Engineering Foundation
*Goal: Establish verified core schemas, error taxonomy, capability detection contracts, and test harness foundations before platform shell generation.*

### Phase 0.1: Domain Contracts & Capability Matrix (`S0.P1`)
- **`S0.P1.1` — Core Domain Types & Error Taxonomy**
  - *Dependencies:* Bootstrap Complete
  - *Deliverable:* Initialize `core/` Cargo workspace and `aissistant-core` crate defining strongly typed IDs (`DeviceId`, `ConversationId`, `SkillId`, `ModelId`), error hierarchy (`CoreError`), structured configuration schemas, and >=90% unit test coverage.
- **`S0.P1.2` — Five-Platform Capability Matrix & Runtime Probe Contract**
  - *Dependencies:* `S0.P1.1`
  - *Deliverable:* Implement `CapabilityRegistry`, `PlatformTarget` enum (Android, Windows, Linux, macOS, iOS), `CapabilityStatus` (`Supported`, `Degraded`, `Unsupported`, `PermissionRequired`), and host OS detection with comprehensive unit tests.

### Phase 0.2: Security Policy Contract & Audit Log Schema (`S0.P2`)
- **`S0.P2.1` — Risk Tiers, Policy Engine & Taint Tracking Core**
  - *Dependencies:* `S0.P1.2`
  - *Deliverable:* Create `aissistant-security` crate with deterministic `PolicyEngine`, 5 risk tiers, untrusted-input taint tracking rules, and adversarial policy unit tests.
- **`S0.P2.2` — Cryptographic Primitives & Hash-Chained Audit Log Contract**
  - *Dependencies:* `S0.P2.1`
  - *Deliverable:* Implement Ed25519 device identity generation, XChaCha20-Poly1305 AEAD helpers (`ZeroizeOnDrop`), and BLAKE3 hash-chained `AuditLogEntry` verifier with tamper-detection tests.

---

## Stage 1: Repository, Flutter/Rust Foundation & Five-Platform Application Shells
*Goal: Establish SQLite storage, the Flutter-Rust FFI bridge, and buildable five-platform application shells connected to live Rust state.*

### Phase 1.1: SQLite Persistence & Encrypted Storage Foundation (`S1.P1`)
- **`S1.P1.1` — SQLite WAL Engine & Schema Migration Runner**
  - *Dependencies:* `S0.P2.2`
  - *Deliverable:* Create `aissistant-storage` crate with SQLite WAL connection pool, deterministic migration runner, settings store, and persistent hash-chained audit log table.
- **`S1.P1.2` — OS Keystore Integration & Encrypted Secret Store**
  - *Dependencies:* `S1.P1.1`
  - *Deliverable:* Integrate OS-native keystore adapters (Windows DPAPI, macOS/iOS Keychain, Linux SecretService, Android Keystore fallback abstraction) to seal the master AEAD key protecting sensitive SQLite records.

### Phase 1.2: Rust-to-Flutter FFI Bridge (`S1.P2`)
- **`S1.P2.1` — `aissistant-ffi` Crate & Async Stream Bridge**
  - *Dependencies:* `S1.P1.2`
  - *Deliverable:* Create `aissistant-ffi` crate exposing system health, capability matrix, audit log inspection, and event streams over C-ABI / `flutter_rust_bridge` with FFI round-trip integration tests.
- **`S1.P2.2` — Core Lifecycle Supervisor & Graceful Shutdown**
  - *Dependencies:* `S1.P2.1`
  - *Deliverable:* Implement `CoreRuntime` lifecycle manager handling initialization, background task supervision, memory pressure signals, and clean shutdown across FFI.

### Phase 1.3: Five-Platform Flutter Application Shell (`S1.P3`)
- **`S1.P3.1` — Shared Flutter App Shell, Adaptive Navigation & Live Core Diagnostics**
  - *Dependencies:* `S1.P2.2`
  - *Deliverable:* Create `app/` Flutter project with adaptive desktop/mobile navigation, accessibility semantics, localization scaffolding, and live binding to `aissistant-ffi` capability and health status.
- **`S1.P3.2` — Five-Platform Native Runner Configuration & Capability Badges**
  - *Dependencies:* `S1.P3.1`
  - *Deliverable:* Configure Windows, Linux, macOS, Android, and iOS native runner manifests, permissions declarations, and runtime capability status views with widget and integration tests.

---

## Stage 2: Offline Voice, Wake Word, STT & TTS
*Goal: Deliver a 100% offline, low-latency audio pipeline with customizable wake words, VAD barge-in, streaming multilingual STT, and neural TTS.*

### Phase 2.1: Cross-Platform Audio I/O & Ring Buffer (`S2.P1`)
- **`S2.P1.1` — Real-Time Safe Audio Ring Buffer & Resampler (`aissistant-audio`)**
  - *Dependencies:* `S1.P3.2`
  - *Deliverable:* Implement lock-free 16 kHz mono PCM circular buffer, sample-rate conversion, and audio level metering with zero allocations on the real-time thread.
- **`S2.P1.2` — Cross-Platform Audio Capture & Playback Engine**
  - *Dependencies:* `S2.P1.1`
  - *Deliverable:* Integrate native audio I/O (`cpal` / OS audio sessions), hot-plug recovery, hardware/software mute enforcement, and active microphone status indicator events.

### Phase 2.2: Voice Activity Detection & Customizable Wake Word (`S2.P2`)
- **`S2.P2.1` — Offline VAD & Barge-In Detector (`sherpa-onnx` / Silero VAD)**
  - *Dependencies:* `S2.P1.2`
  - *Deliverable:* Create `aissistant-speech` crate with offline VAD speech onset/offset segmentation, silence timeout tuning, and immediate barge-in interruption signal.
- **`S2.P2.2` — Customizable Offline Wake-Word Engine (`sherpa-onnx` KWS)**
  - *Dependencies:* `S2.P2.1`
  - *Deliverable:* Implement open-vocabulary/customizable wake-word spotting over the in-memory ring buffer with threshold tuning, false-accept suppression, and WAV fixture verification tests.

### Phase 2.3: Offline Streaming STT & Neural TTS (`S2.P3`)
- **`S2.P3.1` — Offline Multilingual Streaming Speech-to-Text**
  - *Dependencies:* `S2.P2.2`
  - *Deliverable:* Implement streaming and chunked offline STT (`sherpa-onnx` Zipformer/Whisper/SenseVoice ONNX) with partial transcript streaming and language detection.
- **`S2.P3.2` — Offline Neural Text-to-Speech & Sentence Streaming**
  - *Dependencies:* `S2.P3.1`
  - *Deliverable:* Implement offline VITS/Piper ONNX TTS synthesis with sentence chunking, speaking-rate control, and immediate cancellation on user barge-in.
- **`S2.P3.3` — Voice UI Integration, Waveform Feedback & Push-to-Talk/Wake Mode**
  - *Dependencies:* `S2.P3.2`
  - *Deliverable:* Connect the Flutter Voice UI to the live Rust speech pipeline with real-time waveform visualization, live partial transcripts, wake-word customization settings, and E2E voice pipeline tests.

---

## Stage 3: Local LLM Runtime & Model Management
*Goal: Deliver hardware-aware local GGUF model downloading, verification, and `llama.cpp` inference with grammar-constrained outputs.*

### Phase 3.1: Hardware Capability Profiler & Model Registry (`S3.P1`)
- **`S3.P1.1` — CPU/RAM/GPU Hardware Profiler (`aissistant-llm`)**
  - *Dependencies:* `S2.P3.3`
  - *Deliverable:* Detect CPU instruction sets (AVX2, AVX-512, NEON), total/available RAM, and GPU backends (Vulkan, Metal, CUDA) and compute recommended model tiers and context window ceilings.
- **`S3.P1.2` — Resumable Model Downloader, SHA-256 Verifier & GGUF Inspector**
  - *Dependencies:* `S3.P1.1`
  - *Deliverable:* Implement curated open-model catalog, HTTP range-resumable downloader, mandatory SHA-256 checksum verifier, and binary GGUF metadata/header parser with corrupt-file rejection tests.

### Phase 3.2: `llama.cpp` Inference Engine & Optimization (`S3.P2`)
- **`S3.P2.1` — `llama.cpp` Rust Bindings, Session Management & Token Streaming**
  - *Dependencies:* `S3.P1.2`
  - *Deliverable:* Integrate `llama.cpp` inference backend with configurable GPU layer offloading, KV-cache prefix reuse, cooperative token cancellation, and memory watermark auto-unloading.
- **`S3.P2.2` — GBNF Grammar Compiler for Structured JSON Tool Calling**
  - *Dependencies:* `S3.P2.1`
  - *Deliverable:* Compile JSON schemas of active skills into `llama.cpp` GBNF grammars so local models reliably output valid structured tool invocations or conversational responses.
- **`S3.P2.3` — Flutter Model Manager UI & Live Benchmark Diagnostics**
  - *Dependencies:* `S3.P2.2`
  - *Deliverable:* Build the Model Manager UI screen showing hardware compatibility badges, download progress, checksum status, local GGUF file import, and tokens/sec benchmark metrics.

---

## Stage 4: AI Orchestration, Memory & Tool Execution
*Goal: Connect voice, LLM inference, hybrid SQLite memory, and the policy-enforced tool loop into a cohesive conversational agent.*

### Phase 4.1: Episodic & Semantic Memory Engine (`S4.P1`)
- **`S4.P1.1` — Conversation Persistence & SQLite FTS5 Episodic Search**
  - *Dependencies:* `S3.P2.3`
  - *Deliverable:* Implement multi-session conversation history, turn metadata storage, and fast BM25 full-text search via SQLite FTS5.
- **`S4.P1.2` — Local Embedding Generation & Hybrid Vector Memory Retrieval**
  - *Dependencies:* `S4.P1.1`
  - *Deliverable:* Integrate local embedding generation and vector similarity indexing in SQLite with Reciprocal Rank Fusion (RRF) combining keyword and semantic recall.

### Phase 4.2: Turn State Machine & Policy-Gated Tool Loop (`S4.P2`)
- **`S4.P2.1` — Deterministic Orchestrator Turn State Machine (`aissistant-orchestrator`)**
  - *Dependencies:* `S4.P1.2`
  - *Deliverable:* Coordinate STT -> Memory Retrieval -> LLM Streaming -> Tool Dispatch -> TTS Streaming with full cancellation, barge-in handling, and state recovery.
- **`S4.P2.2` — Policy-Enforced Tool Dispatcher & User Confirmation Flow**
  - *Dependencies:* `S4.P2.1`
  - *Deliverable:* Connect LLM tool calls to `PolicyEngine`, pause execution on `RequireConfirmation`, present interactive UI/voice confirmation prompts, and log outcomes to the hash-chained audit table.

---

## Stage 5: Skill Framework & Native Application/System Control
*Goal: Deliver production-grade local skills for file management, application control, system telemetry, and media across all five platforms.*

### Phase 5.1: Extensible Skill Framework & Core File/System Skills (`S5.P1`)
- **`S5.P1.1` — Skill Registry, Sandbox Validator & Built-In Utility Skills**
  - *Dependencies:* `S4.P2.2`
  - *Deliverable:* Implement `aissistant-skills` registry, JSON schema validation, timeout sandboxing, and core skills (clock, timers, alarms, calculator, unit conversion, system resource telemetry).
- **`S5.P1.2` — Safe File Management Skills (Search, Read, Write, Move, Archive, Checksum)**
  - *Dependencies:* `S5.P1.1`
  - *Deliverable:* Implement path-canonicalized, jail-verified file skills with symlink-escape prevention and confirmation gates for destructive operations.

### Phase 5.2: Desktop Native OS Control (Windows, Linux, macOS) (`S5.P2`)
- **`S5.P2.1` — Desktop Application Launch, Process Inspection & Window Management**
  - *Dependencies:* `S5.P1.2`
  - *Deliverable:* Implement native desktop adapters in `aissistant-platform` for listing, launching, focusing, and closing applications on Windows (Win32), Linux (XDG/Wayland/X11), and macOS (`NSWorkspace`/Accessibility).
- **`S5.P2.2` — Desktop System Settings, Clipboard, Audio Volume & Media Transport Control**
  - *Dependencies:* `S5.P2.1`
  - *Deliverable:* Implement native media playback control (Windows SMTC, Linux MPRIS2, macOS MediaRemote), system volume/mute, and clipboard read/write skills.

### Phase 5.3: Mobile Native OS Control (Android & iOS) (`S5.P3`)
- **`S5.P3.1` — Android Native Bridge (Foreground Service, Intents, Media, Accessibility Opt-In)**
  - *Dependencies:* `S5.P2.2`
  - *Deliverable:* Implement Kotlin native plugin for Android foreground voice service, app intents, media session control, system toggles, and opt-in accessibility actions.
- **`S5.P3.2` — iOS Native Bridge (AVAudioSession, App Intents/Siri Shortcuts, Scoped Files)**
  - *Dependencies:* `S5.P3.1`
  - *Deliverable:* Implement Swift native bridge for iOS audio session lifecycle, App Intents integration, document bookmarks, and graceful degradation indicators for iOS sandbox limits.

---

## Stage 6: Device Agents, Pairing & Device Registry
*Goal: Enable devices to discover each other on LAN or overlay networks and establish mutually authenticated cryptographic trust.*

### Phase 6.1: Local Discovery & Cryptographic Identity (`S6.P1`)
- **`S6.P1.1` — Persistent Device Registry & Peer ACL Store**
  - *Dependencies:* `S5.P3.2`
  - *Deliverable:* Implement SQLite `paired_devices` and `peer_permissions` tables storing peer Ed25519 public certificates, trust state, last-seen endpoints, and granular capability ACLs.
- **`S6.P1.2` — mDNS-SD Local Network Discovery (`aissistant-network`)**
  - *Dependencies:* `S6.P1.1`
  - *Deliverable:* Implement privacy-preserving mDNS service advertisement and browsing (`_aissistant._udp.local.`) with automatic IPv4/IPv6 endpoint resolution.

### Phase 6.2: QUIC Transport & Mutual Pairing Ceremony (`S6.P2`)
- **`S6.P2.1` — QUIC/TLS 1.3 Mutual-Authenticated Transport (`quinn` + `rustls`)**
  - *Dependencies:* `S6.P1.2`
  - *Deliverable:* Implement custom `rustls` certificate verifier enforcing pinned Ed25519 peer identities, connection keep-alives, and automatic IP roaming recovery.
- **`S6.P2.2` — Out-of-Band SAS / QR Code Pairing Ceremony & Device Management UI**
  - *Dependencies:* `S6.P2.1`
  - *Deliverable:* Implement TLS exporter 6-digit SAS + QR pairing protocol with brute-force rate limiting and Flutter Device Pairing & ACL Management UI.

---

## Stage 7: Secure Cross-Device Control, Transfer & Streaming
*Goal: Execute remote skills, synchronize clipboards, transfer large files with resumption, and stream live screens across paired devices.*

### Phase 7.1: Cross-Device Skill Dispatch & Clipboard Sync (`S7.P1`)
- **`S7.P1.1` — Authenticated Remote Skill RPC & Target Peer Policy Enforcement**
  - *Dependencies:* `S6.P2.2`
  - *Deliverable:* Route natural-language or structured skill commands to paired devices over QUIC with mutual audit logging and target-side `PolicyEngine` verification.
- **`S7.P1.2` — End-to-End Encrypted Clipboard Synchronization**
  - *Dependencies:* `S7.P1.1`
  - *Deliverable:* Implement on-demand and opt-in auto-sync of text/image clipboard payloads across paired peers with loop prevention and size caps.

### Phase 7.2: Resumable File Transfer & Screen Streaming (`S7.P2`)
- **`S7.P2.1` — Chunked, BLAKE3/SHA-256 Verified Resumable File Transfer**
  - *Dependencies:* `S7.P1.2`
  - *Deliverable:* Implement parallel QUIC stream file transfers with 1 MB chunking, per-chunk cryptographic verification, interruption resume, and UI transfer manager.
- **`S7.P2.2` — Low-Latency Cross-Device Screen Capture & Streaming**
  - *Dependencies:* `S7.P2.1`
  - *Deliverable:* Implement native screen capture adapters (WGC, PipeWire Portal, ScreenCaptureKit, Android MediaProjection, iOS ReplayKit) streaming compressed frames over QUIC to the Flutter viewer.
- **`S7.P2.3` — Permission-Gated Remote Input Control & Session Overlay**
  - *Dependencies:* `S7.P2.2`
  - *Deliverable:* Implement opt-in remote pointer/keyboard input dispatch on supported host platforms with a mandatory "Remote Control Active — Press Esc/Stop to Terminate" host overlay.

---

## Stage 8: Vision, Knowledge, Translation & Advanced Intelligence
*Goal: Add offline document RAG, local vision/OCR screen understanding, and offline multilingual translation.*

### Phase 8.1: Personal Knowledge Base & Local Document RAG (`S8.P1`)
- **`S8.P1.1` — Local Document Ingestion, Chunking & Indexing Pipeline**
  - *Dependencies:* `S7.P2.3`
  - *Deliverable:* Ingest Markdown, TXT, PDF, and source code files into semantic chunks indexed in SQLite FTS5 + vector store with incremental file-hash change detection.
- **`S8.P1.2` — Citation-Grounded Offline RAG & Knowledge Explorer UI**
  - *Dependencies:* `S8.P1.1`
  - *Deliverable:* Augment orchestrator prompts with source-cited document chunks and provide a Knowledge Explorer UI for managing indexed folders and inspecting citations.

### Phase 8.2: Local Vision, OCR & Offline Translation (`S8.P2`)
- **`S8.P2.1` — Offline OCR & Multimodal Vision Model Integration**
  - *Dependencies:* `S8.P1.2`
  - *Deliverable:* Integrate local ONNX OCR and multimodal vision-language GGUF (`llava`/`qwen2-vl`/`minicpm-v` via `llama.cpp`) for offline screenshot, camera, and document analysis.
- **`S8.P2.2` — Offline Multilingual Translation & Live Voice Interpreter Mode**
  - *Dependencies:* `S8.P2.1`
  - *Deliverable:* Implement offline text and bidirectional voice-to-voice translation across supported language pairs using local STT + LLM/NMT + TTS.

---

## Stage 9: Personalization, Automation & Smart-Home Integrations
*Goal: Enable user persona customization, scheduled/event-driven multi-device routines, and local-first smart-home control.*

### Phase 9.1: Personalization & Autonomous Routine Engine (`S9.P1`)
- **`S9.P1.1` — User Persona, Voice Profile & Explicit Preference Manager**
  - *Dependencies:* `S8.P2.2`
  - *Deliverable:* Allow customizable assistant name, wake phrases, response verbosity, preferred language, and inspectable personal fact memory.
- **`S9.P1.2` — Cron, Event-Triggered & Cross-Device Workflow Automation Engine**
  - *Dependencies:* `S9.P1.1`
  - *Deliverable:* Implement persistent automation scheduler supporting time triggers, device events (battery, network, peer connected), and multi-step local/remote skill chains.

### Phase 9.2: Local Smart-Home & Webhook/MQTT Bridge (`S9.P2`)
- **`S9.P2.1` — Local MQTT & Home Assistant / Matter-Bridge Offline Integration**
  - *Dependencies:* `S9.P1.2`
  - *Deliverable:* Implement local-network MQTT client and Home Assistant local REST/WebSocket skill adapters for offline smart-home device discovery and control.
- **`S9.P2.2` — Automation Builder UI & Execution History Inspector**
  - *Dependencies:* `S9.P2.1`
  - *Deliverable:* Build visual Routine & Smart-Home management screens in Flutter with dry-run testing and execution logs.

---

## Stage 10: Security, Performance, Compatibility & Reliability
*Goal: Harden all subsystems against adversarial inputs, network chaos, memory pressure, and latency regressions.*

### Phase 10.1: Security Hardening & Adversarial Auditing (`S10.P1`)
- **`S10.P1.1` — Comprehensive Prompt-Injection, Sandbox Escape & Fuzzing Suite**
  - *Dependencies:* `S9.P2.2`
  - *Deliverable:* Execute automated fuzzing (`cargo-fuzz`/proptest) on GGUF/ONNX parsers, QUIC wire frames, path canonicalization, and prompt-injection taint barriers.
- **`S10.P1.2` — Cryptographic Key Rotation, Revocation & Emergency Lockdown**
  - *Dependencies:* `S10.P1.1`
  - *Deliverable:* Implement one-click peer revocation, device certificate rotation, encrypted backup/restore, and emergency panic-lock clearing active sessions and memory caches.

### Phase 10.2: Performance Optimization & Fault-Recovery Soak Testing (`S10.P2`)
- **`S10.P2.1` — End-to-End Latency & Memory Footprint Optimization**
  - *Dependencies:* `S10.P1.2`
  - *Deliverable:* Profile and optimize cold start, wake-word CPU usage, STT/TTS latency, and idle RAM footprint across desktop and mobile targets to meet all `NFR-1`..`NFR-8` targets.
- **`S10.P2.2` — Network Chaos, Sleep/Wake & Long-Running Soak Verification**
  - *Dependencies:* `S10.P2.1`
  - *Deliverable:* Verify deterministic recovery across OS sleep/hibernate cycles, Wi-Fi-to-hotspot transitions, corrupt SQLite WAL recovery, and 1,000-turn continuous stress tests.

---

## Stage 11: Final Certification, Distribution & Releases
*Goal: Certify native builds across all supported platforms, package installers, and publish reproducible open-source release artifacts.*

### Phase 11.1: Native Platform Packaging & Installer Generation (`S11.P1`)
- **`S11.P1.1` — Desktop Release Packaging (Windows MSIX/Installer, Linux AppImage/DEB, macOS DMG)**
  - *Dependencies:* `S10.P2.2`
  - *Deliverable:* Build reproducible desktop release packages bundling the Rust core shared libraries and default lightweight wake-word/VAD assets.
- **`S11.P1.2` — Mobile Release Packaging (Android APK/AAB & iOS IPA Bundle)**
  - *Dependencies:* `S11.P1.1`
  - *Deliverable:* Configure mobile release build pipelines, ProGuard/R8 rules, iOS framework embedding, and store-compliant permission disclosures.

### Phase 11.2: Final QA Certification & Release Publication (`S11.P2`)
- **`S11.P2.1` — Full Acceptance Matrix Verification, SBOM & SHA-256 Manifests**
  - *Dependencies:* `S11.P1.2`
  - *Deliverable:* Run full regression, offline, security, and performance certification suites; generate SPDX/CycloneDX SBOM and `SHA256SUMS.txt`.
- **`S11.P2.2` — v1.0.0 Production Release Tagging & Final Documentation Sign-Off**
  - *Dependencies:* `S11.P2.1`
  - *Deliverable:* Finalize release notes, user/admin documentation, platform certification badges, and tag `v1.0.0`.
