import 'package:flutter/foundation.dart';

/// Available tone presets for the assistant.
enum PersonaTonePreset {
  balanced('balanced', 'Balanced & Courteous'),
  concise('concise', 'Concise & Direct'),
  technical('technical', 'Technical & Rigorous'),
  casual('casual', 'Warm & Conversational'),
  academic('academic', 'Academic & In-Depth');

  final String value;
  final String label;
  const PersonaTonePreset(this.value, this.label);

  static PersonaTonePreset fromString(String val) {
    return PersonaTonePreset.values.firstWhere(
      (e) => e.value == val.toLowerCase().trim(),
      orElse: () => PersonaTonePreset.balanced,
    );
  }
}

/// Available response verbosity levels.
enum ResponseVerbosityLevel {
  concise('concise', 'Brief (1-3 sentences)'),
  normal('normal', 'Standard & Balanced'),
  detailed('detailed', 'Deep & Comprehensive');

  final String value;
  final String label;
  const ResponseVerbosityLevel(this.value, this.label);

  static ResponseVerbosityLevel fromString(String val) {
    return ResponseVerbosityLevel.values.firstWhere(
      (e) => e.value == val.toLowerCase().trim(),
      orElse: () => ResponseVerbosityLevel.normal,
    );
  }
}

/// User persona representation across the FFI bridge.
@immutable
class UserPersonaItem {
  final String id;
  final String name;
  final String tone;
  final String verbosity;
  final String voiceId;
  final double speakingRate;
  final double pitch;
  final String? systemPromptOverride;
  final bool isActive;
  final int createdAtMs;
  final int updatedAtMs;

  const UserPersonaItem({
    required this.id,
    required this.name,
    required this.tone,
    required this.verbosity,
    required this.voiceId,
    required this.speakingRate,
    required this.pitch,
    this.systemPromptOverride,
    required this.isActive,
    required this.createdAtMs,
    required this.updatedAtMs,
  });

  factory UserPersonaItem.fromJson(Map<String, dynamic> json) {
    return UserPersonaItem(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      tone: json['tone'] as String? ?? 'balanced',
      verbosity: json['verbosity'] as String? ?? 'normal',
      voiceId: json['voice_id'] as String? ?? 'vits_piper_en_ryan',
      speakingRate: (json['speaking_rate'] as num?)?.toDouble() ?? 1.0,
      pitch: (json['pitch'] as num?)?.toDouble() ?? 1.0,
      systemPromptOverride: json['system_prompt_override'] as String?,
      isActive: json['is_active'] as bool? ?? false,
      createdAtMs: json['created_at_ms'] as int? ?? 0,
      updatedAtMs: json['updated_at_ms'] as int? ?? 0,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'name': name,
      'tone': tone,
      'verbosity': verbosity,
      'voice_id': voiceId,
      'speaking_rate': speakingRate,
      'pitch': pitch,
      'system_prompt_override': systemPromptOverride,
      'is_active': isActive,
      'created_at_ms': createdAtMs,
      'updated_at_ms': updatedAtMs,
    };
  }

  UserPersonaItem copyWith({
    String? id,
    String? name,
    String? tone,
    String? verbosity,
    String? voiceId,
    double? speakingRate,
    double? pitch,
    String? systemPromptOverride,
    bool? isActive,
    int? createdAtMs,
    int? updatedAtMs,
  }) {
    return UserPersonaItem(
      id: id ?? this.id,
      name: name ?? this.name,
      tone: tone ?? this.tone,
      verbosity: verbosity ?? this.verbosity,
      voiceId: voiceId ?? this.voiceId,
      speakingRate: speakingRate ?? this.speakingRate,
      pitch: pitch ?? this.pitch,
      systemPromptOverride: systemPromptOverride ?? this.systemPromptOverride,
      isActive: isActive ?? this.isActive,
      createdAtMs: createdAtMs ?? this.createdAtMs,
      updatedAtMs: updatedAtMs ?? this.updatedAtMs,
    );
  }
}

/// Explicit key-value user preference entry.
@immutable
class UserPreferenceItem {
  final String key;
  final String valueJson;
  final String category;
  final int updatedAtMs;

  const UserPreferenceItem({
    required this.key,
    required this.valueJson,
    required this.category,
    required this.updatedAtMs,
  });

  factory UserPreferenceItem.fromJson(Map<String, dynamic> json) {
    return UserPreferenceItem(
      key: json['key'] as String? ?? '',
      valueJson: json['value_json'] as String? ?? '',
      category: json['category'] as String? ?? 'general',
      updatedAtMs: json['updated_at_ms'] as int? ?? 0,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'key': key,
      'value_json': valueJson,
      'category': category,
      'updated_at_ms': updatedAtMs,
    };
  }
}
