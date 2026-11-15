# Rule 02: Engineering & Code Quality Standards

**Scope:** All Rust, Dart/Flutter, C/C++, Kotlin, Swift, and build scripts.

## 1. Zero-Mock & Zero-Placeholder Policy
- Production modules must never return hardcoded fake results, simulated AI responses, stubbed audio transcripts, or mock success statuses.
- UI screens must never include dead buttons or unlinked toggles. Platform features unsupported on the current OS must query the capability matrix and display a clear, non-interactive capability state.

## 2. Error Handling & Resilience
- Never silently swallow errors (`let _ = ...` on fallible operations, empty `catch (_) {}`, or ignored C return codes).
- Rust production crates must use typed errors (`thiserror`) and explicit `Result<T, E>` propagation. Avoid `.unwrap()` and `.expect()` outside unit tests.
- All long-running operations (model downloads, LLM generation, audio streams, file transfers) must support cooperative cancellation, timeouts, and deterministic cleanup.

## 3. FFI & Unsafe Code Discipline
- Keep all business logic, persistence, cryptography, and networking in the Rust core.
- Restrict `unsafe` Rust code strictly to FFI bindings (`llama.cpp`, `sherpa-onnx`, native OS APIs).
- Every `unsafe` block requires a `// SAFETY:` comment documenting the exact preconditions and memory/thread-safety invariants.

## 4. Reproducibility & Resource Hygiene
- Commit lockfiles (`Cargo.lock`, `pubspec.lock`) and pin native dependency versions and SHA-256 checksums.
- Respect the **30 GB maximum container/workspace build storage budget** by cleaning intermediate build artifacts when needed and never committing model binaries (`.gguf`, `.onnx`) to Git.
