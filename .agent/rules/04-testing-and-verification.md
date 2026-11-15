# Rule 04: Testing, Coverage & Verification Gates

**Scope:** All subphases and CI/CD pipelines.

## 1. Mandatory Quality Gates
- **Test Coverage:** At least **90% meaningful automated test coverage** on newly added/modified core logic.
- **Pass Rate:** **100% passing** required tests (`cargo test`, `flutter test`, native integration suites) before marking any subphase `COMPLETED`.
- **Static Checks:** Zero warnings from `cargo fmt --check`, `cargo clippy -- -D warnings`, `dart format`, and `flutter analyze`.

## 2. Required Verification Dimensions
1. **Unit Tests:** State transitions, parsers, policy evaluation, crypto verification, and error paths.
2. **Integration Tests:** SQLite persistence, FFI boundary serialization, model loading, STT/TTS pipeline, and local skill execution.
3. **Offline & Fault-Injection Tests:** Verify full operation with network interfaces disabled, corrupt model files rejected via SHA-256 check, disconnected audio devices handled cleanly, and interrupted file transfers resumed or rolled back safely.
4. **Security Tests:** Unauthorized peer connection rejection, path traversal (`../`) blocks, and unauthorized tool invocation blocks.

## 3. Honest Platform Certification
- Never mark a target OS (Android, Windows, Linux, macOS, iOS) as `VERIFIED` or `CERTIFIED` unless its native build and test suite have actually executed and passed on that target (locally or on a native CI runner).
- If a host machine lacks a target SDK (e.g., Xcode/macOS/iOS on a Windows host), mark host-verifiable code as `HOST_VERIFIED` and target-specific native builds as `PENDING_CI_OR_TARGET_HOST`.
