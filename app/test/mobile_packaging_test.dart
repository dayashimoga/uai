import 'dart:io';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('Mobile Release Packaging & Distribution Tests (S11.P1.2)', () {
    test('Android ProGuard rules protect Flutter embedding and native JNI symbols', () {
      final proguardFile = File('android/app/proguard-rules.pro');
      expect(proguardFile.existsSync(), isTrue, reason: 'proguard-rules.pro must exist');
      final rules = proguardFile.readAsStringSync();

      expect(rules.contains('io.flutter'), isTrue);
      expect(rules.contains('com.aissistant.aissistant'), isTrue);
      expect(rules.contains('native <methods>;'), isTrue);
      expect(rules.contains('@androidx.annotation.Keep'), isTrue);
      expect(rules.contains('-keepattributes'), isTrue);
    });

    test('Android build.gradle.kts configures NDK ABIs, R8 minification, and jniLibs', () {
      final gradleFile = File('android/app/build.gradle.kts');
      expect(gradleFile.existsSync(), isTrue, reason: 'build.gradle.kts must exist');
      final gradle = gradleFile.readAsStringSync();

      expect(gradle.contains('arm64-v8a'), isTrue);
      expect(gradle.contains('armeabi-v7a'), isTrue);
      expect(gradle.contains('x86_64'), isTrue);
      expect(gradle.contains('isMinifyEnabled = true'), isTrue);
      expect(gradle.contains('isShrinkResources = true'), isTrue);
      expect(gradle.contains('proguard-rules.pro'), isTrue);
      expect(gradle.contains('jniLibs.srcDirs'), isTrue);
    });

    test('Android and iOS manifests enforce offline-first privacy and audio permissions', () {
      final androidManifestFile = File('android/app/src/main/AndroidManifest.xml');
      expect(androidManifestFile.existsSync(), isTrue);
      final androidManifest = androidManifestFile.readAsStringSync();

      expect(androidManifest.contains('android.permission.RECORD_AUDIO'), isTrue);
      expect(androidManifest.contains('android.permission.FOREGROUND_SERVICE'), isTrue);
      expect(androidManifest.contains('android.permission.FOREGROUND_SERVICE_MICROPHONE'), isTrue);
      expect(androidManifest.contains('android.permission.INTERNET'), isTrue);
      expect(androidManifest.contains('AssistantForegroundService'), isTrue);

      final iosPlistFile = File('ios/Runner/Info.plist');
      expect(iosPlistFile.existsSync(), isTrue);
      final iosPlist = iosPlistFile.readAsStringSync();

      expect(iosPlist.contains('NSMicrophoneUsageDescription'), isTrue);
      expect(iosPlist.contains('NSSpeechRecognitionUsageDescription'), isTrue);
      expect(iosPlist.contains('NSLocalNetworkUsageDescription'), isTrue);
      expect(iosPlist.contains('<string>audio</string>'), isTrue);
    });

    test('Android and iOS packaging scripts provide complete build, bundling, and hashing logic', () {
      final androidScript = File('../packaging/mobile/android/package_android.sh');
      expect(androidScript.existsSync(), isTrue, reason: 'package_android.sh must exist');
      final androidSh = androidScript.readAsStringSync();

      expect(androidSh.contains('aarch64-linux-android'), isTrue);
      expect(androidSh.contains('armv7-linux-androideabi'), isTrue);
      expect(androidSh.contains('x86_64-linux-android'), isTrue);
      expect(androidSh.contains('flutter build apk --release'), isTrue);
      expect(androidSh.contains('flutter build appbundle --release'), isTrue);
      expect(androidSh.contains('SHA256SUMS_ANDROID.txt'), isTrue);

      final exportOptions = File('../packaging/mobile/ios/ExportOptions.plist');
      expect(exportOptions.existsSync(), isTrue, reason: 'ExportOptions.plist must exist');
      final plist = exportOptions.readAsStringSync();
      expect(plist.contains('method'), isTrue);
      expect(plist.contains('ad-hoc'), isTrue);

      final iosScript = File('../packaging/mobile/ios/package_ios.sh');
      expect(iosScript.existsSync(), isTrue, reason: 'package_ios.sh must exist');
      final iosSh = iosScript.readAsStringSync();

      expect(iosSh.contains('aarch64-apple-ios'), isTrue);
      expect(iosSh.contains('flutter build ios --release'), isTrue);
      expect(iosSh.contains('xcodebuild'), isTrue);
      expect(iosSh.contains('SHA256SUMS_IOS.txt'), isTrue);
    });

    test('GitHub Actions mobile packaging workflow defines multi-platform CI build pipeline', () {
      final workflowFile = File('../.github/workflows/mobile_packaging.yml');
      expect(workflowFile.existsSync(), isTrue, reason: 'mobile_packaging.yml must exist');
      final workflow = workflowFile.readAsStringSync();

      expect(workflow.contains('name: Mobile Release Packaging'), isTrue);
      expect(workflow.contains('runs-on: ubuntu-latest'), isTrue);
      expect(workflow.contains('runs-on: macos-latest'), isTrue);
      expect(workflow.contains('cargo-ndk'), isTrue);
      expect(workflow.contains('package_android.sh'), isTrue);
      expect(workflow.contains('package_ios.sh'), isTrue);
      expect(workflow.contains('SHA256SUMS_MOBILE.txt'), isTrue);
    });
  });
}
