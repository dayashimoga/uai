import 'dart:async';
import 'package:flutter/services.dart';

/// Dart service providing reactive binding to Android and mobile native capabilities:
/// Foreground Voice Service, Assist & Share Intents, MediaSession controls,
/// and opt-in Accessibility automation.
class MobilePlatformBridge {
  static const MethodChannel _defaultChannel =
      MethodChannel('com.aissistant/mobile_bridge');

  final MethodChannel _channel;
  final StreamController<Map<String, dynamic>> _intentStreamController =
      StreamController<Map<String, dynamic>>.broadcast();
  final StreamController<String> _mediaCommandStreamController =
      StreamController<String>.broadcast();
  final StreamController<void> _audioFocusLostController =
      StreamController<void>.broadcast();

  bool _initialized = false;

  /// Creates a [MobilePlatformBridge] optionally accepting a custom [MethodChannel] for testing.
  MobilePlatformBridge({MethodChannel channel = _defaultChannel})
      : _channel = channel;

  /// Stream of system intents received by the mobile host (e.g. text share, assist trigger).
  Stream<Map<String, dynamic>> get onIntentReceived =>
      _intentStreamController.stream;

  /// Stream of media transport commands dispatched from Bluetooth headsets or lockscreen controls.
  Stream<String> get onMediaCommand => _mediaCommandStreamController.stream;

  /// Stream notified when mobile audio focus is preempted by a phone call or alarm.
  Stream<void> get onAudioFocusLost => _audioFocusLostController.stream;

  /// Initializes MethodChannel callback listeners.
  void initialize() {
    if (_initialized) return;
    _initialized = true;

    _channel.setMethodCallHandler((call) async {
      switch (call.method) {
        case 'onIntentReceived':
          if (call.arguments is Map) {
            final map = Map<String, dynamic>.from(call.arguments as Map);
            _intentStreamController.add(map);
          }
          break;
        case 'onMediaCommand':
          if (call.arguments is Map) {
            final cmd = (call.arguments as Map)['command']?.toString();
            if (cmd != null) {
              _mediaCommandStreamController.add(cmd);
            }
          }
          break;
        case 'onAudioFocusLost':
          _audioFocusLostController.add(null);
          break;
      }
    });
  }

  /// Starts the Android Foreground Voice Service with microphone type.
  Future<bool> startVoiceService() async {
    try {
      final result = await _channel.invokeMethod<bool>('startVoiceService');
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Stops the Android Foreground Voice Service and releases wake locks.
  Future<bool> stopVoiceService() async {
    try {
      final result = await _channel.invokeMethod<bool>('stopVoiceService');
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Queries whether the background/foreground voice service is actively running.
  Future<bool> isVoiceServiceRunning() async {
    try {
      final result = await _channel.invokeMethod<bool>('isVoiceServiceRunning');
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Checks whether the user has enabled the opt-in Accessibility Service in Android Settings.
  Future<bool> isAccessibilityEnabled() async {
    try {
      final result =
          await _channel.invokeMethod<bool>('isAccessibilityEnabled');
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Launches the system Accessibility Settings screen for the user to grant permission.
  Future<bool> openAccessibilitySettings() async {
    try {
      final result =
          await _channel.invokeMethod<bool>('openAccessibilitySettings');
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Checks whether battery optimizations are ignored for uninterrupted offline execution.
  Future<bool> isBatteryOptimizationIgnored() async {
    try {
      final result =
          await _channel.invokeMethod<bool>('isBatteryOptimizationIgnored');
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Prompts the user to ignore battery optimizations for the assistant.
  Future<bool> requestIgnoreBatteryOptimizations() async {
    try {
      final result =
          await _channel.invokeMethod<bool>('requestIgnoreBatteryOptimizations');
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Retrieves the initial intent payload that launched or awakened the application.
  Future<Map<String, dynamic>?> getInitialIntentPayload() async {
    try {
      final result = await _channel
          .invokeMapMethod<String, dynamic>('getInitialIntentPayload');
      return result;
    } on MissingPluginException {
      return null;
    } on PlatformException {
      return null;
    }
  }

  /// Requests the active window node hierarchy from the Accessibility Service.
  Future<Map<String, dynamic>?> getAccessibilityActiveWindow() async {
    try {
      final result = await _channel
          .invokeMapMethod<String, dynamic>('getAccessibilityActiveWindow');
      return result;
    } on MissingPluginException {
      return null;
    } on PlatformException {
      return null;
    }
  }

  /// Dispatches an action click on an accessibility node matching [text].
  Future<bool> clickAccessibilityNode(String text) async {
    try {
      final result = await _channel.invokeMethod<bool>(
        'clickAccessibilityNode',
        {'text': text},
      );
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Requests transient audio focus before speech synthesis.
  Future<bool> requestAudioFocus() async {
    try {
      final result = await _channel.invokeMethod<bool>('requestAudioFocus');
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Abandons audio focus after speech completion.
  Future<bool> abandonAudioFocus() async {
    try {
      final result = await _channel.invokeMethod<bool>('abandonAudioFocus');
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Releases stream controllers and listeners.
  void dispose() {
    _channel.setMethodCallHandler(null);
    _intentStreamController.close();
    _mediaCommandStreamController.close();
    _audioFocusLostController.close();
  }
}
