use std::sync::{Arc, Mutex};
use std::time::Instant;

use aissistant_llm::{
    query_model_lifecycle, register_custom_gguf, HardwareProfiler, InferenceMetrics,
    LlmCancellationHandle, LlmEngine, LlmEngineConfig, LocalLlmEngine, ModelCatalog,
    ModelDownloadManager, ModelLifecycleState, SamplingParams, SystemHardwareProfiler, TokenChunk,
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
    pub lifecycle_state: ModelLifecycleState,
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
    download_manager: Arc<ModelDownloadManager>,
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
        let models_dir = ModelCatalog::default_models_dir();
        Self {
            engine: LocalLlmEngine::new(),
            download_manager: Arc::new(ModelDownloadManager::new(models_dir)),
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
                let is_downloading = self.download_manager.is_downloading(&m.id);
                let lifecycle_state = query_model_lifecycle(
                    self.download_manager.models_dir(),
                    &m,
                    is_active,
                    is_downloading,
                );

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
                    lifecycle_state,
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

    /// Loads a model by ID from the catalog into memory.
    pub fn load_model(&mut self, model_id: &str) -> FfiResult<()> {
        let descriptor = ModelCatalog::find_by_id(model_id)
            .ok_or_else(|| FfiError::Model(format!("Model '{model_id}' not found in catalog")))?;

        // Locate weights file on local disk
        let default_path = self
            .download_manager
            .models_dir()
            .join(format!("{}.gguf", descriptor.id));
        let model_path = if default_path.exists() {
            default_path
        } else if let Some(first_url) = descriptor.download_urls.first() {
            let custom_p = std::path::PathBuf::from(first_url);
            if custom_p.exists() {
                custom_p
            } else {
                default_path
            }
        } else {
            default_path
        };

        let config = LlmEngineConfig {
            model_id: descriptor.id.clone(),
            model_path,
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

    /// Initiates a background resumable streaming download for model weights.
    pub fn download_model(&self, model_id: &str) -> FfiResult<()> {
        let descriptor = ModelCatalog::find_by_id(model_id)
            .ok_or_else(|| FfiError::Model(format!("Model '{model_id}' not found in catalog")))?;

        let dm = self.download_manager.clone();
        let sink = self.event_sink.clone();
        let desc = descriptor.clone();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    sink.broadcast(BridgeEvent::ModelDownloadFailed {
                        model_id: desc.id,
                        error: format!("Runtime initialization error: {e}"),
                    });
                    return;
                }
            };

            rt.block_on(async move {
                let sink_progress = sink.clone();
                let res = dm
                    .download_model(&desc, move |progress| {
                        sink_progress.broadcast(BridgeEvent::ModelDownloadProgress {
                            model_id: progress.model_id,
                            downloaded_bytes: progress.downloaded_bytes,
                            total_bytes: progress.total_bytes,
                            speed_bps: progress.speed_bps,
                        });
                    })
                    .await;

                match res {
                    Ok(path) => {
                        sink.broadcast(BridgeEvent::ModelDownloadCompleted {
                            model_id: desc.id,
                            local_path: path.to_string_lossy().to_string(),
                            sha256_verified: true,
                        });
                    }
                    Err(e) => {
                        sink.broadcast(BridgeEvent::ModelDownloadFailed {
                            model_id: desc.id,
                            error: e.to_string(),
                        });
                    }
                }
            });
        });

        Ok(())
    }

    /// Cancels an in-flight model download by ID.
    pub fn cancel_download(&self, model_id: &str) -> FfiResult<()> {
        let cancelled = self.download_manager.cancel_download(model_id);
        if cancelled {
            self.event_sink.broadcast(BridgeEvent::ModelDownloadFailed {
                model_id: model_id.to_string(),
                error: "Download cancelled by user".to_string(),
            });
        }
        Ok(())
    }

    /// Deletes downloaded model weights from local disk and unloads if active.
    pub fn delete_model(&mut self, model_id: &str) -> FfiResult<()> {
        if self.engine.active_model_id() == Some(model_id) {
            let _ = self.unload_model();
        }

        let final_path = self
            .download_manager
            .models_dir()
            .join(format!("{model_id}.gguf"));
        if final_path.exists() {
            let _ = std::fs::remove_file(&final_path);
        }

        let part_path = self
            .download_manager
            .models_dir()
            .join(format!("{model_id}.gguf.part"));
        if part_path.exists() {
            let _ = std::fs::remove_file(&part_path);
        }

        ModelCatalog::remove_custom_model(model_id);

        self.event_sink.broadcast(BridgeEvent::ModelUnloaded {
            model_id: model_id.to_string(),
        });

        Ok(())
    }

    /// Imports a user-provided local .gguf file into the active model catalog.
    pub fn import_local_gguf(&mut self, file_path: &str) -> FfiResult<ModelCatalogItem> {
        let descriptor = register_custom_gguf(std::path::PathBuf::from(file_path))
            .map_err(|e| FfiError::Model(format!("Failed to import local GGUF: {e}")))?;

        self.event_sink
            .broadcast(BridgeEvent::ModelDownloadCompleted {
                model_id: descriptor.id.clone(),
                local_path: file_path.to_string(),
                sha256_verified: true,
            });

        let item = ModelCatalogItem {
            id: descriptor.id,
            name: descriptor.name,
            tier: format!("{:?}", descriptor.tier),
            parameter_count: descriptor.parameter_count,
            quantization: format!("{:?}", descriptor.quantization),
            file_size_bytes: descriptor.file_size_bytes,
            sha256: descriptor.sha256,
            default_context_window: descriptor.default_context_window,
            license: descriptor.license,
            description: descriptor.description,
            is_active: false,
            is_recommended: false,
            lifecycle_state: ModelLifecycleState::DownloadedOnDisk {
                local_path: file_path.to_string(),
                sha256_verified: true,
            },
        };

        Ok(item)
    }

    /// Checks if a model is currently loaded.
    pub fn is_loaded(&self) -> bool {
        self.engine.is_loaded()
    }

    /// Returns the active model ID, if any.
    pub fn active_model_id(&self) -> Option<&str> {
        self.engine.active_model_id()
    }

    /// Runs an on-device inference benchmark on the currently loaded model.
    pub fn run_benchmark(
        &mut self,
        prompt_tokens: usize,
        generate_tokens: usize,
    ) -> FfiResult<BenchmarkReport> {
        if !self.engine.is_loaded() {
            return Err(FfiError::Model(
                "No active model is loaded in memory. Download or load a model before running benchmarks."
                    .to_string(),
            ));
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

        let metrics = res.map_err(|e| FfiError::Model(format!("Benchmark failure: {e}")))?;
        let tps = if total_elapsed_ms > 0 {
            (metrics.generated_tokens as f32 / total_elapsed_ms as f32) * 1000.0
        } else {
            0.0
        };

        let memory_used_bytes = self.engine.memory_footprint_bytes();

        let report = BenchmarkReport {
            prompt_tokens: prompt_tokens.max(metrics.prompt_tokens),
            generated_tokens: metrics.generated_tokens.max(tokens_received),
            ttft_ms,
            total_elapsed_ms,
            tokens_per_second: tps,
            memory_used_bytes,
        };

        let json = serde_json::to_string(&report)
            .map_err(|e| FfiError::Serialization(format!("Serialization error: {e}")))?;
        self.event_sink
            .broadcast(BridgeEvent::BenchmarkCompleted { metrics_json: json });

        Ok(report)
    }

    /// Generates tokens in a streaming loop using the active loaded model.
    pub fn generate_stream(
        &self,
        prompt: &str,
        params: &SamplingParams,
        cancel_handle: Option<&LlmCancellationHandle>,
        on_token: &mut dyn FnMut(TokenChunk) -> aissistant_llm::Result<()>,
    ) -> aissistant_llm::Result<InferenceMetrics> {
        self.engine
            .generate_stream(prompt, params, cancel_handle, on_token)
    }

    /// Handles system memory pressure, unloading models on critical events.
    pub fn on_memory_pressure(&mut self, critical: bool) {
        if critical && self.engine.is_loaded() {
            let _ = self.unload_model();
        }
    }
}

/// Thread-safe adapter implementing `LlmEngine` by delegating to `ModelSupervisor`.
#[derive(Debug)]
pub struct BridgeLlmEngine {
    supervisor: Arc<Mutex<ModelSupervisor>>,
}

impl BridgeLlmEngine {
    /// Creates a new bridge LLM engine wrapping the model supervisor.
    pub fn new(supervisor: Arc<Mutex<ModelSupervisor>>) -> Self {
        Self { supervisor }
    }
}

impl LlmEngine for BridgeLlmEngine {
    fn load_model(&mut self, config: LlmEngineConfig) -> aissistant_llm::Result<()> {
        let mut sup = self
            .supervisor
            .lock()
            .map_err(|e| aissistant_llm::LlmError::InferenceError(e.to_string()))?;
        sup.load_model(&config.model_id)
            .map_err(|e| aissistant_llm::LlmError::InferenceError(e.to_string()))
    }

    fn unload_model(&mut self) {
        if let Ok(mut sup) = self.supervisor.lock() {
            let _ = sup.unload_model();
        }
    }

    fn is_loaded(&self) -> bool {
        self.supervisor
            .lock()
            .map(|s| s.is_loaded())
            .unwrap_or(false)
    }

    fn active_model_id(&self) -> Option<&str> {
        None
    }

    fn active_config(&self) -> Option<&LlmEngineConfig> {
        None
    }

    fn generate_stream(
        &self,
        prompt: &str,
        params: &SamplingParams,
        cancel_handle: Option<&LlmCancellationHandle>,
        on_token: &mut dyn FnMut(TokenChunk) -> aissistant_llm::Result<()>,
    ) -> aissistant_llm::Result<InferenceMetrics> {
        let sup = self
            .supervisor
            .lock()
            .map_err(|e| aissistant_llm::LlmError::InferenceError(e.to_string()))?;
        sup.generate_stream(prompt, params, cancel_handle, on_token)
    }

    fn on_memory_pressure(&mut self, critical: bool) {
        if let Ok(mut sup) = self.supervisor.lock() {
            sup.on_memory_pressure(critical);
        }
    }
}
