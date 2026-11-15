# Universal AI Assistant (`aissistant`)

A production-grade, open-source, **offline-first, voice-first personal AI ecosystem** and secure cross-device automation platform for **Android, Windows, Linux, macOS, and iOS**.

---

## Key Capabilities

- **100% Offline Operation:** Runs entirely on your devices using free open-weight local models (`llama.cpp` GGUF) and offline neural speech processing (`sherpa-onnx`). Zero mandatory cloud APIs, subscriptions, or telemetry.
- **Voice-First & Multilingual:** Continuous low-power wake-word detection with customizable wake phrases, Voice Activity Detection (VAD) with instant barge-in, streaming multilingual Speech-to-Text (STT), and natural offline Text-to-Speech (TTS).
- **Hardware-Aware Local AI:** Automatically profiles CPU SIMD (AVX2, AVX-512, ARM NEON), RAM, and GPU accelerators (Vulkan, Apple Metal, NVIDIA CUDA) to recommend, download, SHA-256 verify, and run optimal local models.
- **Permission-Controlled Skills & OS Automation:** Execute natural-language tasks across files, applications, system settings, and media playback, governed by a fail-closed 5-tier `PolicyEngine` and a tamper-evident BLAKE3 hash-chained audit log.
- **Secure Cross-Device Control & Streaming:** Discover peers via local mDNS-SD, pair with mutual Ed25519 TLS 1.3 over QUIC and 6-digit SAS / QR verification, dispatch remote skills, sync clipboards, transfer large files with chunk-level resume, and stream live screens.
- **Personal Knowledge & Memory:** Hybrid SQLite WAL storage combining FTS5 keyword search and local vector embeddings for private episodic memory and offline document RAG.

---

## Architecture Overview

| Layer | Technology | Location |
| :--- | :--- | :--- |
| **Shared UI Shell** | Flutter (Dart 3+) | `app/` |
| **Shared Core & Orchestrator** | Rust Workspace | `core/crates/` |
| **Local LLM Engine** | `llama.cpp` (GGUF + GBNF Grammar Tool Calling) | `core/crates/aissistant-llm` |
| **Offline Speech Engine** | `sherpa-onnx` (Wake Word, VAD, STT, TTS) | `core/crates/aissistant-speech` |
| **Encrypted Persistence** | SQLite (WAL, FTS5, Vector Index, XChaCha20-Poly1305) | `core/crates/aissistant-storage` |
| **Cross-Device Transport** | QUIC (`quinn` + `rustls` Ed25519) + mDNS-SD | `core/crates/aissistant-network` |
| **Native OS Bridges** | Kotlin, Swift, Win32/WinRT, Linux D-Bus/Portals, macOS AppKit | `core/crates/aissistant-platform` & `app/{android,ios,windows,linux,macos}` |

---

## Project Documentation Map

- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — Detailed system topology, component specs, and data flows.
- [`REQUIREMENTS.md`](./REQUIREMENTS.md) — Functional/non-functional requirements and 5-platform capability matrix.
- [`SECURITY.md`](./SECURITY.md) — Threat model (STRIDE), trust boundaries, permission tiers, and crypto design.
- [`ROADMAP.md`](./ROADMAP.md) — Master execution roadmap (Stages 0–11, phases, and numbered subphases).
- [`EXECUTION_STATUS.md`](./EXECUTION_STATUS.md) — Live subphase completion tracker and verification evidence.
- [`TESTING.md`](./TESTING.md) — Verification tiers, coverage requirements (>=90%), and fault-injection strategy.
- [`ENGINEERING_CONTRACT.md`](./ENGINEERING_CONTRACT.md) — Binding code quality, safety, and verification rules.
- [`AGENTS.md`](./AGENTS.md) & [`CLAUDE.md`](./CLAUDE.md) — Agent governance, team skills, and surgical coding guidelines.
- [`docs/SETUP.md`](./docs/SETUP.md) — Developer toolchain setup instructions for Windows, Linux, macOS, Android, and iOS.
- [`docs/USER_GUIDE.md`](./docs/USER_GUIDE.md) — End-user setup, model management, voice configuration, and device pairing guide.
- [`CONTRIBUTING.md`](./CONTRIBUTING.md) — Contribution workflow and quality gates.

---

## License

Licensed under the [Apache License, Version 2.0](./LICENSE).
