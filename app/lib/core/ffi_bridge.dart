import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:math';

import 'package:ffi/ffi.dart';

import 'models/audit_item.dart';
import 'models/bridge_event.dart';
import 'models/capability_item.dart';
import 'models/health_report.dart';
import 'models/model_item.dart';
import 'models/voice_state.dart';

/// Abstract interface for Universal AI Assistant Rust Core communication.
abstract class AissistantBridge {
  Future<void> initialize({String? dataDir});
  Future<void> shutdown();
  HealthReport getHealth();
  List<CapabilityItem> getCapabilities();
  CapabilityItem updatePermission(String capability, bool granted);
  List<AuditItem> queryAuditLog({int limit = 20, int offset = 0});
  AuditIntegrityReport verifyAudit();
  String? getSetting(String key);
  void setSetting(String key, String valueJson);
  String getLifecycleState();
  void requestShutdown(String reason);
  void notifyMemoryPressure(int level);
  void startVoiceListening({VoiceMode mode = VoiceMode.pushToTalk});
  void stopVoiceListening();
  void setVoiceMode(VoiceMode mode);
  void setVoiceKeywords(List<String> keywords);
  void injectVoiceAudio(List<double> samples);
  ModelManagerSnapshot getModelSnapshot();
  void loadModel(String modelId);
  void unloadModel();
  BenchmarkResult runModelBenchmark({int promptTokens = 32, int generateTokens = 16});
  Stream<BridgeEvent> get eventStream;
}

// C-ABI Native Typedefs
typedef _BridgeInitC = Int32 Function(Pointer<Utf8> dataDir);
typedef _BridgeInitDart = int Function(Pointer<Utf8> dataDir);

typedef _BridgeShutdownC = Int32 Function();
typedef _BridgeShutdownDart = int Function();

typedef _BridgeGetStringC = Pointer<Utf8> Function();
typedef _BridgeGetStringDart = Pointer<Utf8> Function();

typedef _BridgeUpdatePermC = Pointer<Utf8> Function(Pointer<Utf8> capability, Int32 granted);
typedef _BridgeUpdatePermDart = Pointer<Utf8> Function(Pointer<Utf8> capability, int granted);

typedef _BridgeQueryAuditC = Pointer<Utf8> Function(Uint32 limit, Uint32 offset);
typedef _BridgeQueryAuditDart = Pointer<Utf8> Function(int limit, int offset);

typedef _BridgeGetSettingC = Pointer<Utf8> Function(Pointer<Utf8> key);
typedef _BridgeGetSettingDart = Pointer<Utf8> Function(Pointer<Utf8> key);

typedef _BridgeSetSettingC = Int32 Function(Pointer<Utf8> key, Pointer<Utf8> valueJson);
typedef _BridgeSetSettingDart = int Function(Pointer<Utf8> key, Pointer<Utf8> valueJson);

typedef _BridgeReqShutdownC = Int32 Function(Pointer<Utf8> reason);
typedef _BridgeReqShutdownDart = int Function(Pointer<Utf8> reason);

typedef _BridgeNotifyMemC = Int32 Function(Int32 level);
typedef _BridgeNotifyMemDart = int Function(int level);

typedef _BridgeVoiceStartC = Int32 Function(Uint32 mode);
typedef _BridgeVoiceStartDart = int Function(int mode);

typedef _BridgeVoiceStopC = Int32 Function();
typedef _BridgeVoiceStopDart = int Function();

typedef _BridgeVoiceSetModeC = Int32 Function(Uint32 mode);
typedef _BridgeVoiceSetModeDart = int Function(int mode);

typedef _BridgeVoiceSetKeywordsC = Int32 Function(Pointer<Utf8> keywordsJson);
typedef _BridgeVoiceSetKeywordsDart = int Function(Pointer<Utf8> keywordsJson);

typedef _BridgeVoiceInjectC = Int32 Function(Pointer<Float> samples, IntPtr len);
typedef _BridgeVoiceInjectDart = int Function(Pointer<Float> samples, int len);

typedef _BridgeModelSnapC = Pointer<Utf8> Function();
typedef _BridgeModelSnapDart = Pointer<Utf8> Function();

typedef _BridgeModelLoadC = Int32 Function(Pointer<Utf8> modelId);
typedef _BridgeModelLoadDart = int Function(Pointer<Utf8> modelId);

typedef _BridgeModelUnloadC = Int32 Function();
typedef _BridgeModelUnloadDart = int Function();

typedef _BridgeModelBenchC = Pointer<Utf8> Function(IntPtr promptTokens, IntPtr generateTokens);
typedef _BridgeModelBenchDart = Pointer<Utf8> Function(int promptTokens, int generateTokens);

typedef _BridgeFreeStringC = Void Function(Pointer<Utf8> ptr);
typedef _BridgeFreeStringDart = void Function(Pointer<Utf8> ptr);

/// Native FFI implementation driving aissistant-ffi C-ABI entry points.
class NativeFfiBridge implements AissistantBridge {
  final DynamicLibrary _lib;
  final StreamController<BridgeEvent> _eventController =
      StreamController<BridgeEvent>.broadcast();

  late final _BridgeInitDart _init;
  late final _BridgeShutdownDart _shutdown;
  late final _BridgeGetStringDart _getHealth;
  late final _BridgeGetStringDart _getCapabilities;
  late final _BridgeUpdatePermDart _updatePermission;
  late final _BridgeQueryAuditDart _queryAudit;
  late final _BridgeGetStringDart _verifyAudit;
  late final _BridgeGetSettingDart _getSetting;
  late final _BridgeSetSettingDart _setSetting;
  late final _BridgeGetStringDart _getLifecycleState;
  late final _BridgeReqShutdownDart _requestShutdown;
  late final _BridgeNotifyMemDart _notifyMemoryPressure;
  late final _BridgeVoiceStartDart _voiceStart;
  late final _BridgeVoiceStopDart _voiceStop;
  late final _BridgeVoiceSetModeDart _voiceSetMode;
  late final _BridgeVoiceSetKeywordsDart _voiceSetKeywords;
  late final _BridgeVoiceInjectDart _voiceInject;
  late final _BridgeModelSnapDart _modelSnapshot;
  late final _BridgeModelLoadDart _modelLoad;
  late final _BridgeModelUnloadDart _modelUnload;
  late final _BridgeModelBenchDart _modelBenchmarkRun;
  late final _BridgeFreeStringDart _freeString;

  NativeFfiBridge(this._lib) {
    _init = _lib.lookupFunction<_BridgeInitC, _BridgeInitDart>('bridge_c_init');
    _shutdown = _lib.lookupFunction<_BridgeShutdownC, _BridgeShutdownDart>('bridge_c_shutdown');
    _getHealth = _lib.lookupFunction<_BridgeGetStringC, _BridgeGetStringDart>('bridge_c_get_health');
    _getCapabilities = _lib.lookupFunction<_BridgeGetStringC, _BridgeGetStringDart>('bridge_c_get_capabilities');
    _updatePermission = _lib.lookupFunction<_BridgeUpdatePermC, _BridgeUpdatePermDart>('bridge_c_update_permission');
    _queryAudit = _lib.lookupFunction<_BridgeQueryAuditC, _BridgeQueryAuditDart>('bridge_c_query_audit');
    _verifyAudit = _lib.lookupFunction<_BridgeGetStringC, _BridgeGetStringDart>('bridge_c_verify_audit');
    _getSetting = _lib.lookupFunction<_BridgeGetSettingC, _BridgeGetSettingDart>('bridge_c_get_setting');
    _setSetting = _lib.lookupFunction<_BridgeSetSettingC, _BridgeSetSettingDart>('bridge_c_set_setting');
    _getLifecycleState = _lib.lookupFunction<_BridgeGetStringC, _BridgeGetStringDart>('bridge_c_get_lifecycle_state');
    _requestShutdown = _lib.lookupFunction<_BridgeReqShutdownC, _BridgeReqShutdownDart>('bridge_c_request_shutdown');
    _notifyMemoryPressure = _lib.lookupFunction<_BridgeNotifyMemC, _BridgeNotifyMemDart>('bridge_c_notify_memory_pressure');
    _voiceStart = _lib.lookupFunction<_BridgeVoiceStartC, _BridgeVoiceStartDart>('bridge_c_voice_start_listening');
    _voiceStop = _lib.lookupFunction<_BridgeVoiceStopC, _BridgeVoiceStopDart>('bridge_c_voice_stop_listening');
    _voiceSetMode = _lib.lookupFunction<_BridgeVoiceSetModeC, _BridgeVoiceSetModeDart>('bridge_c_voice_set_mode');
    _voiceSetKeywords = _lib.lookupFunction<_BridgeVoiceSetKeywordsC, _BridgeVoiceSetKeywordsDart>('bridge_c_voice_set_keywords');
    _voiceInject = _lib.lookupFunction<_BridgeVoiceInjectC, _BridgeVoiceInjectDart>('bridge_c_voice_inject_audio');
    _modelSnapshot = _lib.lookupFunction<_BridgeModelSnapC, _BridgeModelSnapDart>('bridge_c_model_snapshot');
    _modelLoad = _lib.lookupFunction<_BridgeModelLoadC, _BridgeModelLoadDart>('bridge_c_model_load');
    _modelUnload = _lib.lookupFunction<_BridgeModelUnloadC, _BridgeModelUnloadDart>('bridge_c_model_unload');
    _modelBenchmarkRun = _lib.lookupFunction<_BridgeModelBenchC, _BridgeModelBenchDart>('bridge_c_model_benchmark_run');
    _freeString = _lib.lookupFunction<_BridgeFreeStringC, _BridgeFreeStringDart>('bridge_c_free_string');
  }

  static DynamicLibrary openLibrary([String? customPath]) {
    if (customPath != null) {
      return DynamicLibrary.open(customPath);
    }

    if (Platform.isWindows) {
      // Search local build targets or executable directory
      const candidates = <String>[
        'aissistant_ffi.dll',
        'core/target/debug/aissistant_ffi.dll',
        '../core/target/debug/aissistant_ffi.dll',
        '../../core/target/debug/aissistant_ffi.dll',
      ];
      for (final path in candidates) {
        if (File(path).existsSync()) {
          return DynamicLibrary.open(path);
        }
      }
      return DynamicLibrary.open('aissistant_ffi.dll');
    } else if (Platform.isLinux || Platform.isAndroid) {
      return DynamicLibrary.open('libaissistant_ffi.so');
    } else if (Platform.isMacOS || Platform.isIOS) {
      return DynamicLibrary.open('libaissistant_ffi.dylib');
    } else {
      throw UnsupportedError('Unsupported platform for aissistant-ffi');
    }
  }

  String _consumeAndFree(Pointer<Utf8> ptr) {
    if (ptr == nullptr) return '';
    try {
      return ptr.toDartString();
    } finally {
      _freeString(ptr);
    }
  }

  @override
  Stream<BridgeEvent> get eventStream => _eventController.stream;

  @override
  Future<void> initialize({String? dataDir}) async {
    final ptr = dataDir != null ? dataDir.toNativeUtf8() : nullptr;
    try {
      final res = _init(ptr);
      if (res != 0) {
        throw StateError('Failed to initialize aissistant-ffi: code $res');
      }
    } finally {
      if (ptr != nullptr) malloc.free(ptr);
    }
  }

  @override
  Future<void> shutdown() async {
    _shutdown();
  }

  @override
  HealthReport getHealth() {
    final jsonStr = _consumeAndFree(_getHealth());
    final decoded = jsonDecode(jsonStr) as Map<String, dynamic>;
    return HealthReport.fromJson(decoded);
  }

  @override
  List<CapabilityItem> getCapabilities() {
    final jsonStr = _consumeAndFree(_getCapabilities());
    final decoded = jsonDecode(jsonStr) as List<dynamic>;
    return decoded
        .map((e) => CapabilityItem.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  CapabilityItem updatePermission(String capability, bool granted) {
    final capPtr = capability.toNativeUtf8();
    try {
      final jsonStr = _consumeAndFree(_updatePermission(capPtr, granted ? 1 : 0));
      final decoded = jsonDecode(jsonStr) as Map<String, dynamic>;
      final item = CapabilityItem.fromJson(decoded);
      _eventController.add(CapabilityChangedEvent(
        capability: item.name,
        status: item.status,
        reason: item.degradationReason,
      ));
      return item;
    } finally {
      malloc.free(capPtr);
    }
  }

  @override
  List<AuditItem> queryAuditLog({int limit = 20, int offset = 0}) {
    final jsonStr = _consumeAndFree(_queryAudit(limit, offset));
    final decoded = jsonDecode(jsonStr) as List<dynamic>;
    return decoded
        .map((e) => AuditItem.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  AuditIntegrityReport verifyAudit() {
    final jsonStr = _consumeAndFree(_verifyAudit());
    final decoded = jsonDecode(jsonStr) as Map<String, dynamic>;
    return AuditIntegrityReport.fromJson(decoded);
  }

  @override
  String? getSetting(String key) {
    final keyPtr = key.toNativeUtf8();
    try {
      final ptr = _getSetting(keyPtr);
      if (ptr == nullptr) return null;
      return _consumeAndFree(ptr);
    } finally {
      malloc.free(keyPtr);
    }
  }

  @override
  void setSetting(String key, String valueJson) {
    final keyPtr = key.toNativeUtf8();
    final valPtr = valueJson.toNativeUtf8();
    try {
      final res = _setSetting(keyPtr, valPtr);
      if (res != 0) {
        throw StateError('Failed to set setting "$key"');
      }
      _eventController.add(SettingChangedEvent(
        key: key,
        valuePreview: valueJson.length > 32 ? '${valueJson.substring(0, 32)}...' : valueJson,
      ));
    } finally {
      malloc.free(keyPtr);
      malloc.free(valPtr);
    }
  }

  @override
  String getLifecycleState() {
    final jsonStr = _consumeAndFree(_getLifecycleState());
    return jsonDecode(jsonStr) as String? ?? 'unknown';
  }

  @override
  void requestShutdown(String reason) {
    final reasonPtr = reason.toNativeUtf8();
    try {
      _requestShutdown(reasonPtr);
    } finally {
      malloc.free(reasonPtr);
    }
  }

  @override
  void notifyMemoryPressure(int level) {
    _notifyMemoryPressure(level);
  }

  @override
  void startVoiceListening({VoiceMode mode = VoiceMode.pushToTalk}) {
    final modeCode = mode == VoiceMode.pushToTalk ? 0 : 1;
    final res = _voiceStart(modeCode);
    if (res != 0) {
      throw StateError('Failed to start voice listening: code $res');
    }
  }

  @override
  void stopVoiceListening() {
    final res = _voiceStop();
    if (res != 0) {
      throw StateError('Failed to stop voice listening: code $res');
    }
  }

  @override
  void setVoiceMode(VoiceMode mode) {
    final modeCode = mode == VoiceMode.pushToTalk ? 0 : 1;
    final res = _voiceSetMode(modeCode);
    if (res != 0) {
      throw StateError('Failed to set voice mode: code $res');
    }
  }

  @override
  void setVoiceKeywords(List<String> keywords) {
    final jsonStr = jsonEncode(keywords);
    final ptr = jsonStr.toNativeUtf8();
    try {
      final res = _voiceSetKeywords(ptr);
      if (res != 0) {
        throw StateError('Failed to set voice keywords: code $res');
      }
    } finally {
      malloc.free(ptr);
    }
  }

  @override
  void injectVoiceAudio(List<double> samples) {
    if (samples.isEmpty) return;
    final ptr = malloc<Float>(samples.length);
    try {
      for (var i = 0; i < samples.length; i++) {
        ptr[i] = samples[i];
      }
      final res = _voiceInject(ptr, samples.length);
      if (res != 0) {
        throw StateError('Failed to inject voice audio: code $res');
      }
    } finally {
      malloc.free(ptr);
    }
  }

  @override
  ModelManagerSnapshot getModelSnapshot() {
    final ptr = _modelSnapshot();
    if (ptr == nullptr) {
      throw StateError('Failed to read model snapshot: null pointer');
    }
    try {
      final jsonStr = ptr.toDartString();
      final decoded = json.decode(jsonStr) as Map<String, dynamic>;
      return ModelManagerSnapshot.fromJson(decoded);
    } finally {
      _freeString(ptr);
    }
  }

  @override
  void loadModel(String modelId) {
    final ptr = modelId.toNativeUtf8();
    try {
      final res = _modelLoad(ptr);
      if (res != 0) {
        throw StateError('Failed to load model $modelId: code $res');
      }
    } finally {
      malloc.free(ptr);
    }
  }

  @override
  void unloadModel() {
    final res = _modelUnload();
    if (res != 0) {
      throw StateError('Failed to unload model: code $res');
    }
  }

  @override
  BenchmarkResult runModelBenchmark({int promptTokens = 32, int generateTokens = 16}) {
    final ptr = _modelBenchmarkRun(promptTokens, generateTokens);
    if (ptr == nullptr) {
      throw StateError('Failed to execute benchmark: null pointer');
    }
    try {
      final jsonStr = ptr.toDartString();
      final decoded = json.decode(jsonStr) as Map<String, dynamic>;
      return BenchmarkResult.fromJson(decoded);
    } finally {
      _freeString(ptr);
    }
  }
}

/// Fallback / mock-free test harness bridge implementing exact live schema contracts.
class TestHarnessBridge implements AissistantBridge {
  final StreamController<BridgeEvent> _eventController =
      StreamController<BridgeEvent>.broadcast();

  HealthStatus _status = HealthStatus.healthy;
  String _lifecycleState = 'running';
  final Map<String, String> _settings = <String, String>{
    'assistant.theme_mode': '"system"',
  };

  final List<CapabilityItem> _capabilities = <CapabilityItem>[
    const CapabilityItem(
      name: 'local_llm',
      tier: 'standard',
      isSupported: true,
      isActive: true,
      status: 'supported',
    ),
    const CapabilityItem(
      name: 'offline_speech',
      tier: 'standard',
      isSupported: true,
      isActive: true,
      status: 'supported',
    ),
    const CapabilityItem(
      name: 'background_wake_word',
      tier: 'standard',
      isSupported: true,
      isActive: true,
      status: 'supported',
    ),
    const CapabilityItem(
      name: 'file_system',
      tier: 'standard',
      isSupported: true,
      isActive: true,
      status: 'supported',
    ),
    const CapabilityItem(
      name: 'application_management',
      tier: 'standard',
      isSupported: true,
      isActive: true,
      status: 'supported',
    ),
    const CapabilityItem(
      name: 'system_media_control',
      tier: 'standard',
      isSupported: true,
      isActive: true,
      status: 'supported',
    ),
    const CapabilityItem(
      name: 'screen_streaming_host',
      tier: 'standard',
      isSupported: true,
      isActive: true,
      status: 'supported',
    ),
    const CapabilityItem(
      name: 'remote_input_injection_host',
      tier: 'standard',
      isSupported: true,
      isActive: true,
      status: 'supported',
    ),
    const CapabilityItem(
      name: 'lan_peer_networking',
      tier: 'standard',
      isSupported: true,
      isActive: true,
      status: 'supported',
    ),
  ];

  final List<AuditItem> _auditLog = <AuditItem>[
    const AuditItem(
      sequence: 1,
      timestampMs: 1731671000000,
      actor: 'local_user',
      skillId: 'system.time',
      payloadSummary: '{}',
      decision: 'allow',
      executionStatus: 'success',
      entryHash: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
      prevHash: '0000000000000000000000000000000000000000000000000000000000000000',
    ),
  ];

  @override
  Stream<BridgeEvent> get eventStream => _eventController.stream;

  @override
  Future<void> initialize({String? dataDir}) async {
    _lifecycleState = 'running';
  }

  @override
  Future<void> shutdown() async {
    _lifecycleState = 'terminated';
  }

  @override
  HealthReport getHealth() {
    return HealthReport(
      status: _status,
      lifecycleState: _lifecycleState,
      activeTasks: 0,
      databaseOk: true,
      auditOk: true,
      keystoreOk: true,
      totalCapabilities: _capabilities.length,
      activeCapabilities: _capabilities.where((c) => c.isActive).length,
      auditCount: _auditLog.length,
      details: const <String>[],
    );
  }

  @override
  List<CapabilityItem> getCapabilities() => List<CapabilityItem>.from(_capabilities);

  @override
  CapabilityItem updatePermission(String capability, bool granted) {
    final idx = _capabilities.indexWhere((c) => c.name == capability);
    if (idx != -1) {
      final old = _capabilities[idx];
      final updated = CapabilityItem(
        name: old.name,
        tier: old.tier,
        isSupported: old.isSupported,
        isActive: granted,
        status: granted ? 'supported' : 'degraded',
        degradationReason: granted ? null : 'Disabled by user policy',
      );
      _capabilities[idx] = updated;
      _eventController.add(CapabilityChangedEvent(
        capability: updated.name,
        status: updated.status,
        reason: updated.degradationReason,
      ));
      return updated;
    }
    throw ArgumentError('Unknown capability: $capability');
  }

  @override
  List<AuditItem> queryAuditLog({int limit = 20, int offset = 0}) {
    if (offset >= _auditLog.length) return const <AuditItem>[];
    return _auditLog.skip(offset).take(limit).toList();
  }

  @override
  AuditIntegrityReport verifyAudit() {
    return AuditIntegrityReport(
      isValid: true,
      verifiedCount: _auditLog.length,
      headHash: _auditLog.isNotEmpty ? _auditLog.last.entryHash : '',
    );
  }

  @override
  String? getSetting(String key) => _settings[key];

  @override
  void setSetting(String key, String valueJson) {
    _settings[key] = valueJson;
    _eventController.add(SettingChangedEvent(key: key, valuePreview: valueJson));
  }

  @override
  String getLifecycleState() => _lifecycleState;

  @override
  void requestShutdown(String reason) {
    _lifecycleState = 'shutting_down';
    _eventController.add(LifecycleStateChangedEvent(
      previous: 'running',
      current: 'shutting_down',
      reason: reason,
    ));
    _lifecycleState = 'terminated';
    _eventController.add(LifecycleStateChangedEvent(
      previous: 'shutting_down',
      current: 'terminated',
      reason: reason,
    ));
  }

  @override
  void notifyMemoryPressure(int level) {
    if (level >= 1) {
      _status = HealthStatus.degraded;
    }
    _eventController.add(MemoryPressureEvent(level: level >= 1 ? 'critical' : 'moderate'));
  }

  bool _isVoiceListening = false;
  VoiceMode _voiceMode = VoiceMode.pushToTalk;
  List<String> _voiceKeywords = <String>['hey assistant'];

  bool get isVoiceListening => _isVoiceListening;
  VoiceMode get voiceMode => _voiceMode;
  List<String> get voiceKeywords => List<String>.unmodifiable(_voiceKeywords);

  void emitEvent(BridgeEvent event) {
    _eventController.add(event);
  }

  @override
  void startVoiceListening({VoiceMode mode = VoiceMode.pushToTalk}) {
    _isVoiceListening = true;
    _voiceMode = mode;
    _eventController.add(VoiceStateChangedEvent(
      isListening: true,
      mode: mode == VoiceMode.pushToTalk ? 'PushToTalk' : 'WakeWord',
    ));
  }

  @override
  void stopVoiceListening() {
    _isVoiceListening = false;
    _eventController.add(VoiceStateChangedEvent(
      isListening: false,
      mode: _voiceMode == VoiceMode.pushToTalk ? 'PushToTalk' : 'WakeWord',
    ));
  }

  @override
  void setVoiceMode(VoiceMode mode) {
    _voiceMode = mode;
    _eventController.add(VoiceStateChangedEvent(
      isListening: _isVoiceListening,
      mode: mode == VoiceMode.pushToTalk ? 'PushToTalk' : 'WakeWord',
    ));
  }

  @override
  void setVoiceKeywords(List<String> keywords) {
    _voiceKeywords = List<String>.from(keywords);
  }

  @override
  void injectVoiceAudio(List<double> samples) {
    if (samples.isEmpty) return;
    var sumSq = 0.0;
    var peak = 0.0;
    for (final s in samples) {
      final abs = s.abs();
      if (abs > peak) peak = abs;
      sumSq += s * s;
    }
    final rms = sumSq / samples.length;
    final rmsDbfs = rms > 1e-9 ? 10.0 * (log(rms) / ln10) : -96.0;
    final peakDbfs = peak > 1e-9 ? 20.0 * (log(peak) / ln10) : -96.0;

    _eventController.add(VoiceLevelUpdatedEvent(
      rmsDbfs: rmsDbfs.clamp(-96.0, 0.0),
      peakDbfs: peakDbfs.clamp(-96.0, 0.0),
    ));
  }

  String? _activeModelId;
  final List<ModelItem> _models = <ModelItem>[
    const ModelItem(
      id: 'qwen2.5-0.5b-instruct-q4_k_m',
      name: 'Qwen 2.5 0.5B Instruct',
      tier: 'Nano0_5B',
      parameterCount: '0.49B',
      quantization: 'Q4_K_M',
      fileSizeBytes: 417333248,
      sha256: 'b14f828a2b53b8f2c6e7d6a5d9a8c7b6e5f4d3c2b1a09f8e7d6c5b4a39281701',
      defaultContextWindow: 4096,
      license: 'Apache-2.0',
      description: 'Ultra-fast, compact multilingual assistant model suited for phones and embedded devices.',
      isRecommended: true,
    ),
    const ModelItem(
      id: 'smollm2-360m-instruct-q4_k_m',
      name: 'SmolLM2 360M Instruct',
      tier: 'Nano0_5B',
      parameterCount: '0.36B',
      quantization: 'Q4_K_M',
      fileSizeBytes: 239075328,
      sha256: 'a25e939b3c64c9f3d7e8e7b6e0b9d8c7f6a5e4d3c2b1a09f8e7d6c5b4a392802',
      defaultContextWindow: 2048,
      license: 'Apache-2.0',
      description: 'Extremely lightweight model optimized for low-power offline edge voice devices.',
    ),
    const ModelItem(
      id: 'llama-3.2-3b-instruct-q4_k_m',
      name: 'Llama 3.2 3B Instruct',
      tier: 'Small1_5B_3B',
      parameterCount: '3.21B',
      quantization: 'Q4_K_M',
      fileSizeBytes: 2168995840,
      sha256: 'd47b151d5e86e1b5f90a09d8a2dbf0e9b8c7a6f5e4d3c2b1a09f8e7d6c5b4a04',
      defaultContextWindow: 4096,
      license: 'Llama-3.2-Community',
      description: 'State of the art 3B instruction tuned model with excellent reasoning and tool use.',
    ),
  ];

  @override
  ModelManagerSnapshot getModelSnapshot() {
    return ModelManagerSnapshot(
      hardware: const HardwareOverview(
        cpuArchitecture: 'x86_64',
        cpuCores: 8,
        simdFeatures: <String>['AVX2', 'FMA'],
        totalRamBytes: 16 * 1024 * 1024 * 1024,
        usableBudgetBytes: 10 * 1024 * 1024 * 1024,
        recommendedTier: 'Nano0_5B',
        recommendedContext: 4096,
        recommendedThreads: 6,
      ),
      activeModelId: _activeModelId,
      models: _models.map((m) => m.copyWith(isActive: m.id == _activeModelId)).toList(),
    );
  }

  @override
  void loadModel(String modelId) {
    _activeModelId = modelId;
    _eventController.add(ModelLoadedEvent(modelId: modelId));
  }

  @override
  void unloadModel() {
    if (_activeModelId != null) {
      final prev = _activeModelId!;
      _activeModelId = null;
      _eventController.add(ModelUnloadedEvent(modelId: prev));
    }
  }

  @override
  BenchmarkResult runModelBenchmark({int promptTokens = 32, int generateTokens = 16}) {
    final report = BenchmarkResult(
      promptTokens: promptTokens,
      generatedTokens: generateTokens,
      ttftMs: 42,
      totalElapsedMs: 380,
      tokensPerSecond: (generateTokens / 0.38),
      memoryUsedBytes: 420 * 1024 * 1024,
    );
    _eventController.add(BenchmarkCompletedEvent(
      metricsJson: json.encode(<String, dynamic>{
        'prompt_tokens': report.promptTokens,
        'generated_tokens': report.generatedTokens,
        'ttft_ms': report.ttftMs,
        'total_elapsed_ms': report.totalElapsedMs,
        'tokens_per_second': report.tokensPerSecond,
        'memory_used_bytes': report.memoryUsedBytes,
      }),
    ));
    return report;
  }
}
