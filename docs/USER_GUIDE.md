# Universal AI Assistant — User Guide (`docs/USER_GUIDE.md`)

This guide explains how end users configure and operate the **Universal AI Assistant** across desktop (Windows, Linux, macOS) and mobile (Android, iOS) devices in 100% offline or LAN-connected modes.

---

## 1. First-Run Onboarding & Hardware Profile

On first launch, the assistant performs a completely local hardware inspection:
1. **Hardware Summary:** Displays detected CPU vector extensions (AVX2/NEON), available RAM, and GPU acceleration support (Vulkan, Metal, or CUDA).
2. **Model Recommendation:** Suggests the optimal free local LLM tier and offline speech bundle (`sherpa-onnx` Wake Word + VAD + STT + TTS) for your hardware:
   - **Compact Tier (<4 GB free RAM):** Fast 1B–1.5B quantized model for entry-level phones and background tasks.
   - **Balanced Tier (4–8 GB free RAM):** 3B–4B quantized model for everyday reasoning and tool use.
   - **Pro Tier (>8 GB free RAM/VRAM):** 7B–14B quantized model for complex reasoning, coding, and document analysis.
3. **Offline Bundle Import:** You can either download models directly (verified automatically with SHA-256 checksums) or import `.gguf` and `.onnx` bundles from a USB drive or local folder with zero internet connection.

---

## 2. Voice Interaction & Customizable Wake Words

1. **Activation Modes:**
   - **Push-to-Talk:** Tap the microphone button or press the configurable global hotkey (default `Ctrl+Shift+Space` / `Cmd+Shift+Space`).
   - **Wake Word (Hands-Free):** Enable offline wake-word listening. Define custom wake phrases (e.g., `"Hey Assistant"`, `"Computer"`) and adjust trigger sensitivity in **Settings -> Voice & Audio**.
2. **Barge-In Interruption:** Speak at any time while the assistant is talking to immediately stop TTS playback and start a new request.
3. **Privacy Indicator:** Whenever the microphone is active for wake-word detection or speech capture, a visible status indicator is displayed in the app header and OS system tray / Android foreground notification. Flipping the **Mute Microphone** toggle immediately closes the OS audio input stream.

---

## 3. Permissions, Skills & Security Audit Log

1. **Permission Policies:** Navigate to **Settings -> Skills & Permissions** to configure each skill category:
   - **Always Allow:** Safe read-only actions (time, weather/sensors, reading media status).
   - **Ask Every Time (Default for Sensitive Actions):** File moves/deletions, system setting changes, application control, and cross-device actions.
   - **Deny:** Completely disables the skill.
2. **Audit Log:** Navigate to **Settings -> Security & Audit Log** to inspect the cryptographic hash-chained log of every tool execution, permission decision, and remote peer request, including a one-click **Verify Chain Integrity** button.

---

## 4. Cross-Device Pairing & Remote Control

1. **Discover Peers:** Ensure both devices are on the same local network (or connected via your private WireGuard/Tailscale tunnel) and open the **Devices** tab.
2. **Pair Securely:**
   - Select the target device and click **Pair**.
   - Scan the QR code displayed on the peer device or confirm that the **6-digit verification code (SAS)** matches on both screens.
3. **Configure Peer Permissions:** After pairing, toggle exactly which capabilities the peer is allowed to invoke (e.g., *Allow Clipboard Sync*, *Allow File Receive with Prompt*, *Allow Media Control*, *Allow Screen View*).
4. **Remote Actions:**
   - Say or type: *"Send the PDF on my desktop to my phone"* or *"Pause music on my living room PC"*.
   - Start a live **Screen Stream** from the Devices dashboard; when remote input is enabled on a desktop host, a persistent top-of-screen banner allows the local user to terminate remote control immediately with `Esc`.

---

## 5. Personal Memory & Knowledge Base

- **Memory Manager:** Open **Knowledge & Memory** to search, edit, or delete facts and conversation history stored in your encrypted local SQLite database.
- **Local Folder Indexing (Offline RAG):** Add local document folders to ask questions grounded in your personal Markdown notes, PDFs, and codebases with exact file and line citations.
