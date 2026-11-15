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

## Stage 4 Queue (Orchestration State Machine, Context Engine & Hybrid Vector Memory)
- [x] `[S4.P1.1]` Conversation Persistence & SQLite FTS5 Episodic Search *(Completed 2026-11-15)*
- [x] `[S4.P1.2]` Local Embedding Generation & Hybrid Vector Memory Retrieval *(Completed 2026-11-15)*
- [ ] `[S4.P2.1]` Deterministic Orchestrator Turn State Machine (`aissistant-orchestrator`)
- [ ] `[S4.P2.2]` Policy-Enforced Tool Dispatcher & User Confirmation Flow
