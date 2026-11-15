---
description: Execute a single roadmap subphase with strict pre-approval and post-verification gates
---

# Workflow: Single-Subphase Implementation

This workflow enforces the Mandatory Execution Protocol defined in `AGENTS.md` and `ENGINEERING_CONTRACT.md`.

## Phase 1: Selection & Pre-Implementation Proposal (STOP Gate)

1. Read `ROADMAP.md`, `EXECUTION_STATUS.md`, and `TODO.md`.
2. Identify the single next pending subphase (`S{stage}.P{phase}.{subphase}`) whose upstream dependencies are `COMPLETED`.
3. Verify local environment readiness (required compilers, SDKs, libraries) before proposing implementation.
4. Present the Subphase Proposal to the user:
   - **Subphase ID & Title**
   - **Objective**
   - **Explicit Assumptions**
   - **Dependencies & Environment Check**
   - **In-Scope / Out-of-Scope Files**
   - **Step-by-Step Implementation Plan**
   - **Measurable Acceptance Criteria & Test Plan**
5. **STOP** and wait for explicit user confirmation. Do not write implementation code before confirmation.

## Phase 2: Surgical Implementation

6. Upon user approval, implement **only** the approved subphase scope following `CLAUDE.md` and `ENGINEERING_CONTRACT.md`.
7. Write unit, integration, error-path, and security tests covering all new logic (targeting >=90% coverage).

## Phase 3: Verification & Diff Audit

8. Execute `.agent/workflows/testing-and-quality-gates.md` and `.agent/workflows/code-and-security-review.md`.
9. Fix any test failures, compiler warnings, or linter issues.

## Phase 4: Evidence Recording & Completion (STOP Gate)

10. Update `EXECUTION_STATUS.md` with subphase status (`COMPLETED` or `BLOCKED`), test metrics, and command outputs.
11. Update `TODO.md` (append/mark tasks without deleting history).
12. Append a detailed entry to `CHANGELOG.md`.
13. Commit changes to Git with a structured commit message (`feat(S{stage}.P{phase}.{subphase}): <summary>`).
14. Report to the user:
    - Files changed
    - Tests and builds executed with results
    - Known limitations or target-platform caveats
    - Git commit hash
15. **STOP** and wait for explicit user confirmation before proposing the next subphase.
