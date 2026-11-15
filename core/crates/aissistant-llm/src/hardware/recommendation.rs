use serde::{Deserialize, Serialize};

use super::cpu::CpuFeatures;
use super::gpu::{GpuBackend, GpuProfile};
use super::memory::MemoryProfile;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * 1024 * 1024;

/// Granular model size tiers suited for local edge execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum ModelTier {
    /// Insufficient RAM (< 1.0 GB usable) to safely run an LLM without OOM risk.
    Unsupported,
    /// ~0.5B parameters (e.g. Qwen2.5-0.5B-Instruct, SmolLM-360M). Footprint: ~400MB-800MB.
    Nano0_5B,
    /// ~1.5B to 3B parameters (e.g. Qwen2.5-1.5B, Llama-3.2-3B, Phi-3.5-mini). Footprint: ~1.2GB-2.5GB.
    Small1_5B_3B,
    /// ~7B to 8B parameters (e.g. Llama-3.1-8B, Mistral-7B, Gemma-2-9B). Footprint: ~4.5GB-6.5GB.
    Medium7B_8B,
    /// ~14B+ parameters (e.g. Qwen2.5-14B, DeepSeek-R1-Distill-14B). Footprint: ~9.0GB-14.0GB.
    Large14BPlus,
}

impl ModelTier {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Unsupported => "Unsupported (Low Memory)",
            Self::Nano0_5B => "Nano (~0.5B Parameters)",
            Self::Small1_5B_3B => "Small (~1.5B - 3B Parameters)",
            Self::Medium7B_8B => "Medium (~7B - 8B Parameters)",
            Self::Large14BPlus => "Large (~14B+ Parameters)",
        }
    }
}

/// Comprehensive hardware recommendation calculating safe model bounds and offloading parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareRecommendation {
    pub tier: ModelTier,
    pub max_model_weights_bytes: u64,
    pub recommended_context_tokens: usize,
    pub recommended_threads: usize,
    pub gpu_layers_offload: u32,
    pub kv_cache_budget_bytes: u64,
    pub estimated_tokens_per_sec: f32,
    pub rationale: Vec<String>,
}

impl HardwareRecommendation {
    /// Computes the hardware-optimal recommendation given CPU, memory, and GPU diagnostics.
    pub fn compute(cpu: &CpuFeatures, memory: &MemoryProfile, gpu: &GpuProfile) -> Self {
        let mut rationale = Vec::new();
        let budget = memory.usable_budget_bytes;

        // 1. Determine safe ModelTier based on usable RAM budget
        let (tier, max_weights, default_ctx) = if budget < 900 * MIB {
            rationale.push(format!(
                "Available usable RAM ({:.1} GB) is below the 0.9 GB safety floor.",
                budget as f64 / GIB as f64
            ));
            (ModelTier::Unsupported, 0, 0)
        } else if budget < (2.2 * GIB as f64) as u64 {
            rationale.push(format!(
                "Usable RAM ({:.1} GB) supports lightweight Nano models (~0.5B).",
                budget as f64 / GIB as f64
            ));
            (ModelTier::Nano0_5B, 700 * MIB, 2048)
        } else if budget < (5.5 * GIB as f64) as u64 {
            rationale.push(format!(
                "Usable RAM ({:.1} GB) supports Small models (1.5B - 3B Q4_K_M).",
                budget as f64 / GIB as f64
            ));
            (ModelTier::Small1_5B_3B, (2.6 * GIB as f64) as u64, 4096)
        } else if budget < (12.0 * GIB as f64) as u64 {
            rationale.push(format!(
                "Usable RAM ({:.1} GB) supports standard Medium models (7B - 8B Q4_K_M).",
                budget as f64 / GIB as f64
            ));
            (ModelTier::Medium7B_8B, (5.8 * GIB as f64) as u64, 4096)
        } else {
            rationale.push(format!(
                "Abundant usable RAM ({:.1} GB) enables Large models (14B+ Q4_K_M).",
                budget as f64 / GIB as f64
            ));
            (ModelTier::Large14BPlus, (11.0 * GIB as f64) as u64, 8192)
        };

        if tier == ModelTier::Unsupported {
            return Self {
                tier: ModelTier::Unsupported,
                max_model_weights_bytes: 0,
                recommended_context_tokens: 0,
                recommended_threads: cpu.recommended_threads(),
                gpu_layers_offload: 0,
                kv_cache_budget_bytes: 0,
                estimated_tokens_per_sec: 0.0,
                rationale,
            };
        }

        // 2. Adjust Context Window based on remaining memory after model weight allocation
        let remaining_for_kv = budget.saturating_sub(max_weights);
        let ctx = if remaining_for_kv >= 2 * GIB && default_ctx < 8192 {
            8192
        } else if remaining_for_kv >= GIB && default_ctx < 4096 {
            4096
        } else if remaining_for_kv < 500 * MIB {
            2048
        } else {
            default_ctx
        };

        // Standard estimate: 4 bytes per token per layer (approx 28 layers for 3B/7B)
        let est_kv_bytes = (ctx as u64) * 28 * 4 * 128; // ~350MB for 4096 tokens
        let kv_cache_budget_bytes = est_kv_bytes.min(remaining_for_kv);

        // 3. GPU Layer Offloading Strategy
        let gpu_layers = if gpu.supports_gpu_offload {
            match gpu.selected_backend {
                GpuBackend::Metal => {
                    rationale.push(
                        "Apple Metal unified memory enabled: 100% layer offloading recommended."
                            .to_string(),
                    );
                    33 // standard llama.cpp full offload flag
                }
                GpuBackend::Cuda => {
                    rationale.push("NVIDIA CUDA detected: full GPU layer offloading.".to_string());
                    33
                }
                GpuBackend::Vulkan => {
                    rationale.push(
                        "Vulkan compute detected: offloading layers for tensor acceleration."
                            .to_string(),
                    );
                    33
                }
                GpuBackend::CpuOnly => 0,
            }
        } else {
            rationale.push("GPU acceleration not present; using CPU SIMD inference.".to_string());
            0
        };

        // 4. Threading and Throughput Estimation
        let threads = cpu.recommended_threads();
        let speed_factor = if gpu.supports_gpu_offload {
            match tier {
                ModelTier::Nano0_5B => 45.0,
                ModelTier::Small1_5B_3B => 30.0,
                ModelTier::Medium7B_8B => 18.0,
                ModelTier::Large14BPlus => 10.0,
                ModelTier::Unsupported => 0.0,
            }
        } else {
            let simd_mult = if cpu.avx512 {
                1.4
            } else if cpu.avx2 || cpu.neon {
                1.0
            } else {
                0.4
            };
            match tier {
                ModelTier::Nano0_5B => 22.0 * simd_mult,
                ModelTier::Small1_5B_3B => 12.0 * simd_mult,
                ModelTier::Medium7B_8B => 5.5 * simd_mult,
                ModelTier::Large14BPlus => 2.5 * simd_mult,
                ModelTier::Unsupported => 0.0,
            }
        };

        Self {
            tier,
            max_model_weights_bytes: max_weights,
            recommended_context_tokens: ctx,
            recommended_threads: threads,
            gpu_layers_offload: gpu_layers,
            kv_cache_budget_bytes,
            estimated_tokens_per_sec: (speed_factor as f32).max(1.0),
            rationale,
        }
    }
}
