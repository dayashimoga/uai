import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/ui/adaptive_shell.dart';
import 'package:aissistant/ui/screens/vision_screen.dart';

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
      home: child ?? VisionScreen(controller: controller),
    );
  }

  testWidgets('renders VisionScreen with offline header, sample selector, and empty state',
      (tester) async {
    tester.view.physicalSize = const Size(1024, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    expect(find.text('Offline Vision & OCR'), findsOneWidget);
    expect(find.text('Select Input Sample'), findsOneWidget);
    expect(find.text('Invoice / Receipt'), findsOneWidget);
    expect(find.text('Architecture Diagram'), findsOneWidget);
    expect(find.text('Terminal Diagnostics'), findsOneWidget);
    expect(find.byKey(const Key('ocr_extract_button')), findsOneWidget);
    expect(find.text('Ready to Analyze Images'), findsOneWidget);
  });

  testWidgets('triggers OCR extraction and displays metrics, full text, and line breakdown',
      (tester) async {
    tester.view.physicalSize = const Size(1024, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Trigger OCR extraction
    await tester.tap(find.byKey(const Key('ocr_extract_button')));
    await tester.pumpAndSettle();

    expect(controller.activeOcrResult, isNotNull);
    expect(find.text('Extraction Metrics'), findsOneWidget);
    expect(find.byKey(const Key('ocr_confidence_metric')), findsOneWidget);
    expect(find.byKey(const Key('ocr_lines_metric')), findsOneWidget);
    expect(find.byKey(const Key('ocr_time_metric')), findsOneWidget);
    expect(find.byKey(const Key('ocr_dims_metric')), findsOneWidget);

    // Verify Recognized Full Text Card
    expect(find.text('Recognized Full Text'), findsOneWidget);
    expect(find.byKey(const Key('ocr_full_text_view')), findsOneWidget);
    expect(find.textContaining('UNIVERSAL AI ASSISTANT'), findsWidgets);
    expect(find.textContaining('100% OFFLINE MULTIMODAL VISION'), findsWidgets);
    expect(find.textContaining('ZERO CLOUD DEPENDENCY'), findsWidgets);

    // Verify Layout Breakdown Lines
    expect(find.text('Layout & Bounding Box Breakdown'), findsOneWidget);
    expect(find.text('L1'), findsWidgets);
    expect(find.text('L2'), findsWidgets);
    expect(find.text('L3'), findsWidgets);

    // Verify Visual Bounding Box Overlay Card
    expect(find.text('Visual Detection Overlay & Bounding Boxes'), findsOneWidget);
    expect(find.byKey(const Key('ocr_bbox_box_1')), findsOneWidget);
    expect(find.byKey(const Key('ocr_bbox_box_2')), findsOneWidget);
    expect(find.byKey(const Key('ocr_bbox_box_3')), findsOneWidget);
  });

  testWidgets('switches sample chips, extracts OCR, and clears OCR result', (tester) async {
    tester.view.physicalSize = const Size(1024, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Select Architecture Diagram sample chip
    await tester.tap(find.byKey(const Key('sample_chip_1')));
    await tester.pumpAndSettle();

    // Run OCR
    await tester.tap(find.byKey(const Key('ocr_extract_button')));
    await tester.pumpAndSettle();
    expect(controller.activeOcrResult, isNotNull);

    // Clear OCR result
    await tester.tap(find.byKey(const Key('ocr_clear_button')));
    await tester.pumpAndSettle();

    expect(controller.activeOcrResult, isNull);
    expect(find.text('Ready to Analyze Images'), findsOneWidget);
  });

  testWidgets('copies recognized text to clipboard and shows confirmation snackbar',
      (tester) async {
    tester.view.physicalSize = const Size(1024, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Run OCR
    await tester.tap(find.byKey(const Key('ocr_extract_button')));
    await tester.pumpAndSettle();

    // Tap copy button
    await tester.tap(find.byKey(const Key('ocr_copy_button')));
    await tester.pump();

    expect(find.text('OCR text copied to clipboard'), findsOneWidget);
    await tester.pumpAndSettle();
  });

  testWidgets('adaptive shell displays Vision destination and navigates to it', (tester) async {
    tester.view.physicalSize = const Size(1024, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget(
      child: AdaptiveShell(controller: controller),
    ));
    await tester.pumpAndSettle();

    // Find and tap the Vision destination
    expect(find.text('Vision'), findsWidgets);
    await tester.tap(find.text('Vision').first);
    await tester.pumpAndSettle();

    expect(find.text('Offline Vision & OCR'), findsOneWidget);
    expect(find.byKey(const Key('ocr_extract_button')), findsOneWidget);
  });

  testWidgets('browse button opens dialog and extracts OCR from local file path', (tester) async {
    tester.view.physicalSize = const Size(1024, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('ocr_browse_button')), findsOneWidget);
    await tester.tap(find.byKey(const Key('ocr_browse_button')));
    await tester.pumpAndSettle();

    // Dialog appears
    expect(find.text('Open Local Image or Document'), findsOneWidget);
    expect(find.byKey(const Key('ocr_file_path_input')), findsOneWidget);
    expect(find.byKey(const Key('ocr_submit_file_button')), findsOneWidget);

    // Enter file path
    await tester.enterText(find.byKey(const Key('ocr_file_path_input')), 'C:/test/invoice.png');
    await tester.tap(find.byKey(const Key('ocr_submit_file_button')));
    await tester.pumpAndSettle();

    expect(controller.activeOcrResult, isNotNull);
    expect(find.text('Recognized Full Text'), findsOneWidget);
    expect(find.byKey(const Key('ocr_bbox_box_1')), findsOneWidget);
  });

  testWidgets('capture screen button invokes screen capture OCR and displays result', (tester) async {
    tester.view.physicalSize = const Size(1024, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('ocr_capture_screen_button')), findsOneWidget);
    await tester.tap(find.byKey(const Key('ocr_capture_screen_button')));
    await tester.pumpAndSettle();

    expect(controller.activeOcrResult, isNotNull);
    expect(find.text('Recognized Full Text'), findsOneWidget);
    expect(find.textContaining('Universal AI Assistant Desktop Screen'), findsWidgets);
  });

  testWidgets('ask assistant button sends extracted OCR text to chat and switches tabs', (tester) async {
    tester.view.physicalSize = const Size(1024, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Trigger OCR first
    await tester.tap(find.byKey(const Key('ocr_extract_button')));
    await tester.pumpAndSettle();

    // Tap Ask Assistant
    expect(find.byKey(const Key('ocr_ask_assistant_button')), findsOneWidget);
    await tester.tap(find.byKey(const Key('ocr_ask_assistant_button')));
    await tester.pump();

    expect(find.text('OCR prompt sent to Assistant Chat'), findsOneWidget);
    expect(controller.selectedNavigationTab, equals(1));
    expect(controller.pendingChatPrompt, contains('UNIVERSAL AI ASSISTANT'));
    await tester.pumpAndSettle();
  });

  testWidgets('export button copies OCR document JSON to clipboard', (tester) async {
    tester.view.physicalSize = const Size(1024, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Trigger OCR first
    await tester.tap(find.byKey(const Key('ocr_extract_button')));
    await tester.pumpAndSettle();

    // Tap Export button
    expect(find.byKey(const Key('ocr_export_button')), findsOneWidget);
    await tester.tap(find.byKey(const Key('ocr_export_button')));
    await tester.pump();

    expect(find.text('Exported OCR document JSON to clipboard'), findsOneWidget);
    await tester.pumpAndSettle();
  });
}
