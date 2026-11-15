import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/ui/screens/models_screen.dart';

void main() {
  group('ModelsScreen Widget & Interaction Tests', () {
    late TestHarnessBridge bridge;
    late AssistantController controller;

    setUp(() async {
      bridge = TestHarnessBridge();
      controller = AssistantController(bridge);
      await controller.initialize();
    });

    testWidgets('renders hardware overview and inference budget',
        (tester) async {
      tester.view.physicalSize = const Size(1024, 1200);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        MaterialApp(
          home: ModelsScreen(controller: controller),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Local Models & Benchmarks'), findsOneWidget);
      expect(find.text('Host Hardware & Inference Budget'), findsOneWidget);
      expect(find.text('100% Offline'), findsOneWidget);
      expect(find.text('x86_64 (8T)'), findsOneWidget);
      expect(find.text('16.0 GB'), findsOneWidget);
      expect(find.text('10.0 GB'), findsOneWidget);
      expect(find.text('Nano0_5B'), findsWidgets);
    });

    testWidgets('renders model catalog items with metadata and badges',
        (tester) async {
      tester.view.physicalSize = const Size(1024, 1200);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        MaterialApp(
          home: ModelsScreen(controller: controller),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Curated Models Catalog'), findsOneWidget);
      expect(find.text('Qwen 2.5 0.5B Instruct'), findsOneWidget);
      expect(find.text('SmolLM2 360M Instruct'), findsOneWidget);
      expect(find.text('Llama 3.2 3B Instruct'), findsOneWidget);
      expect(find.text('Recommended'), findsWidgets);
      expect(find.text('SHA-256 Verified'), findsWidgets);
    });

    testWidgets('loads and unloads a model via user interaction',
        (tester) async {
      tester.view.physicalSize = const Size(1024, 1200);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        MaterialApp(
          home: ModelsScreen(controller: controller),
        ),
      );
      await tester.pumpAndSettle();

      // Initially no model loaded
      expect(find.textContaining('No LLM model loaded in memory'),
          findsOneWidget);

      // Tap Load on Qwen
      final loadBtn = find.byKey(
          const Key('load_button_qwen2.5-0.5b-instruct-q4_k_m'));
      expect(loadBtn, findsOneWidget);
      await tester.tap(loadBtn);
      await tester.pumpAndSettle();

      // Active model banner is displayed
      expect(
          find.text('Active Model: Qwen 2.5 0.5B Instruct'), findsOneWidget);
      expect(find.text('Loaded'), findsOneWidget);

      // Tap Unload on the active banner
      final unloadBtn = find.text('Unload');
      expect(unloadBtn, findsOneWidget);
      await tester.tap(unloadBtn);
      await tester.pumpAndSettle();

      // Model is unloaded
      expect(find.textContaining('No LLM model loaded in memory'),
          findsOneWidget);
    });

    testWidgets('executes live benchmark and displays metrics', (tester) async {
      tester.view.physicalSize = const Size(1024, 1200);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        MaterialApp(
          home: ModelsScreen(controller: controller),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Live Inference Benchmark'), findsOneWidget);
      final runBtn = find.byKey(const Key('run_benchmark_button'));
      expect(runBtn, findsOneWidget);

      await tester.tap(runBtn);
      await tester.pumpAndSettle();

      // Metrics displayed
      expect(find.text('Generation Speed'), findsOneWidget);
      expect(find.text('Time to First Token'), findsOneWidget);
      expect(find.text('Total Duration'), findsOneWidget);
      expect(find.text('Memory Footprint'), findsOneWidget);
      expect(find.text('42 ms'), findsOneWidget);
      expect(find.text('380 ms'), findsOneWidget);
      expect(find.text('420 MB'), findsOneWidget);
    });

    testWidgets('filters models catalog by Recommended and Active segments',
        (tester) async {
      tester.view.physicalSize = const Size(1024, 1200);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        MaterialApp(
          home: ModelsScreen(controller: controller),
        ),
      );
      await tester.pumpAndSettle();

      // All shows 3 models
      expect(find.text('Qwen 2.5 0.5B Instruct'), findsOneWidget);
      expect(find.text('SmolLM2 360M Instruct'), findsOneWidget);
      expect(find.text('Llama 3.2 3B Instruct'), findsOneWidget);

      final recommendedSegment = find.descendant(
        of: find.byType(SegmentedButton<String>),
        matching: find.text('Recommended'),
      );
      final activeSegment = find.descendant(
        of: find.byType(SegmentedButton<String>),
        matching: find.text('Active'),
      );
      final allSegment = find.descendant(
        of: find.byType(SegmentedButton<String>),
        matching: find.text('All'),
      );

      // Switch to Recommended
      await tester.tap(recommendedSegment);
      await tester.pumpAndSettle();

      expect(find.text('Qwen 2.5 0.5B Instruct'), findsOneWidget);
      expect(find.text('SmolLM2 360M Instruct'), findsNothing);
      expect(find.text('Llama 3.2 3B Instruct'), findsNothing);

      // Switch to Active (none active initially)
      await tester.tap(activeSegment);
      await tester.pumpAndSettle();

      expect(find.text('No models match the selected filter.'), findsOneWidget);

      // Load model while on active tab by switching to All and loading
      await tester.tap(allSegment);
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(
          const Key('load_button_smollm2-360m-instruct-q4_k_m')));
      await tester.pumpAndSettle();

      // Switch to Active again
      await tester.tap(activeSegment);
      await tester.pumpAndSettle();

      expect(find.text('SmolLM2 360M Instruct'), findsOneWidget);
      expect(find.text('Qwen 2.5 0.5B Instruct'), findsNothing);
    });
  });

  group('AssistantController Model Unit Tests', () {
    late TestHarnessBridge bridge;
    late AssistantController controller;

    setUp(() async {
      bridge = TestHarnessBridge();
      controller = AssistantController(bridge);
      await controller.initialize();
    });

    test('initializes with model catalog snapshot and hardware parameters', () {
      expect(controller.modelSnapshot, isNotNull);
      expect(controller.models.length, 3);
      expect(controller.hardware?.cpuArchitecture, 'x86_64');
      expect(controller.activeModelId, isNull);
    });

    test('loads and unloads models updating activeModelId and notifies listeners',
        () {
      var notifications = 0;
      controller.addListener(() => notifications++);

      controller.loadModel('qwen2.5-0.5b-instruct-q4_k_m');
      expect(controller.activeModelId, 'qwen2.5-0.5b-instruct-q4_k_m');
      expect(notifications, greaterThan(0));

      final prevCount = notifications;
      controller.unloadModel();
      expect(controller.activeModelId, isNull);
      expect(notifications, greaterThan(prevCount));
    });

    test('runs benchmark and records result in state', () async {
      expect(controller.lastBenchmark, isNull);
      final res = await controller.runBenchmark(promptTokens: 16, generateTokens: 8);

      expect(res, isNotNull);
      expect(res?.promptTokens, 16);
      expect(res?.generatedTokens, 8);
      expect(controller.lastBenchmark, isNotNull);
      expect(controller.isBenchmarking, isFalse);
    });
  });
}
