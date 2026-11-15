---
name: mobile-native-integration
description: >-
  Use this skill when implementing Android (Kotlin/JNI) and iOS (Swift/C-ABI)
  native platform services, permissions, foreground services, audio sessions,
  accessibility automation, and system intents.
---

# Android & iOS Native Integration Skill

## Role & Ownership
Owns Android (`app/android/`, Kotlin) and iOS (`app/ios/`, Swift) native bridge code, OS permissions, lifecycle hooks, and mobile system skills.

## Responsibilities
1. **Android (Kotlin):** Manage Foreground Services (`microphone` / `connectedDevice` types), wake locks, AudioFocus, runtime permissions, App Intents, MediaSession control, NotificationListener/AccessibilityService (opt-in with explicit disclosure), and battery optimization guidance.
2. **iOS (Swift):** Manage `AVAudioSession` categories (`.playAndRecord`, interruption handling), App Intents / Siri Shortcuts integration, background audio rules, Keychain integration, and sandboxed file access.
3. Expose explicit platform capability descriptors to the Rust core so unavailable iOS capabilities (e.g., arbitrary cross-app UI automation) degrade cleanly.

## Boundaries
- Never use private/undocumented iOS APIs or bypass Android runtime permission prompts.
- Keep native Kotlin/Swift layers thin; delegate orchestration and state to Rust core.

## Verification & Review Checklist
- [ ] Permission denial and revocation states handled gracefully without crash.
- [ ] Audio interruption (phone call, alarm) pauses and resumes cleanly.
- [ ] Native static analysis (`ktlint`/`detekt` on Android, `swiftlint`/Xcode build checks on iOS) passes.
