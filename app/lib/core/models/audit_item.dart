// Audit item and integrity models matching aissistant-ffi JSON serialization.

class AuditItem {
  final int sequence;
  final int timestampMs;
  final String actor;
  final String skillId;
  final String payloadSummary;
  final String decision;
  final String executionStatus;
  final String entryHash;
  final String prevHash;

  const AuditItem({
    required this.sequence,
    required this.timestampMs,
    required this.actor,
    required this.skillId,
    required this.payloadSummary,
    required this.decision,
    required this.executionStatus,
    required this.entryHash,
    required this.prevHash,
  });

  factory AuditItem.fromJson(Map<String, dynamic> json) {
    return AuditItem(
      sequence: (json['sequence'] as num?)?.toInt() ?? 0,
      timestampMs: (json['timestamp_ms'] as num?)?.toInt() ?? 0,
      actor: json['actor'] as String? ?? 'unknown',
      skillId: json['skill_id'] as String? ?? 'unknown',
      payloadSummary: json['payload_summary'] as String? ?? '{}',
      decision: json['decision'] as String? ?? 'deny',
      executionStatus: json['execution_status'] as String? ?? 'failed',
      entryHash: json['entry_hash'] as String? ?? '',
      prevHash: json['prev_hash'] as String? ?? '',
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'sequence': sequence,
        'timestamp_ms': timestampMs,
        'actor': actor,
        'skill_id': skillId,
        'payload_summary': payloadSummary,
        'decision': decision,
        'execution_status': executionStatus,
        'entry_hash': entryHash,
        'prev_hash': prevHash,
      };

  DateTime get dateTime =>
      DateTime.fromMillisecondsSinceEpoch(timestampMs, isUtc: true).toLocal();
}

class AuditIntegrityReport {
  final bool isValid;
  final int verifiedCount;
  final String headHash;
  final String? errorMessage;

  const AuditIntegrityReport({
    required this.isValid,
    required this.verifiedCount,
    required this.headHash,
    this.errorMessage,
  });

  factory AuditIntegrityReport.fromJson(Map<String, dynamic> json) {
    return AuditIntegrityReport(
      isValid: json['is_valid'] as bool? ?? false,
      verifiedCount: (json['verified_count'] as num?)?.toInt() ?? 0,
      headHash: json['head_hash'] as String? ?? '',
      errorMessage: json['error_message'] as String?,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'is_valid': isValid,
        'verified_count': verifiedCount,
        'head_hash': headHash,
        if (errorMessage != null) 'error_message': errorMessage,
      };
}
