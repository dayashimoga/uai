---
description: Perform comprehensive code quality, architectural boundary, and security review before completing a subphase
---

# Workflow: Code & Security Review

Run this workflow on every subphase before marking it `COMPLETED`.

## 1. Scope & Simplicity Audit
1. Run `git status` and `git diff` to inspect every modified and untracked file.
2. Verify that every changed line maps directly to the active subphase's acceptance criteria.
3. Confirm zero dead code, zero commented-out blocks, zero placeholder UI widgets, and zero mock production implementations.

## 2. Rust Core & FFI Safety Review
1. Inspect all `unsafe` blocks in `git diff`:
   - Confirm each is strictly inside an FFI boundary module.
   - Confirm each has a complete `// SAFETY:` invariant comment.
2. Verify zero `.unwrap()` or `.expect()` calls in non-test production code.
3. Verify all async tasks and streams respect `CancellationToken` and use bounded channels.

## 3. Security & Privacy Review
1. Verify all new skills or remote commands are registered with the `PolicyEngine` and have explicit risk tiers (`ReadOnly`, `StateModifying`, `PrivilegedSystem`, `Destructive`, `CrossDevice`).
2. Confirm path arguments undergo canonicalization and workspace/directory boundary checks.
3. Confirm process execution uses structured argument arrays (`std::process::Command::args`) and never interpolates untrusted strings into a system shell (`sh -c` / `cmd /C` / `powershell -Command`).
4. Confirm zero secrets, private keys, or personal data exist in the working tree or Git staging area.
