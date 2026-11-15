# CLAUDE.md — Behavioral & Engineering Guidelines

Behavioral guidelines to eliminate common LLM coding mistakes, synchronized with [`ENGINEERING_CONTRACT.md`](./ENGINEERING_CONTRACT.md) and [`AGENTS.md`](./AGENTS.md).

**Tradeoff:** These guidelines bias strictly toward correctness, security, and verification over speed. Never sacrifice architectural integrity or skip verification gates.

---

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain about requirements, hardware constraints, or platform APIs, ask.
- Verify toolchains, dependencies, hardware accelerators, and OS capabilities before relying on them.
- If multiple interpretations exist, present them clearly with pros/cons — do not pick silently.
- If a simpler, dependency-lighter approach exists, propose it. Push back against overengineering when warranted.
- If blocked by a missing dependency, tool, or decision, mark the state as `BLOCKED`, explain the exact blocker, and request the minimum necessary decision.

---

## 2. Simplicity First

**Minimum code that solves the problem cleanly. Nothing speculative.**

- Implement only the scope of the currently approved subphase.
- No speculative abstractions for single-use code.
- No unrequested "flexibility" or premature configuration layers.
- Keep FFI boundaries (Flutter <-> Rust <-> Native C/C++/Kotlin/Swift) narrow, strongly typed, and explicitly documented.
- Ask yourself: *"Would a principal systems engineer consider this overcomplicated?"* If yes, simplify.

---

## 3. Surgical Changes & Scope Discipline

**Touch only what you must. Clean up only your own mess.**

- Execute **one subphase at a time** after explicit user approval. Never chain subphases automatically.
- When editing existing code:
  - Do not refactor or reformat adjacent code unrelated to the active subphase.
  - Match existing idiomatic Rust (`rustfmt`/`clippy`), Dart/Flutter (`dart format`/`flutter analyze`), Kotlin, and Swift conventions.
  - Mention unrelated dead code or technical debt in `TODO.md` rather than modifying out-of-scope files.
- When your changes create orphans:
  - Remove imports, variables, or helper functions that your own changes made unused.
- Every changed line must trace directly to the active subphase's acceptance criteria.

---

## 4. Goal-Driven Execution & Zero-Mock Policy

**Define measurable success criteria. Loop until verified with real execution evidence.**

- **Zero Fake/Mock Production Code:** Never ship mock implementations, stubbed success returns, hardcoded demo data, dead UI buttons, or placeholder screens as completed deliverables.
- **No Silent Failures:** Never swallow errors (`let _ = ...` on fallible I/O, empty `catch` blocks, or ignored FFI status codes). Every error must be typed, logged with context, and surfaced appropriately.
- Transform every subphase into verifiable goals:
  1. Define acceptance criteria and test cases before coding.
  2. Write regression tests before fixing defects.
  3. Run unit, integration, security, and build checks.
  4. Record verifiable execution evidence in `EXECUTION_STATUS.md`, `CHANGELOG.md`, and `TODO.md`.
- Never claim a platform target or feature works without actual build and test execution evidence on that target.
