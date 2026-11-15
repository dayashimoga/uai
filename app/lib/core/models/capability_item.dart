// Capability status model matching aissistant-ffi JSON serialization.

class CapabilityItem {
  final String name;
  final String tier;
  final bool isSupported;
  final bool isActive;
  final String status;
  final String? degradationReason;
  final String? missingPermission;

  const CapabilityItem({
    required this.name,
    required this.tier,
    required this.isSupported,
    required this.isActive,
    required this.status,
    this.degradationReason,
    this.missingPermission,
  });

  factory CapabilityItem.fromJson(Map<String, dynamic> json) {
    return CapabilityItem(
      name: json['name'] as String? ?? 'unknown',
      tier: json['tier'] as String? ?? 'standard',
      isSupported: json['is_supported'] as bool? ?? false,
      isActive: json['is_active'] as bool? ?? false,
      status: json['status'] as String? ?? 'unsupported',
      degradationReason: json['degradation_reason'] as String?,
      missingPermission: json['missing_permission'] as String?,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'name': name,
        'tier': tier,
        'is_supported': isSupported,
        'is_active': isActive,
        'status': status,
        if (degradationReason != null) 'degradation_reason': degradationReason,
        if (missingPermission != null) 'missing_permission': missingPermission,
      };

  String get displayName {
    return name
        .split('_')
        .map((word) => word.isEmpty
            ? ''
            : '${word[0].toUpperCase()}${word.substring(1)}')
        .join(' ');
  }
}
