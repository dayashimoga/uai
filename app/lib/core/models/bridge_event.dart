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

      case 'model_download_progress':
        return ModelDownloadProgressEvent(
          modelId: payload['model_id'] as String? ?? '',
          downloadedBytes: (payload['downloaded_bytes'] as num?)?.toInt() ?? 0,
          totalBytes: (payload['total_bytes'] as num?)?.toInt() ?? 0,
          speedBps: (payload['speed_bps'] as num?)?.toInt() ?? 0,
        );

      case 'model_download_completed':
        return ModelDownloadCompletedEvent(
          modelId: payload['model_id'] as String? ?? '',
          localPath: payload['local_path'] as String? ?? '',
          sha256Verified: payload['sha256_verified'] as bool? ?? false,
        );

      case 'model_download_failed':
        return ModelDownloadFailedEvent(
          modelId: payload['model_id'] as String? ?? '',
          error: payload['error'] as String? ?? 'Download failed',
        );

      case 'benchmark_completed':
        return BenchmarkCompletedEvent(
          metricsJson: payload['metrics_json'] as String? ?? '{}',
        );

      case 'device_discovered':
        return DeviceDiscoveredEvent(
          instanceName: payload['instance_name'] as String? ?? '',
          platform: payload['platform'] as String? ?? 'unknown',
          endpoint: payload['endpoint'] as String?,
          fingerprint: payload['fingerprint'] as String? ?? '',
        );

      case 'pairing_state_changed':
        return PairingStateChangedEvent(
          peerInstance: payload['peer_instance'] as String? ?? '',
          status: payload['status'] as String? ?? '',
          sasCode: payload['sas_code'] as String?,
          qrPayload: payload['qr_payload'] as String?,
          error: payload['error'] as String?,
        );

      case 'key_rotated':
        return KeyRotatedEvent(
          timestampMs: (payload['timestamp_ms'] as num?)?.toInt() ?? 0,
        );

      case 'device_revoked':
        return DeviceRevokedEvent(
          deviceId: payload['device_id'] as String? ?? '',
        );

      case 'emergency_lockdown':
        return EmergencyLockdownEvent(
          active: payload['active'] as bool? ?? false,
          reason: payload['reason'] as String? ?? '',
        );

      case 'performance_metrics_updated':
        return PerformanceMetricsUpdatedEvent(
          coldStartMs: (payload['cold_start_ms'] as num?)?.toInt() ?? 0,
          wakeWordFrameMicros:
              (payload['wake_word_frame_micros'] as num?)?.toInt() ?? 0,
          sttLatencyMs: (payload['stt_latency_ms'] as num?)?.toInt() ?? 0,
          ttsFirstAudioMs:
              (payload['tts_first_audio_ms'] as num?)?.toInt() ?? 0,
          residentMemoryBytes:
              (payload['resident_memory_bytes'] as num?)?.toInt() ?? 0,
          idleBudgetBytes:
              (payload['idle_budget_bytes'] as num?)?.toInt() ?? 0,
          memoryBudgetExceeded:
              payload['memory_budget_exceeded'] as bool? ?? false,
        );

      case 'power_transition':
        return PowerTransitionEvent(
          event: payload['event'] as String? ?? 'suspend',
        );

      case 'chat_token_delta':
        return ChatTokenDeltaEvent(
          turnId: payload['turn_id'] as String? ?? '',
          token: payload['token'] as String? ?? '',
        );

      case 'chat_turn_complete':
        return ChatTurnCompleteEvent(
          turnId: payload['turn_id'] as String? ?? '',
          citations: (payload['citations'] as List<dynamic>?)
                  ?.map((e) => e.toString())
                  .toList() ??
              const [],
        );

      case 'tool_confirmation_required':
        return ToolConfirmationRequiredEvent(
          turnId: payload['turn_id'] as String? ?? '',
          toolName: payload['tool_name'] as String? ?? '',
          riskTier: payload['risk_tier'] as String? ?? '',
          paramsJson: payload['params_json'] as String? ?? '{}',
          taintStatus: payload['taint_status'] as bool? ?? false,
        );

      case 'tool_confirmation_resolved':
        return ToolConfirmationResolvedEvent(
          confirmationId: payload['confirmation_id'] as String? ?? '',
          approved: payload['approved'] as bool? ?? false,
        );

      case 'speech_model_progress':
        return SpeechModelProgressEvent(
          status: payload['status'] as String? ?? 'ready',
          downloadedBytes: (payload['downloaded_bytes'] as num?)?.toInt() ?? 0,
          totalBytes: (payload['total_bytes'] as num?)?.toInt() ?? 0,
          percent: (payload['percent'] as num?)?.toInt() ?? 0,
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

class ModelDownloadProgressEvent extends BridgeEvent {
  final String modelId;
  final int downloadedBytes;
  final int totalBytes;
  final int speedBps;

  const ModelDownloadProgressEvent({
    required this.modelId,
    required this.downloadedBytes,
    required this.totalBytes,
    required this.speedBps,
  }) : super('model_download_progress');
}

class ModelDownloadCompletedEvent extends BridgeEvent {
  final String modelId;
  final String localPath;
  final bool sha256Verified;

  const ModelDownloadCompletedEvent({
    required this.modelId,
    required this.localPath,
    required this.sha256Verified,
  }) : super('model_download_completed');
}

class ModelDownloadFailedEvent extends BridgeEvent {
  final String modelId;
  final String error;

  const ModelDownloadFailedEvent({
    required this.modelId,
    required this.error,
  }) : super('model_download_failed');
}

class BenchmarkCompletedEvent extends BridgeEvent {
  final String metricsJson;

  const BenchmarkCompletedEvent({
    required this.metricsJson,
  }) : super('benchmark_completed');
}

class DeviceDiscoveredEvent extends BridgeEvent {
  final String instanceName;
  final String platform;
  final String? endpoint;
  final String fingerprint;

  const DeviceDiscoveredEvent({
    required this.instanceName,
    required this.platform,
    this.endpoint,
    required this.fingerprint,
  }) : super('device_discovered');
}

class PairingStateChangedEvent extends BridgeEvent {
  final String peerInstance;
  final String status;
  final String? sasCode;
  final String? qrPayload;
  final String? error;

  const PairingStateChangedEvent({
    required this.peerInstance,
    required this.status,
    this.sasCode,
    this.qrPayload,
    this.error,
  }) : super('pairing_state_changed');
}

class KeyRotatedEvent extends BridgeEvent {
  final int timestampMs;

  const KeyRotatedEvent({
    required this.timestampMs,
  }) : super('key_rotated');
}

class DeviceRevokedEvent extends BridgeEvent {
  final String deviceId;

  const DeviceRevokedEvent({
    required this.deviceId,
  }) : super('device_revoked');
}

class EmergencyLockdownEvent extends BridgeEvent {
  final bool active;
  final String reason;

  const EmergencyLockdownEvent({
    required this.active,
    required this.reason,
  }) : super('emergency_lockdown');
}

class PerformanceMetricsUpdatedEvent extends BridgeEvent {
  final int coldStartMs;
  final int wakeWordFrameMicros;
  final int sttLatencyMs;
  final int ttsFirstAudioMs;
  final int residentMemoryBytes;
  final int idleBudgetBytes;
  final bool memoryBudgetExceeded;

  const PerformanceMetricsUpdatedEvent({
    required this.coldStartMs,
    required this.wakeWordFrameMicros,
    required this.sttLatencyMs,
    required this.ttsFirstAudioMs,
    required this.residentMemoryBytes,
    required this.idleBudgetBytes,
    required this.memoryBudgetExceeded,
  }) : super('performance_metrics_updated');
}

class PowerTransitionEvent extends BridgeEvent {
  final String event;

  const PowerTransitionEvent({
    required this.event,
  }) : super('power_transition');
}

class ChatTokenDeltaEvent extends BridgeEvent {
  final String turnId;
  final String token;

  const ChatTokenDeltaEvent({
    required this.turnId,
    required this.token,
  }) : super('chat_token_delta');
}

class ChatTurnCompleteEvent extends BridgeEvent {
  final String turnId;
  final List<String> citations;

  const ChatTurnCompleteEvent({
    required this.turnId,
    required this.citations,
  }) : super('chat_turn_complete');
}

class ToolConfirmationRequiredEvent extends BridgeEvent {
  final String turnId;
  final String toolName;
  final String riskTier;
  final String paramsJson;
  final bool taintStatus;

  const ToolConfirmationRequiredEvent({
    required this.turnId,
    required this.toolName,
    required this.riskTier,
    required this.paramsJson,
    required this.taintStatus,
  }) : super('tool_confirmation_required');
}

class ToolConfirmationResolvedEvent extends BridgeEvent {
  final String confirmationId;
  final bool approved;

  const ToolConfirmationResolvedEvent({
    required this.confirmationId,
    required this.approved,
  }) : super('tool_confirmation_resolved');
}

class SpeechModelProgressEvent extends BridgeEvent {
  final String status;
  final int downloadedBytes;
  final int totalBytes;
  final int percent;

  const SpeechModelProgressEvent({
    required this.status,
    required this.downloadedBytes,
    required this.totalBytes,
    required this.percent,
  }) : super('speech_model_progress');
}

class UnknownBridgeEvent extends BridgeEvent {
  final Map<String, dynamic> payload;

  const UnknownBridgeEvent({
    required String type,
    required this.payload,
  }) : super(type);
}
