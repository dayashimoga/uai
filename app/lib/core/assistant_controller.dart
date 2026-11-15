import 'dart:async';
import 'dart:convert';
import 'package:flutter/foundation.dart';

import 'ffi_bridge.dart';
import 'models/audit_item.dart';
import 'models/bridge_event.dart';
import 'models/capability_item.dart';
import 'models/device_models.dart';
import 'models/health_report.dart';
import 'models/knowledge_models.dart';
import 'models/model_item.dart';
import 'models/vision_models.dart';
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
  ModelManagerSnapshot? _modelSnapshot;
  BenchmarkResult? _lastBenchmark;
  bool _isBenchmarking = false;
  List<DiscoveredPeerItem> _discoveredPeers = <DiscoveredPeerItem>[];
  List<PairedDeviceItem> _pairedDevices = <PairedDeviceItem>[];
  PairingSessionInfo? _activePairingSession;
  bool _isPairing = false;
  String? _pairingError;
  List<DocumentRecordItem> _documents = <DocumentRecordItem>[];
  List<KnowledgeCitationItem> _citationSearchResults = <KnowledgeCitationItem>[];
  bool _isSearchingKnowledge = false;
  String? _knowledgeSearchQuery;
  OcrDocumentItem? _activeOcrResult;
  bool _isProcessingOcr = false;
  String? _ocrError;
  bool _isLoading = false;
  String? _errorMessage;

  AssistantController(this._bridge);

  HealthReport? get health => _health;
  List<CapabilityItem> get capabilities => List<CapabilityItem>.unmodifiable(_capabilities);
  List<AuditItem> get auditLogs => List<AuditItem>.unmodifiable(_auditLogs);
  AuditIntegrityReport? get auditIntegrity => _auditIntegrity;
  String get themeMode => _themeMode;
  VoiceState get voiceState => _voiceState;
  ModelManagerSnapshot? get modelSnapshot => _modelSnapshot;
  BenchmarkResult? get lastBenchmark => _lastBenchmark;
  bool get isBenchmarking => _isBenchmarking;
  String? get activeModelId => _modelSnapshot?.activeModelId;
  List<ModelItem> get models => _modelSnapshot?.models ?? const <ModelItem>[];
  HardwareOverview? get hardware => _modelSnapshot?.hardware;
  List<DiscoveredPeerItem> get discoveredPeers => List<DiscoveredPeerItem>.unmodifiable(_discoveredPeers);
  List<PairedDeviceItem> get pairedDevices => List<PairedDeviceItem>.unmodifiable(_pairedDevices);
  PairingSessionInfo? get activePairingSession => _activePairingSession;
  bool get isPairing => _isPairing;
  String? get pairingError => _pairingError;
  List<DocumentRecordItem> get documents => List<DocumentRecordItem>.unmodifiable(_documents);
  List<KnowledgeCitationItem> get citationSearchResults =>
      List<KnowledgeCitationItem>.unmodifiable(_citationSearchResults);
  bool get isSearchingKnowledge => _isSearchingKnowledge;
  String? get knowledgeSearchQuery => _knowledgeSearchQuery;
  OcrDocumentItem? get activeOcrResult => _activeOcrResult;
  bool get isProcessingOcr => _isProcessingOcr;
  String? get ocrError => _ocrError;
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
      _modelSnapshot = _bridge.getModelSnapshot();
      _discoveredPeers = List<DiscoveredPeerItem>.from(_bridge.getDiscoveredPeers());
      _pairedDevices = List<PairedDeviceItem>.from(_bridge.getPairedDevices());
      _documents = List<DocumentRecordItem>.from(_bridge.getKnowledgeDocuments());

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
    } else if (event is ModelLoadedEvent || event is ModelUnloadedEvent) {
      refreshModelSnapshot();
    } else if (event is BenchmarkCompletedEvent) {
      try {
        final decoded = jsonDecode(event.metricsJson) as Map<String, dynamic>;
        _lastBenchmark = BenchmarkResult.fromJson(decoded);
      } catch (_) {}
    } else if (event is DeviceDiscoveredEvent) {
      final existingIdx = _discoveredPeers.indexWhere((p) => p.instanceName == event.instanceName);
      final updatedPeer = DiscoveredPeerItem(
        instanceName: event.instanceName,
        platform: event.platform,
        port: 51820,
        addresses: event.endpoint != null ? <String>[event.endpoint!.split(':').first] : const <String>[],
        fingerprint: event.fingerprint,
        lastSeenMs: DateTime.now().millisecondsSinceEpoch,
      );
      if (existingIdx != -1) {
        _discoveredPeers[existingIdx] = updatedPeer;
      } else {
        _discoveredPeers.add(updatedPeer);
      }
    } else if (event is PairingStateChangedEvent) {
      if (event.status == 'started') {
        _isPairing = true;
        _pairingError = null;
      } else if (event.status == 'confirmed') {
        _isPairing = false;
        _activePairingSession = null;
        _pairingError = null;
        refreshDevices();
      } else if (event.status == 'failed') {
        _isPairing = false;
        _pairingError = event.error ?? 'Pairing verification failed';
      } else if (event.status == 'cancelled') {
        _isPairing = false;
        _activePairingSession = null;
        _pairingError = null;
      }
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

  void refreshModelSnapshot() {
    try {
      _modelSnapshot = _bridge.getModelSnapshot();
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to refresh model snapshot: $e';
      notifyListeners();
    }
  }

  void loadModel(String modelId) {
    try {
      _bridge.loadModel(modelId);
      refreshModelSnapshot();
    } catch (e) {
      _errorMessage = 'Failed to load model "$modelId": $e';
      notifyListeners();
    }
  }

  void unloadModel() {
    try {
      _bridge.unloadModel();
      refreshModelSnapshot();
    } catch (e) {
      _errorMessage = 'Failed to unload model: $e';
      notifyListeners();
    }
  }

  Future<BenchmarkResult?> runBenchmark({
    int promptTokens = 32,
    int generateTokens = 16,
  }) async {
    _isBenchmarking = true;
    _errorMessage = null;
    notifyListeners();

    try {
      final res = _bridge.runModelBenchmark(
        promptTokens: promptTokens,
        generateTokens: generateTokens,
      );
      _lastBenchmark = res;
      return res;
    } catch (e) {
      _errorMessage = 'Benchmark run failed: $e';
      return null;
    } finally {
      _isBenchmarking = false;
      notifyListeners();
    }
  }

  void refreshDevices() {
    try {
      _discoveredPeers = List<DiscoveredPeerItem>.from(_bridge.getDiscoveredPeers());
      _pairedDevices = List<PairedDeviceItem>.from(_bridge.getPairedDevices());
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to refresh devices: $e';
      notifyListeners();
    }
  }

  Future<PairingSessionInfo?> startPairing(String peerInstance) async {
    _isPairing = true;
    _pairingError = null;
    notifyListeners();

    try {
      final info = _bridge.startPairing(peerInstance);
      _activePairingSession = info;
      notifyListeners();
      return info;
    } catch (e) {
      _isPairing = false;
      _pairingError = 'Failed to initiate pairing: $e';
      notifyListeners();
      return null;
    }
  }

  Future<PairedDeviceItem?> confirmPairing(String sasCode) async {
    _pairingError = null;
    notifyListeners();

    try {
      final paired = _bridge.confirmPairing(sasCode);
      _isPairing = false;
      _activePairingSession = null;
      refreshDevices();
      return paired;
    } catch (e) {
      _pairingError = 'Pairing confirmation failed: $e';
      notifyListeners();
      return null;
    }
  }

  void cancelPairing() {
    try {
      _bridge.cancelPairing();
    } catch (_) {}
    _isPairing = false;
    _activePairingSession = null;
    _pairingError = null;
    notifyListeners();
  }

  void updatePeerPermission(String deviceId, String capability, String level) {
    try {
      _bridge.setPeerPermission(deviceId, capability, level);
      refreshDevices();
    } catch (e) {
      _errorMessage = 'Failed to update capability permission: $e';
      notifyListeners();
    }
  }

  void unpairDevice(String deviceId) {
    try {
      _bridge.unpairDevice(deviceId);
      refreshDevices();
    } catch (e) {
      _errorMessage = 'Failed to unpair device: $e';
      notifyListeners();
    }
  }

  void simulateDiscoveredPeer(DiscoveredPeerItem item) {
    try {
      _bridge.simulateDiscoveredPeer(item);
      refreshDevices();
    } catch (e) {
      _errorMessage = 'Failed to simulate discovered peer: $e';
      notifyListeners();
    }
  }

  Future<void> refreshKnowledgeDocuments() async {
    try {
      _documents = List<DocumentRecordItem>.from(_bridge.getKnowledgeDocuments());
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to refresh documents: $e';
      notifyListeners();
    }
  }

  Future<void> searchKnowledge(String query, {int limit = 10}) async {
    final trimmed = query.trim();
    if (trimmed.isEmpty) {
      _citationSearchResults = <KnowledgeCitationItem>[];
      _knowledgeSearchQuery = null;
      _isSearchingKnowledge = false;
      notifyListeners();
      return;
    }

    _isSearchingKnowledge = true;
    _knowledgeSearchQuery = trimmed;
    notifyListeners();

    try {
      _citationSearchResults = List<KnowledgeCitationItem>.from(
        _bridge.searchKnowledge(trimmed, limit: limit),
      );
    } catch (e) {
      _errorMessage = 'Failed to search knowledge base: $e';
    } finally {
      _isSearchingKnowledge = false;
      notifyListeners();
    }
  }

  void clearKnowledgeSearch() {
    _citationSearchResults = <KnowledgeCitationItem>[];
    _knowledgeSearchQuery = null;
    _isSearchingKnowledge = false;
    notifyListeners();
  }

  Future<String?> indexKnowledgeDocument({
    required String path,
    required String fileName,
    required String mimeType,
    required String content,
  }) async {
    try {
      final docId = _bridge.indexKnowledgeDocument(
        path: path,
        fileName: fileName,
        mimeType: mimeType,
        content: content,
      );
      await refreshKnowledgeDocuments();
      if (_knowledgeSearchQuery != null && _knowledgeSearchQuery!.isNotEmpty) {
        await searchKnowledge(_knowledgeSearchQuery!);
      }
      return docId;
    } catch (e) {
      _errorMessage = 'Failed to index document: $e';
      notifyListeners();
      return null;
    }
  }

  Future<bool> deleteKnowledgeDocument(String documentId) async {
    try {
      final deleted = _bridge.deleteKnowledgeDocument(documentId);
      if (deleted) {
        await refreshKnowledgeDocuments();
        if (_knowledgeSearchQuery != null && _knowledgeSearchQuery!.isNotEmpty) {
          await searchKnowledge(_knowledgeSearchQuery!);
        }
      }
      return deleted;
    } catch (e) {
      _errorMessage = 'Failed to delete document: $e';
      notifyListeners();
      return false;
    }
  }

  Future<OcrDocumentItem?> extractOcr(List<int> imageBytes) async {
    _isProcessingOcr = true;
    _ocrError = null;
    notifyListeners();

    try {
      final result = _bridge.extractOcr(imageBytes: imageBytes);
      _activeOcrResult = result;
      return result;
    } catch (e) {
      _ocrError = 'OCR processing failed: $e';
      return null;
    } finally {
      _isProcessingOcr = false;
      notifyListeners();
    }
  }

  void clearOcrResult() {
    _activeOcrResult = null;
    _ocrError = null;
    _isProcessingOcr = false;
    notifyListeners();
  }

  @override
  void dispose() {
    _eventSub?.cancel();
    super.dispose();
  }
}
