import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/ui/adaptive_shell.dart';
import 'package:aissistant/ui/screens/interpreter_screen.dart';

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
      home: child ?? InterpreterScreen(controller: controller),
    );
  }

  testWidgets('renders InterpreterScreen with header, participant tags, and empty state',
      (tester) async {
    tester.view.physicalSize = const Size(1024, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    expect(find.text('Live Voice Interpreter'), findsOneWidget);
    expect(find.text('Offline Bi-Directional Voice Interpreter'), findsOneWidget);
    expect(find.text('Party A'), findsOneWidget);
    expect(find.text('Party B'), findsOneWidget);
    expect(find.text('English'), findsWidgets);
    expect(find.text('Español'), findsWidgets);
    expect(find.text("Party A's Turn (English -> Spanish)"), findsOneWidget);
    expect(find.text('No Conversation Turns Yet'), findsOneWidget);
    expect(find.byKey(const Key('interpreter_input_field')), findsOneWidget);
    expect(find.byKey(const Key('interpreter_submit_button')), findsOneWidget);
  });

  testWidgets('submits turn as Party A, translates to Spanish, and auto-swaps to Party B',
      (tester) async {
    tester.view.physicalSize = const Size(1024, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Enter phrase for Party A
    await tester.enterText(find.byKey(const Key('interpreter_input_field')), 'Hello');
    await tester.tap(find.byKey(const Key('interpreter_submit_button')));
    await tester.pumpAndSettle();

    // Verify turn card
    expect(controller.interpreterTurns.length, 1);
    expect(find.text('Party A'), findsWidgets);
    expect(find.text('Hello'), findsWidgets);
    expect(find.text('Hola'), findsOneWidget);

    // Verify active speaker shifted to Party B
    expect(find.text("Party B's Turn (Spanish -> English)"), findsOneWidget);
  });

  testWidgets('submits alternating turn as Party B, translates to English, and alternates back',
      (tester) async {
    tester.view.physicalSize = const Size(1024, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Turn 1 (Party A)
    await tester.enterText(find.byKey(const Key('interpreter_input_field')), 'Thank you');
    await tester.tap(find.byKey(const Key('interpreter_submit_button')));
    await tester.pumpAndSettle();

    expect(find.text('Gracias'), findsOneWidget);

    // Turn 2 (Party B)
    await tester.enterText(find.byKey(const Key('interpreter_input_field')), 'Adiós');
    await tester.tap(find.byKey(const Key('interpreter_submit_button')));
    await tester.pumpAndSettle();

    expect(controller.interpreterTurns.length, 2);
    expect(find.text('Goodbye'), findsOneWidget);

    // Alternated back to Party A
    expect(find.text("Party A's Turn (English -> Spanish)"), findsOneWidget);
  });

  testWidgets('swaps languages manually via swap button', (tester) async {
    tester.view.physicalSize = const Size(1024, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Tap swap button
    await tester.tap(find.byKey(const Key('interpreter_swap_button')));
    await tester.pumpAndSettle();

    // Now Party A is Spanish and Party B is English
    expect(find.text("Party A's Turn (Spanish -> English)"), findsOneWidget);
  });

  testWidgets('copies translated text to clipboard and shows confirmation snackbar',
      (tester) async {
    tester.view.physicalSize = const Size(1024, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Submit a turn
    await tester.enterText(find.byKey(const Key('interpreter_input_field')), 'Hello');
    await tester.tap(find.byKey(const Key('interpreter_submit_button')));
    await tester.pumpAndSettle();

    // Tap copy button on turn 1
    await tester.tap(find.byKey(const Key('copy_turn_1')));
    await tester.pump();

    expect(find.text('Translated text copied to clipboard'), findsOneWidget);
    await tester.pumpAndSettle();
  });

  testWidgets('clears conversation history via clear button', (tester) async {
    tester.view.physicalSize = const Size(1024, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Submit turn
    await tester.enterText(find.byKey(const Key('interpreter_input_field')), 'Hello');
    await tester.tap(find.byKey(const Key('interpreter_submit_button')));
    await tester.pumpAndSettle();
    expect(controller.interpreterTurns.isNotEmpty, isTrue);

    // Tap clear button in AppBar
    await tester.tap(find.byKey(const Key('interpreter_clear_button')));
    await tester.pumpAndSettle();

    expect(controller.interpreterTurns.isEmpty, isTrue);
    expect(find.text('No Conversation Turns Yet'), findsOneWidget);
  });

  testWidgets('adaptive shell displays Interpreter destination and navigates to it',
      (tester) async {
    tester.view.physicalSize = const Size(1024, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget(
      child: AdaptiveShell(controller: controller),
    ));
    await tester.pumpAndSettle();

    expect(find.text('Interpreter'), findsWidgets);
    await tester.tap(find.text('Interpreter').first);
    await tester.pumpAndSettle();

    expect(find.text('Live Voice Interpreter'), findsOneWidget);
    expect(find.byKey(const Key('interpreter_input_field')), findsOneWidget);
  });
}
