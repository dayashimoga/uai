---
name: speech-wakeword-audio
description: >-
  Use this skill when implementing offline audio capture/playback, Voice
  Activity Detection (VAD), customizable wake-word spotting, streaming STT,
  or neural TTS using sherpa-onnx.
---

# Speech, Wake-Word & Audio Engineering Skill

## Role & Ownership
Owns the `aissistant-speech` and `aissistant-audio` Rust crates, `sherpa-onnx` FFI integration, lock-free audio ring buffers, VAD, wake-word engine, streaming STT, and neural TTS.

## Responsibilities
1. Maintain low-latency cross-platform audio capture and playback streams (16 kHz mono PCM input for speech recognition; native sample rate output for TTS) with automatic resampling and device hot-plug recovery.
2. Implement an in-memory circular audio buffer for continuous wake-word spotting (`sherpa-onnx` keyword spotting) that never writes raw ambient audio to disk.
3. Run Silero/ten-VAD for accurate end-of-speech detection and barge-in interruption (halting TTS playback immediately when the user speaks).
4. Provide offline multilingual streaming STT (Zipformer/Paraformer/Whisper ONNX) and low-latency offline TTS (VITS/Piper ONNX).

## Boundaries
- Never activate the microphone without verifying OS microphone permission and displaying an active listening indicator in the UI/system tray/notification.
- Audio callback threads must be real-time safe (no heap allocations, mutex locks, or blocking I/O inside the audio callback).

## Verification & Review Checklist
- [ ] Deterministic WAV fixture tests verify wake-word trigger, VAD segmentation, STT transcription, and TTS synthesis offline.
- [ ] Barge-in latency and audio device disconnect recovery tests pass.
