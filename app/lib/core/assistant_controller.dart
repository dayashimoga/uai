import 'dart:async';
import 'dart:convert';
import 'package:flutter/foundation.dart';

import 'ffi_bridge.dart';
import 'models/audit_item.dart';
import 'models/bridge_event.dart';
import 'models/capability_item.dart';
import 'models/health_report.dart';
import 'models/voice_state.dart';

/// Reactive application state controller coordinating live Rust core states and the UI shell.
class AssistantController extends ChangeNotifier {
  final AissistantBridge _bridge;
  StreamSubscription<BridgeEvent>? _eventSub;

  HealthReport? _health;
  List<CapabilityItem> _capabilities = <CapabilityItem>[];
  List<AuditItem> _auditLogs = <AuditItem>[];
  AuditIntegrityReport? _auditIntegrity;
  String _themeMode = 'system';
  VoiceState _voiceState = const VoiceState();
  bool _isLoading = false;
  String? _errorMessage;

  AssistantController(this._bridge);

  HealthReport? get health => _health;
  List<CapabilityItem> get capabilities => List<CapabilityItem>.unmodifiable(_capabilities);
  List<AuditItem> get auditLogs => List<AuditItem>.unmodifiable(_auditLogs);
  AuditIntegrityReport? get auditIntegrity => _auditIntegrity;
  String get themeMode => _themeMode;
  VoiceState get voiceState => _voiceState;
  bool get isLoading => _isLoading;
  String? get errorMessage => _errorMessage;

  Future<void> initialize() async {
    _isLoading = true;
    _errorMessage = null;
    notifyListeners();

    try {
      await _bridge.initialize();

      // Subscribe to asynchronous Rust core event stream
      _eventSub = _bridge.eventStream.listen(_handleBridgeEvent);

      // Load initial state snapshot
      _health = _bridge.getHealth();
      _capabilities = List<CapabilityItem>.from(_bridge.getCapabilities());
      _auditLogs = List<AuditItem>.from(_bridge.queryAuditLog(limit: 20, offset: 0));
      _auditIntegrity = _bridge.verifyAudit();

      // Read persisted theme mode
      final savedTheme = _bridge.getSetting('assistant.theme_mode');
      if (savedTheme != null) {
        try {
          _themeMode = jsonDecode(savedTheme) as String? ?? 'system';
        } catch (_) {
          _themeMode = 'system';
        }
      }
    } catch (e) {
      _errorMessage = 'Failed to connect to assistant core: $e';
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  void _handleBridgeEvent(BridgeEvent event) {
    if (event is CapabilityChangedEvent) {
      final idx = _capabilities.indexWhere((c) => c.name == event.capability);
      if (idx != -1) {
        final old = _capabilities[idx];
        _capabilities[idx] = CapabilityItem(
          name: old.name,
          tier: old.tier,
          isSupported: old.isSupported,
          isActive: event.status == 'supported',
          status: event.status,
          degradationReason: event.reason,
          missingPermission: old.missingPermission,
        );
      }
      refreshDiagnostics();
    } else if (event is HealthStatusChangedEvent) {
      refreshDiagnostics();
    } else if (event is AuditLogAppendedEvent) {
      refreshAuditLog();
    } else if (event is LifecycleStateChangedEvent) {
      refreshDiagnostics();
    } else if (event is MemoryPressureEvent) {
      refreshDiagnostics();
    } else if (event is VoiceLevelUpdatedEvent) {
      _voiceState = _voiceState.copyWith(
        rmsDbfs: event.rmsDbfs,
        peakDbfs: event.peakDbfs,
      );
    } else if (event is WakeWordTriggeredEvent) {
      _voiceState = _voiceState.copyWith(
        lastSpottedKeyword: event.keyword,
        lastConfidence: event.confidence,
      );
    } else if (event is TranscriptUpdatedEvent) {
      if (event.isFinal) {
        _voiceState = _voiceState.copyWith(
          finalTranscript: event.text,
          partialTranscript: '',
          language: event.language,
          lastConfidence: event.confidence,
        );
      } else {
        _voiceState = _voiceState.copyWith(
          partialTranscript: event.text,
          language: event.language,
          lastConfidence: event.confidence,
        );
      }
    } else if (event is VoiceStateChangedEvent) {
      final mode = event.mode.toLowerCase().contains('wake')
          ? VoiceMode.wakeWord
          : VoiceMode.pushToTalk;
      _voiceState = _voiceState.copyWith(
        isListening: event.isListening,
        mode: mode,
      );
    }
    notifyListeners();
  }

  void refreshDiagnostics() {
    try {
      _health = _bridge.getHealth();
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to refresh diagnostics: $e';
      notifyListeners();
    }
  }

  void refreshCapabilities() {
    try {
      _capabilities = List<CapabilityItem>.from(_bridge.getCapabilities());
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to refresh capabilities: $e';
      notifyListeners();
    }
  }

  void refreshAuditLog() {
    try {
      _auditLogs = List<AuditItem>.from(_bridge.queryAuditLog(limit: 50, offset: 0));
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to refresh audit log: $e';
      notifyListeners();
    }
  }

  void toggleCapability(String capabilityName, bool granted) {
    try {
      final updated = _bridge.updatePermission(capabilityName, granted);
      final idx = _capabilities.indexWhere((c) => c.name == capabilityName);
      if (idx != -1) {
        _capabilities[idx] = updated;
      }
      _health = _bridge.getHealth();
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to update capability "$capabilityName": $e';
      notifyListeners();
    }
  }

  void verifyAuditIntegrity() {
    try {
      _auditIntegrity = _bridge.verifyAudit();
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to verify audit integrity: $e';
      notifyListeners();
    }
  }

  void triggerMemoryPressure(bool critical) {
    try {
      _bridge.notifyMemoryPressure(critical ? 1 : 0);
      refreshDiagnostics();
    } catch (e) {
      _errorMessage = 'Failed to notify memory pressure: $e';
      notifyListeners();
    }
  }

  void triggerShutdown() {
    try {
      _bridge.requestShutdown('user_requested');
      refreshDiagnostics();
    } catch (e) {
      _errorMessage = 'Failed to request shutdown: $e';
      notifyListeners();
    }
  }

  void setThemeMode(String mode) {
    _themeMode = mode;
    try {
      _bridge.setSetting('assistant.theme_mode', jsonEncode(mode));
    } catch (_) {}
    notifyListeners();
  }

  void startVoiceListening({VoiceMode? mode}) {
    final targetMode = mode ?? _voiceState.mode;
    try {
      _bridge.startVoiceListening(mode: targetMode);
      _voiceState = _voiceState.copyWith(
        isListening: true,
        mode: targetMode,
      );
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to start voice listening: $e';
      notifyListeners();
    }
  }

  void stopVoiceListening() {
    try {
      _bridge.stopVoiceListening();
      _voiceState = _voiceState.copyWith(isListening: false);
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to stop voice listening: $e';
      notifyListeners();
    }
  }

  void setVoiceMode(VoiceMode mode) {
    try {
      _bridge.setVoiceMode(mode);
      _voiceState = _voiceState.copyWith(mode: mode);
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to set voice mode: $e';
      notifyListeners();
    }
  }

  void setVoiceKeywords(List<String> keywords) {
    try {
      _bridge.setVoiceKeywords(keywords);
      _voiceState = _voiceState.copyWith(activeKeywords: keywords);
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to update keywords: $e';
      notifyListeners();
    }
  }

  void clearTranscript() {
    _voiceState = _voiceState.copyWith(
      partialTranscript: '',
      finalTranscript: '',
      lastSpottedKeyword: null,
      lastConfidence: null,
    );
    notifyListeners();
  }

  void injectVoiceAudio(List<double> samples) {
    try {
      _bridge.injectVoiceAudio(samples);
    } catch (e) {
      _errorMessage = 'Failed to inject audio: $e';
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _eventSub?.cancel();
    super.dispose();
  }
}
