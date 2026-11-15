import 'dart:async';
import 'package:flutter/services.dart';

/// Dart service providing reactive binding to iOS native capabilities:
/// `AVAudioSession` lifecycle, audio interruption / route change handling,
/// Siri Shortcuts / custom URL scheme invocation, and security-scoped file bookmarks.
class IosPlatformBridge {
  static const MethodChannel _defaultChannel =
      MethodChannel('com.aissistant/ios_bridge');

  final MethodChannel _channel;
  final StreamController<Map<String, dynamic>> _shortcutStreamController =
      StreamController<Map<String, dynamic>>.broadcast();
  final StreamController<String> _interruptionStreamController =
      StreamController<String>.broadcast();
  final StreamController<String> _routeChangeStreamController =
      StreamController<String>.broadcast();

  bool _initialized = false;

  /// Creates an [IosPlatformBridge] optionally accepting a custom [MethodChannel] for testing.
  IosPlatformBridge({MethodChannel channel = _defaultChannel})
      : _channel = channel;

  /// Stream of Siri Shortcut or custom URL scheme (`aissistant://...`) invocations.
  Stream<Map<String, dynamic>> get onShortcutTriggered =>
      _shortcutStreamController.stream;

  /// Stream of `AVAudioSession` interruptions (cellular call began/ended, FaceTime, Siri).
  Stream<String> get onAudioInterruption =>
      _interruptionStreamController.stream;

  /// Stream of audio route changes (headphone connect/disconnect).
  Stream<String> get onAudioRouteChange =>
      _routeChangeStreamController.stream;

  /// Initializes MethodChannel callback listeners.
  void initialize() {
    if (_initialized) return;
    _initialized = true;

    _channel.setMethodCallHandler((call) async {
      switch (call.method) {
        case 'onShortcutTriggered':
          if (call.arguments is Map) {
            final map = Map<String, dynamic>.from(call.arguments as Map);
            _shortcutStreamController.add(map);
          }
          break;
        case 'onAudioInterruption':
          if (call.arguments is Map) {
            final type = (call.arguments as Map)['type']?.toString();
            if (type != null) {
              _interruptionStreamController.add(type);
            }
          }
          break;
        case 'onAudioRouteChange':
          if (call.arguments is Map) {
            final reason = (call.arguments as Map)['reason']?.toString();
            if (reason != null) {
              _routeChangeStreamController.add(reason);
            }
          }
          break;
      }
    });
  }

  /// Configures `AVAudioSession` with `.playAndRecord` category and `.spokenAudio` mode.
  Future<bool> configureAudioSession() async {
    try {
      final result =
          await _channel.invokeMethod<bool>('configureAudioSession');
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Activates or deactivates the shared `AVAudioSession`.
  Future<bool> activateAudioSession(bool active) async {
    try {
      final result = await _channel.invokeMethod<bool>(
        'activateAudioSession',
        {'active': active},
      );
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Checks whether background audio from other applications is currently playing.
  Future<bool> isOtherAudioPlaying() async {
    try {
      final result =
          await _channel.invokeMethod<bool>('isOtherAudioPlaying');
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Begins accessing a security-scoped document or folder bookmark within the iOS sandbox.
  Future<bool> startAccessingSecurityScopedResource(String url) async {
    try {
      final result = await _channel.invokeMethod<bool>(
        'startAccessingSecurityScopedResource',
        {'url': url},
      );
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Relinquishes access to a security-scoped document or folder bookmark.
  Future<bool> stopAccessingSecurityScopedResource(String url) async {
    try {
      final result = await _channel.invokeMethod<bool>(
        'stopAccessingSecurityScopedResource',
        {'url': url},
      );
      return result ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Retrieves the initial shortcut or URL launch payload, if the app was launched by one.
  Future<Map<String, dynamic>?> getInitialShortcutInvocation() async {
    try {
      final result = await _channel
          .invokeMapMethod<String, dynamic>('getInitialShortcutInvocation');
      return result;
    } on MissingPluginException {
      return null;
    } on PlatformException {
      return null;
    }
  }

  /// Releases stream controllers and listeners.
  void dispose() {
    _channel.setMethodCallHandler(null);
    _shortcutStreamController.close();
    _interruptionStreamController.close();
    _routeChangeStreamController.close();
  }
}
