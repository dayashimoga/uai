# Universal AI Assistant — Shared Multiplatform Flutter Client (`app/`)

This directory contains the production Flutter client application for the **Universal AI Assistant**, providing the shared user interface and native platform runners across **Windows, Linux, macOS, Android, and iOS**.

---

## 1. Multiplatform Architectural Overview

```
                      +-----------------------------------+
                      |     Shared Flutter Client (UI)     |
                      |   Material 3 / Adaptive Shell     |
                      +-----------------+-----------------+
                                        | (dart:ffi C-ABI)
                                        v
                      +-----------------------------------+
                      |   aissistant-ffi Native Bridge    |
                      |  (cdylib / staticlib / .so / dll) |
                      +-----------------+-----------------+
                                        |
     +--------------------+-------------+-------------+--------------------+
     |                    |                           |                    |
     v                    v                           v                    v
aissistant-core   aissistant-security         aissistant-storage    aissistant-audio
(Domain / Probes) (Policy / AEAD / Audit)    (SQLite WAL / Keys)   (Ring Buffer / KWS)
```

### Supported Native Runners
| Platform | Target Architecture | Runner Directory | Permission & Sandbox Model |
| :--- | :--- | :--- | :--- |
| **Windows** | x86_64, aarch64 | `app/windows/` | Win32 desktop runner, CMake dynamic library bundling (`aissistant_ffi.dll`). |
| **Linux** | x86_64, aarch64 | `app/linux/` | GTK+ 3 desktop runner, CMake dynamic library bundling (`libaissistant_ffi.so`). |
| **macOS** | x86_64, aarch64 (Universal) | `app/macos/` | AppKit desktop runner, App Sandbox entitlements (microphone, LAN network server/client). |
| **Android** | arm64-v8a, armeabi-v7a, x86_64 | `app/android/` | MinSdk 24 (Android 7.0+), TargetSdk 34, foreground service & microphone permissions. |
| **iOS** | arm64, x86_64 simulator | `app/ios/` | iOS 14.0+, `Info.plist` microphone & local network privacy strings, background audio mode. |

---

## 2. Platform Permissions & Offline Security

In accordance with [`SECURITY.md`](../SECURITY.md), no platform runner requests cloud-telemetry or unnecessary permissions:
- **Audio Capture (`RECORD_AUDIO`, `NSMicrophoneUsageDescription`):** Strictly used locally on-device for offline speech recognition, VAD, and wake-word spotting.
- **Local Network (`INTERNET`, `ACCESS_WIFI_STATE`, `NSLocalNetworkUsageDescription`):** Strictly utilized for mDNS local peer discovery and mutual-TLS/QUIC pairing over LAN. Never dials external telemetry servers.
- **Background Execution (`FOREGROUND_SERVICE`, `UIBackgroundModes: audio`):** Allows low-latency wake-word listening with active notification indicator.

---

## 3. Directory Layout

- `lib/core/` — FFI bridge bindings, reactive state controllers, and data models matching the Rust core schemas.
- `lib/ui/` — Adaptive shell (`NavigationRail` on desktop, `NavigationBar` on mobile) and system telemetry screens.
  - `screens/diagnostics_screen.dart` — Subsystem health grid, active task count, memory trimmer, and shutdown.
  - `screens/capabilities_screen.dart` — 5-platform capability matrix with tier badges and permission toggles.
  - `screens/audit_log_screen.dart` — BLAKE3 hash-chained audit viewer with live cryptographic verification.
  - `screens/settings_screen.dart` — Appearance theme selection and storage engine info.
- `test/` — Unit, widget, and platform configuration test suites.

---

## 4. Verification Commands

```bash
# Analyze Dart code for strict type conformance and lint compliance
flutter analyze

# Run all unit, widget, and platform test suites
flutter test

# Run desktop runner locally on Windows
flutter run -d windows
```
