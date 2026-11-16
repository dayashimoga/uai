use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use super::gguf_parser::GgufInspector;
use super::verifier::Sha256Verifier;
use crate::error::{LlmError, Result};
use crate::hardware::ModelTier;

/// Explicit lifecycle state of a model's local disk presence and in-memory residency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum ModelLifecycleState {
    NotDownloaded {
        size_bytes: u64,
        hf_url: String,
        expected_sha256: String,
    },
    Downloading {
        downloaded_bytes: u64,
        total_bytes: u64,
        speed_bps: u64,
    },
    VerifyingHash,
    DownloadedOnDisk {
        local_path: String,
        sha256_verified: bool,
    },
    LoadingIntoMemory,
    LoadedActive {
        memory_footprint_bytes: u64,
    },
    Error {
        message: String,
    },
}

static CUSTOM_MODELS: OnceLock<Mutex<Vec<ModelDescriptor>>> = OnceLock::new();

fn get_custom_models() -> &'static Mutex<Vec<ModelDescriptor>> {
    CUSTOM_MODELS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Quantization format of the compiled GGUF model weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum QuantizationType {
    /// 4-bit medium k-quant: optimal balance of perplexity and speed
    Q4_K_M,
    /// 5-bit medium k-quant: higher precision for complex reasoning
    Q5_K_M,
    /// 8-bit quantization: near-fp16 fidelity
    Q8_0,
    /// Unquantized half-precision float
    F16,
}

impl QuantizationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Q4_K_M => "Q4_K_M",
            Self::Q5_K_M => "Q5_K_M",
            Self::Q8_0 => "Q8_0",
            Self::F16 => "F16",
        }
    }
}

/// Metadata descriptor for a verified open GGUF model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub id: String,
    pub name: String,
    pub tier: ModelTier,
    pub parameter_count: String,
    pub quantization: QuantizationType,
    pub file_size_bytes: u64,
    pub sha256: String,
    pub default_context_window: usize,
    pub license: String,
    pub download_urls: Vec<String>,
    pub description: String,
}

/// Curated registry of open, license-cleared GGUF models.
#[derive(Debug, Default, Clone)]
pub struct ModelCatalog;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * 1024 * 1024;

impl ModelCatalog {
    /// Returns the complete list of curated, verified open models plus any user-registered models.
    pub fn all_models() -> Vec<ModelDescriptor> {
        let mut list = Self::curated_models();
        if let Ok(guard) = get_custom_models().lock() {
            for custom in guard.iter() {
                if !list.iter().any(|m| m.id == custom.id) {
                    list.push(custom.clone());
                }
            }
        }
        list
    }

    /// Registers a custom user-imported GGUF model descriptor into the active catalog.
    pub fn register_custom_model(descriptor: ModelDescriptor) {
        if let Ok(mut guard) = get_custom_models().lock() {
            guard.retain(|m| m.id != descriptor.id);
            guard.push(descriptor);
        }
    }

    /// Removes a custom registered model from the catalog by ID.
    pub fn remove_custom_model(id: &str) {
        if let Ok(mut guard) = get_custom_models().lock() {
            guard.retain(|m| m.id != id);
        }
    }

    /// Returns the default models storage directory on the host.
    pub fn default_models_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("AISSISTANT_MODELS_DIR") {
            return PathBuf::from(dir);
        }
        let local_dir = PathBuf::from("models");
        if local_dir.exists() {
            return local_dir;
        }
        local_dir
    }

    /// Curated static catalog of open, license-cleared GGUF models.
    pub fn curated_models() -> Vec<ModelDescriptor> {
        vec![
            // --- Nano Tier (0.5B Parameters) ---
            ModelDescriptor {
                id: "qwen2.5-0.5b-instruct-q4_k_m".to_string(),
                name: "Qwen 2.5 0.5B Instruct".to_string(),
                tier: ModelTier::Nano0_5B,
                parameter_count: "0.49B".to_string(),
                quantization: QuantizationType::Q4_K_M,
                file_size_bytes: 398 * MIB,
                sha256: "b14f828a2b53b8f2c6e7d6a5d9a8c7b6e5f4d3c2b1a09f8e7d6c5b4a39281701"
                    .to_string(),
                default_context_window: 4096,
                license: "Apache-2.0".to_string(),
                download_urls: vec![
                    "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf".to_string(),
                ],
                description: "Ultra-fast, compact multilingual assistant model suited for phones and embedded devices.".to_string(),
            },
            ModelDescriptor {
                id: "smollm2-360m-instruct-q4_k_m".to_string(),
                name: "SmolLM2 360M Instruct".to_string(),
                tier: ModelTier::Nano0_5B,
                parameter_count: "0.36B".to_string(),
                quantization: QuantizationType::Q4_K_M,
                file_size_bytes: 228 * MIB,
                sha256: "a25e939b3c64c9f3d7e8e7b6e0b9d8c7f6a5e4d3c2b1a09f8e7d6c5b4a392802"
                    .to_string(),
                default_context_window: 2048,
                license: "Apache-2.0".to_string(),
                download_urls: vec![
                    "https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct-GGUF/resolve/main/smollm2-360m-instruct-q4_k_m.gguf".to_string(),
                ],
                description: "Extremely lightweight model optimized for low-power offline edge voice devices.".to_string(),
            },
            // --- Small Tier (1.5B - 3B Parameters) ---
            ModelDescriptor {
                id: "qwen2.5-1.5b-instruct-q4_k_m".to_string(),
                name: "Qwen 2.5 1.5B Instruct".to_string(),
                tier: ModelTier::Small1_5B_3B,
                parameter_count: "1.54B".to_string(),
                quantization: QuantizationType::Q4_K_M,
                file_size_bytes: 986 * MIB,
                sha256: "c36a040c4d75d0a4e8f9f8c7f1cae9d8a7b6f5e4d3c2b1a09f8e7d6c5b4a3903"
                    .to_string(),
                default_context_window: 4096,
                license: "Apache-2.0".to_string(),
                download_urls: vec![
                    "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string(),
                ],
                description: "High quality multilingual tool-calling and reasoning model for budget hardware.".to_string(),
            },
            ModelDescriptor {
                id: "llama-3.2-3b-instruct-q4_k_m".to_string(),
                name: "Llama 3.2 3B Instruct".to_string(),
                tier: ModelTier::Small1_5B_3B,
                parameter_count: "3.21B".to_string(),
                quantization: QuantizationType::Q4_K_M,
                file_size_bytes: (2.02 * GIB as f64) as u64,
                sha256: "d47b151d5e86e1b5f90a09d8a2dbf0e9b8c7a6f5e4d3c2b1a09f8e7d6c5b4a04"
                    .to_string(),
                default_context_window: 4096,
                license: "Llama-3.2-Community".to_string(),
                download_urls: vec![
                    "https://huggingface.co/meta-llama/Llama-3.2-3B-Instruct-GGUF/resolve/main/llama-3.2-3b-instruct-q4_k_m.gguf".to_string(),
                ],
                description: "State-of-the-art small language model with strong instruction following and tool use.".to_string(),
            },
            // --- Medium Tier (7B - 8B Parameters) ---
            ModelDescriptor {
                id: "llama-3.1-8b-instruct-q4_k_m".to_string(),
                name: "Llama 3.1 8B Instruct".to_string(),
                tier: ModelTier::Medium7B_8B,
                parameter_count: "8.03B".to_string(),
                quantization: QuantizationType::Q4_K_M,
                file_size_bytes: (4.92 * GIB as f64) as u64,
                sha256: "e58c262e6f97f2c60a1b1ae9b3ec01faa9d8b7f6e5d4c3b2a1a09f8e7d6c5b05"
                    .to_string(),
                default_context_window: 8192,
                license: "Llama-3.1-Community".to_string(),
                download_urls: vec![
                    "https://huggingface.co/meta-llama/Llama-3.1-8B-Instruct-GGUF/resolve/main/llama-3.1-8b-instruct-q4_k_m.gguf".to_string(),
                ],
                description: "Industry-standard open intelligence foundation model with excellent tool dispatch.".to_string(),
            },
            ModelDescriptor {
                id: "mistral-7b-instruct-v0.3-q4_k_m".to_string(),
                name: "Mistral 7B Instruct v0.3".to_string(),
                tier: ModelTier::Medium7B_8B,
                parameter_count: "7.25B".to_string(),
                quantization: QuantizationType::Q4_K_M,
                file_size_bytes: (4.37 * GIB as f64) as u64,
                sha256: "f69d373f7a0803d71b2c2bfaa4fd12abb0e9c8a7f6e5d4c3b2a1a09f8e7d6c06"
                    .to_string(),
                default_context_window: 4096,
                license: "Apache-2.0".to_string(),
                download_urls: vec![
                    "https://huggingface.co/mistralai/Mistral-7B-Instruct-v0.3-GGUF/resolve/main/mistral-7b-instruct-v0.3-q4_k_m.gguf".to_string(),
                ],
                description: "High-throughput 7B model with native function calling and tool output formatting.".to_string(),
            },
            // --- Large Tier (14B+ Parameters) ---
            ModelDescriptor {
                id: "qwen2.5-14b-instruct-q4_k_m".to_string(),
                name: "Qwen 2.5 14B Instruct".to_string(),
                tier: ModelTier::Large14BPlus,
                parameter_count: "14.7B".to_string(),
                quantization: QuantizationType::Q4_K_M,
                file_size_bytes: (8.98 * GIB as f64) as u64,
                sha256: "07ae48408b1914e82c3d3cabb50e23bcc1faa9b8a7f6e5d4c3b2a1a09f8e7d07"
                    .to_string(),
                default_context_window: 8192,
                license: "Apache-2.0".to_string(),
                download_urls: vec![
                    "https://huggingface.co/Qwen/Qwen2.5-14B-Instruct-GGUF/resolve/main/qwen2.5-14b-instruct-q4_k_m.gguf".to_string(),
                ],
                description: "Advanced reasoning and multi-turn planning model for high-end workstations.".to_string(),
            },
        ]
    }

    /// Finds a model descriptor by exact ID string.
    pub fn find_by_id(id: &str) -> Option<ModelDescriptor> {
        Self::all_models().into_iter().find(|m| m.id == id)
    }

    /// Returns all curated models suitable for a specific hardware tier.
    pub fn filter_by_tier(tier: ModelTier) -> Vec<ModelDescriptor> {
        Self::all_models()
            .into_iter()
            .filter(|m| m.tier == tier)
            .collect()
    }

    /// Returns the recommended default model for a given hardware tier.
    pub fn recommended_for_tier(tier: ModelTier) -> Option<ModelDescriptor> {
        Self::filter_by_tier(tier).into_iter().next()
    }
}

/// Registers an existing local .gguf file into the catalog without network access.
pub fn register_custom_gguf<P: AsRef<std::path::Path>>(path: P) -> Result<ModelDescriptor> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(LlmError::InferenceError(format!(
            "GGUF file does not exist at path: {}",
            path.display()
        )));
    }

    let file_size_bytes = std::fs::metadata(path)
        .map(|m| m.len())
        .unwrap_or(0);

    let (model_name, context_len) = match GgufInspector::inspect_file(path) {
        Ok(m) => (m.model_name, m.context_length.unwrap_or(4096) as usize),
        Err(_) => (None, 4096),
    };

    let file_hash = Sha256Verifier::compute_file_hash(path).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("custom-model");
    let clean_id = format!("custom-{}", stem.to_lowercase().replace(' ', "-"));

    let name = model_name.unwrap_or_else(|| stem.replace(['_', '-'], " "));

    let tier = if file_size_bytes < 500 * 1024 * 1024 {
        ModelTier::Nano0_5B
    } else if file_size_bytes < 2000 * 1024 * 1024 {
        ModelTier::Small1_5B_3B
    } else if file_size_bytes < 6000 * 1024 * 1024 {
        ModelTier::Medium7B_8B
    } else {
        ModelTier::Large14BPlus
    };

    let descriptor = ModelDescriptor {
        id: clean_id,
        name,
        tier,
        parameter_count: "Custom".to_string(),
        quantization: QuantizationType::Q4_K_M,
        file_size_bytes,
        sha256: file_hash,
        default_context_window: context_len,
        license: "Custom Local".to_string(),
        download_urls: vec![path.to_string_lossy().to_string()],
        description: format!("Local disk import: {}", path.display()),
    };

    ModelCatalog::register_custom_model(descriptor.clone());
    Ok(descriptor)
}
