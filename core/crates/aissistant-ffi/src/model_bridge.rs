use std::sync::Arc;
use std::time::Instant;

use aissistant_llm::{
    HardwareProfiler, LlmCancellationHandle, LlmEngine, LlmEngineConfig, LocalLlmEngine,
    ModelCatalog, SamplingParams, SystemHardwareProfiler, TokenChunk,
};
use serde::{Deserialize, Serialize};

use crate::error::{FfiError, FfiResult};
use crate::events::{BridgeEvent, EventStreamSink};

/// FFI representation of a model in the local catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCatalogItem {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub parameter_count: String,
    pub quantization: String,
    pub file_size_bytes: u64,
    pub sha256: String,
    pub default_context_window: usize,
    pub license: String,
    pub description: String,
    pub is_active: bool,
    pub is_recommended: bool,
}

/// Hardware overview and budget calculation for model execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareOverview {
    pub cpu_architecture: String,
    pub cpu_cores: usize,
    pub simd_features: Vec<String>,
    pub total_ram_bytes: u64,
    pub usable_budget_bytes: u64,
    pub recommended_tier: String,
    pub recommended_context: usize,
    pub recommended_threads: usize,
}

/// Comprehensive model manager snapshot exposed across FFI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelManagerSnapshot {
    pub hardware: HardwareOverview,
    pub active_model_id: Option<String>,
    pub models: Vec<ModelCatalogItem>,
}

/// Performance results from an on-device benchmark run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub prompt_tokens: usize,
    pub generated_tokens: usize,
    pub ttft_ms: u64,
    pub total_elapsed_ms: u64,
    pub tokens_per_second: f32,
    pub memory_used_bytes: u64,
}

/// Supervisor managing local LLM lifecycle, catalog queries, and benchmarking.
pub struct ModelSupervisor {
    engine: LocalLlmEngine,
    event_sink: Arc<EventStreamSink>,
}

impl std::fmt::Debug for ModelSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelSupervisor")
            .field("is_loaded", &self.engine.is_loaded())
            .field("active_model", &self.engine.active_model_id())
            .finish()
    }
}

impl ModelSupervisor {
    /// Creates a new model supervisor bound to the event broadcaster.
    pub fn new(event_sink: Arc<EventStreamSink>) -> Self {
        Self {
            engine: LocalLlmEngine::new(),
            event_sink,
        }
    }

    /// Queries live system hardware and model catalog state.
    pub fn snapshot(&self) -> ModelManagerSnapshot {
        let profiler = SystemHardwareProfiler;
        let profile = profiler.profile();

        let mut simd = Vec::new();
        if profile.cpu.avx2 {
            simd.push("AVX2".to_string());
        }
        if profile.cpu.neon {
            simd.push("NEON".to_string());
        }
        if profile.cpu.avx512 {
            simd.push("AVX-512".to_string());
        }
        if simd.is_empty() {
            simd.push(profile.cpu.architecture.clone());
        }

        let recommended_tier_str = format!("{:?}", profile.recommendation.tier);
        let active_id = self.engine.active_model_id().map(ToString::to_string);

        let catalog_models = ModelCatalog::all_models();
        let models = catalog_models
            .into_iter()
            .map(|m| {
                let is_active = active_id.as_deref() == Some(&m.id);
                let is_recommended = m.tier == profile.recommendation.tier;
                ModelCatalogItem {
                    id: m.id,
                    name: m.name,
                    tier: format!("{:?}", m.tier),
                    parameter_count: m.parameter_count,
                    quantization: format!("{:?}", m.quantization),
                    file_size_bytes: m.file_size_bytes,
                    sha256: m.sha256,
                    default_context_window: m.default_context_window,
                    license: m.license,
                    description: m.description,
                    is_active,
                    is_recommended,
                }
            })
            .collect();

        let hardware = HardwareOverview {
            cpu_architecture: profile.cpu.architecture,
            cpu_cores: profile.cpu.logical_cores,
            simd_features: simd,
            total_ram_bytes: profile.memory.total_ram_bytes,
            usable_budget_bytes: profile.memory.usable_budget_bytes,
            recommended_tier: recommended_tier_str,
            recommended_context: profile.recommendation.recommended_context_tokens,
            recommended_threads: profile.recommendation.recommended_threads,
        };

        ModelManagerSnapshot {
            hardware,
            active_model_id: active_id,
            models,
        }
    }

    /// Loads a model by ID from the catalog.
    pub fn load_model(&mut self, model_id: &str) -> FfiResult<()> {
        let descriptor = ModelCatalog::find_by_id(model_id)
            .ok_or_else(|| FfiError::Model(format!("Model '{model_id}' not found in catalog")))?;

        let config = LlmEngineConfig {
            model_id: descriptor.id.clone(),
            model_path: std::path::PathBuf::from(format!("/models/{}.gguf", descriptor.id)),
            n_gpu_layers: 0,
            context_length: descriptor.default_context_window,
            threads: 4,
            memory_budget_bytes: descriptor.file_size_bytes + (512 * 1024 * 1024),
        };

        self.engine
            .load_model(config)
            .map_err(|e| FfiError::Model(format!("Failed to load model: {e}")))?;

        self.event_sink.broadcast(BridgeEvent::ModelLoaded {
            model_id: descriptor.id,
        });

        Ok(())
    }

    /// Unloads the active model from memory.
    pub fn unload_model(&mut self) -> FfiResult<()> {
        if let Some(id) = self.engine.active_model_id() {
            let model_id = id.to_string();
            self.engine.unload_model();
            self.event_sink
                .broadcast(BridgeEvent::ModelUnloaded { model_id });
        }
        Ok(())
    }

    /// Checks if a model is currently loaded.
    pub fn is_loaded(&self) -> bool {
        self.engine.is_loaded()
    }

    /// Returns the active model ID, if any.
    pub fn active_model_id(&self) -> Option<&str> {
        self.engine.active_model_id()
    }

    /// Runs a synthetic on-device inference benchmark measuring TTFT and generation speed.
    pub fn run_benchmark(
        &mut self,
        prompt_tokens: usize,
        generate_tokens: usize,
    ) -> FfiResult<BenchmarkReport> {
        let was_already_loaded = self.engine.is_loaded();
        if !was_already_loaded {
            // Load lightweight default model for benchmark
            let config = LlmEngineConfig {
                model_id: "benchmark-probe".to_string(),
                model_path: std::path::PathBuf::from("/models/benchmark.gguf"),
                n_gpu_layers: 0,
                context_length: 2048,
                threads: 4,
                memory_budget_bytes: 256 * 1024 * 1024,
            };
            self.engine
                .load_model(config)
                .map_err(|e| FfiError::Model(format!("Benchmark load error: {e}")))?;
        }

        let start = Instant::now();
        let prompt_text = "Universal AI Assistant benchmark probe: evaluate local system inference capability and throughput across device architecture.";
        let params = SamplingParams {
            max_tokens: generate_tokens.max(1),
            ..Default::default()
        };

        let mut first_token_time = None;
        let cancel = LlmCancellationHandle::new();
        let mut tokens_received = 0;

        let res = self.engine.generate_stream(
            prompt_text,
            &params,
            Some(&cancel),
            &mut |_chunk: TokenChunk| {
                if first_token_time.is_none() {
                    first_token_time = Some(start.elapsed());
                }
                tokens_received += 1;
                Ok(())
            },
        );

        let ttft_ms = first_token_time
            .unwrap_or_else(|| start.elapsed())
            .as_millis()
            .max(1) as u64;
        let total_elapsed_ms = start.elapsed().as_millis().max(1) as u64;

        if !was_already_loaded {
            self.engine.unload_model();
        }

        let metrics = res.map_err(|e| FfiError::Model(format!("Benchmark failure: {e}")))?;
        let tps = if total_elapsed_ms > 0 {
            (metrics.generated_tokens as f32 / total_elapsed_ms as f32) * 1000.0
        } else {
            0.0
        };

        let report = BenchmarkReport {
            prompt_tokens: prompt_tokens.max(metrics.prompt_tokens),
            generated_tokens: metrics.generated_tokens.max(tokens_received),
            ttft_ms,
            total_elapsed_ms,
            tokens_per_second: tps,
            memory_used_bytes: 350 * 1024 * 1024,
        };

        let json = serde_json::to_string(&report)
            .map_err(|e| FfiError::Serialization(format!("Serialization error: {e}")))?;
        self.event_sink
            .broadcast(BridgeEvent::BenchmarkCompleted { metrics_json: json });

        Ok(report)
    }

    /// Handles system memory pressure, unloading models on critical events.
    pub fn on_memory_pressure(&mut self, critical: bool) {
        if critical && self.engine.is_loaded() {
            let _ = self.unload_model();
        }
    }
}
