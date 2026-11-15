# CONTRIBUTING.md — Developer & Agent Contribution Guide

Welcome to the **Universal AI Assistant** (`aissistant`) open-source project. All human contributors and AI agents must follow the engineering standards defined in [`ENGINEERING_CONTRACT.md`](./ENGINEERING_CONTRACT.md), [`CLAUDE.md`](./CLAUDE.md), and [`AGENTS.md`](./AGENTS.md).

---

## 1. Core Contribution Principles

1. **Offline-First & Free Open-Source Models Only:** Never introduce a mandatory paid API or cloud service dependency.
2. **Zero Fake / Mock Production Code:** Do not submit stubbed modules, fake success paths, dead UI buttons, or hardcoded demo screens.
3. **One Subphase Per PR / Cycle:** Every change must map to a single roadmap subphase (`S{stage}.P{phase}.{subphase}`) or a tracked bug fix with regression tests.
4. **Evidence-Backed Verification:** Every contribution must pass formatting, linting, unit, integration, offline, and security tests with >=90% coverage on new core logic.

---

## 2. Development Workflow

1. **Setup Environment:** Follow [`docs/SETUP.md`](./docs/SETUP.md) to configure Rust, CMake, and Flutter for your host OS.
2. **Check Roadmap & Status:** Inspect [`ROADMAP.md`](./ROADMAP.md) and [`EXECUTION_STATUS.md`](./EXECUTION_STATUS.md) to identify the active subphase.
3. **Write Tests First:** Write unit, integration, and negative/error-path tests for your subphase's acceptance criteria.
4. **Run Local Quality Gates:**
   ```powershell
   # Rust Core checks
   cargo fmt --manifest-path core/Cargo.toml --all -- --check
   cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings
   cargo test --manifest-path core/Cargo.toml --workspace

   # Flutter App checks (once app/ is initialized in Stage 1)
   dart format --output=none --set-exit-if-changed app/
   flutter analyze app/
   flutter test app/
   ```
5. **Update Trackers:**
   - Update [`EXECUTION_STATUS.md`](./EXECUTION_STATUS.md) with test execution evidence.
   - Update [`TODO.md`](./TODO.md) without deleting historical items.
   - Append your changes to [`CHANGELOG.md`](./CHANGELOG.md).

---

## 3. Commit Message Convention

Use Conventional Commits scoped to the roadmap subphase:
- `feat(S0.P1.1): implement core domain IDs, config schema, and error taxonomy`
- `fix(S2.P2.1): handle audio device disconnect during active VAD segmentation`
- `sec(S6.P2.1): reject unpinned Ed25519 certificates during QUIC handshake`
- `docs(bootstrap): establish architecture, engineering contract, and roadmap`

---

## 4. Security & Licensing

- All contributions are licensed under the **Apache License 2.0** ([`LICENSE`](./LICENSE)).
- Never commit `.gguf` / `.onnx` model binaries, private keys, or personal data.
- Ensure all third-party Rust and Dart dependencies use permissive open-source licenses (`MIT`, `Apache-2.0`, `BSD-3-Clause`, `ISC`, `MPL-2.0`) verified by `cargo deny`.
