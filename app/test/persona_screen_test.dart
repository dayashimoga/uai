import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/ui/adaptive_shell.dart';
import 'package:aissistant/ui/screens/persona_screen.dart';

void main() {
  late TestHarnessBridge bridge;
  late AssistantController controller;

  setUp(() async {
    bridge = TestHarnessBridge();
    controller = AssistantController(bridge);
    await controller.initialize();
  });

  tearDown(() {
    controller.dispose();
  });

  Widget buildTestableWidget({Widget? child}) {
    return MaterialApp(
      theme: ThemeData(useMaterial3: true),
      home: child ?? PersonaScreen(controller: controller),
    );
  }

  testWidgets('renders PersonaScreen with active persona, saved personas, and preferences',
      (tester) async {
    tester.view.physicalSize = const Size(1024, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    expect(find.text('Default Assistant'), findsWidgets);
    expect(find.text('Active'), findsOneWidget);
    expect(find.text('Voice: vits_piper_en_ryan'), findsOneWidget);
    expect(find.text('Tone: Balanced & Courteous'), findsOneWidget);
    expect(find.text('Depth: Standard & Balanced'), findsOneWidget);
    expect(find.text('Live Voice Tuning'), findsOneWidget);
    expect(find.text('Saved Personas (1)'), findsOneWidget);
    expect(find.text('Explicit Preferences (2)'), findsOneWidget);
    expect(find.text('ui.theme'), findsOneWidget);
    expect(find.text('code.language'), findsOneWidget);
  });

  testWidgets('creates a new custom persona and activates it', (tester) async {
    tester.view.physicalSize = const Size(1024, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Tap "New Persona"
    await tester.tap(find.text('New Persona'));
    await tester.pumpAndSettle();

    expect(find.text('Create New Persona'), findsOneWidget);

    // Enter persona name
    await tester.enterText(
      find.widgetWithText(TextField, 'Persona Name'),
      'Code Architect',
    );
    await tester.pumpAndSettle();

    // Tap Create
    await tester.tap(find.text('Create'));
    await tester.pumpAndSettle();

    expect(find.text('Code Architect'), findsOneWidget);
    expect(find.text('Saved Personas (2)'), findsOneWidget);

    // Activate the newly created persona
    await tester.tap(find.text('Activate'));
    await tester.pumpAndSettle();

    // Verify hero card now displays Code Architect
    expect(controller.activePersona?.name, 'Code Architect');
  });

  testWidgets('adds a new explicit preference and deletes it', (tester) async {
    tester.view.physicalSize = const Size(1024, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Tap "Add Preference"
    await tester.tap(find.text('Add Preference'));
    await tester.pumpAndSettle();

    expect(find.text('Add Explicit User Preference'), findsOneWidget);

    // Enter Key, Value, Category
    await tester.enterText(
      find.widgetWithText(TextField, 'Preference Key'),
      'editor.tab_size',
    );
    await tester.enterText(
      find.widgetWithText(TextField, 'Value (JSON formatted)'),
      '4',
    );
    await tester.enterText(
      find.widgetWithText(TextField, 'Category'),
      'developer',
    );
    await tester.pumpAndSettle();

    // Confirm addition
    await tester.tap(find.widgetWithText(FilledButton, 'Add Preference'));
    await tester.pumpAndSettle();

    expect(find.text('editor.tab_size'), findsOneWidget);
    expect(find.text('Explicit Preferences (3)'), findsOneWidget);

    // Delete preference
    final deleteButtons = find.byIcon(Icons.delete_outline);
    // Find the delete button for editor.tab_size
    await tester.tap(deleteButtons.last);
    await tester.pumpAndSettle();

    expect(find.text('editor.tab_size'), findsNothing);
    expect(find.text('Explicit Preferences (2)'), findsOneWidget);
  });

  testWidgets('navigates to PersonaScreen via AdaptiveShell', (tester) async {
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget(child: AdaptiveShell(controller: controller)));
    await tester.pumpAndSettle();

    // Tap Persona navigation destination
    await tester.tap(find.text('Persona'));
    await tester.pumpAndSettle();

    expect(find.byType(PersonaScreen), findsOneWidget);
    expect(find.text('Saved Personas (1)'), findsOneWidget);
  });
}
