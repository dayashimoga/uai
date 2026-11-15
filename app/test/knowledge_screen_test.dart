import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/ui/screens/knowledge_screen.dart';

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

  Widget buildTestableWidget() {
    return MaterialApp(
      theme: ThemeData(useMaterial3: true),
      home: KnowledgeScreen(controller: controller),
    );
  }

  testWidgets('renders KnowledgeScreen with search bar and tabs', (tester) async {
    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    expect(find.text('Knowledge Base & Offline RAG'), findsOneWidget);
    expect(find.byKey(const Key('knowledge_search_field')), findsOneWidget);
    expect(find.textContaining('Citations (0)'), findsOneWidget);
    expect(find.textContaining('Documents (1)'), findsOneWidget);
    expect(find.byKey(const Key('add_document_fab')), findsOneWidget);
  });

  testWidgets('switches to documents tab and displays indexed documents', (tester) async {
    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Tap Documents tab
    await tester.tap(find.textContaining('Documents (1)'));
    await tester.pumpAndSettle();

    expect(find.text('offline_architecture.md'), findsOneWidget);
    expect(find.textContaining('knowledge/offline_architecture.md'), findsOneWidget);
    expect(find.textContaining('3 chunk(s)'), findsOneWidget);
    expect(find.byKey(const Key('delete_doc_doc_sample_01')), findsOneWidget);
  });

  testWidgets('performs search and displays grounded citation cards', (tester) async {
    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Enter search query
    await tester.enterText(find.byKey(const Key('knowledge_search_field')), 'architecture');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();

    expect(controller.citationSearchResults.isNotEmpty, isTrue);
    expect(find.text('offline_architecture.md'), findsOneWidget);
    expect(find.textContaining('RRF:'), findsOneWidget);
    expect(find.text('Chunk #0'), findsOneWidget);
    expect(find.text('Full Chunk Content'), findsOneWidget);

    // Expand chunk content
    await tester.tap(find.text('Full Chunk Content'));
    await tester.pumpAndSettle();
    expect(find.textContaining('Offline knowledge base snippet'), findsWidgets);

    // Clear search
    await tester.tap(find.byKey(const Key('clear_search_button')));
    await tester.pumpAndSettle();
    expect(find.text('Offline Knowledge Search'), findsOneWidget);
  });

  testWidgets('indexes a new document via dialog and shows in catalog', (tester) async {
    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Tap Index Document FAB
    await tester.tap(find.byKey(const Key('add_document_fab')));
    await tester.pumpAndSettle();

    expect(find.text('Index Local Document'), findsOneWidget);

    // Fill in content
    await tester.enterText(
      find.byKey(const Key('index_content_field')),
      '# Offline Intelligence\nZero cloud connectivity required.',
    );
    await tester.pumpAndSettle();

    // Submit dialog
    await tester.tap(find.byKey(const Key('submit_index_button')));
    await tester.pumpAndSettle();

    // Verify document catalog has 2 documents
    expect(controller.documents.length, 2);
    expect(find.textContaining('Documents (2)'), findsOneWidget);
  });

  testWidgets('deletes an indexed document after confirmation', (tester) async {
    await tester.pumpWidget(buildTestableWidget());
    await tester.pumpAndSettle();

    // Switch to Documents tab
    await tester.tap(find.textContaining('Documents (1)'));
    await tester.pumpAndSettle();

    // Tap delete button
    await tester.tap(find.byKey(const Key('delete_doc_doc_sample_01')));
    await tester.pumpAndSettle();

    expect(find.text('Delete Ingested Document?'), findsOneWidget);

    // Confirm deletion
    await tester.tap(find.byKey(const Key('confirm_delete_doc_sample_01')));
    await tester.pumpAndSettle();

    expect(controller.documents.isEmpty, isTrue);
    expect(find.text('No Ingested Documents'), findsOneWidget);
  });
}
