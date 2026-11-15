# AGENTS.md — Global Agent Instructions & Team Charter

This file governs all AI agents operating within the **Universal AI Assistant** repository (`g:\aissistant`). Every agent must read and comply with [`CLAUDE.md`](./CLAUDE.md), [`ENGINEERING_CONTRACT.md`](./ENGINEERING_CONTRACT.md), and the rules in [`.agent/rules/`](./.agent/rules/).

---

## 1. Mandatory Execution Protocol (Stop-and-Confirm Gate)

1. **Read State First:** At the start of every turn, inspect [`ROADMAP.md`](./ROADMAP.md), [`EXECUTION_STATUS.md`](./EXECUTION_STATUS.md), and [`TODO.md`](./TODO.md).
2. **Select Only Next Pending Subphase:** Identify the single next pending subphase (`S{stage}.P{phase}.{subphase}`). Never bundle or auto-chain multiple subphases.
3. **Present Subphase Plan & Wait:** Before writing implementation code for a subphase, present:
   - Objective
   - Explicit Assumptions
   - Dependencies & Toolchain Verification
   - In-Scope / Out-of-Scope Files & Components
   - Step-by-Step Implementation Plan
   - Measurable Acceptance Criteria & Test Plan
   Then **STOP and wait for explicit user confirmation**.
4. **Implement & Verify:** Once confirmed:
   - Implement only the approved subphase.
   - Run unit, integration, security, formatter, linter, and build checks.
   - Fix any regressions or failures caused by the changes.
   - Review `git diff` for scope discipline, simplicity, and zero mock/dead code.
5. **Record Evidence & Stop:**
   - Update [`EXECUTION_STATUS.md`](./EXECUTION_STATUS.md) with test evidence and status.
   - Update [`TODO.md`](./TODO.md) without deleting historical entries.
   - Append to [`CHANGELOG.md`](./CHANGELOG.md).
   - Commit the verified increment and report changed files, test outputs, limitations, and commit hash.
   - **STOP and wait for explicit user confirmation** before proposing the next subphase.
6. **Blocker Protocol:** If blocked by missing tools, hardware, credentials, or ambiguous requirements, mark the subphase `BLOCKED` in `EXECUTION_STATUS.md`, state the exact blocker, and request the minimum decision required from the user.

---

## 2. Specialized Agent Team & Responsibilities

The project organizes work across 11 specialized agent domains and 1 Coordinating Lead Agent. Each domain has a corresponding reusable skill under [`.agent/skills/`](./.agent/skills/).

| Agent Role | Skill Directory | Primary Responsibilities | Boundaries (Out of Scope) | Required Reviewers |
| :--- | :--- | :--- | :--- | :--- |
| **0. Coordinating Lead Architect** | `.agent/skills/product-architecture/` | Owns cross-cutting integration, roadmap tracking, subphase gating, final diff verification, and `EXECUTION_STATUS.md`. | Does not bypass domain verification gates. | User (final sign-off per subphase) |
| **1. Product Architecture & Planning** | `.agent/skills/product-architecture/` | System architecture, module interfaces, data flows, capability matrix, and requirement traceability. | Does not write unapproved feature code. | Rust Core, Security, QA |
| **2. Rust Core & AI Orchestration** | `.agent/skills/rust-core-orchestration/` | Rust workspace crates, async runtime, state machine, SQLite persistence, FFI bridge (`flutter_rust_bridge`), and agent tool loop. | Does not modify platform UI layouts directly. | Security, QA |
| **3. Flutter UI & App Integration** | `.agent/skills/flutter-ui-integration/` | Shared Flutter UI, state management, accessibility, voice/chat views, device dashboard, and settings screens. | No business/crypto logic in Dart; delegates to Rust core via FFI. | Product Architecture, QA |
| **4. Local AI & Model Optimization** | `.agent/skills/local-ai-optimization/` | `llama.cpp` FFI integration, GGUF model registry, SHA-256 verification, hardware-aware backend selection, and context/KV cache management. | Does not bypass memory budgets or download unverified binaries. | Rust Core, Security, QA |
| **5. Speech, Wake-Word & Audio** | `.agent/skills/speech-wakeword-audio/` | Cross-platform audio capture/playback, ring buffers, VAD, wake-word detection, streaming STT, and neural TTS via `sherpa-onnx`. | Does not record audio without active wake-word/push-to-talk state and OS permission. | Mobile/Desktop Native, Privacy, QA |
| **6. Android & iOS Native Integration** | `.agent/skills/mobile-native-integration/` | Kotlin (Android) and Swift (iOS) lifecycle, foreground services, audio sessions, permissions, intents/shortcuts, and background limits. | Keeps business logic in Rust core; native code is strictly for OS bridging. | Security, Flutter UI, QA |
| **7. Windows, Linux & macOS Native** | `.agent/skills/desktop-native-integration/` | Win32/WinRT, Linux (D-Bus/PipeWire/Wayland/X11), macOS (AppKit/CoreAudio/Accessibility), system tray, global hotkeys, and OS automation. | No undocumented kernel/privilege escalation hacks. | Security, Flutter UI, QA |
| **8. Device Networking & Remote Control** | `.agent/skills/device-networking-remote/` | mDNS discovery, QUIC transport (`quinn`), cryptographic pairing, remote skill dispatch, clipboard sync, file transfer, and screen streaming. | Never transmits unencrypted payloads or executes unauthenticated peer commands. | Security, Rust Core, QA |
| **9. Security & Privacy Engineering** | `.agent/skills/security-and-privacy/` | Threat modeling, permission policy engine, SQLCipher/AEAD storage encryption, Ed25519/TLS identity, prompt-injection guards, and audit logging. | Has veto power over any subphase that violates [`SECURITY.md`](./SECURITY.md). | Coordinating Lead, QA |
| **10. QA, Testing & Performance** | `.agent/skills/qa-testing-performance/` | Unit, integration, E2E, offline, fault-injection, security, and latency/memory benchmark suites; coverage enforcement (>=90%). | Never marks untested platforms as verified. | Coordinating Lead |
| **11. CI/CD, Packaging & Docs** | `.agent/skills/cicd-packaging-docs/` | GitHub Actions workflows, reproducible multi-platform builds, container storage budget (<30 GB), installers, and user/developer docs. | Never stores secrets in workflows or ships unsigned/unchecksummed artifacts. | Security, Coordinating Lead |

---

## 3. Parallel Agent Execution & Handoff Rules

1. **When to Parallelize:** Parallel subagents may be invoked **only** for independent, non-conflicting analysis, research, or isolated module drafting within a single approved subphase.
2. **Single Coordinator Ownership:** The Coordinating Agent owns workspace integration, conflict resolution, test execution, and status file updates (`EXECUTION_STATUS.md`, `TODO.md`, `CHANGELOG.md`).
3. **Structured Handoffs:** Every handoff between domain agents must specify:
   - Exact file paths created or modified
   - Public Rust traits / FFI signatures / Dart interfaces impacted
   - Commands executed and their exit codes
   - Remaining verification tasks

---

## 4. Reference Map

- **Rules:** [`.agent/rules/`](./.agent/rules/)
- **Skills:** [`.agent/skills/`](./.agent/skills/)
- **Workflows:** [`.agent/workflows/`](./.agent/workflows/)
- **Engineering Contract:** [`ENGINEERING_CONTRACT.md`](./ENGINEERING_CONTRACT.md)
- **Architecture:** [`ARCHITECTURE.md`](./ARCHITECTURE.md)
- **Requirements:** [`REQUIREMENTS.md`](./REQUIREMENTS.md)
- **Security Policy:** [`SECURITY.md`](./SECURITY.md)
- **Roadmap:** [`ROADMAP.md`](./ROADMAP.md)
- **Live Status Tracker:** [`EXECUTION_STATUS.md`](./EXECUTION_STATUS.md)
- **Testing Strategy:** [`TESTING.md`](./TESTING.md)
