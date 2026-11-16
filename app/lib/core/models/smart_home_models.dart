import 'dart:convert';
import 'package:flutter/foundation.dart';

/// Supported smart home entity domain categories.
enum SmartHomeDomain {
  light('light', 'Light'),
  switchDevice('switch', 'Switch'),
  climate('climate', 'Climate / Thermostat'),
  sensor('sensor', 'Sensor'),
  cover('cover', 'Cover / Blind'),
  fan('fan', 'Fan'),
  lock('lock', 'Lock'),
  mediaPlayer('media_player', 'Media Player');

  final String value;
  final String label;
  const SmartHomeDomain(this.value, this.label);

  static SmartHomeDomain fromString(String val) {
    return SmartHomeDomain.values.firstWhere(
      (e) => e.value == val.toLowerCase().trim(),
      orElse: () => SmartHomeDomain.switchDevice,
    );
  }
}

/// Supported offline communication protocols for smart home devices.
enum SmartHomeProtocol {
  mqtt('mqtt', 'MQTT'),
  homeAssistant('home_assistant', 'Home Assistant'),
  matterLocal('matter_local', 'Matter Local');

  final String value;
  final String label;
  const SmartHomeProtocol(this.value, this.label);

  static SmartHomeProtocol fromString(String val) {
    return SmartHomeProtocol.values.firstWhere(
      (e) => e.value == val.toLowerCase().trim(),
      orElse: () => SmartHomeProtocol.mqtt,
    );
  }
}

/// Representation of a registered local smart-home entity.
@immutable
class SmartHomeEntityItem {
  final String id;
  final SmartHomeDomain domain;
  final String name;
  final String stateJson;
  final String attributesJson;
  final SmartHomeProtocol protocol;
  final bool isAvailable;
  final int lastUpdatedMs;

  const SmartHomeEntityItem({
    required this.id,
    required this.domain,
    required this.name,
    required this.stateJson,
    required this.attributesJson,
    required this.protocol,
    required this.isAvailable,
    required this.lastUpdatedMs,
  });

  factory SmartHomeEntityItem.fromJson(Map<String, dynamic> json) {
    return SmartHomeEntityItem(
      id: json['id'] as String? ?? '',
      domain: SmartHomeDomain.fromString(json['domain'] as String? ?? 'switch'),
      name: json['name'] as String? ?? '',
      stateJson: json['state_json'] as String? ?? '{}',
      attributesJson: json['attributes_json'] as String? ?? '{}',
      protocol: SmartHomeProtocol.fromString(json['protocol'] as String? ?? 'mqtt'),
      isAvailable: json['is_available'] as bool? ?? false,
      lastUpdatedMs: (json['last_updated_ms'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'domain': domain.value,
      'name': name,
      'state_json': stateJson,
      'attributes_json': attributesJson,
      'protocol': protocol.value,
      'is_available': isAvailable,
      'last_updated_ms': lastUpdatedMs,
    };
  }

  /// Convenience getter for parsed state map.
  Map<String, dynamic> get stateMap {
    try {
      final decoded = jsonDecode(stateJson);
      if (decoded is Map<String, dynamic>) return decoded;
    } catch (_) {}
    return const {};
  }

  /// Convenience getter for parsed attributes map.
  Map<String, dynamic> get attributesMap {
    try {
      final decoded = jsonDecode(attributesJson);
      if (decoded is Map<String, dynamic>) return decoded;
    } catch (_) {}
    return const {};
  }

  /// Whether the device is currently turned on.
  bool get isTurnedOn {
    final state = stateMap['state']?.toString().toLowerCase();
    return state == 'on' || state == 'heat' || state == 'cool' || state == 'open';
  }

  /// Brightness level (0-255) if applicable.
  int? get brightness {
    final b = stateMap['brightness'];
    if (b is num) return b.toInt();
    return null;
  }

  /// Target temperature in Celsius if applicable.
  double? get targetTemperature {
    final t = stateMap['target_temperature'] ?? stateMap['temperature'];
    if (t is num) return t.toDouble();
    return null;
  }

  /// Position percentage (0-100) if applicable.
  int? get position {
    final p = stateMap['position'];
    if (p is num) return p.toInt();
    return null;
  }

  SmartHomeEntityItem copyWith({
    String? id,
    SmartHomeDomain? domain,
    String? name,
    String? stateJson,
    String? attributesJson,
    SmartHomeProtocol? protocol,
    bool? isAvailable,
    int? lastUpdatedMs,
  }) {
    return SmartHomeEntityItem(
      id: id ?? this.id,
      domain: domain ?? this.domain,
      name: name ?? this.name,
      stateJson: stateJson ?? this.stateJson,
      attributesJson: attributesJson ?? this.attributesJson,
      protocol: protocol ?? this.protocol,
      isAvailable: isAvailable ?? this.isAvailable,
      lastUpdatedMs: lastUpdatedMs ?? this.lastUpdatedMs,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is SmartHomeEntityItem &&
          runtimeType == other.runtimeType &&
          id == other.id &&
          domain == other.domain &&
          name == other.name &&
          stateJson == other.stateJson &&
          attributesJson == other.attributesJson &&
          protocol == other.protocol &&
          isAvailable == other.isAvailable &&
          lastUpdatedMs == other.lastUpdatedMs;

  @override
  int get hashCode =>
      id.hashCode ^
      domain.hashCode ^
      name.hashCode ^
      stateJson.hashCode ^
      attributesJson.hashCode ^
      protocol.hashCode ^
      isAvailable.hashCode ^
      lastUpdatedMs.hashCode;
}

/// Actuation command for a smart home device.
@immutable
class SmartHomeCommandItem {
  final String entityId;
  final String command;
  final int? brightness;
  final String? colorHex;
  final double? targetCelsius;
  final int? positionPercent;

  const SmartHomeCommandItem({
    required this.entityId,
    required this.command,
    this.brightness,
    this.colorHex,
    this.targetCelsius,
    this.positionPercent,
  });

  Map<String, dynamic> toJson() {
    final map = <String, dynamic>{
      'entity_id': entityId,
      'command': command,
    };
    if (brightness != null) map['brightness'] = brightness;
    if (colorHex != null) map['color_hex'] = colorHex;
    if (targetCelsius != null) map['target_celsius'] = targetCelsius;
    if (positionPercent != null) map['position_percent'] = positionPercent;
    return map;
  }
}

/// Execution result of a smart home actuation command.
@immutable
class SmartHomeCommandResultItem {
  final bool success;
  final String entityId;
  final String command;
  final String dispatchedVia;
  final String dispatchAction;
  final String dispatchPayloadJson;
  final String newStateJson;

  const SmartHomeCommandResultItem({
    required this.success,
    required this.entityId,
    required this.command,
    required this.dispatchedVia,
    required this.dispatchAction,
    required this.dispatchPayloadJson,
    required this.newStateJson,
  });

  factory SmartHomeCommandResultItem.fromJson(Map<String, dynamic> json) {
    return SmartHomeCommandResultItem(
      success: json['success'] as bool? ?? false,
      entityId: json['entity_id'] as String? ?? '',
      command: json['command'] as String? ?? '',
      dispatchedVia: json['dispatched_via'] as String? ?? '',
      dispatchAction: json['dispatch_action'] as String? ?? '',
      dispatchPayloadJson: json['dispatch_payload_json'] as String? ?? '{}',
      newStateJson: json['new_state_json'] as String? ?? '{}',
    );
  }
}
