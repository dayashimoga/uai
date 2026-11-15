import 'package:aissistant/core/ios_bridge.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('IosPlatformBridge Unit Tests', () {
    const channel = MethodChannel('com.aissistant/ios_bridge');
    late IosPlatformBridge bridge;
    late List<MethodCall> log;

    setUp(() {
      log = <MethodCall>[];
      bridge = IosPlatformBridge(channel: channel);
      bridge.initialize();

      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (MethodCall call) async {
        log.add(call);
        switch (call.method) {
          case 'configureAudioSession':
            return true;
          case 'activateAudioSession':
            return true;
          case 'isOtherAudioPlaying':
            return false;
          case 'startAccessingSecurityScopedResource':
            if (call.arguments is Map &&
                (call.arguments as Map)['url'] ==
                    'file:///private/var/mobile/Containers/Data/Application/doc.txt') {
              return true;
            }
            return false;
          case 'stopAccessingSecurityScopedResource':
            return true;
          case 'getInitialShortcutInvocation':
            return {
              'url': 'aissistant://ask?prompt=Summarize+today+notes',
              'prompt': 'Summarize today notes',
              'scheme': 'aissistant',
              'host': 'ask',
            };
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

    test('configureAudioSession invokes method and returns true', () async {
      final res = await bridge.configureAudioSession();
      expect(res, isTrue);
      expect(log.single.method, 'configureAudioSession');
    });

    test('activateAudioSession toggles active state', () async {
      final active = await bridge.activateAudioSession(true);
      expect(active, isTrue);

      final inactive = await bridge.activateAudioSession(false);
      expect(inactive, isTrue);

      expect(log[0].arguments, {'active': true});
      expect(log[1].arguments, {'active': false});
    });

    test('isOtherAudioPlaying queries background audio status', () async {
      final playing = await bridge.isOtherAudioPlaying();
      expect(playing, isFalse);
      expect(log.single.method, 'isOtherAudioPlaying');
    });

    test('security-scoped resource access begins and ends cleanly', () async {
      const validUrl =
          'file:///private/var/mobile/Containers/Data/Application/doc.txt';
      const invalidUrl = 'file:///etc/forbidden';

      final success =
          await bridge.startAccessingSecurityScopedResource(validUrl);
      expect(success, isTrue);

      final failure =
          await bridge.startAccessingSecurityScopedResource(invalidUrl);
      expect(failure, isFalse);

      final stopped =
          await bridge.stopAccessingSecurityScopedResource(validUrl);
      expect(stopped, isTrue);

      expect(log.map((c) => c.method), [
        'startAccessingSecurityScopedResource',
        'startAccessingSecurityScopedResource',
        'stopAccessingSecurityScopedResource',
      ]);
    });

    test('getInitialShortcutInvocation returns parsed shortcut payload',
        () async {
      final payload = await bridge.getInitialShortcutInvocation();
      expect(payload, isNotNull);
      expect(payload!['scheme'], 'aissistant');
      expect(payload['prompt'], 'Summarize today notes');
    });

    test('incoming native method invocations emit events on streams', () async {
      final shortcutExpectation = expectLater(
        bridge.onShortcutTriggered,
        emits({
          'prompt': 'Play relaxing music',
          'url': 'aissistant://ask?prompt=Play+relaxing+music',
        }),
      );

      final interruptionExpectation = expectLater(
        bridge.onAudioInterruption,
        emits('began'),
      );

      final routeChangeExpectation = expectLater(
        bridge.onAudioRouteChange,
        emits('headset_disconnected'),
      );

      final binding =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      final codec = channel.codec;

      await binding.handlePlatformMessage(
        channel.name,
        codec.encodeMethodCall(const MethodCall('onShortcutTriggered', {
          'prompt': 'Play relaxing music',
          'url': 'aissistant://ask?prompt=Play+relaxing+music',
        })),
        (_) {},
      );

      await binding.handlePlatformMessage(
        channel.name,
        codec.encodeMethodCall(const MethodCall('onAudioInterruption', {
          'type': 'began',
        })),
        (_) {},
      );

      await binding.handlePlatformMessage(
        channel.name,
        codec.encodeMethodCall(const MethodCall('onAudioRouteChange', {
          'reason': 'headset_disconnected',
        })),
        (_) {},
      );

      await Future.wait([
        shortcutExpectation,
        interruptionExpectation,
        routeChangeExpectation,
      ]);
    });

    test('gracefully handles missing plugin or platform errors without crashing',
        () async {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (MethodCall call) async {
        throw PlatformException(code: 'UNAVAILABLE', message: 'Not on iOS');
      });

      expect(await bridge.configureAudioSession(), isFalse);
      expect(await bridge.activateAudioSession(true), isFalse);
      expect(await bridge.isOtherAudioPlaying(), isFalse);
      expect(
          await bridge.startAccessingSecurityScopedResource('dummy'), isFalse);
      expect(await bridge.stopAccessingSecurityScopedResource('dummy'), isFalse);
      expect(await bridge.getInitialShortcutInvocation(), isNull);
    });
  });
}
