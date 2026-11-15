# Rule 01: Mandatory Subphase Execution Protocol

**Scope:** Always Active across the entire repository.

## 1. Single-Subphase Gate
- Never implement more than **one subphase** (`S{stage}.P{phase}.{subphase}`) per user confirmation cycle.
- Never interpret bootstrap approval, architectural approval, or roadmap updates as permission to execute subsequent subphases automatically.

## 2. Pre-Implementation Checklist
Before writing or modifying implementation files for any subphase:
1. Read `ROADMAP.md`, `EXECUTION_STATUS.md`, and `TODO.md`.
2. Select only the next pending subphase whose dependencies are `COMPLETED`.
3. Present to the user:
   - Subphase ID & Title
   - Objective & Explicit Assumptions
   - Required Toolchains & Dependencies (verified on host)
   - In-Scope Files / Modules
   - Implementation Plan
   - Measurable Acceptance Criteria & Verification Commands
4. **STOP** and wait for explicit user approval.

## 3. Post-Implementation Checklist
After implementing the approved subphase:
1. Execute all relevant unit, integration, security, formatter, linter, and build commands.
2. Fix any failures or warnings introduced by the implementation.
3. Inspect `git diff` to verify surgical scope, simplicity, and absence of dead or mock code.
4. Update `EXECUTION_STATUS.md`, `TODO.md` (preserve history), and `CHANGELOG.md` (append-only) with verifiable command output summaries and commit references.
5. Report changed files, executed tests, results, known limitations, and git commit hash.
6. **STOP** and wait for explicit user confirmation before proposing the next subphase.

## 4. Blocker Handling
- If a subphase cannot proceed due to missing tools, hardware, credentials, or unresolved architectural choices, set its status to `BLOCKED` in `EXECUTION_STATUS.md`, document the exact blocker, and request the minimum decision required from the user.
