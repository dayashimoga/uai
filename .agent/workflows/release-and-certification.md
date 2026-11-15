---
description: Certify platform builds, security audits, checksums, and release artifacts for distribution
---

# Workflow: Release & Certification

Execute this workflow during Stage 11 or before publishing any tagged release (`vX.Y.Z`).

## 1. Pre-Release Verification Matrix
1. Confirm all prerequisite subphases in `EXECUTION_STATUS.md` are marked `COMPLETED` with verified evidence.
2. Run the full Rust and Flutter test suites across all supported targets.
3. Run dependency and security audits (`cargo audit`, `cargo deny check`).

## 2. Native Platform Build & Certification
Verify native builds per platform (locally or via native GitHub Actions runners):
- **Windows:** `flutter build windows --release` + Rust `x86_64-pc-windows-msvc` / `gnu` DLL.
- **Linux:** `flutter build linux --release` + Rust `x86_64-unknown-linux-gnu` `.so`.
- **macOS:** `flutter build macos --release` + Universal/Apple Silicon `.dylib`.
- **Android:** `flutter build apk --release` & `flutter build appbundle --release` + JNI `.so` (`arm64-v8a`, `x86_64`).
- **iOS:** `flutter build ipa --release --no-codesign` (or signed) + static `.a` / `.xcframework`.

*Rule:* Any platform not yet built and verified on a native runner or physical/emulator target must be explicitly labeled `UNVERIFIED` in release notes — never falsely certified.

## 3. Artifact Packaging & Integrity
1. Generate SHA-256 checksums (`SHA256SUMS.txt`) for all binaries, installers, and default model manifests.
2. Update `CHANGELOG.md` and `EXECUTION_STATUS.md` with release tag and build verification links.
