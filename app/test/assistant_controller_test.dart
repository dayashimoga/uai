import 'package:flutter_test/flutter_test.dart';
import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/core/models/automation_models.dart';
import 'package:aissistant/core/models/device_models.dart';
import 'package:aissistant/core/models/health_report.dart';
import 'package:aissistant/core/models/voice_state.dart';

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
      expect(controller.capabilities.length, 11);
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

    test('manages voice listening state, modes and keywords', () async {
      await controller.initialize();
      expect(controller.voiceState.isListening, isFalse);
      expect(controller.voiceState.mode, VoiceMode.pushToTalk);

      // Start listening in push-to-talk mode
      controller.startVoiceListening(mode: VoiceMode.pushToTalk);
      expect(controller.voiceState.isListening, isTrue);
      expect(controller.voiceState.mode, VoiceMode.pushToTalk);

      // Stop listening
      controller.stopVoiceListening();
      expect(controller.voiceState.isListening, isFalse);

      // Switch mode to wake-word
      controller.setVoiceMode(VoiceMode.wakeWord);
      expect(controller.voiceState.mode, VoiceMode.wakeWord);

      // Update keywords
      controller.setVoiceKeywords(<String>['hey assistant', 'computer']);
      expect(controller.voiceState.activeKeywords, <String>['hey assistant', 'computer']);
    });

    test('updates voice state from bridge events and clears transcript', () async {
      await controller.initialize();

      // Audio level update
      controller.injectVoiceAudio(<double>[0.1, -0.2, 0.3, -0.1]);
      await Future<void>.delayed(Duration.zero);
      expect(controller.voiceState.rmsDbfs, greaterThan(-96.0));

      // Wake word event simulated
      bridge.setVoiceKeywords(<String>['hey assistant']);
      expect(controller.voiceState.lastSpottedKeyword, isNull);

      // Transcript streaming and clearing
      controller.clearTranscript();
      expect(controller.voiceState.partialTranscript, isEmpty);
      expect(controller.voiceState.finalTranscript, isEmpty);
    });

    test('manages discovered peers, pairing lifecycle and capability ACLs', () async {
      await controller.initialize();

      expect(controller.discoveredPeers.length, 1);
      expect(controller.discoveredPeers.first.instanceName, 'pixel-9-pro');

      expect(controller.pairedDevices.length, 1);
      expect(controller.pairedDevices.first.name, 'MacBook Pro M3');

      // Start pairing
      final session = await controller.startPairing('pixel-9-pro');
      expect(session, isNotNull);
      expect(controller.isPairing, isTrue);
      expect(controller.activePairingSession?.sasCode, '481-923');

      // Confirm pairing
      final paired = await controller.confirmPairing('481-923');
      expect(paired, isNotNull);
      expect(controller.isPairing, isFalse);
      expect(controller.pairedDevices.length, 2);

      // Update ACL
      controller.updatePeerPermission(paired!.deviceId, 'file_system', 'always_allow');
      final updated = controller.pairedDevices.firstWhere((d) => d.deviceId == paired.deviceId);
      final perm = updated.permissions.firstWhere((p) => p.capability == 'file_system');
      expect(perm.permissionLevel, 'always_allow');

      // Unpair device
      controller.unpairDevice(paired.deviceId);
      expect(controller.pairedDevices.length, 1);

      // Simulate discovered peer
      const sim = DiscoveredPeerItem(
        instanceName: 'sim-device',
        platform: 'linux',
        port: 51820,
        addresses: <String>['192.168.1.99'],
        fingerprint: '1122334455667788',
        lastSeenMs: 1000,
      );
      controller.simulateDiscoveredPeer(sim);
      expect(controller.discoveredPeers.any((p) => p.instanceName == 'sim-device'), isTrue);
    });

    test('manages automations: lists, saves, toggles, triggers and deletes workflow rules', () async {
      await controller.initialize();

      // 1. Initial list
      expect(controller.automations.length, 2);
      expect(controller.automations.first.id, 'auto_morning_brief');

      // 2. Toggle rule
      final toggled = await controller.toggleAutomation('auto_morning_brief', false);
      expect(toggled, isTrue);
      expect(
        controller.automations.firstWhere((r) => r.id == 'auto_morning_brief').enabled,
        isFalse,
      );

      // 3. Save new rule
      const newRule = AutomationRuleItem(
        id: 'auto_test_custom',
        name: 'Custom Test Rule',
        enabled: true,
        triggerType: 'manual',
        triggerConfigJson: '{"type":"manual"}',
        actionsJson: '[]',
        createdAtMs: 100,
        updatedAtMs: 100,
      );
      await controller.saveAutomation(newRule);
      expect(controller.automations.length, 3);
      expect(controller.automations.any((r) => r.id == 'auto_test_custom'), isTrue);

      // 4. Trigger automation
      final report = await controller.triggerAutomation('auto_test_custom');
      expect(report, isNotNull);
      expect(report!.status, 'success');
      expect(controller.lastWorkflowReport?.automationId, 'auto_test_custom');
      expect(controller.automationHistory.isNotEmpty, isTrue);

      // 5. Query history
      await controller.loadAutomationHistory(automationId: 'auto_test_custom');
      expect(controller.automationHistory.length, 1);
      expect(controller.automationHistory.first.automationId, 'auto_test_custom');

      // 6. Delete automation
      final deleted = await controller.deleteAutomation('auto_test_custom');
      expect(deleted, isTrue);
      expect(controller.automations.length, 2);
      expect(controller.automations.any((r) => r.id == 'auto_test_custom'), isFalse);
    });
  });
}
