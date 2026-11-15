---
name: desktop-native-integration
description: >-
  Use this skill when implementing Windows (Win32/WinRT), Linux (D-Bus,
  PipeWire, Wayland/X11, XDG), and macOS (AppKit, CoreAudio, Accessibility)
  native desktop integrations, system control, media, screen capture, and
  OS automation.
---

# Windows, Linux & macOS Native Integration Skill

## Role & Ownership
Owns desktop platform adapters in `core/crates/aissistant-platform/` and desktop shells (`app/windows/`, `app/linux/`, `app/macos/`).

## Responsibilities
1. **Windows:** Win32/WinRT APIs for window enumeration, application launch, WASAPI audio, Windows Graphics Capture (screen capture), clipboard, media transport (`GlobalSystemMediaTransportControlsSessionManager`), DPAPI/Credential Locker, and system tray/global hotkeys.
2. **Linux:** D-Bus (`MPRIS2` media, `org.freedesktop.portal.Desktop` for Wayland screen capture/input/file chooser, `SecretService` keyring), PipeWire/ALSA audio, and X11/Wayland capability detection.
3. **macOS:** AppKit/Cocoa, CoreAudio, ScreenCaptureKit, Accessibility (`AXUIElement` with explicit TCC permission checks), Keychain Services, and Menu Bar extras.

## Boundaries
- Always verify TCC/Portal/OS permissions before invoking privileged desktop APIs.
- Never assume X11 on Linux; detect Wayland vs X11 at runtime and use xdg-desktop-portal where appropriate.

## Verification & Review Checklist
- [ ] Capability detection accurately reports available desktop APIs per OS/session type.
- [ ] All OS resource handles (HDC, HWND, D-Bus connections, CFTypeRef) are deterministically freed via RAII wrappers.
