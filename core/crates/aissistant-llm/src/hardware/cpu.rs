use serde::{Deserialize, Serialize};
use sysinfo::System;

/// CPU architecture and SIMD instruction set capabilities detected at runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuFeatures {
    pub architecture: String,
    pub brand: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub avx: bool,
    pub avx2: bool,
    pub avx512: bool,
    pub neon: bool,
    pub arm_fp16: bool,
    pub fma: bool,
}

impl CpuFeatures {
    /// Detects CPU features on the current host machine.
    pub fn detect() -> Self {
        let arch = std::env::consts::ARCH.to_string();

        let mut sys = System::new();
        sys.refresh_cpu_usage();

        let logical_cores = sys.cpus().len().max(1);
        let physical_cores = sys.physical_core_count().unwrap_or(logical_cores).max(1);

        let brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_else(|| "Unknown CPU".to_string());

        let (avx, avx2, avx512, fma) = Self::detect_x86_features();
        let (neon, arm_fp16) = Self::detect_arm_features();

        Self {
            architecture: arch,
            brand,
            physical_cores,
            logical_cores,
            avx,
            avx2,
            avx512,
            neon,
            arm_fp16,
            fma,
        }
    }

    /// Computes the optimal thread count for local LLM token generation.
    ///
    /// For memory-bandwidth bound LLM matrix multiplication, hyperthreading / SMT
    /// often causes L1/L2 cache thrashing. The optimal thread allocation is
    /// typically equal to physical performance cores, bounded to 8 on desktop
    /// and leaving at least 1 core for the OS and audio loop.
    pub fn recommended_threads(&self) -> usize {
        if self.physical_cores <= 2 {
            self.physical_cores
        } else if self.physical_cores <= 4 {
            self.physical_cores.saturating_sub(1).max(2)
        } else if self.physical_cores <= 8 {
            self.physical_cores.saturating_sub(1)
        } else {
            // For high core-count CPUs (>8), 6 to 8 threads maximize L3 cache hit rates.
            8
        }
    }

    /// Evaluates if the CPU supports essential vector acceleration for quantized inference.
    pub fn has_vector_acceleration(&self) -> bool {
        self.avx2 || self.avx512 || self.neon
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn detect_x86_features() -> (bool, bool, bool, bool) {
        let avx = is_x86_feature_detected!("avx");
        let avx2 = is_x86_feature_detected!("avx2");
        let avx512 = is_x86_feature_detected!("avx512f");
        let fma = is_x86_feature_detected!("fma");
        (avx, avx2, avx512, fma)
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    fn detect_x86_features() -> (bool, bool, bool, bool) {
        (false, false, false, false)
    }

    #[cfg(target_arch = "aarch64")]
    fn detect_arm_features() -> (bool, bool) {
        let neon = std::arch::is_aarch64_feature_detected!("neon");
        let fp16 = std::arch::is_aarch64_feature_detected!("fp16");
        (neon, fp16)
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn detect_arm_features() -> (bool, bool) {
        (false, false)
    }
}
