# Universal AI Assistant (`aissistant`)

[![Release](https://img.shields.io/badge/release-v1.0.0-blue.svg)](./RELEASE_NOTES.md)
[![License](https://img.shields.io/badge/license-Apache--2.0-green.svg)](./LICENSE)
[![Offline](https://img.shields.io/badge/offline-100%25%20autonomous-brightgreen.svg)](./REQUIREMENTS.md)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS%20%7C%20Android%20%7C%20iOS-orange.svg)](./docs/ACCEPTANCE_MATRIX.md)
[![SPDX SBOM](https://img.shields.io/badge/SBOM-SPDX%202.3-informational.svg)](./dist/sbom/unified-sbom.spdx.json)

A production-grade, open-source, **100% offline-first, voice-first personal AI assistant** and cryptographically secured cross-device automation platform for **Windows, Linux, macOS, Android, and iOS**.

---

## Key Capabilities

- **100% Offline Autonomy:** Operates completely on-device using local open-weight models (`llama.cpp` GGUF) and offline neural speech processing (`sherpa-onnx`). Zero cloud APIs, accounts, subscriptions, or telemetry.
- **Voice-First & Multilingual:** Continuous low-power wake-word detection with customizable phrases, Voice Activity Detection (VAD) with instant barge-in, streaming multilingual Speech-to-Text (STT), and neural Text-to-Speech (TTS).
- **Hardware-Aware Local AI:** Automatically profiles CPU SIMD (AVX2, AVX-512, ARM NEON), RAM, and GPU accelerators (Vulkan, Apple Metal, NVIDIA CUDA) to recommend, download, SHA-256 verify, and run optimal local models.
- **Permission-Controlled Skills & OS Automation:** Execute natural-language tasks across files, applications, system settings, and media playback, governed by a fail-closed 5-tier `PolicyEngine` and a tamper-evident BLAKE3 hash-chained audit log.
- **Secure Cross-Device Mesh & Screen Streaming:** Discover peers via local mDNS-SD, pair with mutual Ed25519 TLS 1.3 over QUIC (`quinn`) and 6-digit SAS / QR verification, dispatch remote skills, sync clipboards, transfer large files with chunk-level resume, and stream live screens.
- **Personal Knowledge & Memory:** Hybrid SQLite WAL storage combining FTS5 keyword search and local vector embeddings for private episodic memory and offline document RAG.

---

## Architecture Overview

| Layer | Technology | Location |
| :--- | :--- | :--- |
| **Shared UI Shell** | Flutter (Dart 3+) | `app/` |
| **Shared Core & Orchestrator** | Rust Workspace (11 Crates) | `core/crates/` |
| **Local LLM Engine** | `llama.cpp` (GGUF + GBNF Grammar Tool Calling) | `core/crates/aissistant-llm` |
| **Offline Speech Engine** | `sherpa-onnx` (Wake Word, VAD, STT, TTS) | `core/crates/aissistant-speech` |
| **Encrypted Persistence** | SQLite (WAL, FTS5, Vector Index, XChaCha20-Poly1305) | `core/crates/aissistant-storage` |
| **Cross-Device Transport** | QUIC (`quinn` + `rustls` Ed25519) + mDNS-SD | `core/crates/aissistant-network` |
| **Native OS Bridges** | Kotlin, Swift, Win32/WinRT, Linux D-Bus/Portals, macOS AppKit | `core/crates/aissistant-platform` & `app/{android,ios,windows,linux,macos}` |

---

## Quick Start

### Build & Run
```bash
# 1. Compile Core Rust Libraries
cargo build --manifest-path core/Cargo.toml --release

# 2. Run Comprehensive Test Suites (100% Green Required)
cargo test --manifest-path core/Cargo.toml --workspace
cd app && flutter test

# 3. Launch Application
flutter run -d windows  # or linux / macos / android / ios
```

---

## Project Documentation Map

- [`RELEASE_NOTES.md`](./RELEASE_NOTES.md) — Production v1.0.0 release notes, changelog, and benchmarks.
- [`docs/ACCEPTANCE_MATRIX.md`](./docs/ACCEPTANCE_MATRIX.md) — 30/30 requirement traceability matrix & verification certificates.
- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — Detailed system topology, component specs, and data flows.
- [`REQUIREMENTS.md`](./REQUIREMENTS.md) — Functional/non-functional requirements and 5-platform capability matrix.
- [`SECURITY.md`](./SECURITY.md) — Threat model (STRIDE), trust boundaries, permission tiers, and crypto design.
- [`ROADMAP.md`](./ROADMAP.md) — Master execution roadmap (Stages 0–11, phases, and numbered subphases).
- [`EXECUTION_STATUS.md`](./EXECUTION_STATUS.md) — Live subphase completion tracker and verification evidence.
- [`TESTING.md`](./TESTING.md) — Verification tiers, coverage requirements (>=90%), and fault-injection strategy.
- [`ENGINEERING_CONTRACT.md`](./ENGINEERING_CONTRACT.md) — Binding code quality, safety, and verification rules.
- [`docs/SETUP.md`](./docs/SETUP.md) — Developer toolchain setup and packaging guide.
- [`docs/USER_GUIDE.md`](./docs/USER_GUIDE.md) — End-user guide for voice, chat, models, RAG, and devices.
- [`CONTRIBUTING.md`](./CONTRIBUTING.md) — Contribution workflow and quality gates.

---

## License

Licensed under the [Apache License, Version 2.0](./LICENSE).
