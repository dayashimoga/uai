import 'package:flutter/foundation.dart';

/// Supported trigger categories for automated workflows.
enum AutomationTriggerType {
  cron('cron', 'Cron Schedule'),
  event('event', 'System Event'),
  manual('manual', 'Manual Trigger');

  final String value;
  final String label;
  const AutomationTriggerType(this.value, this.label);

  static AutomationTriggerType fromString(String val) {
    return AutomationTriggerType.values.firstWhere(
      (e) => e.value == val.toLowerCase().trim(),
      orElse: () => AutomationTriggerType.manual,
    );
  }
}

/// Execution outcome status for workflow history records.
enum AutomationStatus {
  success('success', 'Success'),
  failed('failed', 'Failed'),
  cancelled('cancelled', 'Cancelled');

  final String value;
  final String label;
  const AutomationStatus(this.value, this.label);

  static AutomationStatus fromString(String val) {
    return AutomationStatus.values.firstWhere(
      (e) => e.value == val.toLowerCase().trim(),
      orElse: () => AutomationStatus.failed,
    );
  }
}

/// Representation of an action step within an automated workflow.
@immutable
class WorkflowActionItem {
  final String actionType;
  final String? skillId;
  final String? peerId;
  final String? payloadJson;
  final String? title;
  final String? message;

  const WorkflowActionItem({
    required this.actionType,
    this.skillId,
    this.peerId,
    this.payloadJson,
    this.title,
    this.message,
  });

  factory WorkflowActionItem.fromJson(Map<String, dynamic> json) {
    return WorkflowActionItem(
      actionType: json['action_type'] as String? ?? 'notification',
      skillId: json['skill_id'] as String?,
      peerId: json['peer_id'] as String?,
      payloadJson: json['payload_json'] as String?,
      title: json['title'] as String?,
      message: json['message'] as String?,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'action_type': actionType,
      if (skillId != null) 'skill_id': skillId,
      if (peerId != null) 'peer_id': peerId,
      if (payloadJson != null) 'payload_json': payloadJson,
      if (title != null) 'title': title,
      if (message != null) 'message': message,
    };
  }
}

/// Automated workflow rule across the FFI bridge.
@immutable
class AutomationRuleItem {
  final String id;
  final String name;
  final bool enabled;
  final String triggerType;
  final String triggerConfigJson;
  final String actionsJson;
  final int? lastExecutedMs;
  final int createdAtMs;
  final int updatedAtMs;

  const AutomationRuleItem({
    required this.id,
    required this.name,
    required this.enabled,
    required this.triggerType,
    required this.triggerConfigJson,
    required this.actionsJson,
    this.lastExecutedMs,
    required this.createdAtMs,
    required this.updatedAtMs,
  });

  factory AutomationRuleItem.fromJson(Map<String, dynamic> json) {
    return AutomationRuleItem(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      enabled: json['enabled'] as bool? ?? true,
      triggerType: json['trigger_type'] as String? ?? 'manual',
      triggerConfigJson: json['trigger_config_json'] as String? ?? '{}',
      actionsJson: json['actions_json'] as String? ?? '[]',
      lastExecutedMs: json['last_executed_ms'] as int?,
      createdAtMs: json['created_at_ms'] as int? ?? 0,
      updatedAtMs: json['updated_at_ms'] as int? ?? 0,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'name': name,
      'enabled': enabled,
      'trigger_type': triggerType,
      'trigger_config_json': triggerConfigJson,
      'actions_json': actionsJson,
      if (lastExecutedMs != null) 'last_executed_ms': lastExecutedMs,
      'created_at_ms': createdAtMs,
      'updated_at_ms': updatedAtMs,
    };
  }

  AutomationRuleItem copyWith({
    String? id,
    String? name,
    bool? enabled,
    String? triggerType,
    String? triggerConfigJson,
    String? actionsJson,
    int? lastExecutedMs,
    int? createdAtMs,
    int? updatedAtMs,
  }) {
    return AutomationRuleItem(
      id: id ?? this.id,
      name: name ?? this.name,
      enabled: enabled ?? this.enabled,
      triggerType: triggerType ?? this.triggerType,
      triggerConfigJson: triggerConfigJson ?? this.triggerConfigJson,
      actionsJson: actionsJson ?? this.actionsJson,
      lastExecutedMs: lastExecutedMs ?? this.lastExecutedMs,
      createdAtMs: createdAtMs ?? this.createdAtMs,
      updatedAtMs: updatedAtMs ?? this.updatedAtMs,
    );
  }
}

/// Historical record documenting an execution of an automation rule.
@immutable
class AutomationHistoryItem {
  final String id;
  final String automationId;
  final String status;
  final String triggeredBy;
  final int executedAtMs;
  final int durationMs;
  final String resultJson;

  const AutomationHistoryItem({
    required this.id,
    required this.automationId,
    required this.status,
    required this.triggeredBy,
    required this.executedAtMs,
    required this.durationMs,
    required this.resultJson,
  });

  factory AutomationHistoryItem.fromJson(Map<String, dynamic> json) {
    return AutomationHistoryItem(
      id: json['id'] as String? ?? '',
      automationId: json['automation_id'] as String? ?? '',
      status: json['status'] as String? ?? 'success',
      triggeredBy: json['triggered_by'] as String? ?? '',
      executedAtMs: json['executed_at_ms'] as int? ?? 0,
      durationMs: json['duration_ms'] as int? ?? 0,
      resultJson: json['result_json'] as String? ?? '[]',
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'automation_id': automationId,
      'status': status,
      'triggered_by': triggeredBy,
      'executed_at_ms': executedAtMs,
      'duration_ms': durationMs,
      'result_json': resultJson,
    };
  }
}

/// Comprehensive report produced when an automation rule executes.
@immutable
class WorkflowExecutionReportItem {
  final String automationId;
  final String status;
  final String triggeredBy;
  final int durationMs;
  final int executedAtMs;
  final String resultJson;

  const WorkflowExecutionReportItem({
    required this.automationId,
    required this.status,
    required this.triggeredBy,
    required this.durationMs,
    required this.executedAtMs,
    required this.resultJson,
  });

  factory WorkflowExecutionReportItem.fromJson(Map<String, dynamic> json) {
    return WorkflowExecutionReportItem(
      automationId: json['automation_id'] as String? ?? '',
      status: json['status'] as String? ?? 'success',
      triggeredBy: json['triggered_by'] as String? ?? '',
      durationMs: json['duration_ms'] as int? ?? 0,
      executedAtMs: json['executed_at_ms'] as int? ?? 0,
      resultJson: json['result_json'] as String? ?? '[]',
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'automation_id': automationId,
      'status': status,
      'triggered_by': triggeredBy,
      'duration_ms': durationMs,
      'executed_at_ms': executedAtMs,
      'result_json': resultJson,
    };
  }
}
