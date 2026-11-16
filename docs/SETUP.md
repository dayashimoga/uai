# Developer Environment Setup Guide (`docs/SETUP.md`)

This guide covers setting up the build, verification, and packaging environment for the **Universal AI Assistant** across Windows, Linux, macOS, Android, and iOS targets, while respecting the **30 GB workspace/container storage budget**.

---

## 1. Required Core Toolchains (All Host Platforms)

1. **Git (2.40+):**
   ```powershell
   git --version
   ```
2. **Rust Toolchain (Stable 1.85+):**
   Install via [rustup.rs](https://rustup.rs):
   ```powershell
   rustup default stable
   rustup component add rustfmt clippy llvm-tools-preview
   ```
3. **CMake (3.24+) & C/C++ Compiler:**
   Required for building native C/C++ dependencies (`llama.cpp` and `sherpa-onnx`):
   - **Windows:** Visual Studio 2022 Build Tools (Desktop development with C++) or MinGW-w64 UCRT + CMake.
   - **Linux (Ubuntu/Debian):**
     ```bash
     sudo apt-get update && sudo apt-get install -y build-essential cmake clang pkg-config \
       libasound2-dev libpipewire-0.3-dev libdbus-1-dev libgtk-3-dev liblzma-dev
     ```
   - **macOS:** Xcode Command Line Tools (`xcode-select --install`) + `brew install cmake`.
4. **Flutter SDK (3.24+ / Dart 3.5+):**
   - **Windows:** Extract official Flutter SDK and add `flutter\bin` to `PATH`.
   - **Verify:** `flutter doctor -v`
5. **Python 3 (3.11+):**
   - Built-in `tomllib` used for deterministic SPDX 2.3 SBOM generation and checksum verification.

---

## 2. Target-Specific SDK Setup

### Android Target
- Install Android Studio or Android Command-Line Tools:
  - Android SDK Platform 34+
  - Android NDK (r26b+) and CMake
- Add Rust Android targets:
  ```powershell
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
  cargo install cargo-ndk
  ```

### iOS & macOS Targets (macOS Host Only)
- Install Xcode 15+ and CocoaPods (`brew install cocoapods`).
- Add Rust Apple targets:
  ```bash
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-darwin x86_64-apple-darwin
  ```

---

## 3. Building & Running Tests

### Rust Core Workspace (`core/`)
```powershell
# Format check
cargo fmt --manifest-path core/Cargo.toml --all -- --check

# Clippy lint check
cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets -- -D warnings

# Run all unit, integration, and security tests (422 tests passing)
cargo test --manifest-path core/Cargo.toml --workspace
```

### Flutter Application (`app/`)
```powershell
cd app
flutter pub get
dart format --output=none --set-exit-if-changed .
flutter analyze
flutter test  # 125 tests passing
```

---

## 4. Release Packaging & SBOM Generation

### Desktop Packages
- **Windows:**
  ```powershell
  & packaging/desktop/windows/package_windows.ps1 -Version 1.0.0
  ```
- **Linux:**
  ```bash
  chmod +x packaging/desktop/linux/*.sh
  ./packaging/desktop/linux/package_deb.sh 1.0.0 amd64
  ./packaging/desktop/linux/package_appimage.sh 1.0.0 x86_64
  ```
- **macOS:**
  ```bash
  chmod +x packaging/desktop/macos/*.sh
  ./packaging/desktop/macos/package_macos.sh 1.0.0
  ```

### Mobile Packages
- **Android:**
  ```bash
  chmod +x packaging/mobile/android/package_android.sh
  ./packaging/mobile/android/package_android.sh 1.0.0
  ```
- **iOS:**
  ```bash
  chmod +x packaging/mobile/ios/package_ios.sh
  ./packaging/mobile/ios/package_ios.sh 1.0.0
  ```

### SBOM & Release Checksums
```bash
# Generate SPDX 2.3 JSON Software Bill of Materials
python scripts/generate_sbom.py

# Aggregate and cryptographically audit release checksums
python scripts/verify_release_checksums.py --verify
```

---

## 5. Storage Budget Management (<30 GB Cap)

- Never commit `.gguf` or `.onnx` files to Git.
- Store downloaded test/development models in a shared user directory outside the build target tree or under `models/` (gitignored).
- Prune stale incremental build artifacts when switching targets:
  ```powershell
  cargo clean --manifest-path core/Cargo.toml
  ```
