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

class UnknownBridgeEvent extends BridgeEvent {
  final Map<String, dynamic> payload;

  const UnknownBridgeEvent({
    required String type,
    required this.payload,
  }) : super(type);
}
