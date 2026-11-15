# Rule 05: Multi-Agent Collaboration & Handoffs

**Scope:** All specialized domain agents and subagents.

## 1. Coordinator Authority
- One Coordinating Lead Agent owns integration, conflict resolution, and updates to `EXECUTION_STATUS.md`, `TODO.md`, and `CHANGELOG.md`.
- Specialized agents operate within their declared module boundaries defined in `AGENTS.md` and `.agent/skills/<skill>/SKILL.md`.

## 2. Safe Parallelism
- Use parallel agents **only** for independent, non-conflicting tasks (e.g., parallel read-only analysis, independent platform adapter design, or disjoint crate testing) within the single currently approved subphase.
- Never run parallel agents that edit the same files or shared Cargo/Flutter lockfiles simultaneously.

## 3. Handoff Contract
Every agent completing a delegated task must return a concise handoff report containing:
1. **Summary of Changes:** Exact files created/modified and architectural alignment.
2. **Interface Impact:** Any changes to Rust traits, FFI signatures, SQLite schema, or Dart state models.
3. **Verification Evidence:** Exact commands executed, exit codes, and test counts.
4. **Open Risks / Follow-ups:** Any platform-specific caveats or edge cases discovered.
