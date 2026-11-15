/// Model catalog descriptor representation in Flutter.
class ModelItem {
  final String id;
  final String name;
  final String tier;
  final String parameterCount;
  final String quantization;
  final int fileSizeBytes;
  final String sha256;
  final int defaultContextWindow;
  final String license;
  final String description;
  final bool isActive;
  final bool isRecommended;

  const ModelItem({
    required this.id,
    required this.name,
    required this.tier,
    required this.parameterCount,
    required this.quantization,
    required this.fileSizeBytes,
    required this.sha256,
    required this.defaultContextWindow,
    required this.license,
    required this.description,
    this.isActive = false,
    this.isRecommended = false,
  });

  factory ModelItem.fromJson(Map<String, dynamic> json) {
    return ModelItem(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      tier: json['tier'] as String? ?? 'Nano0_5B',
      parameterCount: json['parameter_count'] as String? ?? '',
      quantization: json['quantization'] as String? ?? 'Q4_K_M',
      fileSizeBytes: (json['file_size_bytes'] as num?)?.toInt() ?? 0,
      sha256: json['sha256'] as String? ?? '',
      defaultContextWindow:
          (json['default_context_window'] as num?)?.toInt() ?? 2048,
      license: json['license'] as String? ?? 'Apache-2.0',
      description: json['description'] as String? ?? '',
      isActive: json['is_active'] as bool? ?? false,
      isRecommended: json['is_recommended'] as bool? ?? false,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'id': id,
        'name': name,
        'tier': tier,
        'parameter_count': parameterCount,
        'quantization': quantization,
        'file_size_bytes': fileSizeBytes,
        'sha256': sha256,
        'default_context_window': defaultContextWindow,
        'license': license,
        'description': description,
        'is_active': isActive,
        'is_recommended': isRecommended,
      };

  ModelItem copyWith({
    bool? isActive,
    bool? isRecommended,
  }) {
    return ModelItem(
      id: id,
      name: name,
      tier: tier,
      parameterCount: parameterCount,
      quantization: quantization,
      fileSizeBytes: fileSizeBytes,
      sha256: sha256,
      defaultContextWindow: defaultContextWindow,
      license: license,
      description: description,
      isActive: isActive ?? this.isActive,
      isRecommended: isRecommended ?? this.isRecommended,
    );
  }

  String get fileSizeFormatted {
    if (fileSizeBytes >= 1024 * 1024 * 1024) {
      return '${(fileSizeBytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
    }
    return '${(fileSizeBytes / (1024 * 1024)).toStringAsFixed(0)} MB';
  }
}

/// System hardware overview and recommended inference budgets.
class HardwareOverview {
  final String cpuArchitecture;
  final int cpuCores;
  final List<String> simdFeatures;
  final int totalRamBytes;
  final int usableBudgetBytes;
  final String recommendedTier;
  final int recommendedContext;
  final int recommendedThreads;

  const HardwareOverview({
    required this.cpuArchitecture,
    required this.cpuCores,
    required this.simdFeatures,
    required this.totalRamBytes,
    required this.usableBudgetBytes,
    required this.recommendedTier,
    required this.recommendedContext,
    required this.recommendedThreads,
  });

  factory HardwareOverview.fromJson(Map<String, dynamic> json) {
    return HardwareOverview(
      cpuArchitecture: json['cpu_architecture'] as String? ?? 'x86_64',
      cpuCores: (json['cpu_cores'] as num?)?.toInt() ?? 4,
      simdFeatures: (json['simd_features'] as List<dynamic>?)
              ?.map((e) => e.toString())
              .toList() ??
          <String>['AVX2'],
      totalRamBytes: (json['total_ram_bytes'] as num?)?.toInt() ??
          (8 * 1024 * 1024 * 1024),
      usableBudgetBytes: (json['usable_budget_bytes'] as num?)?.toInt() ??
          (4 * 1024 * 1024 * 1024),
      recommendedTier: json['recommended_tier'] as String? ?? 'Small1_5B_3B',
      recommendedContext:
          (json['recommended_context'] as num?)?.toInt() ?? 4096,
      recommendedThreads: (json['recommended_threads'] as num?)?.toInt() ?? 4,
    );
  }

  String get totalRamFormatted {
    return '${(totalRamBytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
  }

  String get usableBudgetFormatted {
    return '${(usableBudgetBytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
  }
}

/// Snapshot of the local model manager state.
class ModelManagerSnapshot {
  final HardwareOverview hardware;
  final String? activeModelId;
  final List<ModelItem> models;

  const ModelManagerSnapshot({
    required this.hardware,
    this.activeModelId,
    required this.models,
  });

  factory ModelManagerSnapshot.fromJson(Map<String, dynamic> json) {
    return ModelManagerSnapshot(
      hardware: HardwareOverview.fromJson(
        json['hardware'] as Map<String, dynamic>? ?? <String, dynamic>{},
      ),
      activeModelId: json['active_model_id'] as String?,
      models: (json['models'] as List<dynamic>?)
              ?.map((e) => ModelItem.fromJson(e as Map<String, dynamic>))
              .toList() ??
          <ModelItem>[],
    );
  }
}

/// Telemetry metrics produced by an on-device benchmark run.
class BenchmarkResult {
  final int promptTokens;
  final int generatedTokens;
  final int ttftMs;
  final int totalElapsedMs;
  final double tokensPerSecond;
  final int memoryUsedBytes;

  const BenchmarkResult({
    required this.promptTokens,
    required this.generatedTokens,
    required this.ttftMs,
    required this.totalElapsedMs,
    required this.tokensPerSecond,
    required this.memoryUsedBytes,
  });

  factory BenchmarkResult.fromJson(Map<String, dynamic> json) {
    return BenchmarkResult(
      promptTokens: (json['prompt_tokens'] as num?)?.toInt() ?? 0,
      generatedTokens: (json['generated_tokens'] as num?)?.toInt() ?? 0,
      ttftMs: (json['ttft_ms'] as num?)?.toInt() ?? 0,
      totalElapsedMs: (json['total_elapsed_ms'] as num?)?.toInt() ?? 0,
      tokensPerSecond: (json['tokens_per_second'] as num?)?.toDouble() ?? 0.0,
      memoryUsedBytes: (json['memory_used_bytes'] as num?)?.toInt() ?? 0,
    );
  }

  String get memoryUsedFormatted {
    return '${(memoryUsedBytes / (1024 * 1024)).toStringAsFixed(0)} MB';
  }
}
