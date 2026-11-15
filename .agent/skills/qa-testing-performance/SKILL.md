---
name: qa-testing-performance
description: >-
  Use this skill when writing or executing unit, integration, end-to-end,
  offline, fault-injection, security, or performance benchmark tests, and when
  verifying subphase acceptance criteria.
---

# QA, Testing & Performance Engineering Skill

## Role & Ownership
Owns `TESTING.md`, test fixtures, coverage reporting, fault-injection harnesses, and latency/memory benchmark suites.

## Responsibilities
1. Enforce the >=90% meaningful test coverage gate and 100% test pass requirement across Rust and Flutter codebases.
2. Maintain deterministic offline fixtures (sample GGUF micro-models for CI, PCM audio fixtures for wake-word/STT/TTS, mock peer nodes for network chaos testing).
3. Measure and enforce latency and resource budgets:
   - Wake-word detection latency: <150 ms
   - VAD end-of-speech to STT completion: <300 ms on reference hardware
   - Idle background RAM footprint (without LLM loaded): <80 MB desktop / <60 MB mobile
4. Verify that every subphase records reproducible test execution outputs in `EXECUTION_STATUS.md`.

## Boundaries
- Never accept stubbed tests (`assert!(true)`) or skipped critical tests (`#[ignore]` without tracked hardware blocker) as coverage evidence.

## Verification & Review Checklist
- [ ] Unit, integration, negative/error-path, and offline tests executed and logged.
- [ ] No memory leaks, zombie child processes, or dangling file locks after test teardown.
