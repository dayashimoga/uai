// Voice interaction state model.

enum VoiceMode {
  pushToTalk,
  wakeWord,
}

class VoiceState {
  final bool isListening;
  final VoiceMode mode;
  final double rmsDbfs;
  final double peakDbfs;
  final String partialTranscript;
  final String finalTranscript;
  final String language;
  final List<String> activeKeywords;
  final String? lastSpottedKeyword;
  final double? lastConfidence;

  const VoiceState({
    this.isListening = false,
    this.mode = VoiceMode.pushToTalk,
    this.rmsDbfs = -96.0,
    this.peakDbfs = -96.0,
    this.partialTranscript = '',
    this.finalTranscript = '',
    this.language = 'en',
    this.activeKeywords = const ['hey assistant'],
    this.lastSpottedKeyword,
    this.lastConfidence,
  });

  VoiceState copyWith({
    bool? isListening,
    VoiceMode? mode,
    double? rmsDbfs,
    double? peakDbfs,
    String? partialTranscript,
    String? finalTranscript,
    String? language,
    List<String>? activeKeywords,
    String? lastSpottedKeyword,
    double? lastConfidence,
  }) {
    return VoiceState(
      isListening: isListening ?? this.isListening,
      mode: mode ?? this.mode,
      rmsDbfs: rmsDbfs ?? this.rmsDbfs,
      peakDbfs: peakDbfs ?? this.peakDbfs,
      partialTranscript: partialTranscript ?? this.partialTranscript,
      finalTranscript: finalTranscript ?? this.finalTranscript,
      language: language ?? this.language,
      activeKeywords: activeKeywords ?? this.activeKeywords,
      lastSpottedKeyword: lastSpottedKeyword ?? this.lastSpottedKeyword,
      lastConfidence: lastConfidence ?? this.lastConfidence,
    );
  }
}
