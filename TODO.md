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
- [ ] `[ENV-02]` Install or add Flutter SDK (`flutter` / `dart`) to `PATH` prior to Stage 1 Subphase `S1.P3.1`.

---

## Stage 0 Queue
- [x] `[S0.P1.1]` Initialize `core/` Cargo workspace and implement `aissistant-core` domain IDs, error taxonomy (`CoreError`), and configuration schemas with >=90% test coverage. *(Completed 2026-11-15)*
- [ ] `[S0.P1.2]` Implement `CapabilityRegistry`, 5-platform capability matrix, and host runtime probe in `aissistant-core`.
- [ ] `[S0.P2.1]` Implement `aissistant-security` `PolicyEngine`, risk tiers, and untrusted input taint tracking.
- [ ] `[S0.P2.2]` Implement Ed25519 device identity keys, XChaCha20-Poly1305 AEAD helpers, and BLAKE3 hash-chained `AuditLogEntry` verification.
