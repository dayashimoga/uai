import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:math';

import 'package:ffi/ffi.dart';

import 'models/audit_item.dart';
import 'models/automation_models.dart';
import 'models/bridge_event.dart';
import 'models/capability_item.dart';
import 'models/device_models.dart';
import 'models/health_report.dart';
import 'models/interpreter_models.dart';
import 'models/knowledge_models.dart';
import 'models/model_item.dart';
import 'models/persona_models.dart';
import 'models/performance_models.dart';
import 'models/smart_home_models.dart';
import 'models/vision_models.dart';
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
  void notifyPowerTransition(String event);
  void startVoiceListening({VoiceMode mode = VoiceMode.pushToTalk});
  void stopVoiceListening();
  void setVoiceMode(VoiceMode mode);
  void setVoiceKeywords(List<String> keywords);
  void injectVoiceAudio(List<double> samples);
  ModelManagerSnapshot getModelSnapshot();
  void loadModel(String modelId);
  void unloadModel();
  void downloadModel(String modelId);
  void cancelDownload(String modelId);
  void deleteModel(String modelId);
  void importLocalGguf(String filePath);
  BenchmarkResult runModelBenchmark({int promptTokens = 32, int generateTokens = 16});
  List<DiscoveredPeerItem> getDiscoveredPeers();
  List<PairedDeviceItem> getPairedDevices();
  PairingSessionInfo startPairing(String peerInstance);
  PairedDeviceItem confirmPairing(String sasCode);
  void cancelPairing();
  void setPeerPermission(String deviceId, String capability, String level);
  void unpairDevice(String deviceId);
  void simulateDiscoveredPeer(DiscoveredPeerItem item);
  List<DocumentRecordItem> getKnowledgeDocuments();
  String indexKnowledgeDocument({
    required String path,
    required String fileName,
    required String mimeType,
    required String content,
  });
  bool deleteKnowledgeDocument(String documentId);
  List<KnowledgeCitationItem> searchKnowledge(String query, {int limit = 10});
  OcrDocumentItem extractOcr({required List<int> imageBytes});
  TranslationResultItem translateText({
    required String text,
    required String sourceLang,
    required String targetLang,
  });
  InterpreterSessionStatusItem startInterpreterSession({
    required String partyALang,
    required String partyBLang,
  });
  InterpreterTurnItem submitInterpreterTurn({required String text});
  List<InterpreterTurnItem> getInterpreterTurns();
  InterpreterSessionStatusItem swapInterpreterLanguages();
  void clearInterpreterSession();
  UserPersonaItem? getActivePersona();
  List<UserPersonaItem> listPersonas();
  void savePersona(UserPersonaItem persona);
  void setActivePersona(String id);
  bool deletePersona(String id);
  UserPreferenceItem? getPreference(String key);
  void setPreference(String key, String valueJson, String category);
  List<UserPreferenceItem> listPreferences({String? category});
  bool deletePreference(String key);
  List<AutomationRuleItem> listAutomations({bool enabledOnly = false});
  AutomationRuleItem? getAutomation(String id);
  void saveAutomation(AutomationRuleItem rule);
  bool toggleAutomation(String id, bool enabled);
  bool deleteAutomation(String id);
  WorkflowExecutionReportItem triggerAutomation(String id);
  List<AutomationHistoryItem> listAutomationHistory({String? automationId, int limit = 50});
  int clearAutomationHistory({String? automationId});
  List<SmartHomeEntityItem> listHomeEntities({String? domain});
  SmartHomeEntityItem? getHomeEntity(String id);
  void upsertHomeEntity(SmartHomeEntityItem item);
  bool deleteHomeEntity(String id);
  SmartHomeCommandResultItem controlHomeDevice(SmartHomeCommandItem command);
  void rotateMasterKey();
  void revokeDevice(String deviceId);
  void triggerEmergencyLockdown(String reason);
  void liftEmergencyLockdown();
  bool isLockdownActive();
  PerformanceSnapshotModel getPerformanceSnapshot();
  bool enforceMemoryBudget();
  PipelineBenchmarkReportModel runPipelineBenchmark();
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
typedef _BridgeNotifyPowerC = Int32 Function(Int32 transition);
typedef _BridgeNotifyPowerDart = int Function(int transition);

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

typedef _BridgeDevicePeersC = Pointer<Utf8> Function();
typedef _BridgeDevicePeersDart = Pointer<Utf8> Function();

typedef _BridgeDevicePairedC = Pointer<Utf8> Function();
typedef _BridgeDevicePairedDart = Pointer<Utf8> Function();

typedef _BridgeDeviceStartPairC = Pointer<Utf8> Function(Pointer<Utf8> peerInstance);
typedef _BridgeDeviceStartPairDart = Pointer<Utf8> Function(Pointer<Utf8> peerInstance);

typedef _BridgeDeviceConfirmPairC = Pointer<Utf8> Function(Pointer<Utf8> sasCode);
typedef _BridgeDeviceConfirmPairDart = Pointer<Utf8> Function(Pointer<Utf8> sasCode);

typedef _BridgeDeviceCancelPairC = Int32 Function();
typedef _BridgeDeviceCancelPairDart = int Function();

typedef _BridgeDeviceSetPermC = Int32 Function(
  Pointer<Utf8> deviceId,
  Pointer<Utf8> capability,
  Pointer<Utf8> level,
);
typedef _BridgeDeviceSetPermDart = int Function(
  Pointer<Utf8> deviceId,
  Pointer<Utf8> capability,
  Pointer<Utf8> level,
);

typedef _BridgeDeviceUnpairC = Int32 Function(Pointer<Utf8> deviceId);
typedef _BridgeDeviceUnpairDart = int Function(Pointer<Utf8> deviceId);

typedef _BridgeDeviceSimPeerC = Int32 Function(Pointer<Utf8> peerJson);
typedef _BridgeDeviceSimPeerDart = int Function(Pointer<Utf8> peerJson);

typedef _BridgeKnowledgeListC = Pointer<Utf8> Function();
typedef _BridgeKnowledgeListDart = Pointer<Utf8> Function();

typedef _BridgeKnowledgeIndexC = Pointer<Utf8> Function(
  Pointer<Utf8> path,
  Pointer<Utf8> fileName,
  Pointer<Utf8> mimeType,
  Pointer<Utf8> content,
);
typedef _BridgeKnowledgeIndexDart = Pointer<Utf8> Function(
  Pointer<Utf8> path,
  Pointer<Utf8> fileName,
  Pointer<Utf8> mimeType,
  Pointer<Utf8> content,
);

typedef _BridgeKnowledgeDeleteC = Int32 Function(Pointer<Utf8> docId);
typedef _BridgeKnowledgeDeleteDart = int Function(Pointer<Utf8> docId);

typedef _BridgeKnowledgeSearchC = Pointer<Utf8> Function(Pointer<Utf8> query, IntPtr limit);
typedef _BridgeKnowledgeSearchDart = Pointer<Utf8> Function(Pointer<Utf8> query, int limit);

typedef _BridgeVisionOcrC = Pointer<Utf8> Function(Pointer<Uint8> imagePtr, IntPtr imageLen);
typedef _BridgeVisionOcrDart = Pointer<Utf8> Function(Pointer<Uint8> imagePtr, int imageLen);

typedef _BridgeTranslateTextC = Pointer<Utf8> Function(
  Pointer<Utf8> text,
  Pointer<Utf8> sourceLang,
  Pointer<Utf8> targetLang,
);
typedef _BridgeTranslateTextDart = Pointer<Utf8> Function(
  Pointer<Utf8> text,
  Pointer<Utf8> sourceLang,
  Pointer<Utf8> targetLang,
);

typedef _BridgeInterpreterStartC = Pointer<Utf8> Function(
  Pointer<Utf8> partyALang,
  Pointer<Utf8> partyBLang,
);
typedef _BridgeInterpreterStartDart = Pointer<Utf8> Function(
  Pointer<Utf8> partyALang,
  Pointer<Utf8> partyBLang,
);

typedef _BridgeInterpreterSubmitTurnC = Pointer<Utf8> Function(Pointer<Utf8> text);
typedef _BridgeInterpreterSubmitTurnDart = Pointer<Utf8> Function(Pointer<Utf8> text);

typedef _BridgeInterpreterGetTurnsC = Pointer<Utf8> Function();
typedef _BridgeInterpreterGetTurnsDart = Pointer<Utf8> Function();

typedef _BridgeInterpreterSwapC = Pointer<Utf8> Function();
typedef _BridgeInterpreterSwapDart = Pointer<Utf8> Function();

typedef _BridgeInterpreterClearC = Int32 Function();
typedef _BridgeInterpreterClearDart = int Function();

typedef _BridgePersonaGetActiveC = Pointer<Utf8> Function();
typedef _BridgePersonaGetActiveDart = Pointer<Utf8> Function();

typedef _BridgePersonaListC = Pointer<Utf8> Function();
typedef _BridgePersonaListDart = Pointer<Utf8> Function();

typedef _BridgePersonaSaveC = Pointer<Utf8> Function(Pointer<Utf8> personaJson);
typedef _BridgePersonaSaveDart = Pointer<Utf8> Function(Pointer<Utf8> personaJson);

typedef _BridgePersonaSetActiveC = Pointer<Utf8> Function(Pointer<Utf8> id);
typedef _BridgePersonaSetActiveDart = Pointer<Utf8> Function(Pointer<Utf8> id);

typedef _BridgePersonaDeleteC = Pointer<Utf8> Function(Pointer<Utf8> id);
typedef _BridgePersonaDeleteDart = Pointer<Utf8> Function(Pointer<Utf8> id);

typedef _BridgePreferenceGetC = Pointer<Utf8> Function(Pointer<Utf8> key);
typedef _BridgePreferenceGetDart = Pointer<Utf8> Function(Pointer<Utf8> key);

typedef _BridgePreferenceSetC = Pointer<Utf8> Function(
  Pointer<Utf8> key,
  Pointer<Utf8> valueJson,
  Pointer<Utf8> category,
);
typedef _BridgePreferenceSetDart = Pointer<Utf8> Function(
  Pointer<Utf8> key,
  Pointer<Utf8> valueJson,
  Pointer<Utf8> category,
);

typedef _BridgePreferenceListC = Pointer<Utf8> Function(Pointer<Utf8> category);
typedef _BridgePreferenceListDart = Pointer<Utf8> Function(Pointer<Utf8> category);

typedef _BridgePreferenceDeleteC = Pointer<Utf8> Function(Pointer<Utf8> key);
typedef _BridgePreferenceDeleteDart = Pointer<Utf8> Function(Pointer<Utf8> key);

typedef _BridgeAutomationListC = Pointer<Utf8> Function(Int32 enabledOnly);
typedef _BridgeAutomationListDart = Pointer<Utf8> Function(int enabledOnly);

typedef _BridgeAutomationGetC = Pointer<Utf8> Function(Pointer<Utf8> id);
typedef _BridgeAutomationGetDart = Pointer<Utf8> Function(Pointer<Utf8> id);

typedef _BridgeAutomationSaveC = Pointer<Utf8> Function(Pointer<Utf8> ruleJson);
typedef _BridgeAutomationSaveDart = Pointer<Utf8> Function(Pointer<Utf8> ruleJson);

typedef _BridgeAutomationToggleC = Pointer<Utf8> Function(Pointer<Utf8> id, Int32 enabled);
typedef _BridgeAutomationToggleDart = Pointer<Utf8> Function(Pointer<Utf8> id, int enabled);

typedef _BridgeAutomationDeleteC = Pointer<Utf8> Function(Pointer<Utf8> id);
typedef _BridgeAutomationDeleteDart = Pointer<Utf8> Function(Pointer<Utf8> id);

typedef _BridgeAutomationTriggerC = Pointer<Utf8> Function(Pointer<Utf8> id);
typedef _BridgeAutomationTriggerDart = Pointer<Utf8> Function(Pointer<Utf8> id);

typedef _BridgeAutomationHistoryC = Pointer<Utf8> Function(Pointer<Utf8> automationId, IntPtr limit);
typedef _BridgeAutomationHistoryDart = Pointer<Utf8> Function(Pointer<Utf8> automationId, int limit);

typedef _BridgeAutomationClearHistC = Pointer<Utf8> Function(Pointer<Utf8> automationId);
typedef _BridgeAutomationClearHistDart = Pointer<Utf8> Function(Pointer<Utf8> automationId);

typedef _BridgeHomeListC = Pointer<Utf8> Function(Pointer<Utf8> domain);
typedef _BridgeHomeListDart = Pointer<Utf8> Function(Pointer<Utf8> domain);

typedef _BridgeHomeGetC = Pointer<Utf8> Function(Pointer<Utf8> id);
typedef _BridgeHomeGetDart = Pointer<Utf8> Function(Pointer<Utf8> id);

typedef _BridgeHomeUpsertC = Pointer<Utf8> Function(Pointer<Utf8> itemJson);
typedef _BridgeHomeUpsertDart = Pointer<Utf8> Function(Pointer<Utf8> itemJson);

typedef _BridgeHomeDeleteC = Pointer<Utf8> Function(Pointer<Utf8> id);
typedef _BridgeHomeDeleteDart = Pointer<Utf8> Function(Pointer<Utf8> id);

typedef _BridgeHomeControlC = Pointer<Utf8> Function(Pointer<Utf8> cmdJson);
typedef _BridgeHomeControlDart = Pointer<Utf8> Function(Pointer<Utf8> cmdJson);

typedef _BridgeRotateMasterKeyC = Pointer<Utf8> Function();
typedef _BridgeRotateMasterKeyDart = Pointer<Utf8> Function();

typedef _BridgeDeviceRevokeC = Pointer<Utf8> Function(Pointer<Utf8> deviceId);
typedef _BridgeDeviceRevokeDart = Pointer<Utf8> Function(Pointer<Utf8> deviceId);

typedef _BridgeEmergencyLockdownC = Pointer<Utf8> Function(Pointer<Utf8> reason);
typedef _BridgeEmergencyLockdownDart = Pointer<Utf8> Function(Pointer<Utf8> reason);

typedef _BridgeLiftEmergencyLockdownC = Pointer<Utf8> Function();
typedef _BridgeLiftEmergencyLockdownDart = Pointer<Utf8> Function();

typedef _BridgeIsLockdownActiveC = Pointer<Utf8> Function();
typedef _BridgeIsLockdownActiveDart = Pointer<Utf8> Function();

typedef _BridgeGetPerfSnapshotC = Pointer<Utf8> Function();
typedef _BridgeGetPerfSnapshotDart = Pointer<Utf8> Function();

typedef _BridgeEnforceMemBudgetC = Pointer<Utf8> Function();
typedef _BridgeEnforceMemBudgetDart = Pointer<Utf8> Function();

typedef _BridgeRunPipelineBenchC = Pointer<Utf8> Function();
typedef _BridgeRunPipelineBenchDart = Pointer<Utf8> Function();

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
  late final _BridgeNotifyPowerDart _notifyPowerTransition;
  late final _BridgeVoiceStartDart _voiceStart;
  late final _BridgeVoiceStopDart _voiceStop;
  late final _BridgeVoiceSetModeDart _voiceSetMode;
  late final _BridgeVoiceSetKeywordsDart _voiceSetKeywords;
  late final _BridgeVoiceInjectDart _voiceInject;
  late final _BridgeModelSnapDart _modelSnapshot;
  late final _BridgeModelLoadDart _modelLoad;
  late final _BridgeModelUnloadDart _modelUnload;
  late final _BridgeModelLoadDart _modelDownload;
  late final _BridgeModelLoadDart _modelCancelDownload;
  late final _BridgeModelLoadDart _modelDelete;
  late final _BridgeModelLoadDart _modelImport;
  late final _BridgeModelBenchDart _modelBenchmarkRun;
  late final _BridgeDevicePeersDart _deviceDiscoveredPeers;
  late final _BridgeDevicePairedDart _devicePairedList;
  late final _BridgeDeviceStartPairDart _deviceStartPairing;
  late final _BridgeDeviceConfirmPairDart _deviceConfirmPairing;
  late final _BridgeDeviceCancelPairDart _deviceCancelPairing;
  late final _BridgeDeviceSetPermDart _deviceSetPermission;
  late final _BridgeDeviceUnpairDart _deviceUnpair;
  late final _BridgeDeviceSimPeerDart _deviceSimulatePeer;
  late final _BridgeKnowledgeListDart _knowledgeList;
  late final _BridgeKnowledgeIndexDart _knowledgeIndex;
  late final _BridgeKnowledgeDeleteDart _knowledgeDelete;
  late final _BridgeKnowledgeSearchDart _knowledgeSearch;
  late final _BridgeVisionOcrDart _visionOcrExtract;
  late final _BridgeTranslateTextDart _translateText;
  late final _BridgeInterpreterStartDart _interpreterStart;
  late final _BridgeInterpreterSubmitTurnDart _interpreterSubmitTurn;
  late final _BridgeInterpreterGetTurnsDart _interpreterGetTurns;
  late final _BridgeInterpreterSwapDart _interpreterSwap;
  late final _BridgeInterpreterClearDart _interpreterClear;
  late final _BridgePersonaGetActiveDart _personaGetActive;
  late final _BridgePersonaListDart _personaList;
  late final _BridgePersonaSaveDart _personaSave;
  late final _BridgePersonaSetActiveDart _personaSetActive;
  late final _BridgePersonaDeleteDart _personaDelete;
  late final _BridgePreferenceGetDart _preferenceGet;
  late final _BridgePreferenceSetDart _preferenceSet;
  late final _BridgePreferenceListDart _preferenceList;
  late final _BridgePreferenceDeleteDart _preferenceDelete;
  late final _BridgeAutomationListDart _automationList;
  late final _BridgeAutomationGetDart _automationGet;
  late final _BridgeAutomationSaveDart _automationSave;
  late final _BridgeAutomationToggleDart _automationToggle;
  late final _BridgeAutomationDeleteDart _automationDelete;
  late final _BridgeAutomationTriggerDart _automationTrigger;
  late final _BridgeAutomationHistoryDart _automationHistory;
  late final _BridgeAutomationClearHistDart _automationClearHistory;
  late final _BridgeHomeListDart _homeList;
  late final _BridgeHomeGetDart _homeGet;
  late final _BridgeHomeUpsertDart _homeUpsert;
  late final _BridgeHomeDeleteDart _homeDelete;
  late final _BridgeHomeControlDart _homeControl;
  late final _BridgeRotateMasterKeyDart _rotateMasterKey;
  late final _BridgeDeviceRevokeDart _deviceRevoke;
  late final _BridgeEmergencyLockdownDart _emergencyLockdown;
  late final _BridgeLiftEmergencyLockdownDart _liftEmergencyLockdown;
  late final _BridgeIsLockdownActiveDart _isLockdownActive;
  late final _BridgeGetPerfSnapshotDart _getPerformanceSnapshot;
  late final _BridgeEnforceMemBudgetDart _enforceMemoryBudget;
  late final _BridgeRunPipelineBenchDart _runPipelineBenchmark;
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
    _notifyPowerTransition = _lib.lookupFunction<_BridgeNotifyPowerC, _BridgeNotifyPowerDart>('bridge_c_notify_power_transition');
    _voiceStart = _lib.lookupFunction<_BridgeVoiceStartC, _BridgeVoiceStartDart>('bridge_c_voice_start_listening');
    _voiceStop = _lib.lookupFunction<_BridgeVoiceStopC, _BridgeVoiceStopDart>('bridge_c_voice_stop_listening');
    _voiceSetMode = _lib.lookupFunction<_BridgeVoiceSetModeC, _BridgeVoiceSetModeDart>('bridge_c_voice_set_mode');
    _voiceSetKeywords = _lib.lookupFunction<_BridgeVoiceSetKeywordsC, _BridgeVoiceSetKeywordsDart>('bridge_c_voice_set_keywords');
    _voiceInject = _lib.lookupFunction<_BridgeVoiceInjectC, _BridgeVoiceInjectDart>('bridge_c_voice_inject_audio');
    _modelSnapshot = _lib.lookupFunction<_BridgeModelSnapC, _BridgeModelSnapDart>('bridge_c_model_snapshot');
    _modelLoad = _lib.lookupFunction<_BridgeModelLoadC, _BridgeModelLoadDart>('bridge_c_model_load');
    _modelUnload = _lib.lookupFunction<_BridgeModelUnloadC, _BridgeModelUnloadDart>('bridge_c_model_unload');
    _modelDownload = _lib.lookupFunction<_BridgeModelLoadC, _BridgeModelLoadDart>('bridge_c_model_download');
    _modelCancelDownload = _lib.lookupFunction<_BridgeModelLoadC, _BridgeModelLoadDart>('bridge_c_model_cancel_download');
    _modelDelete = _lib.lookupFunction<_BridgeModelLoadC, _BridgeModelLoadDart>('bridge_c_model_delete');
    _modelImport = _lib.lookupFunction<_BridgeModelLoadC, _BridgeModelLoadDart>('bridge_c_model_import_local_gguf');
    _modelBenchmarkRun = _lib.lookupFunction<_BridgeModelBenchC, _BridgeModelBenchDart>('bridge_c_model_benchmark_run');
    _deviceDiscoveredPeers = _lib.lookupFunction<_BridgeDevicePeersC, _BridgeDevicePeersDart>('bridge_c_device_discovered_peers');
    _devicePairedList = _lib.lookupFunction<_BridgeDevicePairedC, _BridgeDevicePairedDart>('bridge_c_device_paired_list');
    _deviceStartPairing = _lib.lookupFunction<_BridgeDeviceStartPairC, _BridgeDeviceStartPairDart>('bridge_c_device_start_pairing');
    _deviceConfirmPairing = _lib.lookupFunction<_BridgeDeviceConfirmPairC, _BridgeDeviceConfirmPairDart>('bridge_c_device_confirm_pairing');
    _deviceCancelPairing = _lib.lookupFunction<_BridgeDeviceCancelPairC, _BridgeDeviceCancelPairDart>('bridge_c_device_cancel_pairing');
    _deviceSetPermission = _lib.lookupFunction<_BridgeDeviceSetPermC, _BridgeDeviceSetPermDart>('bridge_c_device_set_permission');
    _deviceUnpair = _lib.lookupFunction<_BridgeDeviceUnpairC, _BridgeDeviceUnpairDart>('bridge_c_device_unpair');
    _deviceSimulatePeer = _lib.lookupFunction<_BridgeDeviceSimPeerC, _BridgeDeviceSimPeerDart>('bridge_c_device_simulate_peer');
    _knowledgeList = _lib.lookupFunction<_BridgeKnowledgeListC, _BridgeKnowledgeListDart>('bridge_c_knowledge_list');
    _knowledgeIndex = _lib.lookupFunction<_BridgeKnowledgeIndexC, _BridgeKnowledgeIndexDart>('bridge_c_knowledge_index');
    _knowledgeDelete = _lib.lookupFunction<_BridgeKnowledgeDeleteC, _BridgeKnowledgeDeleteDart>('bridge_c_knowledge_delete');
    _knowledgeSearch = _lib.lookupFunction<_BridgeKnowledgeSearchC, _BridgeKnowledgeSearchDart>('bridge_c_knowledge_search');
    _visionOcrExtract = _lib.lookupFunction<_BridgeVisionOcrC, _BridgeVisionOcrDart>('bridge_c_vision_ocr_extract');
    _translateText = _lib.lookupFunction<_BridgeTranslateTextC, _BridgeTranslateTextDart>('bridge_c_translate_text');
    _interpreterStart = _lib.lookupFunction<_BridgeInterpreterStartC, _BridgeInterpreterStartDart>('bridge_c_interpreter_start');
    _interpreterSubmitTurn = _lib.lookupFunction<_BridgeInterpreterSubmitTurnC, _BridgeInterpreterSubmitTurnDart>('bridge_c_interpreter_submit_turn');
    _interpreterGetTurns = _lib.lookupFunction<_BridgeInterpreterGetTurnsC, _BridgeInterpreterGetTurnsDart>('bridge_c_interpreter_get_turns');
    _interpreterSwap = _lib.lookupFunction<_BridgeInterpreterSwapC, _BridgeInterpreterSwapDart>('bridge_c_interpreter_swap_languages');
    _interpreterClear = _lib.lookupFunction<_BridgeInterpreterClearC, _BridgeInterpreterClearDart>('bridge_c_interpreter_clear');
    _personaGetActive = _lib.lookupFunction<_BridgePersonaGetActiveC, _BridgePersonaGetActiveDart>('bridge_c_persona_get_active');
    _personaList = _lib.lookupFunction<_BridgePersonaListC, _BridgePersonaListDart>('bridge_c_persona_list');
    _personaSave = _lib.lookupFunction<_BridgePersonaSaveC, _BridgePersonaSaveDart>('bridge_c_persona_save');
    _personaSetActive = _lib.lookupFunction<_BridgePersonaSetActiveC, _BridgePersonaSetActiveDart>('bridge_c_persona_set_active');
    _personaDelete = _lib.lookupFunction<_BridgePersonaDeleteC, _BridgePersonaDeleteDart>('bridge_c_persona_delete');
    _preferenceGet = _lib.lookupFunction<_BridgePreferenceGetC, _BridgePreferenceGetDart>('bridge_c_preference_get');
    _preferenceSet = _lib.lookupFunction<_BridgePreferenceSetC, _BridgePreferenceSetDart>('bridge_c_preference_set');
    _preferenceList = _lib.lookupFunction<_BridgePreferenceListC, _BridgePreferenceListDart>('bridge_c_preference_list');
    _preferenceDelete = _lib.lookupFunction<_BridgePreferenceDeleteC, _BridgePreferenceDeleteDart>('bridge_c_preference_delete');
    _automationList = _lib.lookupFunction<_BridgeAutomationListC, _BridgeAutomationListDart>('bridge_c_automation_list');
    _automationGet = _lib.lookupFunction<_BridgeAutomationGetC, _BridgeAutomationGetDart>('bridge_c_automation_get');
    _automationSave = _lib.lookupFunction<_BridgeAutomationSaveC, _BridgeAutomationSaveDart>('bridge_c_automation_save');
    _automationToggle = _lib.lookupFunction<_BridgeAutomationToggleC, _BridgeAutomationToggleDart>('bridge_c_automation_toggle');
    _automationDelete = _lib.lookupFunction<_BridgeAutomationDeleteC, _BridgeAutomationDeleteDart>('bridge_c_automation_delete');
    _automationTrigger = _lib.lookupFunction<_BridgeAutomationTriggerC, _BridgeAutomationTriggerDart>('bridge_c_automation_trigger');
    _automationHistory = _lib.lookupFunction<_BridgeAutomationHistoryC, _BridgeAutomationHistoryDart>('bridge_c_automation_history');
    _automationClearHistory = _lib.lookupFunction<_BridgeAutomationClearHistC, _BridgeAutomationClearHistDart>('bridge_c_automation_clear_history');
    _homeList = _lib.lookupFunction<_BridgeHomeListC, _BridgeHomeListDart>('bridge_c_home_entity_list');
    _homeGet = _lib.lookupFunction<_BridgeHomeGetC, _BridgeHomeGetDart>('bridge_c_home_entity_get');
    _homeUpsert = _lib.lookupFunction<_BridgeHomeUpsertC, _BridgeHomeUpsertDart>('bridge_c_home_entity_upsert');
    _homeDelete = _lib.lookupFunction<_BridgeHomeDeleteC, _BridgeHomeDeleteDart>('bridge_c_home_entity_delete');
    _homeControl = _lib.lookupFunction<_BridgeHomeControlC, _BridgeHomeControlDart>('bridge_c_home_entity_control');
    _rotateMasterKey = _lib.lookupFunction<_BridgeRotateMasterKeyC, _BridgeRotateMasterKeyDart>('bridge_c_rotate_master_key');
    _deviceRevoke = _lib.lookupFunction<_BridgeDeviceRevokeC, _BridgeDeviceRevokeDart>('bridge_c_device_revoke');
    _emergencyLockdown = _lib.lookupFunction<_BridgeEmergencyLockdownC, _BridgeEmergencyLockdownDart>('bridge_c_emergency_lockdown');
    _liftEmergencyLockdown = _lib.lookupFunction<_BridgeLiftEmergencyLockdownC, _BridgeLiftEmergencyLockdownDart>('bridge_c_lift_emergency_lockdown');
    _isLockdownActive = _lib.lookupFunction<_BridgeIsLockdownActiveC, _BridgeIsLockdownActiveDart>('bridge_c_is_lockdown_active');
    _getPerformanceSnapshot = _lib.lookupFunction<_BridgeGetPerfSnapshotC, _BridgeGetPerfSnapshotDart>('bridge_c_get_performance_snapshot');
    _enforceMemoryBudget = _lib.lookupFunction<_BridgeEnforceMemBudgetC, _BridgeEnforceMemBudgetDart>('bridge_c_enforce_memory_budget');
    _runPipelineBenchmark = _lib.lookupFunction<_BridgeRunPipelineBenchC, _BridgeRunPipelineBenchDart>('bridge_c_run_pipeline_benchmark');
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
  void notifyPowerTransition(String event) {
    final code = event.toLowerCase() == 'suspend' ? 0 : 1;
    _notifyPowerTransition(code);
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
  void downloadModel(String modelId) {
    final ptr = modelId.toNativeUtf8();
    try {
      final res = _modelDownload(ptr);
      if (res != 0) {
        throw StateError('Failed to start model download $modelId: code $res');
      }
    } finally {
      malloc.free(ptr);
    }
  }

  @override
  void cancelDownload(String modelId) {
    final ptr = modelId.toNativeUtf8();
    try {
      final res = _modelCancelDownload(ptr);
      if (res != 0) {
        throw StateError('Failed to cancel model download $modelId: code $res');
      }
    } finally {
      malloc.free(ptr);
    }
  }

  @override
  void deleteModel(String modelId) {
    final ptr = modelId.toNativeUtf8();
    try {
      final res = _modelDelete(ptr);
      if (res != 0) {
        throw StateError('Failed to delete model $modelId: code $res');
      }
    } finally {
      malloc.free(ptr);
    }
  }

  @override
  void importLocalGguf(String filePath) {
    final ptr = filePath.toNativeUtf8();
    try {
      final res = _modelImport(ptr);
      if (res != 0) {
        throw StateError('Failed to import local GGUF $filePath: code $res');
      }
    } finally {
      malloc.free(ptr);
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

  @override
  List<DiscoveredPeerItem> getDiscoveredPeers() {
    final jsonStr = _consumeAndFree(_deviceDiscoveredPeers());
    final decoded = jsonDecode(jsonStr) as List<dynamic>;
    return decoded
        .map((e) => DiscoveredPeerItem.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  List<PairedDeviceItem> getPairedDevices() {
    final jsonStr = _consumeAndFree(_devicePairedList());
    final decoded = jsonDecode(jsonStr) as List<dynamic>;
    return decoded
        .map((e) => PairedDeviceItem.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  PairingSessionInfo startPairing(String peerInstance) {
    final ptr = peerInstance.toNativeUtf8();
    try {
      final jsonStr = _consumeAndFree(_deviceStartPairing(ptr));
      final decoded = jsonDecode(jsonStr) as Map<String, dynamic>;
      return PairingSessionInfo.fromJson(decoded);
    } finally {
      malloc.free(ptr);
    }
  }

  @override
  PairedDeviceItem confirmPairing(String sasCode) {
    final ptr = sasCode.toNativeUtf8();
    try {
      final jsonStr = _consumeAndFree(_deviceConfirmPairing(ptr));
      final decoded = jsonDecode(jsonStr) as Map<String, dynamic>;
      return PairedDeviceItem.fromJson(decoded);
    } finally {
      malloc.free(ptr);
    }
  }

  @override
  void cancelPairing() {
    _deviceCancelPairing();
  }

  @override
  void setPeerPermission(String deviceId, String capability, String level) {
    final idPtr = deviceId.toNativeUtf8();
    final capPtr = capability.toNativeUtf8();
    final lvlPtr = level.toNativeUtf8();
    try {
      final res = _deviceSetPermission(idPtr, capPtr, lvlPtr);
      if (res != 0) {
        throw StateError('Failed to set peer permission for $deviceId');
      }
    } finally {
      malloc.free(idPtr);
      malloc.free(capPtr);
      malloc.free(lvlPtr);
    }
  }

  @override
  void unpairDevice(String deviceId) {
    final idPtr = deviceId.toNativeUtf8();
    try {
      final res = _deviceUnpair(idPtr);
      if (res != 0) {
        throw StateError('Failed to unpair device $deviceId');
      }
    } finally {
      malloc.free(idPtr);
    }
  }

  @override
  void simulateDiscoveredPeer(DiscoveredPeerItem item) {
    final jsonStr = jsonEncode(item.toJson());
    final ptr = jsonStr.toNativeUtf8();
    try {
      final res = _deviceSimulatePeer(ptr);
      if (res != 0) {
        throw StateError('Failed to simulate discovered peer');
      }
    } finally {
      malloc.free(ptr);
    }
  }

  @override
  List<DocumentRecordItem> getKnowledgeDocuments() {
    final ptr = _knowledgeList();
    if (ptr == nullptr) return <DocumentRecordItem>[];
    try {
      final jsonStr = ptr.toDartString();
      final list = jsonDecode(jsonStr) as List<dynamic>? ?? <dynamic>[];
      return list
          .map((item) => DocumentRecordItem.fromJson(item as Map<String, dynamic>))
          .toList();
    } finally {
      _freeString(ptr);
    }
  }

  @override
  String indexKnowledgeDocument({
    required String path,
    required String fileName,
    required String mimeType,
    required String content,
  }) {
    final pathPtr = path.toNativeUtf8();
    final namePtr = fileName.toNativeUtf8();
    final mimePtr = mimeType.toNativeUtf8();
    final contentPtr = content.toNativeUtf8();
    try {
      final resPtr = _knowledgeIndex(pathPtr, namePtr, mimePtr, contentPtr);
      if (resPtr == nullptr) {
        throw StateError('Failed to index document: returned null');
      }
      try {
        final jsonStr = resPtr.toDartString();
        return jsonDecode(jsonStr) as String? ?? '';
      } finally {
        _freeString(resPtr);
      }
    } finally {
      malloc.free(pathPtr);
      malloc.free(namePtr);
      malloc.free(mimePtr);
      malloc.free(contentPtr);
    }
  }

  @override
  bool deleteKnowledgeDocument(String documentId) {
    final idPtr = documentId.toNativeUtf8();
    try {
      final res = _knowledgeDelete(idPtr);
      return res == 0;
    } finally {
      malloc.free(idPtr);
    }
  }

  @override
  List<KnowledgeCitationItem> searchKnowledge(String query, {int limit = 10}) {
    final queryPtr = query.toNativeUtf8();
    try {
      final ptr = _knowledgeSearch(queryPtr, limit);
      if (ptr == nullptr) return <KnowledgeCitationItem>[];
      try {
        final jsonStr = ptr.toDartString();
        final list = jsonDecode(jsonStr) as List<dynamic>? ?? <dynamic>[];
        return list
            .map((item) => KnowledgeCitationItem.fromJson(item as Map<String, dynamic>))
            .toList();
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(queryPtr);
    }
  }

  @override
  OcrDocumentItem extractOcr({required List<int> imageBytes}) {
    final nativeBytes = malloc<Uint8>(imageBytes.length);
    final byteList = nativeBytes.asTypedList(imageBytes.length);
    byteList.setAll(0, imageBytes);
    try {
      final ptr = _visionOcrExtract(nativeBytes, imageBytes.length);
      if (ptr == nullptr) {
        throw StateError('OCR extraction returned null pointer');
      }
      try {
        final jsonStr = ptr.toDartString();
        final map = jsonDecode(jsonStr) as Map<String, dynamic>;
        if (map.containsKey('code') || map.containsKey('error')) {
          final msg = map['message'] ?? map['error'] ?? 'Unknown OCR error';
          throw StateError('OCR extraction error: $msg');
        }
        return OcrDocumentItem.fromJson(map);
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(nativeBytes);
    }
  }

  @override
  TranslationResultItem translateText({
    required String text,
    required String sourceLang,
    required String targetLang,
  }) {
    final textPtr = text.toNativeUtf8();
    final srcPtr = sourceLang.toNativeUtf8();
    final tgtPtr = targetLang.toNativeUtf8();
    try {
      final ptr = _translateText(textPtr, srcPtr, tgtPtr);
      if (ptr == nullptr) {
        throw StateError('Translation returned null pointer');
      }
      try {
        final jsonStr = ptr.toDartString();
        final map = jsonDecode(jsonStr) as Map<String, dynamic>;
        if (map.containsKey('code') || map.containsKey('error')) {
          final msg = map['message'] ?? map['error'] ?? 'Translation error';
          throw StateError('Translation failed: $msg');
        }
        return TranslationResultItem.fromJson(map);
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(textPtr);
      malloc.free(srcPtr);
      malloc.free(tgtPtr);
    }
  }

  @override
  InterpreterSessionStatusItem startInterpreterSession({
    required String partyALang,
    required String partyBLang,
  }) {
    final aPtr = partyALang.toNativeUtf8();
    final bPtr = partyBLang.toNativeUtf8();
    try {
      final ptr = _interpreterStart(aPtr, bPtr);
      if (ptr == nullptr) {
        throw StateError('Interpreter start returned null pointer');
      }
      try {
        final jsonStr = ptr.toDartString();
        final map = jsonDecode(jsonStr) as Map<String, dynamic>;
        if (map.containsKey('code') || map.containsKey('error')) {
          final msg = map['message'] ?? map['error'] ?? 'Interpreter start error';
          throw StateError('Interpreter start failed: $msg');
        }
        return InterpreterSessionStatusItem.fromJson(map);
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(aPtr);
      malloc.free(bPtr);
    }
  }

  @override
  InterpreterTurnItem submitInterpreterTurn({required String text}) {
    final textPtr = text.toNativeUtf8();
    try {
      final ptr = _interpreterSubmitTurn(textPtr);
      if (ptr == nullptr) {
        throw StateError('Interpreter turn returned null pointer');
      }
      try {
        final jsonStr = ptr.toDartString();
        final map = jsonDecode(jsonStr) as Map<String, dynamic>;
        if (map.containsKey('code') || map.containsKey('error')) {
          final msg = map['message'] ?? map['error'] ?? 'Interpreter turn error';
          throw StateError('Interpreter turn failed: $msg');
        }
        return InterpreterTurnItem.fromJson(map);
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(textPtr);
    }
  }

  @override
  List<InterpreterTurnItem> getInterpreterTurns() {
    final ptr = _interpreterGetTurns();
    if (ptr == nullptr) return <InterpreterTurnItem>[];
    try {
      final jsonStr = ptr.toDartString();
      final list = jsonDecode(jsonStr) as List<dynamic>? ?? <dynamic>[];
      return list
          .map((item) => InterpreterTurnItem.fromJson(item as Map<String, dynamic>))
          .toList();
    } finally {
      _freeString(ptr);
    }
  }

  @override
  InterpreterSessionStatusItem swapInterpreterLanguages() {
    final ptr = _interpreterSwap();
    if (ptr == nullptr) {
      throw StateError('Interpreter swap returned null pointer');
    }
    try {
      final jsonStr = ptr.toDartString();
      final map = jsonDecode(jsonStr) as Map<String, dynamic>;
      if (map.containsKey('code') || map.containsKey('error')) {
        final msg = map['message'] ?? map['error'] ?? 'Interpreter swap error';
        throw StateError('Interpreter swap failed: $msg');
      }
      return InterpreterSessionStatusItem.fromJson(map);
    } finally {
      _freeString(ptr);
    }
  }

  @override
  void clearInterpreterSession() {
    final res = _interpreterClear();
    if (res != 0) {
      throw StateError('Failed to clear interpreter session');
    }
  }

  @override
  UserPersonaItem? getActivePersona() {
    final ptr = _personaGetActive();
    if (ptr.address == 0) return null;
    try {
      final jsonStr = ptr.toDartString();
      if (jsonStr == 'null') return null;
      final map = jsonDecode(jsonStr);
      if (map is Map<String, dynamic>) {
        if (map.containsKey('code') || map.containsKey('error')) return null;
        return UserPersonaItem.fromJson(map);
      }
      return null;
    } finally {
      _freeString(ptr);
    }
  }

  @override
  List<UserPersonaItem> listPersonas() {
    final ptr = _personaList();
    if (ptr.address == 0) return [];
    try {
      final jsonStr = ptr.toDartString();
      final decoded = jsonDecode(jsonStr);
      if (decoded is List) {
        return decoded
            .map((e) => UserPersonaItem.fromJson(e as Map<String, dynamic>))
            .toList();
      }
      return [];
    } finally {
      _freeString(ptr);
    }
  }

  @override
  void savePersona(UserPersonaItem persona) {
    final jsonStr = jsonEncode(persona.toJson());
    final jsonPtr = jsonStr.toNativeUtf8();
    try {
      final ptr = _personaSave(jsonPtr);
      if (ptr.address != 0) {
        try {
          final res = ptr.toDartString();
          if (res != 'null') {
            final map = jsonDecode(res);
            if (map is Map<String, dynamic> &&
                (map.containsKey('code') || map.containsKey('error'))) {
              throw StateError(
                (map['message'] ?? 'Failed to save persona').toString(),
              );
            }
          }
        } finally {
          _freeString(ptr);
        }
      }
    } finally {
      malloc.free(jsonPtr);
    }
  }

  @override
  void setActivePersona(String id) {
    final idPtr = id.toNativeUtf8();
    try {
      final ptr = _personaSetActive(idPtr);
      if (ptr.address != 0) {
        try {
          final res = ptr.toDartString();
          if (res != 'null') {
            final map = jsonDecode(res);
            if (map is Map<String, dynamic> &&
                (map.containsKey('code') || map.containsKey('error'))) {
              throw StateError(
                (map['message'] ?? 'Failed to activate persona').toString(),
              );
            }
          }
        } finally {
          _freeString(ptr);
        }
      }
    } finally {
      malloc.free(idPtr);
    }
  }

  @override
  bool deletePersona(String id) {
    final idPtr = id.toNativeUtf8();
    try {
      final ptr = _personaDelete(idPtr);
      if (ptr.address == 0) return false;
      try {
        final res = ptr.toDartString();
        final decoded = jsonDecode(res);
        if (decoded is bool) return decoded;
        if (decoded is Map<String, dynamic> &&
            (decoded.containsKey('code') || decoded.containsKey('error'))) {
          throw StateError(
            (decoded['message'] ?? 'Failed to delete persona').toString(),
          );
        }
        return false;
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(idPtr);
    }
  }

  @override
  UserPreferenceItem? getPreference(String key) {
    final keyPtr = key.toNativeUtf8();
    try {
      final ptr = _preferenceGet(keyPtr);
      if (ptr.address == 0) return null;
      try {
        final res = ptr.toDartString();
        if (res == 'null') return null;
        final decoded = jsonDecode(res);
        if (decoded is Map<String, dynamic>) {
          if (decoded.containsKey('code') || decoded.containsKey('error')) return null;
          return UserPreferenceItem.fromJson(decoded);
        }
        return null;
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(keyPtr);
    }
  }

  @override
  void setPreference(String key, String valueJson, String category) {
    final keyPtr = key.toNativeUtf8();
    final valPtr = valueJson.toNativeUtf8();
    final catPtr = category.toNativeUtf8();
    try {
      final ptr = _preferenceSet(keyPtr, valPtr, catPtr);
      if (ptr.address != 0) {
        try {
          final res = ptr.toDartString();
          if (res != 'null') {
            final map = jsonDecode(res);
            if (map is Map<String, dynamic> &&
                (map.containsKey('code') || map.containsKey('error'))) {
              throw StateError(
                (map['message'] ?? 'Failed to set preference').toString(),
              );
            }
          }
        } finally {
          _freeString(ptr);
        }
      }
    } finally {
      malloc.free(keyPtr);
      malloc.free(valPtr);
      malloc.free(catPtr);
    }
  }

  @override
  List<UserPreferenceItem> listPreferences({String? category}) {
    final catPtr = category != null ? category.toNativeUtf8() : Pointer<Utf8>.fromAddress(0);
    try {
      final ptr = _preferenceList(catPtr);
      if (ptr.address == 0) return [];
      try {
        final res = ptr.toDartString();
        final decoded = jsonDecode(res);
        if (decoded is List) {
          return decoded
              .map((e) => UserPreferenceItem.fromJson(e as Map<String, dynamic>))
              .toList();
        }
        return [];
      } finally {
        _freeString(ptr);
      }
    } finally {
      if (category != null) {
        malloc.free(catPtr);
      }
    }
  }

  @override
  bool deletePreference(String key) {
    final keyPtr = key.toNativeUtf8();
    try {
      final ptr = _preferenceDelete(keyPtr);
      if (ptr.address == 0) return false;
      try {
        final res = ptr.toDartString();
        final decoded = jsonDecode(res);
        if (decoded is bool) return decoded;
        return false;
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(keyPtr);
    }
  }

  @override
  List<AutomationRuleItem> listAutomations({bool enabledOnly = false}) {
    final ptr = _automationList(enabledOnly ? 1 : 0);
    if (ptr.address == 0) return [];
    try {
      final jsonStr = ptr.toDartString();
      final decoded = jsonDecode(jsonStr);
      if (decoded is List) {
        return decoded
            .map((e) => AutomationRuleItem.fromJson(e as Map<String, dynamic>))
            .toList();
      }
      return [];
    } finally {
      _freeString(ptr);
    }
  }

  @override
  AutomationRuleItem? getAutomation(String id) {
    final idPtr = id.toNativeUtf8();
    try {
      final ptr = _automationGet(idPtr);
      if (ptr.address == 0) return null;
      try {
        final jsonStr = ptr.toDartString();
        if (jsonStr == 'null') return null;
        final decoded = jsonDecode(jsonStr);
        if (decoded is Map<String, dynamic>) {
          if (decoded.containsKey('code') || decoded.containsKey('error')) return null;
          return AutomationRuleItem.fromJson(decoded);
        }
        return null;
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(idPtr);
    }
  }

  @override
  void saveAutomation(AutomationRuleItem rule) {
    final jsonStr = jsonEncode(rule.toJson());
    final jsonPtr = jsonStr.toNativeUtf8();
    try {
      final ptr = _automationSave(jsonPtr);
      if (ptr.address != 0) {
        try {
          final res = ptr.toDartString();
          if (res != 'null') {
            final map = jsonDecode(res);
            if (map is Map<String, dynamic> &&
                (map.containsKey('code') || map.containsKey('error'))) {
              throw StateError(
                (map['message'] ?? 'Failed to save automation rule').toString(),
              );
            }
          }
        } finally {
          _freeString(ptr);
        }
      }
    } finally {
      malloc.free(jsonPtr);
    }
  }

  @override
  bool toggleAutomation(String id, bool enabled) {
    final idPtr = id.toNativeUtf8();
    try {
      final ptr = _automationToggle(idPtr, enabled ? 1 : 0);
      if (ptr.address == 0) return false;
      try {
        final res = ptr.toDartString();
        final decoded = jsonDecode(res);
        if (decoded is bool) return decoded;
        return false;
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(idPtr);
    }
  }

  @override
  bool deleteAutomation(String id) {
    final idPtr = id.toNativeUtf8();
    try {
      final ptr = _automationDelete(idPtr);
      if (ptr.address == 0) return false;
      try {
        final res = ptr.toDartString();
        final decoded = jsonDecode(res);
        if (decoded is bool) return decoded;
        return false;
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(idPtr);
    }
  }

  @override
  WorkflowExecutionReportItem triggerAutomation(String id) {
    final idPtr = id.toNativeUtf8();
    try {
      final ptr = _automationTrigger(idPtr);
      if (ptr.address == 0) {
        throw StateError('Trigger automation returned null pointer');
      }
      try {
        final res = ptr.toDartString();
        final map = jsonDecode(res);
        if (map is Map<String, dynamic>) {
          if (map.containsKey('code') || map.containsKey('error')) {
            throw StateError(
              (map['message'] ?? 'Failed to trigger automation').toString(),
            );
          }
          return WorkflowExecutionReportItem.fromJson(map);
        }
        throw StateError('Invalid execution report response');
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(idPtr);
    }
  }

  @override
  List<AutomationHistoryItem> listAutomationHistory({String? automationId, int limit = 50}) {
    final idPtr = automationId != null ? automationId.toNativeUtf8() : Pointer<Utf8>.fromAddress(0);
    try {
      final ptr = _automationHistory(idPtr, limit);
      if (ptr.address == 0) return [];
      try {
        final res = ptr.toDartString();
        final decoded = jsonDecode(res);
        if (decoded is List) {
          return decoded
              .map((e) => AutomationHistoryItem.fromJson(e as Map<String, dynamic>))
              .toList();
        }
        return [];
      } finally {
        _freeString(ptr);
      }
    } finally {
      if (automationId != null) {
        malloc.free(idPtr);
      }
    }
  }

  @override
  int clearAutomationHistory({String? automationId}) {
    final idPtr = automationId != null ? automationId.toNativeUtf8() : Pointer<Utf8>.fromAddress(0);
    try {
      final ptr = _automationClearHistory(idPtr);
      if (ptr.address == 0) return 0;
      try {
        final res = ptr.toDartString();
        final decoded = jsonDecode(res);
        if (decoded is int) return decoded;
        return 0;
      } finally {
        _freeString(ptr);
      }
    } finally {
      if (automationId != null) {
        malloc.free(idPtr);
      }
    }
  }

  @override
  List<SmartHomeEntityItem> listHomeEntities({String? domain}) {
    final domPtr = domain != null ? domain.toNativeUtf8() : Pointer<Utf8>.fromAddress(0);
    try {
      final ptr = _homeList(domPtr);
      if (ptr.address == 0) return [];
      try {
        final jsonStr = ptr.toDartString();
        final decoded = jsonDecode(jsonStr);
        if (decoded is List) {
          return decoded
              .map((e) => SmartHomeEntityItem.fromJson(e as Map<String, dynamic>))
              .toList();
        }
        return [];
      } finally {
        _freeString(ptr);
      }
    } finally {
      if (domain != null) {
        malloc.free(domPtr);
      }
    }
  }

  @override
  SmartHomeEntityItem? getHomeEntity(String id) {
    final idPtr = id.toNativeUtf8();
    try {
      final ptr = _homeGet(idPtr);
      if (ptr.address == 0) return null;
      try {
        final jsonStr = ptr.toDartString();
        if (jsonStr == 'null') return null;
        final decoded = jsonDecode(jsonStr);
        if (decoded is Map<String, dynamic>) {
          if (decoded.containsKey('code') || decoded.containsKey('error')) return null;
          return SmartHomeEntityItem.fromJson(decoded);
        }
        return null;
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(idPtr);
    }
  }

  @override
  void upsertHomeEntity(SmartHomeEntityItem item) {
    final jsonStr = jsonEncode(item.toJson());
    final jsonPtr = jsonStr.toNativeUtf8();
    try {
      final ptr = _homeUpsert(jsonPtr);
      if (ptr.address != 0) {
        try {
          final res = ptr.toDartString();
          if (res != 'null') {
            final map = jsonDecode(res);
            if (map is Map<String, dynamic> &&
                (map.containsKey('code') || map.containsKey('error'))) {
              throw StateError(
                (map['message'] ?? 'Failed to upsert smart home entity').toString(),
              );
            }
          }
        } finally {
          _freeString(ptr);
        }
      }
    } finally {
      malloc.free(jsonPtr);
    }
  }

  @override
  bool deleteHomeEntity(String id) {
    final idPtr = id.toNativeUtf8();
    try {
      final ptr = _homeDelete(idPtr);
      if (ptr.address == 0) return false;
      try {
        final res = ptr.toDartString();
        final decoded = jsonDecode(res);
        if (decoded is bool) return decoded;
        return false;
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(idPtr);
    }
  }

  @override
  SmartHomeCommandResultItem controlHomeDevice(SmartHomeCommandItem command) {
    final jsonStr = jsonEncode(command.toJson());
    final jsonPtr = jsonStr.toNativeUtf8();
    try {
      final ptr = _homeControl(jsonPtr);
      if (ptr.address == 0) {
        throw StateError('Control home device returned null pointer');
      }
      try {
        final res = ptr.toDartString();
        final map = jsonDecode(res);
        if (map is Map<String, dynamic>) {
          if (map.containsKey('code') || map.containsKey('error')) {
            throw StateError(
              (map['message'] ?? 'Failed to control home device').toString(),
            );
          }
          return SmartHomeCommandResultItem.fromJson(map);
        }
        throw StateError('Invalid control result response');
      } finally {
        _freeString(ptr);
      }
    } finally {
      malloc.free(jsonPtr);
    }
  }

  @override
  void rotateMasterKey() {
    final ptr = _rotateMasterKey();
    if (ptr.address != 0) {
      try {
        final res = ptr.toDartString();
        if (res != 'null') {
          final map = jsonDecode(res);
          if (map is Map<String, dynamic> &&
              (map.containsKey('code') || map.containsKey('error'))) {
            throw StateError(
              (map['message'] ?? 'Failed to rotate master key').toString(),
            );
          }
        }
      } finally {
        _freeString(ptr);
      }
    }
  }

  @override
  void revokeDevice(String deviceId) {
    final idPtr = deviceId.toNativeUtf8();
    try {
      final ptr = _deviceRevoke(idPtr);
      if (ptr.address != 0) {
        try {
          final res = ptr.toDartString();
          if (res != 'null') {
            final map = jsonDecode(res);
            if (map is Map<String, dynamic> &&
                (map.containsKey('code') || map.containsKey('error'))) {
              throw StateError(
                (map['message'] ?? 'Failed to revoke device').toString(),
              );
            }
          }
        } finally {
          _freeString(ptr);
        }
      }
    } finally {
      malloc.free(idPtr);
    }
  }

  @override
  void triggerEmergencyLockdown(String reason) {
    final reasonPtr = reason.toNativeUtf8();
    try {
      final ptr = _emergencyLockdown(reasonPtr);
      if (ptr.address != 0) {
        try {
          final res = ptr.toDartString();
          if (res != 'null') {
            final map = jsonDecode(res);
            if (map is Map<String, dynamic> &&
                (map.containsKey('code') || map.containsKey('error'))) {
              throw StateError(
                (map['message'] ?? 'Failed to trigger emergency lockdown').toString(),
              );
            }
          }
        } finally {
          _freeString(ptr);
        }
      }
    } finally {
      malloc.free(reasonPtr);
    }
  }

  @override
  void liftEmergencyLockdown() {
    final ptr = _liftEmergencyLockdown();
    if (ptr.address != 0) {
      try {
        final res = ptr.toDartString();
        if (res != 'null') {
          final map = jsonDecode(res);
          if (map is Map<String, dynamic> &&
              (map.containsKey('code') || map.containsKey('error'))) {
            throw StateError(
              (map['message'] ?? 'Failed to lift emergency lockdown').toString(),
            );
          }
        }
      } finally {
        _freeString(ptr);
      }
    }
  }

  @override
  bool isLockdownActive() {
    final ptr = _isLockdownActive();
    if (ptr.address == 0) return false;
    try {
      final res = ptr.toDartString();
      return res == 'true';
    } finally {
      _freeString(ptr);
    }
  }

  @override
  PerformanceSnapshotModel getPerformanceSnapshot() {
    final ptr = _getPerformanceSnapshot();
    if (ptr.address == 0) {
      throw Exception('Failed to retrieve performance snapshot: null pointer');
    }
    try {
      final jsonStr = ptr.toDartString();
      final map = jsonDecode(jsonStr) as Map<String, dynamic>;
      return PerformanceSnapshotModel.fromJson(map);
    } finally {
      _freeString(ptr);
    }
  }

  @override
  bool enforceMemoryBudget() {
    final ptr = _enforceMemoryBudget();
    if (ptr.address == 0) return false;
    try {
      final jsonStr = ptr.toDartString();
      final res = jsonDecode(jsonStr);
      return res == true;
    } finally {
      _freeString(ptr);
    }
  }

  @override
  PipelineBenchmarkReportModel runPipelineBenchmark() {
    final ptr = _runPipelineBenchmark();
    if (ptr.address == 0) {
      throw Exception('Failed to run pipeline benchmark: null pointer');
    }
    try {
      final jsonStr = ptr.toDartString();
      final map = jsonDecode(jsonStr) as Map<String, dynamic>;
      return PipelineBenchmarkReportModel.fromJson(map);
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
    const CapabilityItem(
      name: 'vision_ocr',
      tier: 'standard',
      isSupported: true,
      isActive: true,
      status: 'supported',
    ),
    const CapabilityItem(
      name: 'live_interpreter',
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

  final List<DiscoveredPeerItem> _discoveredPeers = <DiscoveredPeerItem>[
    const DiscoveredPeerItem(
      instanceName: 'pixel-9-pro',
      platform: 'android',
      port: 51820,
      addresses: <String>['192.168.1.105'],
      fingerprint: '3a4f8b1c9d2e5a7f',
      lastSeenMs: 1731671000000,
    ),
  ];

  final List<PairedDeviceItem> _pairedDevices = <PairedDeviceItem>[
    const PairedDeviceItem(
      deviceId: 'dev_macbook_pro',
      name: 'MacBook Pro M3',
      platform: 'macos',
      fingerprint: '7b2c9a1d4f6e8c3a',
      trustState: 'trusted',
      lastEndpoint: '192.168.1.50:51820',
      lastSeenMs: 1731671000000,
      pairedAtMs: 1731670000000,
      permissions: <PeerPermissionItem>[
        PeerPermissionItem(capability: 'local_llm', permissionLevel: 'always_allow'),
        PeerPermissionItem(capability: 'file_system', permissionLevel: 'ask_each_time'),
        PeerPermissionItem(capability: 'application_management', permissionLevel: 'ask_each_time'),
        PeerPermissionItem(capability: 'lan_peer_networking', permissionLevel: 'always_allow'),
      ],
    ),
  ];

  PairingSessionInfo? _activePairing;

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

  @override
  void notifyPowerTransition(String event) {
    _eventController.add(PowerTransitionEvent(event: event));
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
      lifecycleState: ModelLifecycleState(
        status: ModelLifecycleStatus.downloadedOnDisk,
        localPath: '/models/qwen2.5-0.5b-instruct-q4_k_m.gguf',
        sha256Verified: true,
      ),
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
      lifecycleState: ModelLifecycleState(
        status: ModelLifecycleStatus.downloadedOnDisk,
        localPath: '/models/smollm2-360m-instruct-q4_k_m.gguf',
        sha256Verified: true,
      ),
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
      lifecycleState: ModelLifecycleState(
        status: ModelLifecycleStatus.notDownloaded,
        totalBytes: 2168995840,
      ),
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
      models: _models.map((m) {
        final isActive = m.id == _activeModelId;
        final state = isActive
            ? ModelLifecycleState(
                status: ModelLifecycleStatus.loadedActive,
                memoryFootprintBytes: m.fileSizeBytes + (256 * 1024 * 1024),
              )
            : m.lifecycleState;
        return m.copyWith(isActive: isActive, lifecycleState: state);
      }).toList(),
    );
  }

  @override
  void loadModel(String modelId) {
    _activeModelId = modelId;
    final idx = _models.indexWhere((m) => m.id == modelId);
    if (idx != -1) {
      _models[idx] = _models[idx].copyWith(
        isActive: true,
        lifecycleState: ModelLifecycleState(
          status: ModelLifecycleStatus.loadedActive,
          memoryFootprintBytes: _models[idx].fileSizeBytes + (256 * 1024 * 1024),
        ),
      );
    }
    _eventController.add(ModelLoadedEvent(modelId: modelId));
  }

  @override
  void unloadModel() {
    if (_activeModelId != null) {
      final prev = _activeModelId!;
      final idx = _models.indexWhere((m) => m.id == prev);
      if (idx != -1) {
        _models[idx] = _models[idx].copyWith(
          isActive: false,
          lifecycleState: ModelLifecycleState(
            status: ModelLifecycleStatus.downloadedOnDisk,
            localPath: '/models/$prev.gguf',
            sha256Verified: true,
          ),
        );
      }
      _activeModelId = null;
      _eventController.add(ModelUnloadedEvent(modelId: prev));
    }
  }

  @override
  void downloadModel(String modelId) {
    final idx = _models.indexWhere((m) => m.id == modelId);
    if (idx != -1) {
      _models[idx] = _models[idx].copyWith(
        lifecycleState: ModelLifecycleState(
          status: ModelLifecycleStatus.downloading,
          downloadedBytes: 50 * 1024 * 1024,
          totalBytes: _models[idx].fileSizeBytes,
          speedBps: 15 * 1024 * 1024,
        ),
      );
      _eventController.add(ModelDownloadProgressEvent(
        modelId: modelId,
        downloadedBytes: 50 * 1024 * 1024,
        totalBytes: _models[idx].fileSizeBytes,
        speedBps: 15 * 1024 * 1024,
      ));
    }
  }

  void completeDownload(String modelId) {
    final i = _models.indexWhere((m) => m.id == modelId);
    if (i != -1) {
      _models[i] = _models[i].copyWith(
        lifecycleState: ModelLifecycleState(
          status: ModelLifecycleStatus.downloadedOnDisk,
          localPath: '/models/$modelId.gguf',
          sha256Verified: true,
        ),
      );
      _eventController.add(ModelDownloadCompletedEvent(
        modelId: modelId,
        localPath: '/models/$modelId.gguf',
        sha256Verified: true,
      ));
    }
  }

  @override
  void cancelDownload(String modelId) {
    final idx = _models.indexWhere((m) => m.id == modelId);
    if (idx != -1) {
      _models[idx] = _models[idx].copyWith(
        lifecycleState: const ModelLifecycleState(status: ModelLifecycleStatus.notDownloaded),
      );
      _eventController.add(ModelDownloadFailedEvent(
        modelId: modelId,
        error: 'Download cancelled by user',
      ));
    }
  }

  @override
  void deleteModel(String modelId) {
    if (_activeModelId == modelId) {
      unloadModel();
    }
    final idx = _models.indexWhere((m) => m.id == modelId);
    if (idx != -1) {
      if (_models[idx].id.startsWith('custom-')) {
        _models.removeAt(idx);
      } else {
        _models[idx] = _models[idx].copyWith(
          lifecycleState: const ModelLifecycleState(status: ModelLifecycleStatus.notDownloaded),
        );
      }
      _eventController.add(ModelUnloadedEvent(modelId: modelId));
    }
  }

  @override
  void importLocalGguf(String filePath) {
    final name = filePath.split(RegExp(r'[\\/]')).last.replaceAll('.gguf', '');
    final customItem = ModelItem(
      id: 'custom-${name.toLowerCase().replaceAll(' ', '-')}',
      name: name,
      tier: 'Nano0_5B',
      parameterCount: 'Custom',
      quantization: 'Q4_K_M',
      fileSizeBytes: 400 * 1024 * 1024,
      sha256: 'custom-sha256-hash',
      defaultContextWindow: 4096,
      license: 'Custom Local',
      description: 'Imported local file: $filePath',
      lifecycleState: ModelLifecycleState(
        status: ModelLifecycleStatus.downloadedOnDisk,
        localPath: filePath,
        sha256Verified: true,
      ),
    );
    _models.add(customItem);
    _eventController.add(ModelDownloadCompletedEvent(
      modelId: customItem.id,
      localPath: filePath,
      sha256Verified: true,
    ));
  }

  @override
  BenchmarkResult runModelBenchmark({int promptTokens = 32, int generateTokens = 16}) {
    if (_activeModelId == null) {
      throw StateError('No active model is loaded in memory');
    }
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

  @override
  List<DiscoveredPeerItem> getDiscoveredPeers() {
    return List<DiscoveredPeerItem>.unmodifiable(_discoveredPeers);
  }

  @override
  List<PairedDeviceItem> getPairedDevices() {
    return List<PairedDeviceItem>.unmodifiable(_pairedDevices);
  }

  @override
  PairingSessionInfo startPairing(String peerInstance) {
    final info = PairingSessionInfo(
      sessionId: 'sess_${DateTime.now().millisecondsSinceEpoch}',
      peerInstance: peerInstance,
      peerPlatform: 'android',
      sasCode: '481-923',
      qrPayload: '{"instance":"$peerInstance","sas":"481-923"}',
      createdAtSecs: DateTime.now().millisecondsSinceEpoch ~/ 1000,
    );
    _activePairing = info;
    _eventController.add(PairingStateChangedEvent(
      peerInstance: peerInstance,
      status: 'started',
      sasCode: info.sasCode,
      qrPayload: info.qrPayload,
    ));
    return info;
  }

  @override
  PairedDeviceItem confirmPairing(String sasCode) {
    final instance = _activePairing?.peerInstance ?? 'Discovered Device';
    final paired = PairedDeviceItem(
      deviceId: 'dev_${DateTime.now().millisecondsSinceEpoch}',
      name: instance,
      platform: 'android',
      fingerprint: '3a4f8b1c9d2e5a7f',
      trustState: 'trusted',
      lastEndpoint: '192.168.1.105:51820',
      lastSeenMs: DateTime.now().millisecondsSinceEpoch,
      pairedAtMs: DateTime.now().millisecondsSinceEpoch,
      permissions: const <PeerPermissionItem>[
        PeerPermissionItem(capability: 'local_llm', permissionLevel: 'always_allow'),
        PeerPermissionItem(capability: 'file_system', permissionLevel: 'ask_each_time'),
        PeerPermissionItem(capability: 'application_management', permissionLevel: 'ask_each_time'),
        PeerPermissionItem(capability: 'lan_peer_networking', permissionLevel: 'always_allow'),
      ],
    );
    _pairedDevices.add(paired);
    _discoveredPeers.removeWhere((p) => p.instanceName == instance);
    _activePairing = null;
    _eventController.add(PairingStateChangedEvent(
      peerInstance: instance,
      status: 'confirmed',
    ));
    return paired;
  }

  @override
  void cancelPairing() {
    final instance = _activePairing?.peerInstance ?? '';
    _activePairing = null;
    _eventController.add(PairingStateChangedEvent(
      peerInstance: instance,
      status: 'cancelled',
    ));
  }

  @override
  void setPeerPermission(String deviceId, String capability, String level) {
    final idx = _pairedDevices.indexWhere((d) => d.deviceId == deviceId);
    if (idx != -1) {
      final dev = _pairedDevices[idx];
      final newPerms = List<PeerPermissionItem>.from(dev.permissions);
      final permIdx = newPerms.indexWhere((p) => p.capability == capability);
      if (permIdx != -1) {
        newPerms[permIdx] = PeerPermissionItem(capability: capability, permissionLevel: level);
      } else {
        newPerms.add(PeerPermissionItem(capability: capability, permissionLevel: level));
      }
      _pairedDevices[idx] = PairedDeviceItem(
        deviceId: dev.deviceId,
        name: dev.name,
        platform: dev.platform,
        fingerprint: dev.fingerprint,
        trustState: dev.trustState,
        lastEndpoint: dev.lastEndpoint,
        lastSeenMs: dev.lastSeenMs,
        pairedAtMs: dev.pairedAtMs,
        permissions: newPerms,
      );
    }
  }

  @override
  void unpairDevice(String deviceId) {
    _pairedDevices.removeWhere((d) => d.deviceId == deviceId);
  }

  @override
  void simulateDiscoveredPeer(DiscoveredPeerItem item) {
    _discoveredPeers.removeWhere((p) => p.instanceName == item.instanceName);
    _discoveredPeers.add(item);
    _eventController.add(DeviceDiscoveredEvent(
      instanceName: item.instanceName,
      platform: item.platform,
      endpoint: item.addresses.isNotEmpty ? '${item.addresses.first}:${item.port}' : null,
      fingerprint: item.fingerprint,
    ));
  }

  final List<DocumentRecordItem> _documents = <DocumentRecordItem>[
    DocumentRecordItem(
      id: 'doc_sample_01',
      path: 'knowledge/offline_architecture.md',
      fileName: 'offline_architecture.md',
      mimeType: 'text/markdown',
      contentHash: 'a1b2c3d4e5f6',
      fileSizeBytes: 2048,
      chunkCount: 3,
      indexedAtMs: DateTime.now().subtract(const Duration(hours: 2)).millisecondsSinceEpoch,
      updatedAtMs: DateTime.now().subtract(const Duration(hours: 2)).millisecondsSinceEpoch,
    ),
  ];

  @override
  List<DocumentRecordItem> getKnowledgeDocuments() {
    return List<DocumentRecordItem>.from(_documents);
  }

  @override
  String indexKnowledgeDocument({
    required String path,
    required String fileName,
    required String mimeType,
    required String content,
  }) {
    final now = DateTime.now().millisecondsSinceEpoch;
    final docId = 'doc_${DateTime.now().microsecondsSinceEpoch.toRadixString(16)}';
    final doc = DocumentRecordItem(
      id: docId,
      path: path,
      fileName: fileName,
      mimeType: mimeType,
      contentHash: 'hash_${content.hashCode}',
      fileSizeBytes: content.length,
      chunkCount: (content.length / 200).ceil().clamp(1, 10),
      indexedAtMs: now,
      updatedAtMs: now,
    );
    _documents.removeWhere((d) => d.path == path);
    _documents.insert(0, doc);
    return docId;
  }

  @override
  bool deleteKnowledgeDocument(String documentId) {
    final initialLen = _documents.length;
    _documents.removeWhere((d) => d.id == documentId);
    return _documents.length < initialLen;
  }

  @override
  List<KnowledgeCitationItem> searchKnowledge(String query, {int limit = 10}) {
    final lowerQ = query.toLowerCase();
    final results = <KnowledgeCitationItem>[];
    for (final doc in _documents) {
      if (doc.fileName.toLowerCase().contains(lowerQ) ||
          doc.path.toLowerCase().contains(lowerQ) ||
          lowerQ.contains('offline') ||
          lowerQ.contains('flutter') ||
          lowerQ.contains('rust')) {
        results.add(KnowledgeCitationItem(
          chunkId: 'chk_${doc.id}_0',
          documentId: doc.id,
          path: doc.path,
          fileName: doc.fileName,
          chunkIndex: 0,
          rrfScore: 0.032,
          bm25Rank: 1,
          vectorRank: 1,
          cosineSimilarity: 0.88,
          content: 'Offline knowledge base snippet for ${doc.fileName}. Contains relevant context matching "$query".',
          snippet: 'Offline knowledge base snippet matching <mark>$query</mark> in ${doc.fileName}.',
        ));
      }
    }
    return results.take(limit).toList();
  }

  @override
  OcrDocumentItem extractOcr({required List<int> imageBytes}) {
    return const OcrDocumentItem(
      lines: <OcrTextLineItem>[
        OcrTextLineItem(
          lineNumber: 1,
          text: 'UNIVERSAL AI ASSISTANT',
          bbox: OcrBoundingBoxItem(x: 0.1, y: 0.1, width: 0.8, height: 0.15),
          confidence: 0.98,
          words: <OcrWordItem>[
            OcrWordItem(
              text: 'UNIVERSAL',
              bbox: OcrBoundingBoxItem(x: 0.1, y: 0.1, width: 0.25, height: 0.15),
              confidence: 0.98,
            ),
            OcrWordItem(
              text: 'AI',
              bbox: OcrBoundingBoxItem(x: 0.38, y: 0.1, width: 0.08, height: 0.15),
              confidence: 0.99,
            ),
            OcrWordItem(
              text: 'ASSISTANT',
              bbox: OcrBoundingBoxItem(x: 0.49, y: 0.1, width: 0.41, height: 0.15),
              confidence: 0.97,
            ),
          ],
        ),
        OcrTextLineItem(
          lineNumber: 2,
          text: '100% OFFLINE MULTIMODAL VISION',
          bbox: OcrBoundingBoxItem(x: 0.1, y: 0.35, width: 0.75, height: 0.12),
          confidence: 0.96,
          words: <OcrWordItem>[
            OcrWordItem(
              text: '100%',
              bbox: OcrBoundingBoxItem(x: 0.1, y: 0.35, width: 0.12, height: 0.12),
              confidence: 0.99,
            ),
            OcrWordItem(
              text: 'OFFLINE',
              bbox: OcrBoundingBoxItem(x: 0.24, y: 0.35, width: 0.18, height: 0.12),
              confidence: 0.95,
            ),
            OcrWordItem(
              text: 'MULTIMODAL',
              bbox: OcrBoundingBoxItem(x: 0.44, y: 0.35, width: 0.24, height: 0.12),
              confidence: 0.96,
            ),
            OcrWordItem(
              text: 'VISION',
              bbox: OcrBoundingBoxItem(x: 0.70, y: 0.35, width: 0.15, height: 0.12),
              confidence: 0.95,
            ),
          ],
        ),
        OcrTextLineItem(
          lineNumber: 3,
          text: 'ZERO CLOUD DEPENDENCY',
          bbox: OcrBoundingBoxItem(x: 0.1, y: 0.6, width: 0.65, height: 0.12),
          confidence: 0.95,
          words: <OcrWordItem>[
            OcrWordItem(
              text: 'ZERO',
              bbox: OcrBoundingBoxItem(x: 0.1, y: 0.6, width: 0.15, height: 0.12),
              confidence: 0.96,
            ),
            OcrWordItem(
              text: 'CLOUD',
              bbox: OcrBoundingBoxItem(x: 0.27, y: 0.6, width: 0.16, height: 0.12),
              confidence: 0.94,
            ),
            OcrWordItem(
              text: 'DEPENDENCY',
              bbox: OcrBoundingBoxItem(x: 0.45, y: 0.6, width: 0.30, height: 0.12),
              confidence: 0.95,
            ),
          ],
        ),
      ],
      fullText: 'UNIVERSAL AI ASSISTANT\n100% OFFLINE MULTIMODAL VISION\nZERO CLOUD DEPENDENCY',
      averageConfidence: 0.963,
      imageWidth: 640,
      imageHeight: 480,
      processingTimeMs: 42,
    );
  }

  InterpreterSessionStatusItem _interpreterStatus = const InterpreterSessionStatusItem(
    sessionId: 'session_mock_01',
    partyALang: 'en',
    partyBLang: 'es',
    activeSpeaker: 'party_a',
    turnCount: 0,
  );
  final List<InterpreterTurnItem> _interpreterTurns = <InterpreterTurnItem>[];

  @override
  TranslationResultItem translateText({
    required String text,
    required String sourceLang,
    required String targetLang,
  }) {
    final lower = text.trim().toLowerCase();
    String translated;
    if (sourceLang == 'en' && targetLang == 'es') {
      if (lower == 'hello') {
        translated = 'Hola';
      } else if (lower == 'good morning') {
        translated = 'Buenos días';
      } else if (lower == 'thank you') {
        translated = 'Gracias';
      } else if (lower == 'goodbye') {
        translated = 'Adiós';
      } else {
        translated = '[ES] $text';
      }
    } else if (sourceLang == 'es' && targetLang == 'en') {
      if (lower == 'hola') {
        translated = 'Hello';
      } else if (lower == 'gracias') {
        translated = 'Thank you';
      } else if (lower == 'adiós' || lower == 'adios') {
        translated = 'Goodbye';
      } else {
        translated = '[EN] $text';
      }
    } else {
      translated = '[${targetLang.toUpperCase()}] $text';
    }

    return TranslationResultItem(
      sourceText: text,
      translatedText: translated,
      sourceLang: sourceLang,
      targetLang: targetLang,
      confidence: 0.98,
      latencyMs: 14,
      engine: 'mock_phrasebook',
    );
  }

  @override
  InterpreterSessionStatusItem startInterpreterSession({
    required String partyALang,
    required String partyBLang,
  }) {
    _interpreterStatus = InterpreterSessionStatusItem(
      sessionId: 'session_mock_01',
      partyALang: partyALang,
      partyBLang: partyBLang,
      activeSpeaker: 'party_a',
      turnCount: _interpreterTurns.length,
    );
    return _interpreterStatus;
  }

  @override
  InterpreterTurnItem submitInterpreterTurn({required String text}) {
    final isPartyA = _interpreterStatus.activeSpeaker == 'party_a';
    final srcLang = isPartyA ? _interpreterStatus.partyALang : _interpreterStatus.partyBLang;
    final tgtLang = isPartyA ? _interpreterStatus.partyBLang : _interpreterStatus.partyALang;

    final result = translateText(text: text, sourceLang: srcLang, targetLang: tgtLang);
    final turn = InterpreterTurnItem(
      turnId: _interpreterTurns.length + 1,
      speaker: _interpreterStatus.activeSpeaker,
      sourceText: text,
      translatedText: result.translatedText,
      sourceLang: srcLang,
      targetLang: tgtLang,
      confidence: result.confidence,
      latencyMs: result.latencyMs,
      timestampMs: DateTime.now().millisecondsSinceEpoch,
    );

    _interpreterTurns.add(turn);
    _interpreterStatus = InterpreterSessionStatusItem(
      sessionId: _interpreterStatus.sessionId,
      partyALang: _interpreterStatus.partyALang,
      partyBLang: _interpreterStatus.partyBLang,
      activeSpeaker: isPartyA ? 'party_b' : 'party_a',
      turnCount: _interpreterTurns.length,
    );

    return turn;
  }

  @override
  List<InterpreterTurnItem> getInterpreterTurns() {
    return List<InterpreterTurnItem>.from(_interpreterTurns);
  }

  @override
  InterpreterSessionStatusItem swapInterpreterLanguages() {
    _interpreterStatus = InterpreterSessionStatusItem(
      sessionId: _interpreterStatus.sessionId,
      partyALang: _interpreterStatus.partyBLang,
      partyBLang: _interpreterStatus.partyALang,
      activeSpeaker: _interpreterStatus.activeSpeaker,
      turnCount: _interpreterTurns.length,
    );
    return _interpreterStatus;
  }

  @override
  void clearInterpreterSession() {
    _interpreterTurns.clear();
    _interpreterStatus = InterpreterSessionStatusItem(
      sessionId: _interpreterStatus.sessionId,
      partyALang: _interpreterStatus.partyALang,
      partyBLang: _interpreterStatus.partyBLang,
      activeSpeaker: 'party_a',
      turnCount: 0,
    );
  }

  final List<UserPersonaItem> _personas = [
    const UserPersonaItem(
      id: 'persona_default',
      name: 'Default Assistant',
      tone: 'balanced',
      verbosity: 'normal',
      voiceId: 'vits_piper_en_ryan',
      speakingRate: 1.0,
      pitch: 1.0,
      systemPromptOverride: null,
      isActive: true,
      createdAtMs: 1731670000000,
      updatedAtMs: 1731670000000,
    ),
  ];

  final List<UserPreferenceItem> _preferences = [
    const UserPreferenceItem(
      key: 'ui.theme',
      valueJson: '"system"',
      category: 'appearance',
      updatedAtMs: 1731670000000,
    ),
    const UserPreferenceItem(
      key: 'code.language',
      valueJson: '"rust"',
      category: 'developer',
      updatedAtMs: 1731670000000,
    ),
  ];

  final List<AutomationRuleItem> _automations = [
    const AutomationRuleItem(
      id: 'auto_morning_brief',
      name: 'Daily Morning Brief',
      enabled: true,
      triggerType: 'cron',
      triggerConfigJson: '{"type":"cron","expression":"@daily"}',
      actionsJson: '[{"action_type":"notification","title":"Morning Brief","message":"Weather and top news are ready."}]',
      lastExecutedMs: 1731670000000,
      createdAtMs: 1731670000000,
      updatedAtMs: 1731670000000,
    ),
    const AutomationRuleItem(
      id: 'auto_battery_saver',
      name: 'Low Battery Auto-Mute',
      enabled: true,
      triggerType: 'event',
      triggerConfigJson: '{"type":"event","event_name":"battery_low","filter_json":"{\\"threshold\\":15}"}',
      actionsJson: '[{"action_type":"notification","title":"Battery Saver","message":"Battery below 15%."}]',
      lastExecutedMs: null,
      createdAtMs: 1731670000000,
      updatedAtMs: 1731670000000,
    ),
  ];

  final List<AutomationHistoryItem> _automationHistory = [
    const AutomationHistoryItem(
      id: 'hist_mock_01',
      automationId: 'auto_morning_brief',
      status: 'success',
      triggeredBy: 'cron_scheduler',
      executedAtMs: 1731670000000,
      durationMs: 45,
      resultJson: '[{"action_index":0,"action_type":"notification","success":true,"duration_ms":45}]',
    ),
  ];

  @override
  UserPersonaItem? getActivePersona() {
    return _personas.cast<UserPersonaItem?>().firstWhere(
      (p) => p?.isActive == true,
      orElse: () => _personas.isNotEmpty ? _personas.first : null,
    );
  }

  @override
  List<UserPersonaItem> listPersonas() => List.unmodifiable(_personas);

  @override
  void savePersona(UserPersonaItem persona) {
    final idx = _personas.indexWhere((p) => p.id == persona.id);
    if (persona.isActive) {
      for (var i = 0; i < _personas.length; i++) {
        _personas[i] = _personas[i].copyWith(isActive: false);
      }
    }
    if (idx >= 0) {
      _personas[idx] = persona;
    } else {
      _personas.add(persona);
    }
  }

  @override
  void setActivePersona(String id) {
    final exists = _personas.any((p) => p.id == id);
    if (!exists) {
      throw StateError('Persona $id not found');
    }
    for (var i = 0; i < _personas.length; i++) {
      _personas[i] = _personas[i].copyWith(isActive: _personas[i].id == id);
    }
  }

  @override
  bool deletePersona(String id) {
    if (id == 'persona_default') {
      throw StateError('Cannot delete default persona');
    }
    final persona = _personas.firstWhere(
      (p) => p.id == id,
      orElse: () => throw StateError('Not found'),
    );
    if (persona.isActive) {
      throw StateError('Cannot delete active persona');
    }
    return _personas.remove(persona);
  }

  @override
  UserPreferenceItem? getPreference(String key) {
    return _preferences.cast<UserPreferenceItem?>().firstWhere(
      (p) => p?.key == key,
      orElse: () => null,
    );
  }

  @override
  void setPreference(String key, String valueJson, String category) {
    final idx = _preferences.indexWhere((p) => p.key == key);
    final item = UserPreferenceItem(
      key: key,
      valueJson: valueJson,
      category: category,
      updatedAtMs: DateTime.now().millisecondsSinceEpoch,
    );
    if (idx >= 0) {
      _preferences[idx] = item;
    } else {
      _preferences.add(item);
    }
  }

  @override
  List<UserPreferenceItem> listPreferences({String? category}) {
    if (category != null && category.isNotEmpty) {
      return _preferences.where((p) => p.category == category).toList();
    }
    return List.unmodifiable(_preferences);
  }

  @override
  bool deletePreference(String key) {
    final idx = _preferences.indexWhere((p) => p.key == key);
    if (idx >= 0) {
      _preferences.removeAt(idx);
      return true;
    }
    return false;
  }

  @override
  List<AutomationRuleItem> listAutomations({bool enabledOnly = false}) {
    if (enabledOnly) {
      return _automations.where((r) => r.enabled).toList();
    }
    return List.unmodifiable(_automations);
  }

  @override
  AutomationRuleItem? getAutomation(String id) {
    return _automations.cast<AutomationRuleItem?>().firstWhere(
      (r) => r?.id == id,
      orElse: () => null,
    );
  }

  @override
  void saveAutomation(AutomationRuleItem rule) {
    final idx = _automations.indexWhere((r) => r.id == rule.id);
    if (idx >= 0) {
      _automations[idx] = rule;
    } else {
      _automations.add(rule);
    }
  }

  @override
  bool toggleAutomation(String id, bool enabled) {
    final idx = _automations.indexWhere((r) => r.id == id);
    if (idx >= 0) {
      _automations[idx] = _automations[idx].copyWith(enabled: enabled);
      return true;
    }
    return false;
  }

  @override
  bool deleteAutomation(String id) {
    final idx = _automations.indexWhere((r) => r.id == id);
    if (idx >= 0) {
      _automations.removeAt(idx);
      _automationHistory.removeWhere((h) => h.automationId == id);
      return true;
    }
    return false;
  }

  @override
  WorkflowExecutionReportItem triggerAutomation(String id) {
    final rule = getAutomation(id);
    if (rule == null) {
      throw StateError('Automation rule $id not found');
    }
    final now = DateTime.now().millisecondsSinceEpoch;
    final report = WorkflowExecutionReportItem(
      automationId: id,
      status: 'success',
      triggeredBy: 'manual_ui',
      durationMs: 25,
      executedAtMs: now,
      resultJson: '[{"action_index":0,"action_type":"simulated","success":true,"duration_ms":25}]',
    );
    _automationHistory.insert(
      0,
      AutomationHistoryItem(
        id: 'hist_$now',
        automationId: id,
        status: 'success',
        triggeredBy: 'manual_ui',
        executedAtMs: now,
        durationMs: 25,
        resultJson: report.resultJson,
      ),
    );
    return report;
  }

  @override
  List<AutomationHistoryItem> listAutomationHistory({String? automationId, int limit = 50}) {
    var items = _automationHistory;
    if (automationId != null && automationId.isNotEmpty) {
      items = items.where((h) => h.automationId == automationId).toList();
    }
    return items.take(limit).toList();
  }

  @override
  int clearAutomationHistory({String? automationId}) {
    if (automationId != null && automationId.isNotEmpty) {
      final initial = _automationHistory.length;
      _automationHistory.removeWhere((h) => h.automationId == automationId);
      return initial - _automationHistory.length;
    } else {
      final count = _automationHistory.length;
      _automationHistory.clear();
      return count;
    }
  }

  final List<SmartHomeEntityItem> _homeEntities = [
    const SmartHomeEntityItem(
      id: 'light.living_room',
      domain: SmartHomeDomain.light,
      name: 'Living Room Light',
      stateJson: '{"state":"on","brightness":200}',
      attributesJson: '{"friendly_name":"Living Room Light"}',
      protocol: SmartHomeProtocol.homeAssistant,
      isAvailable: true,
      lastUpdatedMs: 1731670000000,
    ),
    const SmartHomeEntityItem(
      id: 'climate.hall',
      domain: SmartHomeDomain.climate,
      name: 'Hall Thermostat',
      stateJson: '{"state":"heat","target_temperature":22.0}',
      attributesJson: '{"friendly_name":"Main Thermostat"}',
      protocol: SmartHomeProtocol.mqtt,
      isAvailable: true,
      lastUpdatedMs: 1731670000000,
    ),
  ];

  @override
  List<SmartHomeEntityItem> listHomeEntities({String? domain}) {
    if (domain != null && domain.isNotEmpty) {
      final dom = SmartHomeDomain.fromString(domain);
      return _homeEntities.where((e) => e.domain == dom).toList();
    }
    return List.unmodifiable(_homeEntities);
  }

  @override
  SmartHomeEntityItem? getHomeEntity(String id) {
    return _homeEntities.cast<SmartHomeEntityItem?>().firstWhere(
      (e) => e?.id == id,
      orElse: () => null,
    );
  }

  @override
  void upsertHomeEntity(SmartHomeEntityItem item) {
    final idx = _homeEntities.indexWhere((e) => e.id == item.id);
    if (idx >= 0) {
      _homeEntities[idx] = item;
    } else {
      _homeEntities.add(item);
    }
  }

  @override
  bool deleteHomeEntity(String id) {
    final idx = _homeEntities.indexWhere((e) => e.id == id);
    if (idx >= 0) {
      _homeEntities.removeAt(idx);
      return true;
    }
    return false;
  }

  @override
  SmartHomeCommandResultItem controlHomeDevice(SmartHomeCommandItem command) {
    final entity = getHomeEntity(command.entityId);
    if (entity == null) {
      throw StateError('Device ${command.entityId} not found');
    }
    final newState = Map<String, dynamic>.from(entity.stateMap);
    switch (command.command) {
      case 'turn_on':
        newState['state'] = 'on';
        if (command.brightness != null) newState['brightness'] = command.brightness;
        if (command.colorHex != null) newState['color_hex'] = command.colorHex;
        break;
      case 'turn_off':
        newState['state'] = 'off';
        break;
      case 'toggle':
        newState['state'] = entity.isTurnedOn ? 'off' : 'on';
        break;
      case 'set_temperature':
        newState['state'] = 'heat';
        if (command.targetCelsius != null) {
          newState['target_temperature'] = command.targetCelsius;
        }
        break;
      case 'set_position':
        final pos = command.positionPercent ?? 0;
        newState['state'] = pos == 0 ? 'closed' : 'open';
        newState['position'] = pos;
        break;
    }
    final updated = entity.copyWith(
      stateJson: jsonEncode(newState),
      lastUpdatedMs: DateTime.now().millisecondsSinceEpoch,
    );
    upsertHomeEntity(updated);
    return SmartHomeCommandResultItem(
      success: true,
      entityId: command.entityId,
      command: command.command,
      dispatchedVia: entity.protocol.value,
      dispatchAction: '${entity.protocol.value}:dispatch:${command.command}',
      dispatchPayloadJson: jsonEncode(command.toJson()),
      newStateJson: jsonEncode(newState),
    );
  }

  bool _isLockdownActive = false;

  @override
  void rotateMasterKey() {
    _eventController.add(KeyRotatedEvent(timestampMs: DateTime.now().millisecondsSinceEpoch));
  }

  @override
  void revokeDevice(String deviceId) {
    final idx = _pairedDevices.indexWhere((d) => d.deviceId == deviceId);
    if (idx != -1) {
      final dev = _pairedDevices[idx];
      _pairedDevices[idx] = PairedDeviceItem(
        deviceId: dev.deviceId,
        name: dev.name,
        platform: dev.platform,
        fingerprint: dev.fingerprint,
        trustState: 'revoked',
        lastEndpoint: dev.lastEndpoint,
        lastSeenMs: dev.lastSeenMs,
        pairedAtMs: dev.pairedAtMs,
        permissions: dev.permissions,
      );
    }
    _eventController.add(DeviceRevokedEvent(deviceId: deviceId));
  }

  @override
  void triggerEmergencyLockdown(String reason) {
    _isLockdownActive = true;
    _activePairing = null;
    _eventController.add(EmergencyLockdownEvent(active: true, reason: reason));
  }

  @override
  void liftEmergencyLockdown() {
    _isLockdownActive = false;
    _eventController.add(const EmergencyLockdownEvent(active: false, reason: 'lifted_by_user'));
  }

  @override
  bool isLockdownActive() => _isLockdownActive;

  PerformanceSnapshotModel? _simulatedPerfSnapshot;

  @override
  PerformanceSnapshotModel getPerformanceSnapshot() {
    return _simulatedPerfSnapshot ??= PerformanceSnapshotModel(
      coldStartMs: 42,
      wakeWordFrameMicros: 350,
      sttLatencyMs: 210,
      ttsFirstAudioMs: 120,
      residentMemoryBytes: 24 * 1024 * 1024,
      idleBudgetBytes: 80 * 1024 * 1024,
      memoryBudgetExceeded: false,
      platform: Platform.operatingSystem,
      sqliteMemoryBytes: 819200,
      timestampMs: DateTime.now().millisecondsSinceEpoch,
    );
  }

  @override
  bool enforceMemoryBudget() {
    _simulatedPerfSnapshot = PerformanceSnapshotModel(
      coldStartMs: 42,
      wakeWordFrameMicros: 320,
      sttLatencyMs: 200,
      ttsFirstAudioMs: 110,
      residentMemoryBytes: 20 * 1024 * 1024,
      idleBudgetBytes: 80 * 1024 * 1024,
      memoryBudgetExceeded: false,
      platform: Platform.operatingSystem,
      sqliteMemoryBytes: 409600,
      timestampMs: DateTime.now().millisecondsSinceEpoch,
    );
    _eventController.add(const PerformanceMetricsUpdatedEvent(
      coldStartMs: 42,
      wakeWordFrameMicros: 320,
      sttLatencyMs: 200,
      ttsFirstAudioMs: 110,
      residentMemoryBytes: 20 * 1024 * 1024,
      idleBudgetBytes: 80 * 1024 * 1024,
      memoryBudgetExceeded: false,
    ));
    return true;
  }

  @override
  PipelineBenchmarkReportModel runPipelineBenchmark() {
    const report = PipelineBenchmarkReportModel(
      wakeWordFramesTested: 100,
      wakeWordAvgFrameMicros: 350,
      wakeWordRtf: 0.0175,
      ringBufferThroughputMsps: 8.5,
      coldStartMs: 42,
      vadWindowMicros: 45,
      sqliteQueryMicros: 120,
      memoryBeforeBytes: 24 * 1024 * 1024,
      memoryAfterBytes: 22 * 1024 * 1024,
      allBudgetsPassed: true,
    );
    _eventController.add(const PerformanceMetricsUpdatedEvent(
      coldStartMs: 42,
      wakeWordFrameMicros: 350,
      sttLatencyMs: 200,
      ttsFirstAudioMs: 110,
      residentMemoryBytes: 22 * 1024 * 1024,
      idleBudgetBytes: 80 * 1024 * 1024,
      memoryBudgetExceeded: false,
    ));
    return report;
  }
}
