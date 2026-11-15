---
description: Execute formatting, linting, unit, integration, offline, security, and build checks
---

# Workflow: Testing & Quality Gates

Execute the applicable checks below for every subphase implementation and record their exact exit codes and summary counts in `EXECUTION_STATUS.md`.

## 1. Rust Core Quality Gates
```powershell
# Formatting check
cargo fmt --manifest-path core/Cargo.toml --all -- --check

# Strict linter check
cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings

# Unit, integration, and security test suites
cargo test --manifest-path core/Cargo.toml --workspace -- --nocapture
```

## 2. Flutter & Dart Quality Gates (When `app/` is present)
```powershell
# Dart formatting check
dart format --output=none --set-exit-if-changed app/

# Static analysis
flutter analyze app/

# Widget and unit tests
flutter test app/
```

## 3. Offline & Fault-Injection Verification
1. Verify all inference, STT/TTS, persistence, and local skill tests pass without external network calls.
2. Execute negative tests (corrupt GGUF/ONNX checksums, invalid SQLite inputs, unauthorized QUIC peers, revoked permissions).

## 4. Storage Budget Check
1. Check workspace/target directory disk usage (`Get-ChildItem -Recurse | Measure-Object -Property Length -Sum`) to ensure total usage remains well below the 30 GB budget.
