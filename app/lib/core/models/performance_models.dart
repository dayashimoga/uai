// Performance and latency benchmark data models matching aissistant-ffi.

class PerformanceSnapshotModel {
  final int coldStartMs;
  final int wakeWordFrameMicros;
  final int sttLatencyMs;
  final int ttsFirstAudioMs;
  final int residentMemoryBytes;
  final int idleBudgetBytes;
  final bool memoryBudgetExceeded;
  final String platform;
  final int sqliteMemoryBytes;
  final int timestampMs;

  const PerformanceSnapshotModel({
    required this.coldStartMs,
    required this.wakeWordFrameMicros,
    required this.sttLatencyMs,
    required this.ttsFirstAudioMs,
    required this.residentMemoryBytes,
    required this.idleBudgetBytes,
    required this.memoryBudgetExceeded,
    required this.platform,
    required this.sqliteMemoryBytes,
    required this.timestampMs,
  });

  factory PerformanceSnapshotModel.fromJson(Map<String, dynamic> json) {
    return PerformanceSnapshotModel(
      coldStartMs: (json['cold_start_ms'] as num?)?.toInt() ?? 0,
      wakeWordFrameMicros:
          (json['wake_word_frame_micros'] as num?)?.toInt() ?? 0,
      sttLatencyMs: (json['stt_latency_ms'] as num?)?.toInt() ?? 0,
      ttsFirstAudioMs: (json['tts_first_audio_ms'] as num?)?.toInt() ?? 0,
      residentMemoryBytes:
          (json['resident_memory_bytes'] as num?)?.toInt() ?? 0,
      idleBudgetBytes: (json['idle_budget_bytes'] as num?)?.toInt() ?? 0,
      memoryBudgetExceeded:
          json['memory_budget_exceeded'] as bool? ?? false,
      platform: json['platform'] as String? ?? 'unknown',
      sqliteMemoryBytes: (json['sqlite_memory_bytes'] as num?)?.toInt() ?? 0,
      timestampMs: (json['timestamp_ms'] as num?)?.toInt() ?? 0,
    );
  }

  double get residentMemoryMb => residentMemoryBytes / (1024 * 1024);
  double get idleBudgetMb => idleBudgetBytes / (1024 * 1024);
}

class PipelineBenchmarkReportModel {
  final int wakeWordFramesTested;
  final int wakeWordAvgFrameMicros;
  final double wakeWordRtf;
  final double ringBufferThroughputMsps;
  final int coldStartMs;
  final int vadWindowMicros;
  final int sqliteQueryMicros;
  final int memoryBeforeBytes;
  final int memoryAfterBytes;
  final bool allBudgetsPassed;

  const PipelineBenchmarkReportModel({
    required this.wakeWordFramesTested,
    required this.wakeWordAvgFrameMicros,
    required this.wakeWordRtf,
    required this.ringBufferThroughputMsps,
    required this.coldStartMs,
    required this.vadWindowMicros,
    required this.sqliteQueryMicros,
    required this.memoryBeforeBytes,
    required this.memoryAfterBytes,
    required this.allBudgetsPassed,
  });

  factory PipelineBenchmarkReportModel.fromJson(Map<String, dynamic> json) {
    return PipelineBenchmarkReportModel(
      wakeWordFramesTested:
          (json['wake_word_frames_tested'] as num?)?.toInt() ?? 0,
      wakeWordAvgFrameMicros:
          (json['wake_word_avg_frame_micros'] as num?)?.toInt() ?? 0,
      wakeWordRtf: (json['wake_word_rtf'] as num?)?.toDouble() ?? 0.0,
      ringBufferThroughputMsps:
          (json['ring_buffer_throughput_msps'] as num?)?.toDouble() ?? 0.0,
      coldStartMs: (json['cold_start_ms'] as num?)?.toInt() ?? 0,
      vadWindowMicros: (json['vad_window_micros'] as num?)?.toInt() ?? 0,
      sqliteQueryMicros: (json['sqlite_query_micros'] as num?)?.toInt() ?? 0,
      memoryBeforeBytes:
          (json['memory_before_bytes'] as num?)?.toInt() ?? 0,
      memoryAfterBytes: (json['memory_after_bytes'] as num?)?.toInt() ?? 0,
      allBudgetsPassed: json['all_budgets_passed'] as bool? ?? false,
    );
  }
}
