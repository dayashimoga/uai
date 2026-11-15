import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:aissistant/core/assistant_controller.dart';
import 'package:aissistant/core/ffi_bridge.dart';
import 'package:aissistant/core/models/bridge_event.dart';
import 'package:aissistant/core/models/voice_state.dart';
import 'package:aissistant/ui/screens/voice_screen.dart';

void main() {
  group('VoiceScreen Widget & Interaction Tests', () {
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

    Widget createVoiceApp() {
      return MaterialApp(
        theme: ThemeData(useMaterial3: true),
        home: VoiceScreen(controller: controller),
      );
    }

    testWidgets('renders initial voice screen UI elements and 100% offline badge', (tester) async {
      await tester.pumpWidget(createVoiceApp());
      await tester.pumpAndSettle();

      // AppBar title & privacy chip
      expect(find.text('Voice Assistant'), findsOneWidget);
      expect(find.text('100% Offline'), findsOneWidget);

      // Status header
      expect(find.text('Microphone Idle'), findsOneWidget);

      // Mode selector
      expect(find.text('Push-to-Talk'), findsOneWidget);
      expect(find.text('Hands-Free Wake Word'), findsOneWidget);

      // Waveform visualizer
      expect(find.byType(WaveformVisualizer), findsOneWidget);

      // Transcript card empty state
      expect(find.text('Live Transcript'), findsOneWidget);
      expect(find.textContaining('No speech captured yet'), findsOneWidget);
    });

    testWidgets('push-to-talk button tap toggles listening state', (tester) async {
      await tester.pumpWidget(createVoiceApp());
      await tester.pumpAndSettle();

      expect(controller.voiceState.isListening, isFalse);

      // Tap microphone action button to start listening
      final actionButtonFinder = find.byKey(const Key('voice_action_button'));
      expect(actionButtonFinder, findsOneWidget);
      await tester.tap(actionButtonFinder);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));

      expect(controller.voiceState.isListening, isTrue);
      expect(find.text('Recording & Transcribing'), findsOneWidget);

      // Tap microphone action button again to stop listening
      await tester.tap(actionButtonFinder);
      await tester.pumpAndSettle();

      expect(controller.voiceState.isListening, isFalse);
      expect(find.text('Microphone Idle'), findsOneWidget);
    });

    testWidgets('switching to wake-word mode displays keyword selector chips', (tester) async {
      tester.view.physicalSize = const Size(1024, 900);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(createVoiceApp());
      await tester.pumpAndSettle();

      // Keywords selector should not be visible in Push-to-Talk mode
      expect(find.text('Active Wake Keywords'), findsNothing);

      // Tap Hands-Free Wake Word mode
      await tester.tap(find.text('Hands-Free Wake Word'));
      await tester.pumpAndSettle();

      expect(controller.voiceState.mode, VoiceMode.wakeWord);
      expect(find.text('Active Wake Keywords'), findsOneWidget);
      expect(find.text('hey assistant'), findsOneWidget);
      expect(find.text('computer'), findsOneWidget);
      expect(find.text('jarvis'), findsOneWidget);

      // Ensure 'computer' chip is visible and toggle it
      await tester.ensureVisible(find.text('computer'));
      await tester.tap(find.text('computer'));
      await tester.pumpAndSettle();

      expect(controller.voiceState.activeKeywords, contains('computer'));
    });

    testWidgets('streaming transcript events render in transcript card with language badge', (tester) async {
      tester.view.physicalSize = const Size(1024, 900);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(createVoiceApp());
      await tester.pumpAndSettle();

      // Inject audio
      controller.injectVoiceAudio(<double>[0.2, 0.4, -0.3]);
      await tester.pump(const Duration(milliseconds: 50));

      // Update state via simulated transcript
      controller.startVoiceListening(mode: VoiceMode.pushToTalk);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));

      // Emit live partial and final transcript events
      bridge.emitEvent(const TranscriptUpdatedEvent(
        text: 'What time is',
        isFinal: false,
        language: 'en',
        confidence: 0.88,
      ));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.textContaining('What time is'), findsOneWidget);

      bridge.emitEvent(const TranscriptUpdatedEvent(
        text: 'What time is it in Tokyo?',
        isFinal: true,
        language: 'en',
        confidence: 0.96,
      ));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.textContaining('What time is it in Tokyo?'), findsOneWidget);

      // Stop listening so animations cease
      controller.stopVoiceListening();
      await tester.pumpAndSettle();

      // Verify clear transcript function
      controller.clearTranscript();
      await tester.pumpAndSettle();
      expect(controller.voiceState.finalTranscript, isEmpty);
      expect(controller.voiceState.partialTranscript, isEmpty);
    });

    testWidgets('waveform visualizer adapts bar heights when receiving RMS levels', (tester) async {
      await tester.pumpWidget(
        const MaterialApp(
          home: Scaffold(
            body: WaveformVisualizer(
              rmsDbfs: -12.0,
              peakDbfs: -3.0,
              isListening: true,
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(WaveformVisualizer), findsOneWidget);
      expect(find.byType(AnimatedContainer), findsNWidgets(9));
    });
  });
}
