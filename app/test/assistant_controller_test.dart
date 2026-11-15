import 'package:flutter_test/flutter_test.dart';
import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/core/models/health_report.dart';

void main() {
  group('AssistantController State Management Tests', () {
    late TestHarnessBridge bridge;
    late AssistantController controller;

    setUp(() {
      bridge = TestHarnessBridge();
      controller = AssistantController(bridge);
    });

    tearDown(() {
      controller.dispose();
    });

    test('initializes and loads live snapshot from bridge', () async {
      expect(controller.health, isNull);
      expect(controller.capabilities, isEmpty);

      await controller.initialize();

      expect(controller.health, isNotNull);
      expect(controller.health!.status, HealthStatus.healthy);
      expect(controller.capabilities.length, 9);
      expect(controller.auditLogs.length, 1);
      expect(controller.auditIntegrity?.isValid, isTrue);
      expect(controller.themeMode, 'system');
      expect(controller.isLoading, isFalse);
    });

    test('toggles capability permission and updates active state', () async {
      await controller.initialize();

      // Deactivate file_system
      controller.toggleCapability('file_system', false);
      final cap = controller.capabilities.firstWhere((c) => c.name == 'file_system');
      expect(cap.isActive, isFalse);
      expect(cap.status, 'degraded');

      // Reactivate file_system
      controller.toggleCapability('file_system', true);
      final capActive = controller.capabilities.firstWhere((c) => c.name == 'file_system');
      expect(capActive.isActive, isTrue);
      expect(capActive.status, 'supported');
    });

    test('verifies audit log integrity', () async {
      await controller.initialize();
      controller.verifyAuditIntegrity();

      expect(controller.auditIntegrity, isNotNull);
      expect(controller.auditIntegrity!.isValid, isTrue);
      expect(controller.auditIntegrity!.verifiedCount, 1);
    });

    test('sets theme mode and updates setting', () async {
      await controller.initialize();
      expect(controller.themeMode, 'system');

      controller.setThemeMode('dark');
      expect(controller.themeMode, 'dark');
      expect(bridge.getSetting('assistant.theme_mode'), '"dark"');
    });

    test('handles memory pressure and updates diagnostics', () async {
      await controller.initialize();
      controller.triggerMemoryPressure(true); // critical

      expect(controller.health?.status, HealthStatus.degraded);
    });

    test('triggers graceful shutdown and updates lifecycle state', () async {
      await controller.initialize();
      expect(controller.health?.lifecycleState, 'running');

      controller.triggerShutdown();
      expect(controller.health?.lifecycleState, 'terminated');
    });
  });
}
