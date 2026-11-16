import 'package:flutter_test/flutter_test.dart';
import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/core/models/bridge_event.dart';
import 'package:aissistant/core/models/health_report.dart';

void main() {
  group('AssistantController Soak & Chaos Tests', () {
    late TestHarnessBridge bridge;
    late AssistantController controller;

    setUp(() {
      bridge = TestHarnessBridge();
      controller = AssistantController(bridge);
    });

    tearDown(() {
      controller.dispose();
    });

    test('50-turn controller soak stress loop maintains consistent state', () async {
      await controller.initialize();
      expect(controller.health?.status, HealthStatus.healthy);

      // Start multi-turn conversational session
      final session = await controller.startInterpreterSession(
        partyALang: 'en',
        partyBLang: 'es',
      );
      expect(session, isNotNull);

      for (int i = 1; i <= 50; i++) {
        // Submit simulated turn
        final turn = await controller.submitInterpreterTurn('Soak query turn #$i');
        expect(turn, isNotNull);
        expect(controller.interpreterTurns.length, i);

        // Every 10 turns, run memory optimization
        if (i % 10 == 0) {
          final withinBudget = await controller.optimizeMemory();
          expect(withinBudget, isTrue);
          expect(controller.performanceSnapshot, isNotNull);
        }
      }

      expect(controller.interpreterTurns.length, 50);
      expect(controller.errorMessage, isNull);
    });

    test('OS power transition suspend and resume cycle recovers cleanly', () async {
      await controller.initialize();
      expect(controller.lastPowerTransition, isNull);

      // 1. Suspend via controller action
      await controller.handlePowerTransition('suspend');
      expect(controller.lastPowerTransition, 'suspend');

      // 2. Resume via bridge event broadcast
      bridge.emitEvent(const PowerTransitionEvent(event: 'resume'));
      await Future<void>.delayed(const Duration(milliseconds: 10));

      expect(controller.lastPowerTransition, 'resume');
      expect(controller.health?.status, HealthStatus.healthy);
      expect(controller.errorMessage, isNull);
    });

    test('interleaved chaos events (memory pressure, power cycle, performance telemetry) are handled safely', () async {
      await controller.initialize();

      // Interleave memory pressure, power cycle, and telemetry events
      bridge.emitEvent(const MemoryPressureEvent(level: 'critical'));
      bridge.emitEvent(const PowerTransitionEvent(event: 'suspend'));
      bridge.emitEvent(const PowerTransitionEvent(event: 'resume'));
      bridge.emitEvent(const PerformanceMetricsUpdatedEvent(
        coldStartMs: 120,
        wakeWordFrameMicros: 350,
        sttLatencyMs: 280,
        ttsFirstAudioMs: 190,
        residentMemoryBytes: 42 * 1024 * 1024,
        idleBudgetBytes: 80 * 1024 * 1024,
        memoryBudgetExceeded: false,
      ));

      await Future<void>.delayed(const Duration(milliseconds: 20));

      expect(controller.lastPowerTransition, 'resume');
      expect(controller.performanceSnapshot?.residentMemoryBytes, 42 * 1024 * 1024);
      expect(controller.performanceSnapshot?.memoryBudgetExceeded, isFalse);
      expect(controller.errorMessage, isNull);
    });
  });
}
