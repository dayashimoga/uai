import 'package:aissistant/core/mobile_bridge.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('MobilePlatformBridge Unit Tests', () {
    const channel = MethodChannel('com.aissistant/mobile_bridge');
    late MobilePlatformBridge bridge;
    late List<MethodCall> log;

    setUp(() {
      log = <MethodCall>[];
      bridge = MobilePlatformBridge(channel: channel);
      bridge.initialize();

      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (MethodCall call) async {
        log.add(call);
        switch (call.method) {
          case 'startVoiceService':
            return true;
          case 'stopVoiceService':
            return true;
          case 'isVoiceServiceRunning':
            return true;
          case 'isAccessibilityEnabled':
            return true;
          case 'openAccessibilitySettings':
            return true;
          case 'isBatteryOptimizationIgnored':
            return true;
          case 'requestIgnoreBatteryOptimizations':
            return true;
          case 'getInitialIntentPayload':
            return {
              'action': 'android.intent.action.SEND',
              'text': 'Summarize this document please',
              'type': 'text/plain',
            };
          case 'getAccessibilityActiveWindow':
            return {
              'packageName': 'com.android.settings',
              'className': 'android.widget.FrameLayout',
              'text': 'Settings',
              'isClickable': false,
              'children': [
                {
                  'packageName': 'com.android.settings',
                  'className': 'android.widget.Button',
                  'text': 'Wi-Fi',
                  'isClickable': true,
                }
              ]
            };
          case 'clickAccessibilityNode':
            if (call.arguments is Map &&
                (call.arguments as Map)['text'] == 'Wi-Fi') {
              return true;
            }
            return false;
          case 'requestAudioFocus':
            return true;
          case 'abandonAudioFocus':
            return true;
          default:
            return null;
        }
      });
    });

    tearDown(() {
      bridge.dispose();
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null);
    });

    test('startVoiceService invokes method and returns true', () async {
      final res = await bridge.startVoiceService();
      expect(res, isTrue);
      expect(log.single.method, 'startVoiceService');
    });

    test('stopVoiceService invokes method and returns true', () async {
      final res = await bridge.stopVoiceService();
      expect(res, isTrue);
      expect(log.single.method, 'stopVoiceService');
    });

    test('isVoiceServiceRunning queries service state', () async {
      final res = await bridge.isVoiceServiceRunning();
      expect(res, isTrue);
      expect(log.single.method, 'isVoiceServiceRunning');
    });

    test('isAccessibilityEnabled queries accessibility state', () async {
      final res = await bridge.isAccessibilityEnabled();
      expect(res, isTrue);
      expect(log.single.method, 'isAccessibilityEnabled');
    });

    test('openAccessibilitySettings launches system settings', () async {
      final res = await bridge.openAccessibilitySettings();
      expect(res, isTrue);
      expect(log.single.method, 'openAccessibilitySettings');
    });

    test('battery optimization methods check and request whitelisting', () async {
      final isIgnored = await bridge.isBatteryOptimizationIgnored();
      expect(isIgnored, isTrue);

      final req = await bridge.requestIgnoreBatteryOptimizations();
      expect(req, isTrue);

      expect(log.map((c) => c.method), [
        'isBatteryOptimizationIgnored',
        'requestIgnoreBatteryOptimizations',
      ]);
    });

    test('getInitialIntentPayload returns serialized intent parameters', () async {
      final payload = await bridge.getInitialIntentPayload();
      expect(payload, isNotNull);
      expect(payload!['action'], 'android.intent.action.SEND');
      expect(payload['text'], 'Summarize this document please');
    });

    test('getAccessibilityActiveWindow returns node hierarchy map', () async {
      final hierarchy = await bridge.getAccessibilityActiveWindow();
      expect(hierarchy, isNotNull);
      expect(hierarchy!['packageName'], 'com.android.settings');
      expect(hierarchy['children'], isA<List<dynamic>>());
    });

    test('clickAccessibilityNode passes target text and returns result', () async {
      final success = await bridge.clickAccessibilityNode('Wi-Fi');
      expect(success, isTrue);

      final failed = await bridge.clickAccessibilityNode('Bluetooth');
      expect(failed, isFalse);

      expect(log[0].arguments, {'text': 'Wi-Fi'});
      expect(log[1].arguments, {'text': 'Bluetooth'});
    });

    test('audio focus methods request and abandon focus cleanly', () async {
      final req = await bridge.requestAudioFocus();
      expect(req, isTrue);

      final abandon = await bridge.abandonAudioFocus();
      expect(abandon, isTrue);

      expect(log.map((c) => c.method), [
        'requestAudioFocus',
        'abandonAudioFocus',
      ]);
    });

    test('incoming native method invocations emit events on streams', () async {
      final intentExpectation = expectLater(
        bridge.onIntentReceived,
        emits({
          'action': 'android.intent.action.ASSIST',
          'isAssist': true,
        }),
      );

      final mediaExpectation = expectLater(
        bridge.onMediaCommand,
        emits('play_pause'),
      );

      final focusExpectation = expectLater(
        bridge.onAudioFocusLost,
        emits(null),
      );

      // Simulate native calling into Flutter
      final binding =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      final codec = channel.codec;

      await binding.handlePlatformMessage(
        channel.name,
        codec.encodeMethodCall(const MethodCall('onIntentReceived', {
          'action': 'android.intent.action.ASSIST',
          'isAssist': true,
        })),
        (_) {},
      );

      await binding.handlePlatformMessage(
        channel.name,
        codec.encodeMethodCall(const MethodCall('onMediaCommand', {
          'command': 'play_pause',
        })),
        (_) {},
      );

      await binding.handlePlatformMessage(
        channel.name,
        codec.encodeMethodCall(const MethodCall('onAudioFocusLost', null)),
        (_) {},
      );

      await Future.wait([
        intentExpectation,
        mediaExpectation,
        focusExpectation,
      ]);
    });

    test('gracefully handles missing plugin or platform errors without crashing', () async {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (MethodCall call) async {
        throw PlatformException(code: 'UNAVAILABLE', message: 'Not on Android');
      });

      expect(await bridge.startVoiceService(), isFalse);
      expect(await bridge.stopVoiceService(), isFalse);
      expect(await bridge.isVoiceServiceRunning(), isFalse);
      expect(await bridge.isAccessibilityEnabled(), isFalse);
      expect(await bridge.openAccessibilitySettings(), isFalse);
      expect(await bridge.isBatteryOptimizationIgnored(), isFalse);
      expect(await bridge.requestIgnoreBatteryOptimizations(), isFalse);
      expect(await bridge.getInitialIntentPayload(), isNull);
      expect(await bridge.getAccessibilityActiveWindow(), isNull);
      expect(await bridge.clickAccessibilityNode('Test'), isFalse);
      expect(await bridge.requestAudioFocus(), isFalse);
      expect(await bridge.abandonAudioFocus(), isFalse);
    });
  });
}
