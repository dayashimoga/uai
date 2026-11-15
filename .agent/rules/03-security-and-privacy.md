# Rule 03: Security, Privacy & Trust Boundaries

**Scope:** All storage, networking, AI tool execution, audio capture, and OS automation.

## 1. Offline-First Privacy Guarantee
- Zero telemetry, analytics, voice recordings, prompts, or personal memory may be transmitted to external servers without explicit user configuration and per-domain consent.
- Continuous audio capture is restricted to an in-memory ring buffer strictly for local wake-word detection when enabled; audio is never persisted to disk unless explicitly configured by the user.

## 2. Fail-Closed Permission & Skill Policy
- Every skill/tool invocation (local or remote) must pass through the Rust `PolicyEngine` before execution.
- Destructive or privileged actions (file deletion, process termination, shell/system setting modification, remote input injection, outbound transfers) default to `RequireConfirmation` or `Deny`.
- Every skill execution, permission grant/denial, and pairing event must be recorded in the tamper-evident SQLite audit log.

## 3. Untrusted Input & Prompt Injection Mitigation
- Treat external content (file contents, clipboard text, OCR/screen text, LAN peer payloads) as untrusted data.
- Isolate untrusted data within structured delimiters before passing to the LLM context.
- Disallow automatic invocation of high-risk tools when the conversation context contains untrusted external inputs unless explicitly confirmed by the user on the local UI.

## 4. Cryptographic Identity & Transport
- All cross-device communication must use mutual-authenticated QUIC (TLS 1.3 with Ed25519 device certificates pinned during pairing).
- Device pairing requires out-of-band verification (QR code or 6-digit Short Authentication String) with strict rate limiting and short-lived pairing tokens.
- Never commit secrets, keys, or tokens to version control.
