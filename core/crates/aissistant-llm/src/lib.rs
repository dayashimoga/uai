pub mod error;
pub mod hardware;

pub use error::{LlmError, Result};
pub use hardware::{
    CpuFeatures, GpuBackend, GpuDevice, GpuProfile, HardwareProfile, HardwareProfiler,
    HardwareRecommendation, MemoryProfile, ModelTier, SystemHardwareProfiler,
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;
    const MIB: u64 = 1024 * 1024;

    fn sample_cpu(physical_cores: usize, logical_cores: usize, avx2: bool) -> CpuFeatures {
        CpuFeatures {
            architecture: "x86_64".to_string(),
            brand: "Test CPU".to_string(),
            physical_cores,
            logical_cores,
            avx: avx2,
            avx2,
            avx512: false,
            neon: false,
            arm_fp16: false,
            fma: avx2,
        }
    }

    #[test]
    fn test_system_hardware_profiler_live_probe() {
        let profiler = SystemHardwareProfiler;
        let profile = profiler.profile();

        assert!(!profile.cpu.architecture.is_empty());
        assert!(profile.cpu.physical_cores >= 1);
        assert!(profile.cpu.logical_cores >= 1);
        assert!(profile.memory.total_ram_bytes > 0);
        assert!(profile.memory.os_headroom_bytes > 0);
        assert!(profile.recommendation.recommended_threads >= 1);
    }

    #[test]
    fn test_cpu_features_recommended_threads() {
        // Dual-core
        let cpu_2 = sample_cpu(2, 4, true);
        assert_eq!(cpu_2.recommended_threads(), 2);

        // Quad-core
        let cpu_4 = sample_cpu(4, 8, true);
        assert_eq!(cpu_4.recommended_threads(), 3);

        // 8-core
        let cpu_8 = sample_cpu(8, 16, true);
        assert_eq!(cpu_8.recommended_threads(), 7);

        // 16-core workstation
        let cpu_16 = sample_cpu(16, 32, true);
        assert_eq!(cpu_16.recommended_threads(), 8);
    }

    #[test]
    fn test_memory_profile_headroom_and_budget_scaling() {
        // 2 GB phone
        let mem_2gb = MemoryProfile::from_values(2 * GIB, 1200 * MIB, 800 * MIB, 0, 0);
        assert_eq!(mem_2gb.os_headroom_bytes, 768 * MIB);
        assert_eq!(mem_2gb.usable_budget_bytes, (1200 - 768) * MIB);

        // 8 GB laptop with 5 GB free
        let mem_8gb = MemoryProfile::from_values(8 * GIB, 5 * GIB, 3 * GIB, 2 * GIB, 2 * GIB);
        assert_eq!(mem_8gb.os_headroom_bytes, (1.5 * GIB as f64) as u64);
        assert_eq!(mem_8gb.usable_budget_bytes, (3.5 * GIB as f64) as u64);

        // 32 GB workstation with 20 GB free
        let mem_32gb = MemoryProfile::from_values(32 * GIB, 20 * GIB, 12 * GIB, 8 * GIB, 8 * GIB);
        assert_eq!(mem_32gb.os_headroom_bytes, 4 * GIB);
        assert_eq!(mem_32gb.usable_budget_bytes, 16 * GIB);
    }

    #[test]
    fn test_model_tier_recommendations_across_devices() {
        let cpu = sample_cpu(8, 16, true);
        let gpu = GpuProfile::cpu_only();

        // Very low memory (< 900 MB usable) -> Unsupported
        let mem_low = MemoryProfile::from_values(2 * GIB, 1000 * MIB, 1000 * MIB, 0, 0);
        let rec_low = HardwareRecommendation::compute(&cpu, &mem_low, &gpu);
        assert_eq!(rec_low.tier, ModelTier::Unsupported);
        assert_eq!(rec_low.max_model_weights_bytes, 0);
        assert_eq!(rec_low.recommended_context_tokens, 0);

        // 3 GB phone (1.5 GB usable) -> Nano 0.5B
        let mem_nano = MemoryProfile::from_values(4 * GIB, 2500 * MIB, 1500 * MIB, 0, 0);
        let rec_nano = HardwareRecommendation::compute(&cpu, &mem_nano, &gpu);
        assert_eq!(rec_nano.tier, ModelTier::Nano0_5B);
        assert!(rec_nano.recommended_context_tokens >= 2048);

        // 8 GB PC (4 GB usable) -> Small 1.5B-3B
        let mem_small = MemoryProfile::from_values(8 * GIB, 5500 * MIB, 2500 * MIB, 0, 0);
        let rec_small = HardwareRecommendation::compute(&cpu, &mem_small, &gpu);
        assert_eq!(rec_small.tier, ModelTier::Small1_5B_3B);
        assert_eq!(rec_small.recommended_context_tokens, 4096);

        // 16 GB PC (10 GB usable) -> Medium 7B-8B
        let mem_med = MemoryProfile::from_values(16 * GIB, 12 * GIB, 4 * GIB, 0, 0);
        let rec_med = HardwareRecommendation::compute(&cpu, &mem_med, &gpu);
        assert_eq!(rec_med.tier, ModelTier::Medium7B_8B);

        // 32 GB workstation (20 GB usable) -> Large 14B+
        let mem_large = MemoryProfile::from_values(32 * GIB, 24 * GIB, 8 * GIB, 0, 0);
        let rec_large = HardwareRecommendation::compute(&cpu, &mem_large, &gpu);
        assert_eq!(rec_large.tier, ModelTier::Large14BPlus);
        assert_eq!(rec_large.recommended_context_tokens, 8192);
    }

    #[test]
    fn test_gpu_offload_recommendations() {
        let cpu = sample_cpu(8, 16, true);
        let mem = MemoryProfile::from_values(16 * GIB, 12 * GIB, 4 * GIB, 0, 0);

        // Metal GPU offload
        let gpu_metal = GpuProfile::from_device(GpuDevice {
            name: "Apple M2 Pro".to_string(),
            backend: GpuBackend::Metal,
            total_vram_bytes: None,
            is_integrated: true,
        });
        let rec_metal = HardwareRecommendation::compute(&cpu, &mem, &gpu_metal);
        assert_eq!(rec_metal.gpu_layers_offload, 33);
        assert!(rec_metal.estimated_tokens_per_sec > 15.0);

        // Vulkan GPU offload
        let gpu_vulkan = GpuProfile::from_device(GpuDevice {
            name: "AMD Radeon 780M".to_string(),
            backend: GpuBackend::Vulkan,
            total_vram_bytes: None,
            is_integrated: true,
        });
        let rec_vulkan = HardwareRecommendation::compute(&cpu, &mem, &gpu_vulkan);
        assert_eq!(rec_vulkan.gpu_layers_offload, 33);

        // CPU-only fallback
        let gpu_cpu = GpuProfile::cpu_only();
        let rec_cpu = HardwareRecommendation::compute(&cpu, &mem, &gpu_cpu);
        assert_eq!(rec_cpu.gpu_layers_offload, 0);
    }

    #[test]
    fn test_kv_cache_estimation() {
        // 2048 tokens, 28 layers, 128 embd/head
        let bytes_2k = MemoryProfile::estimate_kv_cache_bytes(2048, 28, 128);
        assert!(bytes_2k > 20 * MIB);
        assert!(bytes_2k < 500 * MIB);

        // 8192 tokens should scale linearly (4x)
        let bytes_8k = MemoryProfile::estimate_kv_cache_bytes(8192, 28, 128);
        assert_eq!(bytes_8k, bytes_2k * 4);
    }

    #[test]
    fn test_hardware_profile_serde_roundtrip() {
        let profile = HardwareProfile::detect();
        let serialized = serde_json::to_string_pretty(&profile).unwrap();
        assert!(!serialized.is_empty());

        let deserialized: HardwareProfile = serde_json::from_str(&serialized).unwrap();
        assert_eq!(profile.cpu.architecture, deserialized.cpu.architecture);
        assert_eq!(
            profile.recommendation.tier,
            deserialized.recommendation.tier
        );
    }
}
