/// Data models for local network peer discovery, device pairing, and ACL management.
library;

class DiscoveredPeerItem {
  final String instanceName;
  final String platform;
  final int port;
  final List<String> addresses;
  final String fingerprint;
  final int lastSeenMs;

  const DiscoveredPeerItem({
    required this.instanceName,
    required this.platform,
    required this.port,
    required this.addresses,
    required this.fingerprint,
    required this.lastSeenMs,
  });

  factory DiscoveredPeerItem.fromJson(Map<String, dynamic> json) {
    return DiscoveredPeerItem(
      instanceName: json['instance_name'] as String? ?? '',
      platform: json['platform'] as String? ?? 'unknown',
      port: (json['port'] as num?)?.toInt() ?? 0,
      addresses: (json['addresses'] as List<dynamic>?)
              ?.map((e) => e.toString())
              .toList() ??
          const <String>[],
      fingerprint: json['fingerprint'] as String? ?? '',
      lastSeenMs: (json['last_seen_ms'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, dynamic> toJson() => {
        'instance_name': instanceName,
        'platform': platform,
        'port': port,
        'addresses': addresses,
        'fingerprint': fingerprint,
        'last_seen_ms': lastSeenMs,
      };
}

class PeerPermissionItem {
  final String capability;
  final String permissionLevel;

  const PeerPermissionItem({
    required this.capability,
    required this.permissionLevel,
  });

  factory PeerPermissionItem.fromJson(Map<String, dynamic> json) {
    return PeerPermissionItem(
      capability: json['capability'] as String? ?? '',
      permissionLevel: json['permission_level'] as String? ?? 'ask_each_time',
    );
  }

  Map<String, dynamic> toJson() => {
        'capability': capability,
        'permission_level': permissionLevel,
      };
}

class PairedDeviceItem {
  final String deviceId;
  final String name;
  final String platform;
  final String fingerprint;
  final String trustState;
  final String? lastEndpoint;
  final int lastSeenMs;
  final int pairedAtMs;
  final List<PeerPermissionItem> permissions;

  const PairedDeviceItem({
    required this.deviceId,
    required this.name,
    required this.platform,
    required this.fingerprint,
    required this.trustState,
    this.lastEndpoint,
    required this.lastSeenMs,
    required this.pairedAtMs,
    required this.permissions,
  });

  factory PairedDeviceItem.fromJson(Map<String, dynamic> json) {
    return PairedDeviceItem(
      deviceId: json['device_id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      platform: json['platform'] as String? ?? 'unknown',
      fingerprint: json['fingerprint'] as String? ?? '',
      trustState: json['trust_state'] as String? ?? 'trusted',
      lastEndpoint: json['last_endpoint'] as String?,
      lastSeenMs: (json['last_seen_ms'] as num?)?.toInt() ?? 0,
      pairedAtMs: (json['paired_at_ms'] as num?)?.toInt() ?? 0,
      permissions: (json['permissions'] as List<dynamic>?)
              ?.map((e) => PeerPermissionItem.fromJson(e as Map<String, dynamic>))
              .toList() ??
          const <PeerPermissionItem>[],
    );
  }

  Map<String, dynamic> toJson() => {
        'device_id': deviceId,
        'name': name,
        'platform': platform,
        'fingerprint': fingerprint,
        'trust_state': trustState,
        'last_endpoint': lastEndpoint,
        'last_seen_ms': lastSeenMs,
        'paired_at_ms': pairedAtMs,
        'permissions': permissions.map((p) => p.toJson()).toList(),
      };
}

class PairingSessionInfo {
  final String sessionId;
  final String peerInstance;
  final String peerPlatform;
  final String sasCode;
  final String qrPayload;
  final int createdAtSecs;

  const PairingSessionInfo({
    required this.sessionId,
    required this.peerInstance,
    required this.peerPlatform,
    required this.sasCode,
    required this.qrPayload,
    required this.createdAtSecs,
  });

  factory PairingSessionInfo.fromJson(Map<String, dynamic> json) {
    return PairingSessionInfo(
      sessionId: json['session_id'] as String? ?? '',
      peerInstance: json['peer_instance'] as String? ?? '',
      peerPlatform: json['peer_platform'] as String? ?? '',
      sasCode: json['sas_code'] as String? ?? '',
      qrPayload: json['qr_payload'] as String? ?? '',
      createdAtSecs: (json['created_at_secs'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, dynamic> toJson() => {
        'session_id': sessionId,
        'peer_instance': peerInstance,
        'peer_platform': peerPlatform,
        'sas_code': sasCode,
        'qr_payload': qrPayload,
        'created_at_secs': createdAtSecs,
      };
}
