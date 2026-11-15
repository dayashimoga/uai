import 'package:flutter_test/flutter_test.dart';
import 'package:aissistant/core/models/audit_item.dart';
import 'package:aissistant/core/models/bridge_event.dart';
import 'package:aissistant/core/models/capability_item.dart';
import 'package:aissistant/core/models/health_report.dart';

void main() {
  group('HealthReport Model Tests', () {
    test('parses full json from Rust core correctly', () {
      final json = <String, dynamic>{
        'status': 'healthy',
        'lifecycle_state': 'running',
        'active_tasks': 2,
        'database_ok': true,
        'audit_ok': true,
        'keystore_ok': true,
        'total_capabilities': 9,
        'active_capabilities': 7,
        'audit_count': 42,
        'details': <String>['Notice: mic permission granted'],
      };

      final report = HealthReport.fromJson(json);

      expect(report.status, HealthStatus.healthy);
      expect(report.lifecycleState, 'running');
      expect(report.activeTasks, 2);
      expect(report.databaseOk, isTrue);
      expect(report.auditOk, isTrue);
      expect(report.keystoreOk, isTrue);
      expect(report.totalCapabilities, 9);
      expect(report.activeCapabilities, 7);
      expect(report.auditCount, 42);
      expect(report.details, contains('Notice: mic permission granted'));

      final encoded = report.toJson();
      expect(encoded['status'], 'healthy');
      expect(encoded['active_tasks'], 2);
    });

    test('gracefully handles degraded status and empty details', () {
      final json = <String, dynamic>{
        'status': 'degraded',
      };
      final report = HealthReport.fromJson(json);
      expect(report.status, HealthStatus.degraded);
      expect(report.details, isEmpty);
      expect(report.databaseOk, isFalse);
    });
  });

  group('CapabilityItem Model Tests', () {
    test('parses capability item and computes display name', () {
      final json = <String, dynamic>{
        'name': 'offline_speech',
        'tier': 'standard',
        'is_supported': true,
        'is_active': true,
        'status': 'supported',
        'degradation_reason': null,
        'missing_permission': null,
      };

      final item = CapabilityItem.fromJson(json);
      expect(item.name, 'offline_speech');
      expect(item.displayName, 'Offline Speech');
      expect(item.isSupported, isTrue);
      expect(item.isActive, isTrue);
      expect(item.status, 'supported');
    });

    test('serializes capability item correctly', () {
      const item = CapabilityItem(
        name: 'screen_streaming_host',
        tier: 'privileged',
        isSupported: true,
        isActive: false,
        status: 'requires_permission',
        missingPermission: 'tcc.screen_capture',
      );

      final map = item.toJson();
      expect(map['name'], 'screen_streaming_host');
      expect(map['missing_permission'], 'tcc.screen_capture');
    });
  });

  group('AuditItem & Integrity Report Tests', () {
    test('parses audit item and formats timestamp', () {
      final json = <String, dynamic>{
        'sequence': 10,
        'timestamp_ms': 1731671000000,
        'actor': 'local_user',
        'skill_id': 'file.read',
        'payload_summary': '{"path":"/tmp/test"}',
        'decision': 'allow',
        'execution_status': 'success',
        'entry_hash': 'abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890',
        'prev_hash': '0000000000000000000000000000000000000000000000000000000000000000',
      };

      final entry = AuditItem.fromJson(json);
      expect(entry.sequence, 10);
      expect(entry.actor, 'local_user');
      expect(entry.skillId, 'file.read');
      expect(entry.decision, 'allow');
      expect(entry.executionStatus, 'success');
      expect(entry.entryHash, startsWith('abcdef'));
      expect(entry.dateTime.year, greaterThanOrEqualTo(2024));
    });

    test('parses audit integrity report', () {
      final json = <String, dynamic>{
        'is_valid': true,
        'verified_count': 100,
        'head_hash': 'feedbeef00112233',
      };

      final report = AuditIntegrityReport.fromJson(json);
      expect(report.isValid, isTrue);
      expect(report.verifiedCount, 100);
      expect(report.headHash, 'feedbeef00112233');
      expect(report.errorMessage, isNull);
    });
  });

  group('BridgeEvent Tests', () {
    test('parses health status changed event', () {
      final json = <String, dynamic>{
        'type': 'health_status_changed',
        'payload': <String, dynamic>{
          'status': 'degraded',
          'details': <String>['Storage near threshold'],
        },
      };

      final event = BridgeEvent.fromJson(json);
      expect(event, isA<HealthStatusChangedEvent>());
      final typed = event as HealthStatusChangedEvent;
      expect(typed.status, 'degraded');
      expect(typed.details, contains('Storage near threshold'));
    });

    test('parses capability changed event', () {
      final json = <String, dynamic>{
        'type': 'capability_changed',
        'payload': <String, dynamic>{
          'capability': 'file_system',
          'status': 'supported',
          'reason': null,
        },
      };

      final event = BridgeEvent.fromJson(json);
      expect(event, isA<CapabilityChangedEvent>());
      final typed = event as CapabilityChangedEvent;
      expect(typed.capability, 'file_system');
      expect(typed.status, 'supported');
    });

    test('parses audit log appended event', () {
      final json = <String, dynamic>{
        'type': 'audit_log_appended',
        'payload': <String, dynamic>{
          'sequence': 5,
          'actor': 'llm_orchestrator',
          'skill_id': 'system.clock',
          'decision': 'allow',
          'execution_status': 'success',
          'entry_hash': 'hash123',
        },
      };

      final event = BridgeEvent.fromJson(json);
      expect(event, isA<AuditLogAppendedEvent>());
      final typed = event as AuditLogAppendedEvent;
      expect(typed.sequence, 5);
      expect(typed.skillId, 'system.clock');
    });

    test('parses lifecycle state changed event', () {
      final json = <String, dynamic>{
        'type': 'lifecycle_state_changed',
        'payload': <String, dynamic>{
          'previous': 'starting',
          'current': 'running',
          'reason': 'bootstrapped',
        },
      };

      final event = BridgeEvent.fromJson(json);
      expect(event, isA<LifecycleStateChangedEvent>());
      final typed = event as LifecycleStateChangedEvent;
      expect(typed.previous, 'starting');
      expect(typed.current, 'running');
    });

    test('parses memory pressure event', () {
      final json = <String, dynamic>{
        'type': 'memory_pressure',
        'payload': <String, dynamic>{
          'level': 'critical',
        },
      };

      final event = BridgeEvent.fromJson(json);
      expect(event, isA<MemoryPressureEvent>());
      final typed = event as MemoryPressureEvent;
      expect(typed.level, 'critical');
    });
  });
}
