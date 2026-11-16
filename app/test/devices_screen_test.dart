import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/ui/screens/devices_screen.dart';

void main() {
  group('DevicesScreen Widget & Pairing Ceremony Tests', () {
    late TestHarnessBridge bridge;
    late AssistantController controller;

    setUp(() async {
      bridge = TestHarnessBridge();
      controller = AssistantController(bridge);
      await controller.initialize();
    });

    Widget createTestApp(WidgetTester tester) {
      tester.view.physicalSize = const Size(1024, 1200);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      return MaterialApp(
        theme: ThemeData.light(useMaterial3: true),
        home: DevicesScreen(controller: controller),
      );
    }

    testWidgets('renders device mesh header and offline badge', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      expect(find.text('Devices & Cross-Device Mesh'), findsOneWidget);
      expect(find.text('100% Offline LAN'), findsOneWidget);
      expect(find.text('Refresh'), findsOneWidget);
      expect(find.text('Simulate Peer'), findsOneWidget);
    });

    testWidgets('renders initial discovered peers and paired devices', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      // Discovered peer from mock
      expect(find.text('Nearby Discovered Peers (1)'), findsOneWidget);
      expect(find.text('pixel-9-pro'), findsOneWidget);
      expect(find.text('ANDROID'), findsWidgets);

      // Paired device from mock
      expect(find.text('Paired Devices (1)'), findsOneWidget);
      expect(find.text('MacBook Pro M3'), findsOneWidget);
      expect(find.text('TRUSTED'), findsOneWidget);
      expect(find.text('Capability Access Control List (ACL)'), findsOneWidget);
    });

    testWidgets('initiates pairing ceremony, displays SAS code and QR representation', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      // Find and click the Pair button on pixel-9-pro
      final pairBtn = find.byKey(const Key('pair_button_pixel-9-pro'));
      expect(pairBtn, findsOneWidget);
      await tester.tap(pairBtn);
      await tester.pumpAndSettle();

      // Verify pairing ceremony card is visible
      expect(find.text('Pairing Ceremony with pixel-9-pro'), findsOneWidget);
      expect(find.byKey(const Key('sas_code_display')), findsOneWidget);
      expect(find.text('481-923'), findsOneWidget);
      expect(find.text('QR Payload Ready'), findsOneWidget);
      expect(find.byKey(const Key('confirm_pairing_button')), findsOneWidget);
      expect(find.byKey(const Key('cancel_pairing_button')), findsOneWidget);
    });

    testWidgets('confirms pairing ceremony, moves peer to paired devices', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      // Start pairing
      final pairBtn = find.byKey(const Key('pair_button_pixel-9-pro'));
      await tester.tap(pairBtn);
      await tester.pumpAndSettle();

      // Confirm pairing
      final confirmBtn = find.byKey(const Key('confirm_pairing_button'));
      await tester.tap(confirmBtn);
      await tester.pumpAndSettle();

      // Ceremony card should be dismissed
      expect(find.text('Pairing Ceremony with pixel-9-pro'), findsNothing);

      // pixel-9-pro should now be in Paired Devices (total 2)
      expect(find.text('Paired Devices (2)'), findsOneWidget);
      expect(find.text('Nearby Discovered Peers (0)'), findsOneWidget);
    });

    testWidgets('cancels active pairing ceremony', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      // Start pairing
      final pairBtn = find.byKey(const Key('pair_button_pixel-9-pro'));
      await tester.tap(pairBtn);
      await tester.pumpAndSettle();

      expect(find.text('Pairing Ceremony with pixel-9-pro'), findsOneWidget);

      // Cancel pairing
      final cancelBtn = find.byKey(const Key('cancel_pairing_button'));
      await tester.tap(cancelBtn);
      await tester.pumpAndSettle();

      // Ceremony card should be dismissed
      expect(find.text('Pairing Ceremony with pixel-9-pro'), findsNothing);
      expect(find.text('Nearby Discovered Peers (1)'), findsOneWidget);
    });

    testWidgets('unpairs device with confirmation dialog', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      expect(find.text('Paired Devices (1)'), findsOneWidget);
      final unpairBtn = find.byKey(const Key('unpair_button_dev_macbook_pro'));
      expect(unpairBtn, findsOneWidget);

      await tester.tap(unpairBtn);
      await tester.pumpAndSettle();

      // Confirmation dialog
      expect(find.text('Unpair Device?'), findsOneWidget);
      expect(find.text('Are you sure you want to unpair "MacBook Pro M3"? All granted permissions and cryptographic trust will be revoked immediately.'), findsOneWidget);

      // Tap confirm Unpair in dialog
      final dialogUnpairBtn = find.widgetWithText(FilledButton, 'Unpair');
      await tester.tap(dialogUnpairBtn);
      await tester.pumpAndSettle();

      // Device list now empty
      expect(find.text('Paired Devices (0)'), findsOneWidget);
      expect(find.text('No paired devices. Pair a nearby peer to securely sync clipboard and dispatch remote skills.'), findsOneWidget);
    });

    testWidgets('simulates new discovered peer via button', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      expect(find.text('Nearby Discovered Peers (1)'), findsOneWidget);

      // Tap simulate peer button
      final simBtn = find.widgetWithText(OutlinedButton, 'Simulate Peer');
      expect(simBtn, findsOneWidget);
      await tester.tap(simBtn);
      await tester.pumpAndSettle();

      // Now 2 discovered peers
      expect(find.text('Nearby Discovered Peers (2)'), findsOneWidget);
    });

    testWidgets('revokes device trust via revoke button and confirmation dialog', (tester) async {
      await tester.pumpWidget(createTestApp(tester));
      await tester.pumpAndSettle();

      expect(find.text('TRUSTED'), findsOneWidget);
      final revokeBtn = find.byKey(const Key('revoke_button_dev_macbook_pro'));
      expect(revokeBtn, findsOneWidget);

      await tester.tap(revokeBtn);
      await tester.pumpAndSettle();

      // Confirmation dialog
      expect(find.text('Revoke Device Trust?'), findsOneWidget);

      // Confirm in dialog
      final dialogRevokeBtn = find.widgetWithText(FilledButton, 'Revoke');
      await tester.tap(dialogRevokeBtn);
      await tester.pumpAndSettle();

      // Device now in REVOKED state
      expect(find.text('REVOKED'), findsOneWidget);
    });
  });
}
