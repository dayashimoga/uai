import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/ui/screens/settings_screen.dart';

void main() {
  group('SettingsScreen Widget & Security Action Tests', () {
    late TestHarnessBridge bridge;
    late AssistantController controller;

    setUp(() async {
      bridge = TestHarnessBridge();
      controller = AssistantController(bridge);
      await controller.initialize();
    });

    Widget createTestApp(WidgetTester tester) {
      tester.view.physicalSize = const Size(1024, 1200);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      return MaterialApp(
        theme: ThemeData.light(useMaterial3: true),
        home: AnimatedBuilder(
          animation: controller,
          builder: (context, _) => SettingsScreen(controller: controller),
        ),
      );
    }

    testWidgets('renders settings screen with appearance and storage tiles', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      expect(find.text('Settings'), findsOneWidget);
      expect(find.text('Appearance & Display'), findsOneWidget);
      expect(find.text('Offline Storage & Privacy'), findsOneWidget);
      expect(find.text('Master Secret Encryption'), findsOneWidget);
      expect(find.text('Audit Log Cryptography'), findsOneWidget);
      expect(find.text('Master Encryption Key'), findsOneWidget);
      expect(find.text('Emergency Security Lockdown'), findsOneWidget);
    });

    testWidgets('rotates master key via dialog and confirmation', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      final rotateBtn = find.byKey(const Key('rotate_key_button'));
      expect(rotateBtn, findsOneWidget);

      await tester.tap(rotateBtn);
      await tester.pumpAndSettle();

      expect(find.text('Rotate Master Key?'), findsOneWidget);

      final confirmBtn = find.widgetWithText(FilledButton, 'Rotate Key');
      expect(confirmBtn, findsOneWidget);

      await tester.tap(confirmBtn);
      await tester.pumpAndSettle();

      expect(find.text('Master key rotated successfully.'), findsOneWidget);
    });

    testWidgets('engages emergency lockdown, shows warning banner, and lifts lockdown', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      expect(controller.isLockdownActive, isFalse);
      expect(find.text('EMERGENCY LOCKDOWN ACTIVE'), findsNothing);

      // Tap lockdown button
      final lockdownBtn = find.byKey(const Key('emergency_lockdown_button'));
      expect(lockdownBtn, findsOneWidget);

      await tester.tap(lockdownBtn);
      await tester.pumpAndSettle();

      // Confirmation dialog
      expect(find.text('Engage Emergency Lockdown?'), findsOneWidget);

      final confirmBtn = find.widgetWithText(FilledButton, 'Lockdown Now');
      expect(confirmBtn, findsOneWidget);

      await tester.tap(confirmBtn);
      await tester.pumpAndSettle();

      // Lockdown is now engaged
      expect(controller.isLockdownActive, isTrue);
      expect(find.text('EMERGENCY LOCKDOWN ACTIVE'), findsOneWidget);
      expect(find.text('All peer mesh networking and automation triggers are frozen.'), findsOneWidget);

      // Lift lockdown via banner button
      final liftBtn = find.byKey(const Key('lift_lockdown_button'));
      expect(liftBtn, findsOneWidget);

      await tester.tap(liftBtn);
      await tester.pumpAndSettle();

      expect(controller.isLockdownActive, isFalse);
      expect(find.text('EMERGENCY LOCKDOWN ACTIVE'), findsNothing);
    });

    testWidgets('renders performance and memory diagnostics and optimizes memory', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      expect(find.text('Performance & Memory Diagnostics (NFR-1..NFR-8)'), findsOneWidget);
      expect(find.byKey(const Key('memory_budget_chip')), findsOneWidget);
      expect(find.text('Budget Passed'), findsOneWidget);
      expect(find.text('Cold Start'), findsOneWidget);
      expect(find.text('Wake Word'), findsOneWidget);
      expect(find.text('STT Latency'), findsOneWidget);
      expect(find.text('TTS Latency'), findsOneWidget);

      // Tap optimize memory
      final optBtn = find.byKey(const Key('optimize_memory_button'));
      expect(optBtn, findsOneWidget);
      await tester.ensureVisible(optBtn);
      await tester.tap(optBtn);
      await tester.pumpAndSettle();

      expect(find.text('Memory caches reclaimed. Resident RAM within budget!'), findsOneWidget);

      // Dismiss first snackbar
      ScaffoldMessenger.of(tester.element(optBtn)).hideCurrentSnackBar();
      await tester.pumpAndSettle();

      // Tap run benchmark
      final benchBtn = find.byKey(const Key('run_pipeline_benchmark_button'));
      expect(benchBtn, findsOneWidget);
      await tester.ensureVisible(benchBtn);
      await tester.tap(benchBtn);
      await tester.pumpAndSettle();

      expect(find.text('All latency & throughput budgets passed!'), findsOneWidget);
      expect(find.text('Pipeline Benchmark Completed'), findsOneWidget);
    });
  });
}
