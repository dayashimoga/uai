---
name: product-architecture
description: >-
  Use this skill when designing or reviewing system architecture, cross-module
  interfaces, requirement traceability, capability matrices, or subphase plans
  for the Universal AI Assistant.
---

# Product Architecture & Technical Planning Skill

## Role & Ownership
Owns system architecture (`ARCHITECTURE.md`), requirements traceability (`REQUIREMENTS.md`), roadmap dependencies (`ROADMAP.md`), and cross-layer interface contracts across Flutter, Rust Core, and Native Platform bridges.

## Responsibilities
1. Define clean, versioned Rust traits and FFI boundary schemas (`flutter_rust_bridge`) before implementation starts.
2. Maintain the 5-platform capability matrix (Android, Windows, Linux, macOS, iOS) and ensure graceful degradation strategies are specified for every feature.
3. Decompose subphase objectives into surgical, testable increments with measurable acceptance criteria.
4. Review diffs to ensure zero architectural drift, zero circular dependencies across Rust crates, and strict separation of UI from core logic.

## Boundaries
- Never bypass the single-subphase execution protocol (`Rule 01`).
- Never place business logic, cryptography, or direct SQLite access inside the Flutter UI layer.

## Handoff & Review Checklist
- [ ] All public Rust traits and FFI data structures documented with error semantics.
- [ ] Platform capability matrix updated for any OS-dependent behavior.
- [ ] Acceptance criteria mapped 1:1 to automated tests.
