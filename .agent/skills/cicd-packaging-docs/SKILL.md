---
name: cicd-packaging-docs
description: >-
  Use this skill when creating or maintaining GitHub Actions workflows,
  cross-platform build scripts, release packaging, checksum generation, or
  developer and user documentation.
---

# CI/CD, Packaging & Documentation Skill

## Role & Ownership
Owns `.github/workflows/`, build scripts, installers/packages (APK/AAB, MSIX/EXE, AppImage/DEB/Flatpak, DMG, IPA), `CONTRIBUTING.md`, `README.md`, and `docs/`.

## Responsibilities
1. Maintain independent, reproducible GitHub Actions pipelines for Rust core verification, Flutter analysis/testing, platform builds (Windows, Linux, macOS, Android, iOS), and security audits.
2. Respect the **30 GB container/runner disk budget** by scoping caches, pruning intermediate build artifacts, and avoiding bloated container layers.
3. Generate SHA-256 checksum manifests and SBOMs for all release artifacts.
4. Keep `README.md`, `docs/SETUP.md`, `docs/USER_GUIDE.md`, `EXECUTION_STATUS.md`, `TODO.md`, and `CHANGELOG.md` synchronized with actual repository state.

## Boundaries
- Never hardcode secrets or signing keys in workflow YAML files; use GitHub Actions secrets and OIDC permissions with least privilege (`permissions: contents: read` by default).

## Verification & Review Checklist
- [ ] Workflow YAML syntax and least-privilege permissions verified.
- [ ] Documentation links and setup commands match actual workspace paths and scripts.
