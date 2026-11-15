// Health report models matching aissistant-ffi JSON serialization.

enum HealthStatus {
  healthy,
  degraded,
  critical;

  static HealthStatus fromString(String val) {
    switch (val.toLowerCase()) {
      case 'healthy':
        return HealthStatus.healthy;
      case 'degraded':
        return HealthStatus.degraded;
      case 'critical':
      default:
        return HealthStatus.critical;
    }
  }

  String get label {
    switch (this) {
      case HealthStatus.healthy:
        return 'Healthy';
      case HealthStatus.degraded:
        return 'Degraded';
      case HealthStatus.critical:
        return 'Critical';
    }
  }
}

class HealthReport {
  final HealthStatus status;
  final String lifecycleState;
  final int activeTasks;
  final bool databaseOk;
  final bool auditOk;
  final bool keystoreOk;
  final int totalCapabilities;
  final int activeCapabilities;
  final int auditCount;
  final List<String> details;

  const HealthReport({
    required this.status,
    required this.lifecycleState,
    required this.activeTasks,
    required this.databaseOk,
    required this.auditOk,
    required this.keystoreOk,
    required this.totalCapabilities,
    required this.activeCapabilities,
    required this.auditCount,
    required this.details,
  });

  factory HealthReport.fromJson(Map<String, dynamic> json) {
    return HealthReport(
      status: HealthStatus.fromString(json['status'] as String? ?? 'critical'),
      lifecycleState: json['lifecycle_state'] as String? ?? 'uninitialized',
      activeTasks: (json['active_tasks'] as num?)?.toInt() ?? 0,
      databaseOk: json['database_ok'] as bool? ?? false,
      auditOk: json['audit_ok'] as bool? ?? false,
      keystoreOk: json['keystore_ok'] as bool? ?? false,
      totalCapabilities: (json['total_capabilities'] as num?)?.toInt() ?? 0,
      activeCapabilities: (json['active_capabilities'] as num?)?.toInt() ?? 0,
      auditCount: (json['audit_count'] as num?)?.toInt() ?? 0,
      details: (json['details'] as List<dynamic>?)
              ?.map((e) => e.toString())
              .toList() ??
          const <String>[],
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'status': status.name,
        'lifecycle_state': lifecycleState,
        'active_tasks': activeTasks,
        'database_ok': databaseOk,
        'audit_ok': auditOk,
        'keystore_ok': keystoreOk,
        'total_capabilities': totalCapabilities,
        'active_capabilities': activeCapabilities,
        'audit_count': auditCount,
        'details': details,
      };
}
