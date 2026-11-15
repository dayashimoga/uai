---
name: local-ai-optimization
description: >-
  Use this skill when integrating llama.cpp, managing GGUF models, detecting
  hardware accelerators (CPU SIMD, Vulkan, Metal, CUDA), optimizing KV cache
  and memory budgets, or configuring grammar-constrained local inference.
---

# Local AI & Model Optimization Skill

## Role & Ownership
Owns the `aissistant-llm` Rust crate, `llama.cpp` native build/bindings, hardware capability detection, model downloader/verifier, and inference optimization.

## Responsibilities
1. Detect host CPU features (AVX2, AVX-512, ARM NEON/dotprod), RAM/VRAM availability, and GPU backends (Vulkan, Metal, CUDA) before selecting model quantization and context size.
2. Manage resumable GGUF downloads from open model repositories with mandatory SHA-256 checksum verification before loading.
3. Enforce strict memory budgets (e.g., mobile background limits, low-RAM fallback, automatic model unloading on memory pressure).
4. Support streaming token generation, prompt caching (KV prefix reuse), and GBNF/JSON-schema grammar constraints for reliable offline tool calling.

## Boundaries
- Never load an unverified or corrupted model file without header/checksum validation.
- Never hardcode a single hardware backend; always provide a verified CPU fallback.

## Verification & Review Checklist
- [ ] SHA-256 mismatch and truncated GGUF file tests pass (graceful error, zero crash).
- [ ] Token streaming cancellation within <100 ms verified.
- [ ] Memory watermark and clean unload tests pass without leaks.
