// Event models matching aissistant-ffi JSON serialization.

abstract class BridgeEvent {
  final String type;

  const BridgeEvent(this.type);

  factory BridgeEvent.fromJson(Map<String, dynamic> json) {
    final type = json['type'] as String? ?? 'unknown';
    final payload = json['payload'] as Map<String, dynamic>? ?? <String, dynamic>{};

    switch (type) {
      case 'health_status_changed':
        return HealthStatusChangedEvent(
          status: payload['status'] as String? ?? 'critical',
          details: (payload['details'] as List<dynamic>?)
                  ?.map((e) => e.toString())
                  .toList() ??
              const <String>[],
        );

      case 'capability_changed':
        return CapabilityChangedEvent(
          capability: payload['capability'] as String? ?? '',
          status: payload['status'] as String? ?? '',
          reason: payload['reason'] as String?,
        );

      case 'audit_log_appended':
        return AuditLogAppendedEvent(
          sequence: (payload['sequence'] as num?)?.toInt() ?? 0,
          actor: payload['actor'] as String? ?? '',
          skillId: payload['skill_id'] as String? ?? '',
          decision: payload['decision'] as String? ?? '',
          executionStatus: payload['execution_status'] as String? ?? '',
          entryHash: payload['entry_hash'] as String? ?? '',
        );

      case 'setting_changed':
        return SettingChangedEvent(
          key: payload['key'] as String? ?? '',
          valuePreview: payload['value_preview'] as String?,
        );

      case 'error_occurred':
        return ErrorOccurredEvent(
          code: payload['code'] as String? ?? '',
          message: payload['message'] as String? ?? '',
          recoverable: payload['recoverable'] as bool? ?? false,
        );

      case 'lifecycle_state_changed':
        return LifecycleStateChangedEvent(
          previous: payload['previous'] as String? ?? '',
          current: payload['current'] as String? ?? '',
          reason: payload['reason'] as String?,
        );

      case 'memory_pressure':
        return MemoryPressureEvent(
          level: payload['level'] as String? ?? 'moderate',
        );

      case 'voice_level_updated':
        return VoiceLevelUpdatedEvent(
          rmsDbfs: (payload['rms_dbfs'] as num?)?.toDouble() ?? -96.0,
          peakDbfs: (payload['peak_dbfs'] as num?)?.toDouble() ?? -96.0,
        );

      case 'wake_word_triggered':
        return WakeWordTriggeredEvent(
          keyword: payload['keyword'] as String? ?? '',
          confidence: (payload['confidence'] as num?)?.toDouble() ?? 0.0,
        );

      case 'transcript_updated':
        return TranscriptUpdatedEvent(
          text: payload['text'] as String? ?? '',
          isFinal: payload['is_final'] as bool? ?? false,
          language: payload['language'] as String? ?? 'en',
          confidence: (payload['confidence'] as num?)?.toDouble() ?? 0.0,
        );

      case 'voice_state_changed':
        return VoiceStateChangedEvent(
          isListening: payload['is_listening'] as bool? ?? false,
          mode: payload['mode'] as String? ?? 'idle',
        );

      case 'model_loaded':
        return ModelLoadedEvent(
          modelId: payload['model_id'] as String? ?? '',
        );

      case 'model_unloaded':
        return ModelUnloadedEvent(
          modelId: payload['model_id'] as String? ?? '',
        );

      case 'benchmark_completed':
        return BenchmarkCompletedEvent(
          metricsJson: payload['metrics_json'] as String? ?? '{}',
        );

      default:
        return UnknownBridgeEvent(type: type, payload: payload);
    }
  }
}

class HealthStatusChangedEvent extends BridgeEvent {
  final String status;
  final List<String> details;

  const HealthStatusChangedEvent({
    required this.status,
    required this.details,
  }) : super('health_status_changed');
}

class CapabilityChangedEvent extends BridgeEvent {
  final String capability;
  final String status;
  final String? reason;

  const CapabilityChangedEvent({
    required this.capability,
    required this.status,
    this.reason,
  }) : super('capability_changed');
}

class AuditLogAppendedEvent extends BridgeEvent {
  final int sequence;
  final String actor;
  final String skillId;
  final String decision;
  final String executionStatus;
  final String entryHash;

  const AuditLogAppendedEvent({
    required this.sequence,
    required this.actor,
    required this.skillId,
    required this.decision,
    required this.executionStatus,
    required this.entryHash,
  }) : super('audit_log_appended');
}

class SettingChangedEvent extends BridgeEvent {
  final String key;
  final String? valuePreview;

  const SettingChangedEvent({
    required this.key,
    this.valuePreview,
  }) : super('setting_changed');
}

class ErrorOccurredEvent extends BridgeEvent {
  final String code;
  final String message;
  final bool recoverable;

  const ErrorOccurredEvent({
    required this.code,
    required this.message,
    required this.recoverable,
  }) : super('error_occurred');
}

class LifecycleStateChangedEvent extends BridgeEvent {
  final String previous;
  final String current;
  final String? reason;

  const LifecycleStateChangedEvent({
    required this.previous,
    required this.current,
    this.reason,
  }) : super('lifecycle_state_changed');
}

class MemoryPressureEvent extends BridgeEvent {
  final String level;

  const MemoryPressureEvent({
    required this.level,
  }) : super('memory_pressure');
}

class VoiceLevelUpdatedEvent extends BridgeEvent {
  final double rmsDbfs;
  final double peakDbfs;

  const VoiceLevelUpdatedEvent({
    required this.rmsDbfs,
    required this.peakDbfs,
  }) : super('voice_level_updated');
}

class WakeWordTriggeredEvent extends BridgeEvent {
  final String keyword;
  final double confidence;

  const WakeWordTriggeredEvent({
    required this.keyword,
    required this.confidence,
  }) : super('wake_word_triggered');
}

class TranscriptUpdatedEvent extends BridgeEvent {
  final String text;
  final bool isFinal;
  final String language;
  final double confidence;

  const TranscriptUpdatedEvent({
    required this.text,
    required this.isFinal,
    required this.language,
    required this.confidence,
  }) : super('transcript_updated');
}

class VoiceStateChangedEvent extends BridgeEvent {
  final bool isListening;
  final String mode;

  const VoiceStateChangedEvent({
    required this.isListening,
    required this.mode,
  }) : super('voice_state_changed');
}

class ModelLoadedEvent extends BridgeEvent {
  final String modelId;

  const ModelLoadedEvent({
    required this.modelId,
  }) : super('model_loaded');
}

class ModelUnloadedEvent extends BridgeEvent {
  final String modelId;

  const ModelUnloadedEvent({
    required this.modelId,
  }) : super('model_unloaded');
}

class BenchmarkCompletedEvent extends BridgeEvent {
  final String metricsJson;

  const BenchmarkCompletedEvent({
    required this.metricsJson,
  }) : super('benchmark_completed');
}

class UnknownBridgeEvent extends BridgeEvent {
  final Map<String, dynamic> payload;

  const UnknownBridgeEvent({
    required String type,
    required this.payload,
  }) : super(type);
}
