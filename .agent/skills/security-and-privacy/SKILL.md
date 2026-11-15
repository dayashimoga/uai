---
name: security-and-privacy
description: >-
  Use this skill when designing or auditing threat models, permission policies,
  cryptographic storage, OS keychain integration, prompt injection defenses,
  sandboxing, or dependency vulnerability scans.
---

# Security & Privacy Engineering Skill

## Role & Ownership
Owns `SECURITY.md`, the `aissistant-security` Rust crate, the `PolicyEngine`, cryptographic key hierarchy, audit logging, and security verification suites.

## Responsibilities
1. Maintain the centralized `PolicyEngine` governing local and remote skill execution across risk tiers (`ReadOnly`, `StateModifying`, `PrivilegedSystem`, `Destructive`, `CrossDevice`).
2. Manage hardware-backed / OS-keystore master keys (Android Keystore, iOS/macOS Keychain, Windows DPAPI, Linux Secret Service) protecting SQLite encrypted stores (AEAD XChaCha20-Poly1305 / SQLCipher).
3. Enforce strict path canonicalization, workspace jail boundaries, argument validation (no raw shell string interpolation), and prompt-injection context isolation.
4. Maintain tamper-evident HMAC/hash-chained audit logs for all sensitive actions.

## Boundaries
- Security controls are non-bypassable in production builds; no debug backdoors or master override flags.

## Verification & Review Checklist
- [ ] Path traversal, command injection, replay attack, and unauthorized peer tests pass.
- [ ] `cargo audit` / `cargo deny` reports zero unmitigated vulnerabilities.
- [ ] Sensitive key material is zeroized on drop (`zeroize` crate).
