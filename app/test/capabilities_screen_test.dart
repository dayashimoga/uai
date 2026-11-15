import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/ui/screens/capabilities_screen.dart';

void main() {
  group('CapabilitiesScreen Widget & Platform Badge Tests', () {
    late TestHarnessBridge bridge;
    late AssistantController controller;

    setUp(() async {
      bridge = TestHarnessBridge();
      controller = AssistantController(bridge);
      await controller.initialize();
    });

    testWidgets('renders platform host runner card and capability count', (tester) async {
      await tester.pumpWidget(
        MaterialApp(
          home: CapabilitiesScreen(controller: controller),
        ),
      );
      await tester.pumpAndSettle();

      // Verify Host Runner card is rendered
      expect(find.textContaining('Host Runner'), findsOneWidget);
      expect(find.textContaining('Active'), findsOneWidget);
      expect(find.text('Platform Capabilities (10)'), findsOneWidget);
    });

    testWidgets('displays tier badges and capability names correctly', (tester) async {
      await tester.pumpWidget(
        MaterialApp(
          home: CapabilitiesScreen(controller: controller),
        ),
      );
      await tester.pumpAndSettle();

      // Verify standard capability titles
      expect(find.text('Local Llm'), findsOneWidget);
      expect(find.text('Offline Speech'), findsOneWidget);
      expect(find.text('Background Wake Word'), findsOneWidget);
      expect(find.text('File System'), findsOneWidget);

      // Verify tier badge labels
      expect(find.text('Tier: Standard'), findsWidgets);
    });

    testWidgets('toggling capability switch updates controller state', (tester) async {
      await tester.pumpWidget(
        MaterialApp(
          home: CapabilitiesScreen(controller: controller),
        ),
      );
      await tester.pumpAndSettle();

      // Find the first Switch widget
      final switchFinder = find.byType(Switch).first;
      expect(tester.widget<Switch>(switchFinder).value, isTrue);

      // Tap the switch to toggle it
      await tester.tap(switchFinder);
      await tester.pumpAndSettle();

      // Verify status changed
      final firstCap = controller.capabilities.first;
      expect(firstCap.isActive, isFalse);
    });
  });
}
