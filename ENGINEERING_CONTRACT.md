# ENGINEERING CONTRACT

**Status:** Binding Project-Wide Engineering & Quality Contract  
**Scope:** All source code, native integrations, AI runtimes, build pipelines, documentation, and agent workflows in the Universal AI Assistant repository.

---

## 1. Core Mandate

The Universal AI Assistant is a production-grade, open-source, offline-first, voice-first personal AI ecosystem targeting **Android, Windows, Linux, macOS, and iOS**.

1. **Offline-First & Zero Mandatory Cloud:** Every core capability (wake word, STT, TTS, LLM inference, memory, local automation, and LAN device collaboration) must operate 100% offline with free local models and zero mandatory paid APIs or proprietary cloud services.
2. **Real Implementations Only:** No mock production modules, no simulated success paths, no placeholder UI screens, no dead buttons, and no hardcoded demonstration outputs.
3. **Evidence-Backed Completion:** No subphase, feature, or platform build may be marked `COMPLETED` without reproducible build and test execution logs.

---

## 2. Technology Stack & Architectural Boundaries

| Layer | Technology | Responsibility |
| :--- | :--- | :--- |
| **Shared UI & App Shell** | Flutter (Dart 3+) | Cross-platform reactive UI, accessibility, settings, permission prompts, visual feedback, and stream rendering. |
| **Shared Core Engine** | Rust (Stable, Edition 2021/2024) | AI orchestration, model lifecycle, audio pipeline routing, SQLite persistence, cryptography, QUIC/mDNS networking, skill sandbox, and FFI bridge (`flutter_rust_bridge` / C-ABI). |
| **Local LLM Runtime** | `llama.cpp` (via Rust FFI bindings) | GGUF model loading, hardware-aware acceleration (CPU SIMD, Vulkan, Metal, CUDA, OpenCL/NPU where verified), token streaming, and grammar-constrained tool calling. |
| **Offline Speech Engine** | `sherpa-onnx` (via Rust FFI bindings) | Offline wake-word spotting, streaming Speech-to-Text (STT), Voice Activity Detection (VAD), and offline neural Text-to-Speech (TTS). |
| **Persistence & Memory** | SQLite (WAL mode, FTS5, vector index, SQLCipher/AEAD encryption for sensitive stores) | Conversation history, episodic/semantic personal memory, audit logs, device registry, and automation schedules. |
| **Native Platform Bridges** | Kotlin (Android), Swift (iOS/macOS), Win32/WinRT (Windows), D-Bus/XDG/PipeWire/Wayland/X11 (Linux) | OS-level permissions, foreground/background services, audio session management, system tray/overlay, accessibility/window automation, and media controls. |
| **Cross-Device Transport** | Rust `quinn` (QUIC + TLS 1.3 / Noise) + mDNS-SD | Mutual-authenticated, end-to-end encrypted peer discovery, remote skill dispatch, clipboard sync, chunked file transfer, and low-latency screen streaming. |

---

## 3. Code Quality & Safety Standards

### 3.1 Rust Core Standards
- **Compiler & Linter Gates:** `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` must pass with zero warnings.
- **Unsafe Code Policy:** `unsafe` blocks are strictly restricted to FFI boundaries (`llama.cpp`, `sherpa-onnx`, OS native APIs). Every `unsafe` block MUST carry a `// SAFETY:` comment explaining the invariants upheld.
- **Error Handling:** `.unwrap()` and `.expect()` are forbidden in production library and service code (permitted only in unit tests or statically provable invariants documented inline). Use `thiserror` for domain errors and explicit `Result<T, E>` propagation.
- **Concurrency & Resource Budgets:** All async tasks (`tokio`) must support cooperative cancellation (`CancellationToken`) and bounded channels (`mpsc::channel(capacity)`) to prevent unbounded memory growth.

### 3.2 Flutter & Dart Standards
- **Static Analysis:** `dart format --output=none --set-exit-if-changed .` and `flutter analyze --no-pub` must pass with zero issues.
- **UI Integrity:** Every interactive control must bind to a real state/command handler. Unavailable capabilities on a given OS must gracefully degrade with an explicit capability badge/explanation rather than failing silently or rendering non-functional controls.

### 3.3 Native Platform Standards (Kotlin / Swift / C++)
- Platform channels and FFI callbacks must never block the UI thread.
- Runtime permissions must be checked prior to every privileged operation and fail closed if revoked.

---

## 4. Security, Privacy & Trust Requirements

1. **Fail-Closed Permission Engine:** All AI skills and remote commands execute through a centralized policy engine (`PolicyDecision`: `Allow`, `RequireConfirmation`, `Deny`) backed by an append-only tamper-evident audit log.
2. **Prompt Injection Defense:** Untrusted content (files, clipboard, web/screen text, peer messages) must be demarcated as untrusted data and cannot escalate tool permissions or execute destructive actions without explicit user confirmation.
3. **Cryptographic Device Pairing:** Cross-device control requires mutual Ed25519 identity verification, Short Authentication String (SAS) / QR out-of-band verification, and TLS 1.3 (QUIC) with pinned peer certificates.
4. **Zero Secrets in VCS:** Never commit private keys, API tokens, signing certificates, or personal user data.
5. **Reproducible Dependencies:** Pin dependencies via `Cargo.lock` and `pubspec.lock`. Audit dependencies via `cargo deny` / `cargo audit`.

---

## 5. Quality Gates & Verification Metrics

Every subphase must satisfy the following gates before completion:

- **Automated Test Coverage:** Minimum **90% meaningful line/branch coverage** on newly introduced core logic; **100% passing rate** on all required test suites.
- **Verification Tiers:**
  1. **Unit Tests:** Pure logic, parsers, state machines, cryptographic primitives, and policy rules.
  2. **Integration Tests:** SQLite migrations, FFI round-trips, `llama.cpp` & `sherpa-onnx` inference pipelines, and local skill execution.
  3. **Resilience & Fault-Injection Tests:** Corrupt model files, out-of-memory conditions, microphone disconnect, network drop mid-transfer, and revoked OS permissions.
  4. **Security Tests:** Unauthorized peer rejection, path traversal prevention, command injection rejection, and capability boundary enforcement.
  5. **Platform Build Verification:** Native compilation and smoke verification on available host toolchains and CI runners; unverified platforms must remain explicitly marked `UNVERIFIED` or `PENDING_CI` until tested.

---

## 6. Mandatory Execution Protocol

1. **Single-Subphase Execution:** Agents may only implement **one subphase** per user confirmation cycle.
2. **Pre-Implementation Proposal:** Present objective, assumptions, dependencies, scope, implementation plan, and measurable acceptance criteria; then **STOP and wait for user approval**.
3. **Post-Implementation Evidence:** Update `EXECUTION_STATUS.md`, `TODO.md` (append/update without deleting history), and `CHANGELOG.md` (append-only) with exact test commands run, results, commit hashes, and known limitations; then **STOP and wait for user approval** before moving to the next subphase.
