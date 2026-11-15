import 'dart:io';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('Five-Platform Runner Configuration & Permission Manifest Tests', () {
    test('Android manifest contains required offline permissions and zero telemetry', () {
      final manifestFile = File('android/app/src/main/AndroidManifest.xml');
      expect(manifestFile.existsSync(), isTrue, reason: 'AndroidManifest.xml must exist');

      final content = manifestFile.readAsStringSync();
      expect(content.contains('android.permission.RECORD_AUDIO'), isTrue);
      expect(content.contains('android.permission.INTERNET'), isTrue);
      expect(content.contains('android.permission.ACCESS_NETWORK_STATE'), isTrue);
      expect(content.contains('android.permission.CHANGE_WIFI_MULTICAST_STATE'), isTrue);
      expect(content.contains('android.permission.FOREGROUND_SERVICE'), isTrue);
      expect(content.contains('android.permission.POST_NOTIFICATIONS'), isTrue);
    });

    test('iOS Info.plist contains offline voice and LAN privacy descriptions', () {
      final plistFile = File('ios/Runner/Info.plist');
      expect(plistFile.existsSync(), isTrue, reason: 'iOS Info.plist must exist');

      final content = plistFile.readAsStringSync();
      expect(content.contains('<key>NSMicrophoneUsageDescription</key>'), isTrue);
      expect(content.contains('<key>NSLocalNetworkUsageDescription</key>'), isTrue);
      expect(content.contains('<string>_aissistant._tcp</string>'), isTrue);
      expect(content.contains('<string>audio</string>'), isTrue);
    });

    test('macOS entitlements contain microphone and local networking permissions', () {
      final debugEntitlements = File('macos/Runner/DebugProfile.entitlements');
      final releaseEntitlements = File('macos/Runner/Release.entitlements');
      expect(debugEntitlements.existsSync(), isTrue);
      expect(releaseEntitlements.existsSync(), isTrue);

      final debugContent = debugEntitlements.readAsStringSync();
      final releaseContent = releaseEntitlements.readAsStringSync();

      expect(debugContent.contains('com.apple.security.device.audio-input'), isTrue);
      expect(debugContent.contains('com.apple.security.network.client'), isTrue);
      expect(debugContent.contains('com.apple.security.network.server'), isTrue);

      expect(releaseContent.contains('com.apple.security.device.audio-input'), isTrue);
      expect(releaseContent.contains('com.apple.security.network.client'), isTrue);
      expect(releaseContent.contains('com.apple.security.network.server'), isTrue);
    });

    test('Windows CMakeLists.txt contains aissistant_ffi.dll bundling rules', () {
      final cmakeFile = File('windows/CMakeLists.txt');
      expect(cmakeFile.existsSync(), isTrue);

      final content = cmakeFile.readAsStringSync();
      expect(content.contains('aissistant_ffi.dll'), isTrue);
      expect(content.contains('AISSISTANT_FFI_DIR'), isTrue);
    });

    test('Linux CMakeLists.txt contains libaissistant_ffi.so bundling rules', () {
      final cmakeFile = File('linux/CMakeLists.txt');
      expect(cmakeFile.existsSync(), isTrue);

      final content = cmakeFile.readAsStringSync();
      expect(content.contains('libaissistant_ffi.so'), isTrue);
      expect(content.contains('AISSISTANT_FFI_DIR'), isTrue);
    });
  });
}
