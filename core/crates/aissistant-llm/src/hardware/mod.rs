pub mod cpu;
pub mod gpu;
pub mod memory;
pub mod recommendation;

pub use cpu::CpuFeatures;
pub use gpu::{GpuBackend, GpuDevice, GpuProfile};
pub use memory::MemoryProfile;
pub use recommendation::{HardwareRecommendation, ModelTier};

use serde::{Deserialize, Serialize};

/// Complete snapshot of host machine hardware capabilities and AI model recommendation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub cpu: CpuFeatures,
    pub memory: MemoryProfile,
    pub gpu: GpuProfile,
    pub recommendation: HardwareRecommendation,
}

impl HardwareProfile {
    /// Detects host hardware and computes the optimal model recommendation.
    pub fn detect() -> Self {
        let cpu = CpuFeatures::detect();
        let memory = MemoryProfile::detect();
        let gpu = GpuProfile::detect();
        let recommendation = HardwareRecommendation::compute(&cpu, &memory, &gpu);

        Self {
            cpu,
            memory,
            gpu,
            recommendation,
        }
    }
}

/// Abstract contract for hardware profiling, enabling deterministic testing.
pub trait HardwareProfiler: Send + Sync {
    fn profile(&self) -> HardwareProfile;
}

/// Production hardware profiler querying live system CPU, RAM, and GPU.
#[derive(Debug, Default, Clone)]
pub struct SystemHardwareProfiler;

impl HardwareProfiler for SystemHardwareProfiler {
    fn profile(&self) -> HardwareProfile {
        HardwareProfile::detect()
    }
}
