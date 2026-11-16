# TODO.md — Persistent Project Task Log

*Policy: Append and update tasks without deleting historical entries. Every task traces to a roadmap subphase or governance requirement.*

---

## [2026-11-15] Project Bootstrap (`BOOTSTRAP`)
- [x] `[BOOTSTRAP-01]` Inspect workspace (`g:\aissistant`), host OS capabilities, and installed toolchains (`git`, `rustc`, `cargo`, `cmake`, `python`).
- [x] `[BOOTSTRAP-02]` Initialize local Git repository on `main` branch.
- [x] `[BOOTSTRAP-03]` Establish `AGENTS.md`, `CLAUDE.md`, and `ENGINEERING_CONTRACT.md`.
- [x] `[BOOTSTRAP-04]` Create `.agent/rules/` (`01-execution-protocol.md` through `05-agent-collaboration.md`).
- [x] `[BOOTSTRAP-05]` Create 11 specialized reusable skills under `.agent/skills/`.
- [x] `[BOOTSTRAP-06]` Create 4 execution, review, testing, and release workflows under `.agent/workflows/`.
- [x] `[BOOTSTRAP-07]` Author `ARCHITECTURE.md`, `REQUIREMENTS.md`, `SECURITY.md`, and `TESTING.md`.
- [x] `[BOOTSTRAP-08]` Author `ROADMAP.md` (Stages 0–11), `EXECUTION_STATUS.md`, `TODO.md`, and `CHANGELOG.md`.
- [x] `[BOOTSTRAP-09]` Author `CONTRIBUTING.md`, `README.md`, `docs/SETUP.md`, and `docs/USER_GUIDE.md`.
- [x] `[BOOTSTRAP-10]` Create GitHub Actions CI/CD workflows (`.github/workflows/ci-rust.yml`, `ci-flutter.yml`, `security-audit.yml`, `release.yml`).

---

## Environment & Remote Repository Tasks
- [ ] `[ENV-01]` Connect local Git repository (`g:\aissistant`) to remote GitHub repository (requires user remote URL or `gh` CLI installation & auth).
- [x] `[ENV-02]` Verify Flutter SDK (`flutter` / `dart`) on host: verified at `C:\flutter\bin\flutter.bat` (Flutter 3.44.6, Dart 3.12.2).

---

## Stage 0 Queue (Completed)
- [x] `[S0.P1.1]` Initialize `core/` Cargo workspace and implement `aissistant-core` domain IDs, error taxonomy (`CoreError`), and configuration schemas with >=90% test coverage. *(Completed 2026-11-15)*
- [x] `[S0.P1.2]` Implement `CapabilityRegistry`, 5-platform capability matrix, and host runtime probe in `aissistant-core`. *(Completed 2026-11-15)*
- [x] `[S0.P2.1]` Implement `aissistant-security` `PolicyEngine`, risk tiers, and untrusted input taint tracking. *(Completed 2026-11-15)*
- [x] `[S0.P2.2]` Implement Ed25519 device identity keys, XChaCha20-Poly1305 AEAD helpers, and BLAKE3 hash-chained `AuditLogEntry` verification. *(Completed 2026-11-15)*

---

## Stage 1 Queue (Completed — 100%)
- [x] `[S1.P1.1]` Create `aissistant-storage` crate with SQLite WAL connection pool, deterministic migration runner, settings store, and persistent hash-chained audit log table. *(Completed 2026-11-15)*
- [x] `[S1.P1.2]` Integrate OS-native keystore adapters (Windows DPAPI, macOS/iOS Keychain, Linux SecretService, Android Keystore fallback abstraction) to seal the master AEAD key. *(Completed 2026-11-15)*
- [x] `[S1.P2.1]` Create `aissistant-ffi` crate exposing system health, capability matrix, audit log inspection, and event streams. *(Completed 2026-11-15)*
- [x] `[S1.P2.2]` Implement `CoreRuntime` lifecycle manager handling initialization, background task supervision, and clean shutdown across FFI. *(Completed 2026-11-15)*
- [x] `[S1.P3.1]` Create `app/` Flutter project with adaptive navigation, accessibility semantics, and live binding to `aissistant-ffi`. *(Completed 2026-11-15)*
- [x] `[S1.P3.2]` Configure Windows, Linux, macOS, Android, and iOS native runner manifests, permissions declarations, and capability status views. *(Completed 2026-11-15)*

---

## Stage 2 Queue (Completed — 100%)
- [x] `[S2.P1.1]` Real-Time Safe Audio Ring Buffer & Resampler (`aissistant-audio`) *(Completed 2026-11-15)*
- [x] `[S2.P1.2]` Cross-Platform Audio Capture & Playback Engine *(Completed 2026-11-15)*
- [x] `[S2.P2.1]` Offline VAD & Barge-In Detector (`sherpa-onnx` / Silero VAD) *(Completed 2026-11-15)*
- [x] `[S2.P2.2]` Customizable Offline Wake-Word Engine (`sherpa-onnx` KWS) *(Completed 2026-11-15)*
- [x] `[S2.P3.1]` Offline Multilingual Streaming Speech-to-Text *(Completed 2026-11-15)*
- [x] `[S2.P3.2]` Offline Neural Text-to-Speech & Sentence Streaming *(Completed 2026-11-15)*
- [x] `[S2.P3.3]` Voice UI Integration, Waveform Feedback & Push-to-Talk/Wake Mode *(Completed 2026-11-15)*

---

## Stage 3 Queue (Local LLM Runtime & Model Management)
- [x] `[S3.P1.1]` CPU/RAM/GPU Hardware Profiler (`aissistant-llm`) *(Completed 2026-11-15)*
- [x] `[S3.P1.2]` Resumable Model Downloader, SHA-256 Verifier & GGUF Inspector *(Completed 2026-11-15)*
- [x] `[S3.P2.1]` `llama.cpp` Rust Bindings, Session Management & Token Streaming *(Completed 2026-11-15)*
- [x] `[S3.P2.2]` GBNF Grammar Compiler for Structured JSON Tool Calling *(Completed 2026-11-15)*
- [x] `[S3.P2.3]` Flutter Model Manager UI & Live Benchmark Diagnostics *(Completed 2026-11-15)*

---

## Stage 4 Queue (Completed — 100%)
- [x] `[S4.P1.1]` Conversation Persistence & SQLite FTS5 Episodic Search *(Completed 2026-11-15)*
- [x] `[S4.P1.2]` Local Embedding Generation & Hybrid Vector Memory Retrieval *(Completed 2026-11-15)*
- [x] `[S4.P2.1]` Deterministic Orchestrator Turn State Machine (`aissistant-orchestrator`) *(Completed 2026-11-15)*
- [x] `[S4.P2.2]` Policy-Enforced Tool Dispatcher & User Confirmation Flow *(Completed 2026-11-15)*

---

## Stage 5 Queue (Skill Framework & Native Application/System Control)
- [x] `[S5.P1.1]` Skill Registry, Sandbox Validator & Built-In Utility Skills *(Completed 2026-11-15)*
- [x] `[S5.P1.2]` Safe File Management Skills (Search, Read, Write, Move, Archive, Checksum) *(Completed 2026-11-15)*
- [x] `[S5.P2.1]` Desktop Application Launch, Process Inspection & Window Management *(Completed 2026-11-15)*
- [x] `[S5.P2.2]` Desktop System Settings, Clipboard, Audio Volume & Media Transport Control *(Completed 2026-11-15)*
- [x] `[S5.P3.1]` Android Native Bridge (Foreground Service, Intents, Media, Accessibility) *(Completed 2026-11-15)*
- [x] `[S5.P3.2]` iOS Native Bridge (AVAudioSession, App Intents/Siri Shortcuts, Scoped Files) *(Completed 2026-11-15)*

---

## Stage 6 Queue (Device Agents, Pairing & Device Registry)
- [x] `[S6.P1.1]` Persistent Device Registry & Peer ACL Store *(Completed 2026-11-15)*
- [x] `[S6.P1.2]` mDNS-SD Local Network Discovery (`aissistant-network`) *(Completed 2026-11-15)*
- [x] `[S6.P2.1]` QUIC/TLS 1.3 Mutual-Authenticated Transport (`quinn` + `rustls`) *(Completed 2026-11-15)*
- [x] `[S6.P2.2]` Out-of-Band SAS / QR Code Pairing Ceremony & Device Management UI *(Completed 2026-11-15)*

---

## Stage 7 Queue (Remote Skills, Clipboard Sync, Screen Streaming & File Transfer)
- [x] `[S7.P1.1]` Authenticated Remote Skill RPC & Target Peer Policy Enforcement *(Completed 2026-11-15)*
- [x] `[S7.P1.2]` End-to-End Encrypted Clipboard Synchronization *(Completed 2026-11-15)*
- [x] `[S7.P2.1]` Chunked, BLAKE3/SHA-256 Verified Resumable File Transfer *(Completed 2026-11-15)*
- [x] `[S7.P2.2]` Low-Latency Cross-Device Screen Capture & Streaming *(Completed 2026-11-15)*
- [x] `[S7.P2.3]` Permission-Gated Remote Input Control & Session Overlay *(Completed 2026-11-15)*

---

## Stage 8 Queue (Vision, Knowledge, Translation & Advanced Intelligence)
- [x] `[S8.P1.1]` Local Document Ingestion, Chunking & Indexing Pipeline *(Completed 2026-11-15)*
- [x] `[S8.P1.2]` Citation-Grounded Offline RAG & Knowledge Explorer UI *(Completed 2026-11-15)*
- [x] `[S8.P2.1]` Offline OCR & Multimodal Vision Model Integration *(Completed 2026-11-15)*
- [x] `[S8.P2.2]` Offline Multilingual Translation & Live Voice Interpreter Mode *(Completed 2026-11-15)*

---

## Stage 9 Queue (Personalization, Workflows & Proactive Automation)
- [x] `[S9.P1.1]` User Persona, Voice Profile & Explicit Preference Manager *(Completed 2026-11-15)*
- [x] `[S9.P1.2]` Cron, Event-Triggered & Cross-Device Workflow Automation Engine *(Completed 2026-11-16)*
- [x] `[S9.P2.1]` Local MQTT & Home Assistant / Matter-Bridge Offline Integration *(Completed 2026-11-16)*
- [x] `[S9.P2.2]` Automation Builder UI & Execution History Inspector *(Completed 2026-11-16)*

---

## Stage 10 Queue (Security Hardening, Performance & Reliability)
- [x] `[S10.P1.1]` Comprehensive Prompt-Injection, Sandbox Escape & Fuzzing Suite *(Completed 2026-11-16)*
- [x] `[S10.P1.2]` Cryptographic Key Rotation, Revocation & Emergency Lockdown *(Completed 2026-11-16)*
- [x] `[S10.P2.1]` End-to-End Latency & Memory Footprint Optimization *(Completed 2026-11-16)*
- [x] `[S10.P2.2]` Network Chaos, Sleep/Wake & Long-Running Soak Verification *(Completed 2026-11-16)*

---

## Stage 11 Queue (Packaging, Distribution & Final Certification)
- [x] `[S11.P1.1]` Desktop Release Packaging (Windows, Linux, macOS) *(Completed 2026-11-16)*
- [x] `[S11.P1.2]` Mobile Release Packaging (Android APK/AAB & iOS IPA) *(Completed 2026-11-16)*
- [x] `[S11.P2.1]` Full Acceptance Matrix Verification, SBOM & SHA-256 Manifests *(Completed 2026-11-16)*
- [x] `[S11.P2.2]` v1.0.0 Production Release Tagging & Final Documentation Sign-Off *(Completed 2026-11-16)*
