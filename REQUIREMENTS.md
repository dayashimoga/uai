# REQUIREMENTS.md — Functional, Non-Functional & Platform Matrix

## 1. Functional Requirements (FR)

### FR-1: Offline Voice & Multimodal Conversation
- **FR-1.1 (Wake Word):** Support continuous, low-power offline wake-word detection using `sherpa-onnx` with customizable wake phrases and sensitivity tuning.
- **FR-1.2 (Voice Activity & Barge-In):** Detect speech onset/offset accurately offline and immediately interrupt TTS playback or LLM generation when the user barges in.
- **FR-1.3 (Offline STT):** Transcribe speech completely offline with streaming partial results, punctuation, and multilingual language support.
- **FR-1.4 (Offline TTS):** Synthesize natural offline speech with configurable voice models, speaking rate, and audio output routing.
- **FR-1.5 (Text & Voice Parity):** Every assistant capability must be accessible via both natural voice conversation and text chat UI.

### FR-2: Local LLM Inference & Model Management
- **FR-2.1 (Local Inference):** Execute GGUF language models locally via `llama.cpp` without internet access.
- **FR-2.2 (Hardware Profiling):** Automatically inspect CPU SIMD extensions, RAM, and GPU backends (Vulkan, Metal, CUDA) to recommend compatible models and context lengths.
- **FR-2.3 (Model Lifecycle):** Browse, download (with pause/resume), verify (SHA-256 + GGUF header validation), switch, import from local disk, and delete models.
- **FR-2.4 (Structured Tool Calling):** Enforce grammar-constrained JSON output for deterministic tool/skill invocation.

### FR-3: Personal Knowledge, Memory & Automation
- **FR-3.1 (Episodic & Semantic Memory):** Persist conversation history and extracted user facts locally in SQLite with FTS5 keyword search and vector similarity retrieval.
- **FR-3.2 (Memory Transparency):** Allow users to inspect, search, edit, export, or permanently purge any stored memory or conversation turn.
- **FR-3.3 (Local Document Indexing):** Ingest local text, Markdown, PDF, and code files into the personal knowledge base for offline retrieval-augmented generation (RAG).
- **FR-3.4 (Routines & Schedules):** Support cron/time-based, event-based (e.g., device connected, battery threshold), and multi-step macro automations.

### FR-4: Permission-Controlled Skills & Native System Control
- **FR-4.1 (File Management):** Search, read, create, move, copy, archive, and checksum local files within authorized directories.
- **FR-4.2 (Application & Window Control):** List running applications, launch/focus/close apps, and manage windows where supported by the OS.
- **FR-4.3 (System & Media Control):** Query system telemetry (CPU, RAM, disk, battery, network), adjust volume/brightness, control media playback (play/pause/next/previous), and manage system notifications/timers/alarms.
- **FR-4.4 (Policy Enforcement):** Gate all skills through a configurable policy engine (`Allow`, `RequireConfirmation`, `Deny`) with full audit logging.

### FR-5: Cross-Device Pairing, Remote Control & Streaming
- **FR-5.1 (LAN Discovery):** Automatically discover peer devices on the local network via mDNS-SD.
- **FR-5.2 (Optional Remote Connectivity):** Support direct IP/port and user-configured encrypted overlay networks (e.g., WireGuard/Tailscale or self-hosted QUIC relay) without requiring proprietary cloud accounts.
- **FR-5.3 (Cryptographic Pairing):** Pair devices via mutual Ed25519 TLS 1.3 and QR / 6-digit SAS verification.
- **FR-5.4 (Cross-Device Skill Execution):** Route natural-language commands or structured skills from one paired device to another (e.g., phone controlling desktop media or locking workstation) subject to target-device ACLs.
- **FR-5.5 (Clipboard & File Transfer):** Sync clipboard items on demand or automatically between trusted peers, and transfer arbitrary files with chunk-level resume and SHA-256 verification.
- **FR-5.6 (Screen Streaming & Remote Input):** Stream live screen frames over QUIC with adaptive bitrate and optional permission-gated remote pointer/keyboard control.

---

## 2. Non-Functional Requirements (NFR)

| ID | Category | Requirement | Target Metric |
| :--- | :--- | :--- | :--- |
| **NFR-1** | **Offline Autonomy** | Full core operation with zero external network connectivity. | 100% of core features functional in airplane/offline mode. |
| **NFR-2** | **Wake-Word Latency** | Time from end of spoken wake phrase to trigger event. | <= 150 ms on reference desktop/mobile CPUs. |
| **NFR-3** | **STT End-of-Turn Latency** | Time from VAD speech end to final transcript availability. | <= 350 ms for utterances <= 10 seconds on reference CPU. |
| **NFR-4** | **TTS Time-to-First-Audio** | Time from first completed sentence to audio playback start. | <= 250 ms using local ONNX TTS. |
| **NFR-5** | **Idle Memory Footprint** | Resident memory when idle (wake-word active, LLM unloaded or mmap-paged). | <= 80 MB on desktop; <= 60 MB on mobile. |
| **NFR-6** | **Crash & Error Resilience** | Zero unhandled panics across FFI boundaries; graceful recovery on corrupt models or device disconnects. | 0 FFI panics; 100% typed error returns. |
| **NFR-7** | **Test Coverage** | Automated test coverage across core crates and UI state logic. | >= 90% meaningful coverage; 100% passing required tests. |
| **NFR-8** | **Build Storage Budget** | Total workspace and container build cache footprint. | <= 30 GB maximum disk usage at all times. |

---

## 3. Five-Platform Capability & Degradation Matrix

Because operating systems enforce distinct sandboxing and hardware policies, the assistant detects capabilities at runtime and degrades gracefully with explicit user feedback.

| Capability | Windows (10/11) | Linux (X11 / Wayland) | macOS (13+) | Android (10+ / API 29+) | iOS (16+) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Local LLM (`llama.cpp`)** | Full (CPU AVX2, Vulkan, CUDA) | Full (CPU AVX2/NEON, Vulkan, CUDA) | Full (CPU, Apple Metal) | Full (ARM64 NEON/dotprod, Vulkan opt-in) | Full (ARM64 Metal within iOS memory limit; Tier 1/2 models) |
| **Offline STT & TTS (`sherpa-onnx`)** | Full | Full | Full | Full | Full |
| **Background Wake Word** | Full (System Tray Service) | Full (Background Daemon) | Full (Menu Bar Agent) | Full (Foreground Service with Mic Notification) | **Degraded:** Active while app is foregrounded or in active `AVAudioSession`; background trigger via Siri Shortcut / App Intent / Action Button due to iOS background mic termination. |
| **File System Management** | Full (User-authorized paths) | Full (User-authorized paths / XDG Portal) | Full (TCC-granted folders / Sandbox bookmarks) | Scoped Storage / SAF-granted directories | App Sandbox & UIDocumentPicker security-scoped bookmarks |
| **App Launch & Window Management** | Full (Win32 / ShellExecute) | Full on X11; Portal/App-launch on Wayland | Full (NSWorkspace + Accessibility `AXUIElement`) | App launch via Intents; UI actions via opt-in AccessibilityService | URL Schemes / App Intents only (iOS sandbox prohibits cross-app window control) |
| **System Media Control** | Full (SMTC WinRT) | Full (D-Bus MPRIS2) | Full (MediaRemote / NowPlaying) | Full (`MediaSessionManager`) | Local playback + standard system NowPlaying |
| **Screen Streaming (Capture)** | Full (Windows Graphics Capture) | Full (PipeWire `xdg-desktop-portal` / X11 SHM) | Full (`ScreenCaptureKit` with TCC grant) | Full (`MediaProjection` with user prompt) | View-only via `ReplayKit` Broadcast Extension |
| **Remote Input Injection (Host)** | Full (`SendInput` when enabled) | Full (`libei`/Portal on Wayland, `XTest` on X11) | Full (`CGEventPost` with Accessibility TCC grant) | Opt-in via Android `AccessibilityService` gestures | **Unsupported as Host** (iOS prohibits synthetic cross-app input injection; iOS acts as full **Remote Controller Client**). |
| **LAN Discovery & QUIC Peer** | Full | Full | Full | Full (`NsdManager` / MulticastLock) | Full (`NWBrowser` / Local Network TCC permission) |
