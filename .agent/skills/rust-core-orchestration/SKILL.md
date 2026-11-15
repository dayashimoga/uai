---
name: rust-core-orchestration
description: >-
  Use this skill when implementing or modifying Rust workspace crates, the AI
  orchestration state machine, SQLite persistence, memory indexing, skill
  dispatch, or the Flutter-Rust FFI bridge.
---

# Rust Core & AI Orchestration Skill

## Role & Ownership
Owns the `core/` Rust workspace crates: `aissistant-core`, `aissistant-storage`, `aissistant-orchestrator`, `aissistant-skills`, and `aissistant-ffi`.

## Responsibilities
1. Implement deterministic async state machines (`tokio`) managing conversation turns, voice-to-LLM-to-TTS pipelines, and cancellation tokens.
2. Manage SQLite persistence (WAL mode, FTS5 full-text search, vector embeddings store, encrypted secrets/memory, schema migrations).
3. Implement the grammar-constrained tool/skill parser, policy-gated execution dispatcher, and context window manager.
4. Expose safe, ergonomic, non-blocking APIs to Flutter via `flutter_rust_bridge`.

## Boundaries
- No `.unwrap()` or `.expect()` in production crate code; use `thiserror` and `Result<T, E>`.
- Keep `unsafe` blocks isolated to dedicated `-sys` or FFI binding modules with `// SAFETY:` comments.
- Do not embed platform UI concerns in Rust crates.

## Verification & Review Checklist
- [ ] `cargo fmt --check` passes.
- [ ] `cargo clippy --all-targets -- -D warnings` passes with zero warnings.
- [ ] `cargo test --workspace` passes with >=90% coverage on new orchestration/storage logic.
- [ ] Cancellation and timeout paths tested explicitly.
