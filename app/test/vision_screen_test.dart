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
    tester.view.physicalSize = const Size(1024, 1200);
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
    tester.view.physicalSize = const Size(1024, 1200);
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
    expect(find.text('L1'), findsOneWidget);
    expect(find.text('L2'), findsOneWidget);
    expect(find.text('L3'), findsOneWidget);
  });

  testWidgets('switches sample chips, extracts OCR, and clears OCR result', (tester) async {
    tester.view.physicalSize = const Size(1024, 1200);
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
    tester.view.physicalSize = const Size(1024, 1200);
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
    tester.view.physicalSize = const Size(1024, 1200);
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
}
