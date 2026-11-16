import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/ui/screens/automation_screen.dart';

void main() {
  group('AutomationScreen Widget & Routine Tests', () {
    late TestHarnessBridge bridge;
    late AssistantController controller;

    setUp(() {
      bridge = TestHarnessBridge();
      controller = AssistantController(bridge);
    });

    tearDown(() {
      controller.dispose();
    });

    Widget createScreen() {
      return MaterialApp(
        theme: ThemeData(useMaterial3: true),
        home: AutomationScreen(controller: controller),
      );
    }

    testWidgets('renders AutomationScreen with routines, smart home entities, and history',
        (tester) async {
      tester.view.physicalSize = const Size(1024, 768);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await controller.initialize();
      await tester.pumpWidget(createScreen());
      await tester.pumpAndSettle();

      // Verify AppBar header & offline badge
      expect(find.text('Automations & Smart Home'), findsOneWidget);
      expect(find.text('100% Offline'), findsOneWidget);

      // Verify Tab bars with count badges
      expect(find.text('Routines (2)'), findsOneWidget);
      expect(find.text('Smart Home (2)'), findsOneWidget);
      expect(find.text('History (1)'), findsOneWidget);

      // Verify routines in initial list
      expect(find.text('Daily Morning Brief'), findsOneWidget);
      expect(find.text('Low Battery Auto-Mute'), findsOneWidget);
    });

    testWidgets('toggles routine enabled switch', (tester) async {
      tester.view.physicalSize = const Size(1024, 768);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await controller.initialize();
      await tester.pumpWidget(createScreen());
      await tester.pumpAndSettle();

      // Find initial enabled state
      final rule =
          controller.automations.firstWhere((r) => r.id == 'auto_morning_brief');
      expect(rule.enabled, isTrue);

      // Tap Switch on first routine card
      final switchFinder = find.byType(Switch).first;
      await tester.tap(switchFinder);
      await tester.pumpAndSettle();

      // Verify state was toggled to false
      final updated =
          controller.automations.firstWhere((r) => r.id == 'auto_morning_brief');
      expect(updated.enabled, isFalse);
    });

    testWidgets('triggers manual routine execution via Run Now', (tester) async {
      tester.view.physicalSize = const Size(1024, 768);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await controller.initialize();
      await tester.pumpWidget(createScreen());
      await tester.pumpAndSettle();

      // Tap Run Now on Daily Morning Brief
      final runNowFinder = find.widgetWithText(FilledButton, 'Run Now').first;
      await tester.tap(runNowFinder);
      await tester.pumpAndSettle();

      // Verify execution banner appears
      expect(
        find.text('Last Run: auto_morning_brief • SUCCESS'),
        findsOneWidget,
      );
      expect(find.textContaining('Execution duration: 25ms'), findsOneWidget);
    });

    testWidgets('creates a new routine via dialog', (tester) async {
      tester.view.physicalSize = const Size(1024, 768);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await controller.initialize();
      await tester.pumpWidget(createScreen());
      await tester.pumpAndSettle();

      // Tap New Routine FAB
      final fabFinder = find.widgetWithText(FloatingActionButton, 'New Routine');
      await tester.tap(fabFinder);
      await tester.pumpAndSettle();

      // Verify dialog is shown
      expect(find.text('Create Automation Routine'), findsOneWidget);

      // Fill name field
      final nameField = find.widgetWithText(TextField, 'Morning Routine');
      await tester.enterText(nameField, 'Evening Relaxation Routine');

      // Submit dialog
      final createBtn = find.widgetWithText(FilledButton, 'Create Routine');
      await tester.tap(createBtn);
      await tester.pumpAndSettle();

      // Verify new routine appears in list and controller
      expect(controller.automations.length, 3);
      expect(find.text('Evening Relaxation Routine'), findsOneWidget);
    });

    testWidgets('deletes routine from list', (tester) async {
      tester.view.physicalSize = const Size(1024, 768);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await controller.initialize();
      await tester.pumpWidget(createScreen());
      await tester.pumpAndSettle();

      expect(controller.automations.length, 2);

      // Tap delete on first routine
      final deleteBtn = find.widgetWithText(OutlinedButton, 'Delete').first;
      await tester.tap(deleteBtn);
      await tester.pumpAndSettle();

      // Verify item deleted
      expect(controller.automations.length, 1);
    });

    testWidgets('switches to Smart Home tab and controls devices', (tester) async {
      tester.view.physicalSize = const Size(1024, 768);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await controller.initialize();
      await tester.pumpWidget(createScreen());
      await tester.pumpAndSettle();

      // Switch to Smart Home tab
      final smartHomeTab = find.text('Smart Home (2)');
      await tester.tap(smartHomeTab);
      await tester.pumpAndSettle();

      // Verify devices rendered
      expect(find.text('Living Room Light'), findsOneWidget);
      expect(find.text('Hall Thermostat'), findsOneWidget);
      expect(find.textContaining('Home Assistant'), findsOneWidget);
      expect(find.textContaining('MQTT'), findsOneWidget);

      // Verify initial light state is on
      final light =
          controller.homeEntities.firstWhere((e) => e.id == 'light.living_room');
      expect(light.isTurnedOn, isTrue);

      // Tap Switch to turn off light
      final lightSwitch = find.byType(Switch).first;
      await tester.tap(lightSwitch);
      await tester.pumpAndSettle();

      // Verify light was turned off
      final updatedLight =
          controller.homeEntities.firstWhere((e) => e.id == 'light.living_room');
      expect(updatedLight.isTurnedOn, isFalse);

      // Filter by CLIMATE domain
      final climateChip = find.widgetWithText(FilterChip, 'CLIMATE');
      await tester.tap(climateChip);
      await tester.pumpAndSettle();

      // Living room light is filtered out, hall thermostat remains
      expect(find.text('Hall Thermostat'), findsOneWidget);
      expect(find.text('Living Room Light'), findsNothing);
    });

    testWidgets('registers a new smart home device via dialog', (tester) async {
      tester.view.physicalSize = const Size(1024, 768);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await controller.initialize();
      await tester.pumpWidget(createScreen());
      await tester.pumpAndSettle();

      // Switch to Smart Home tab
      await tester.tap(find.text('Smart Home (2)'));
      await tester.pumpAndSettle();

      // Tap Add Device FAB
      final addBtn = find.widgetWithText(FloatingActionButton, 'Add Device');
      await tester.tap(addBtn);
      await tester.pumpAndSettle();

      // Verify dialog is shown
      expect(find.text('Register Smart Home Device'), findsOneWidget);

      // Enter name
      final nameField = find.widgetWithText(TextField, 'Desk Lamp');
      await tester.enterText(nameField, 'Office Light');

      // Tap register
      await tester.tap(find.widgetWithText(FilledButton, 'Register Device'));
      await tester.pumpAndSettle();

      // Verify entity added
      expect(controller.homeEntities.length, 3);
      expect(find.text('Office Light'), findsOneWidget);
    });

    testWidgets('switches to History tab and clears execution logs', (tester) async {
      tester.view.physicalSize = const Size(1024, 768);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await controller.initialize();
      await tester.pumpWidget(createScreen());
      await tester.pumpAndSettle();

      // Switch to History tab
      final historyTab = find.text('History (1)');
      await tester.tap(historyTab);
      await tester.pumpAndSettle();

      // Verify execution log item
      expect(find.text('Execution Logs (1)'), findsOneWidget);
      expect(find.text('auto_morning_brief'), findsOneWidget);
      expect(find.text('SUCCESS'), findsOneWidget);

      // Tap Clear All
      final clearBtn = find.widgetWithText(TextButton, 'Clear All');
      await tester.tap(clearBtn);
      await tester.pumpAndSettle();

      // Verify empty state is rendered
      expect(find.text('No Execution History'), findsOneWidget);
      expect(controller.automationHistory, isEmpty);
    });
  });
}
