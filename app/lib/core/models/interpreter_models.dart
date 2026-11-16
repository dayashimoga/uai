import 'package:flutter/foundation.dart';

/// Language descriptor with ISO 639-1 code and localized display names.
@immutable
class LanguageItem {
  final String code;
  final String name;
  final String nativeName;

  const LanguageItem({
    required this.code,
    required this.name,
    required this.nativeName,
  });

  /// Canonical list of supported offline translation languages.
  static const List<LanguageItem> supportedLanguages = <LanguageItem>[
    LanguageItem(code: 'en', name: 'English', nativeName: 'English'),
    LanguageItem(code: 'es', name: 'Spanish', nativeName: 'Español'),
    LanguageItem(code: 'fr', name: 'French', nativeName: 'Français'),
    LanguageItem(code: 'de', name: 'German', nativeName: 'Deutsch'),
    LanguageItem(code: 'zh', name: 'Chinese', nativeName: '中文'),
    LanguageItem(code: 'ja', name: 'Japanese', nativeName: '日本語'),
    LanguageItem(code: 'it', name: 'Italian', nativeName: 'Italiano'),
    LanguageItem(code: 'pt', name: 'Portuguese', nativeName: 'Português'),
    LanguageItem(code: 'ru', name: 'Russian', nativeName: 'Русский'),
    LanguageItem(code: 'ko', name: 'Korean', nativeName: '한국어'),
    LanguageItem(code: 'hi', name: 'Hindi', nativeName: 'हिन्दी'),
    LanguageItem(code: 'kn', name: 'Kannada', nativeName: 'ಕನ್ನಡ'),
  ];

  static LanguageItem findByCode(String code) {
    return supportedLanguages.firstWhere(
      (l) => l.code == code.toLowerCase().trim(),
      orElse: () => LanguageItem(code: code, name: code, nativeName: code),
    );
  }
}

/// Multilingual translation result.
@immutable
class TranslationResultItem {
  final String sourceText;
  final String translatedText;
  final String sourceLang;
  final String targetLang;
  final double confidence;
  final int latencyMs;
  final String engine;

  const TranslationResultItem({
    required this.sourceText,
    required this.translatedText,
    required this.sourceLang,
    required this.targetLang,
    required this.confidence,
    required this.latencyMs,
    required this.engine,
  });

  factory TranslationResultItem.fromJson(Map<String, dynamic> json) {
    return TranslationResultItem(
      sourceText: json['source_text'] as String? ?? '',
      translatedText: json['translated_text'] as String? ?? '',
      sourceLang: json['source_lang'] as String? ?? 'en',
      targetLang: json['target_lang'] as String? ?? 'es',
      confidence: (json['confidence'] as num?)?.toDouble() ?? 0.0,
      latencyMs: (json['latency_ms'] as num?)?.toInt() ?? 0,
      engine: json['engine'] as String? ?? 'offline',
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'source_text': sourceText,
        'translated_text': translatedText,
        'source_lang': sourceLang,
        'target_lang': targetLang,
        'confidence': confidence,
        'latency_ms': latencyMs,
        'engine': engine,
      };
}

/// Recorded utterance turn in a two-party interpreter conversation.
@immutable
class InterpreterTurnItem {
  final int turnId;
  final String speaker;
  final String sourceText;
  final String translatedText;
  final String sourceLang;
  final String targetLang;
  final double confidence;
  final int latencyMs;
  final int timestampMs;

  const InterpreterTurnItem({
    required this.turnId,
    required this.speaker,
    required this.sourceText,
    required this.translatedText,
    required this.sourceLang,
    required this.targetLang,
    required this.confidence,
    required this.latencyMs,
    required this.timestampMs,
  });

  bool get isPartyA => speaker == 'party_a';
  bool get isPartyB => speaker == 'party_b';

  factory InterpreterTurnItem.fromJson(Map<String, dynamic> json) {
    return InterpreterTurnItem(
      turnId: (json['turn_id'] as num?)?.toInt() ?? 0,
      speaker: json['speaker'] as String? ?? 'party_a',
      sourceText: json['source_text'] as String? ?? '',
      translatedText: json['translated_text'] as String? ?? '',
      sourceLang: json['source_lang'] as String? ?? 'en',
      targetLang: json['target_lang'] as String? ?? 'es',
      confidence: (json['confidence'] as num?)?.toDouble() ?? 0.0,
      latencyMs: (json['latency_ms'] as num?)?.toInt() ?? 0,
      timestampMs: (json['timestamp_ms'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'turn_id': turnId,
        'speaker': speaker,
        'source_text': sourceText,
        'translated_text': translatedText,
        'source_lang': sourceLang,
        'target_lang': targetLang,
        'confidence': confidence,
        'latency_ms': latencyMs,
        'timestamp_ms': timestampMs,
      };
}

/// Active status snapshot of the interpreter session.
@immutable
class InterpreterSessionStatusItem {
  final String sessionId;
  final String partyALang;
  final String partyBLang;
  final String activeSpeaker;
  final int turnCount;

  const InterpreterSessionStatusItem({
    required this.sessionId,
    required this.partyALang,
    required this.partyBLang,
    required this.activeSpeaker,
    required this.turnCount,
  });

  bool get isPartyAActive => activeSpeaker == 'party_a';
  bool get isPartyBActive => activeSpeaker == 'party_b';

  factory InterpreterSessionStatusItem.fromJson(Map<String, dynamic> json) {
    return InterpreterSessionStatusItem(
      sessionId: json['session_id'] as String? ?? '',
      partyALang: json['party_a_lang'] as String? ?? 'en',
      partyBLang: json['party_b_lang'] as String? ?? 'es',
      activeSpeaker: json['active_speaker'] as String? ?? 'party_a',
      turnCount: (json['turn_count'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'session_id': sessionId,
        'party_a_lang': partyALang,
        'party_b_lang': partyBLang,
        'active_speaker': activeSpeaker,
        'turn_count': turnCount,
      };
}
