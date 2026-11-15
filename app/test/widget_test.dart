import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/ui/adaptive_shell.dart';

void main() {
  testWidgets('AdaptiveShell renders desktop NavigationRail on wide screen', (tester) async {
    // Set desktop screen size
    tester.view.physicalSize = const Size(1024, 768);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    final bridge = TestHarnessBridge();
    final controller = AssistantController(bridge);
    await controller.initialize();

    await tester.pumpWidget(
      MaterialApp(
        home: AdaptiveShell(controller: controller),
      ),
    );
    await tester.pumpAndSettle();

    // Verify NavigationRail is rendered
    expect(find.byType(NavigationRail), findsOneWidget);
    expect(find.byType(NavigationBar), findsNothing);

    // Verify Diagnostics screen content is initially shown
    expect(find.text('System Diagnostics'), findsOneWidget);
    expect(find.text('Core Subsystems'), findsOneWidget);
    expect(find.text('Overall Status: Healthy'), findsOneWidget);
  });

  testWidgets('AdaptiveShell renders mobile NavigationBar on narrow screen', (tester) async {
    // Set mobile screen size
    tester.view.physicalSize = const Size(400, 800);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    final bridge = TestHarnessBridge();
    final controller = AssistantController(bridge);
    await controller.initialize();

    await tester.pumpWidget(
      MaterialApp(
        home: AdaptiveShell(controller: controller),
      ),
    );
    await tester.pumpAndSettle();

    // Verify NavigationBar is rendered
    expect(find.byType(NavigationBar), findsOneWidget);
    expect(find.byType(NavigationRail), findsNothing);
  });

  testWidgets('Navigation switches between screens', (tester) async {
    tester.view.physicalSize = const Size(1024, 768);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    final bridge = TestHarnessBridge();
    final controller = AssistantController(bridge);
    await controller.initialize();

    await tester.pumpWidget(
      MaterialApp(
        home: AdaptiveShell(controller: controller),
      ),
    );
    await tester.pumpAndSettle();

    // Navigate to Voice
    await tester.tap(find.text('Voice'));
    await tester.pumpAndSettle();
    expect(find.text('Voice Assistant'), findsOneWidget);
    expect(find.text('100% Offline'), findsOneWidget);
    expect(find.text('Push-to-Talk'), findsOneWidget);

    // Navigate to Capabilities
    await tester.tap(find.text('Capabilities'));
    await tester.pumpAndSettle();
    expect(find.text('Capability Matrix'), findsOneWidget);
    expect(find.text('Local Llm'), findsOneWidget);
    expect(find.text('Offline Speech'), findsOneWidget);

    // Navigate to Audit Log
    await tester.tap(find.text('Audit Log'));
    await tester.pumpAndSettle();
    expect(find.text('Security Audit Log'), findsOneWidget);
    expect(find.text('Chain Integrity Intact'), findsOneWidget);

    // Navigate to Settings
    await tester.tap(find.text('Settings'));
    await tester.pumpAndSettle();
    expect(find.text('Appearance & Display'), findsOneWidget);
    expect(find.text('Offline Storage & Privacy'), findsOneWidget);
  });
}
