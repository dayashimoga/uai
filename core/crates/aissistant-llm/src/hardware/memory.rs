use serde::{Deserialize, Serialize};
use sysinfo::System;

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

/// System RAM and swap metrics with enforced OS safety headroom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProfile {
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub used_ram_bytes: u64,
    pub total_swap_bytes: u64,
    pub available_swap_bytes: u64,
    pub os_headroom_bytes: u64,
    pub usable_budget_bytes: u64,
}

impl MemoryProfile {
    /// Detects current system memory metrics.
    pub fn detect() -> Self {
        let mut sys = System::new();
        sys.refresh_memory();

        let total_ram_bytes = sys.total_memory();
        let available_ram_bytes = sys.available_memory();
        let used_ram_bytes = sys.used_memory();
        let total_swap_bytes = sys.total_swap();
        let available_swap_bytes = sys.free_swap();

        Self::from_values(
            total_ram_bytes,
            available_ram_bytes,
            used_ram_bytes,
            total_swap_bytes,
            available_swap_bytes,
        )
    }

    /// Constructs a `MemoryProfile` from specific values and computes OS headroom and safe usable budget.
    pub fn from_values(
        total_ram_bytes: u64,
        available_ram_bytes: u64,
        used_ram_bytes: u64,
        total_swap_bytes: u64,
        available_swap_bytes: u64,
    ) -> Self {
        let os_headroom_bytes = Self::calculate_os_headroom(total_ram_bytes);

        // Usable budget is available RAM minus OS headroom, never exceeding 75% of total RAM.
        let raw_budget = available_ram_bytes.saturating_sub(os_headroom_bytes);
        let max_budget_cap = (total_ram_bytes as f64 * 0.75) as u64;
        let usable_budget_bytes = raw_budget.min(max_budget_cap);

        Self {
            total_ram_bytes,
            available_ram_bytes,
            used_ram_bytes,
            total_swap_bytes,
            available_swap_bytes,
            os_headroom_bytes,
            usable_budget_bytes,
        }
    }

    /// Calculates minimum OS memory reservation to prevent system freezing or OOM killer invocations.
    pub fn calculate_os_headroom(total_ram_bytes: u64) -> u64 {
        if total_ram_bytes <= 3 * GIB {
            // Low-end mobile (2GB-3GB): 768 MB reservation
            768 * MIB
        } else if total_ram_bytes <= 4 * GIB {
            // Budget mobile / PC (4GB): 1.0 GB reservation
            GIB
        } else if total_ram_bytes <= 8 * GIB {
            // Standard laptop / phone (6GB-8GB): 1.5 GB reservation
            (1.5 * GIB as f64) as u64
        } else if total_ram_bytes <= 16 * GIB {
            // Modern laptop / desktop (12GB-16GB): 2.5 GB reservation
            (2.5 * GIB as f64) as u64
        } else {
            // High-end workstation (32GB+): 4.0 GB reservation
            4 * GIB
        }
    }

    /// Estimates required KV-cache memory in bytes for a given context length and model architecture.
    ///
    /// For 16-bit FP16 KV-cache:
    /// `bytes = 2 * n_layers * 2 * n_embd * context_tokens * sizeof(f16)`
    pub fn estimate_kv_cache_bytes(
        context_tokens: usize,
        n_layers: usize,
        n_embd_head_count: usize,
    ) -> u64 {
        // 2 elements (key, value) * 2 bytes (FP16) * layers * channels * tokens
        let bytes_per_token = 4 * n_layers * n_embd_head_count;
        (bytes_per_token * context_tokens) as u64
    }
}
