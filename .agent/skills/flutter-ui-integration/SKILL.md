---
name: flutter-ui-integration
description: >-
  Use this skill when building or updating the shared Flutter application shell,
  voice interaction UI, model manager screens, device pairing dashboards,
  permission dialogs, or accessibility features across all five platforms.
---

# Flutter UI & Application Integration Skill

## Role & Ownership
Owns the `app/` Flutter application, state management, responsive adaptive layouts (mobile portrait/landscape and desktop multi-pane), accessibility semantics, and FFI stream consumption.

## Responsibilities
1. Build responsive, low-latency UI components for voice waveform visualization, streaming token output, tool confirmation modals, and device status panels.
2. Bind every UI action directly to Rust core FFI methods or verified platform channels — zero dead buttons or mock screens.
3. Query the Rust `CapabilityRegistry` at runtime to adapt UI controls gracefully when a platform capability or permission is unavailable.
4. Support full keyboard navigation, screen readers (TalkBack, VoiceOver, Narrator, Orca), and localization (`arb` resources).

## Boundaries
- Do not implement business logic, SQL queries, cryptographic operations, or direct network sockets in Dart.
- Never block the Flutter UI isolate with synchronous FFI calls.

## Verification & Review Checklist
- [ ] `dart format --output=none --set-exit-if-changed .` passes.
- [ ] `flutter analyze` reports zero issues.
- [ ] `flutter test` widget and integration tests pass.
- [ ] Every interactive widget has accessibility semantics and handles error/loading states explicitly.
