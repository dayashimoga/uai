import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/ui/screens/chat_screen.dart';

void main() {
  group('ChatScreen Widget & Turn Orchestration Tests', () {
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

    Widget createChatApp() {
      return MaterialApp(
        theme: ThemeData(useMaterial3: true),
        home: ChatScreen(controller: controller),
      );
    }

    testWidgets('renders initial ChatScreen with empty state and suggestions', (tester) async {
      await tester.pumpWidget(createChatApp());
      await tester.pumpAndSettle();

      // Header and active conversation
      expect(find.text('General Assistant'), findsWidgets);
      expect(find.text('New Chat'), findsOneWidget);

      // Empty state prompt
      expect(find.text('How can I help you today?'), findsOneWidget);
      expect(find.text('Show live performance snapshot'), findsOneWidget);
      expect(find.text('Explain local security policies'), findsOneWidget);

      // Composer field & send button
      expect(find.byType(TextField), findsNWidgets(2)); // Search + Composer
      expect(find.byIcon(Icons.arrow_upward), findsOneWidget);
    });

    testWidgets('sends chat message and renders assistant streaming response', (tester) async {
      await tester.pumpWidget(createChatApp());
      await tester.pumpAndSettle();

      // Enter a chat message in the composer (the second TextField)
      final composerField = find.byType(TextField).last;
      await tester.enterText(composerField, 'Hello local assistant');
      await tester.tap(find.byIcon(Icons.arrow_upward));
      await tester.pump();

      // User bubble should be rendered immediately
      expect(find.text('Hello local assistant'), findsWidgets);

      // Let timer and token stream pump
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pumpAndSettle();

      // Assistant turn should now be present
      expect(find.textContaining('Local AI model processed your request'), findsOneWidget);

      // Audit trail should have entries
      expect(controller.auditLogs.isNotEmpty, isTrue);
    });

    testWidgets('displays security confirmation card on destructive tool action and approves it', (tester) async {
      await tester.pumpWidget(createChatApp());
      await tester.pumpAndSettle();

      // Trigger a message with "delete" to trigger confirmation gate in TestHarnessBridge
      final composerField = find.byType(TextField).last;
      await tester.enterText(composerField, 'delete system logs');
      await tester.tap(find.byIcon(Icons.arrow_upward));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 50));

      // Confirmation card should appear
      expect(find.text('Security Policy Confirmation Required'), findsOneWidget);
      expect(find.textContaining('system_execute_action'), findsOneWidget);
      expect(find.text('Destructive'), findsOneWidget);
      expect(find.text('Approve'), findsOneWidget);
      expect(find.text('Deny'), findsOneWidget);

      // Approve confirmation
      await tester.tap(find.text('Approve'));
      await tester.pumpAndSettle();

      // Confirmation card should be gone and turn completed
      expect(find.text('Security Policy Confirmation Required'), findsNothing);
      expect(find.text('Action confirmed and executed successfully.'), findsOneWidget);
    });

    testWidgets('creates a new conversation and switches between threads', (tester) async {
      await tester.pumpWidget(createChatApp());
      await tester.pumpAndSettle();

      // Tap + New Chat
      await tester.tap(find.byType(FilledButton).first); // New Chat button
      await tester.pumpAndSettle();

      // A second conversation should exist in the list
      expect(controller.conversations.length, greaterThanOrEqualTo(2));
      expect(find.text('New Chat'), findsWidgets);
    });

    testWidgets('FTS5 search box filters conversation messages', (tester) async {
      await tester.pumpWidget(createChatApp());
      await tester.pumpAndSettle();

      // Send a message first so that it exists in conversation history
      final composerField = find.byType(TextField).last;
      await tester.enterText(composerField, 'UniqueKeyphrase123 message');
      await tester.tap(find.byIcon(Icons.arrow_upward));
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pumpAndSettle();

      // Enter search query in the search box (first TextField)
      final searchBox = find.byType(TextField).first;
      await tester.enterText(searchBox, 'UniqueKeyphrase123');
      await tester.pumpAndSettle();

      // Verify search hit
      expect(find.textContaining('UniqueKeyphrase123'), findsWidgets);
    });
  });
}
