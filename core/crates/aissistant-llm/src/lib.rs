pub mod error;
pub mod hardware;
pub mod models;

pub use error::{LlmError, Result};
pub use hardware::{
    CpuFeatures, GpuBackend, GpuDevice, GpuProfile, HardwareProfile, HardwareProfiler,
    HardwareRecommendation, MemoryProfile, ModelTier, SystemHardwareProfiler,
};
pub use models::{
    DownloadProgress, DownloadStatus, GgufInspector, GgufMetadata, GgufType, LocalModelDownloader,
    ModelCatalog, ModelDescriptor, ModelDownloader, QuantizationType, Sha256Verifier, GGUF_MAGIC,
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

    #[test]
    fn test_catalog_lookup_and_tier_filtering() {
        let all = ModelCatalog::all_models();
        assert!(all.len() >= 6);

        // Find by exact ID
        let qwen_nano = ModelCatalog::find_by_id("qwen2.5-0.5b-instruct-q4_k_m");
        assert!(qwen_nano.is_some());
        let m = qwen_nano.unwrap();
        assert_eq!(m.tier, ModelTier::Nano0_5B);
        assert_eq!(m.quantization, QuantizationType::Q4_K_M);
        assert!(!m.download_urls.is_empty());

        // Filter by tier
        let small_models = ModelCatalog::filter_by_tier(ModelTier::Small1_5B_3B);
        assert!(small_models.len() >= 2);
        assert!(small_models.iter().any(|m| m.id.contains("llama-3.2-3b")));

        // Recommended default for tier
        let rec = ModelCatalog::recommended_for_tier(ModelTier::Medium7B_8B);
        assert!(rec.is_some());
        assert_eq!(rec.unwrap().tier, ModelTier::Medium7B_8B);
    }

    #[test]
    fn test_sha256_verifier_streaming_and_file() {
        // Standard test vector 1: empty string
        let v_empty = Sha256Verifier::new();
        assert_eq!(
            v_empty.finalize_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        // Standard test vector 2: "hello world"
        let mut v_hello = Sha256Verifier::new();
        v_hello.update(b"hello world");
        let hash_hello = v_hello.finalize_hex();
        assert_eq!(
            hash_hello,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );

        // File-based verification with temporary file
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_model.bin");
        std::fs::write(&file_path, b"mock-model-weights-payload").unwrap();

        let computed = Sha256Verifier::compute_file_hash(&file_path).unwrap();
        assert!(!computed.is_empty());

        // Verify match
        let valid = Sha256Verifier::verify_file(&file_path, "test_model", &computed).unwrap();
        assert!(valid);

        // Tamper detection: altered expected hash must return ChecksumMismatch error
        let bad_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let res = Sha256Verifier::verify_file(&file_path, "test_model", bad_hash);
        assert!(matches!(res, Err(LlmError::ChecksumMismatch { .. })));
    }

    fn create_synthetic_gguf(arch: &str, name: &str, ctx: u64, embd: u64, blocks: u64) -> Vec<u8> {
        use byteorder::{LittleEndian, WriteBytesExt};
        let mut buf = Vec::new();

        // 1. Magic
        buf.extend_from_slice(&GGUF_MAGIC);
        // 2. Version 3
        buf.write_u32::<LittleEndian>(3).unwrap();
        // 3. Tensor count
        buf.write_u64::<LittleEndian>(140).unwrap();
        // 4. KV count: 5 pairs
        buf.write_u64::<LittleEndian>(5).unwrap();

        // KV 1: general.architecture (String)
        let key1 = "general.architecture";
        buf.write_u64::<LittleEndian>(key1.len() as u64).unwrap();
        buf.extend_from_slice(key1.as_bytes());
        buf.write_u32::<LittleEndian>(GgufType::String as u32)
            .unwrap();
        buf.write_u64::<LittleEndian>(arch.len() as u64).unwrap();
        buf.extend_from_slice(arch.as_bytes());

        // KV 2: general.name (String)
        let key2 = "general.name";
        buf.write_u64::<LittleEndian>(key2.len() as u64).unwrap();
        buf.extend_from_slice(key2.as_bytes());
        buf.write_u32::<LittleEndian>(GgufType::String as u32)
            .unwrap();
        buf.write_u64::<LittleEndian>(name.len() as u64).unwrap();
        buf.extend_from_slice(name.as_bytes());

        // KV 3: <arch>.context_length (Uint64)
        let key3 = format!("{arch}.context_length");
        buf.write_u64::<LittleEndian>(key3.len() as u64).unwrap();
        buf.extend_from_slice(key3.as_bytes());
        buf.write_u32::<LittleEndian>(GgufType::Uint64 as u32)
            .unwrap();
        buf.write_u64::<LittleEndian>(ctx).unwrap();

        // KV 4: <arch>.embedding_length (Uint64)
        let key4 = format!("{arch}.embedding_length");
        buf.write_u64::<LittleEndian>(key4.len() as u64).unwrap();
        buf.extend_from_slice(key4.as_bytes());
        buf.write_u32::<LittleEndian>(GgufType::Uint64 as u32)
            .unwrap();
        buf.write_u64::<LittleEndian>(embd).unwrap();

        // KV 5: <arch>.block_count (Uint64)
        let key5 = format!("{arch}.block_count");
        buf.write_u64::<LittleEndian>(key5.len() as u64).unwrap();
        buf.extend_from_slice(key5.as_bytes());
        buf.write_u32::<LittleEndian>(GgufType::Uint64 as u32)
            .unwrap();
        buf.write_u64::<LittleEndian>(blocks).unwrap();

        buf
    }

    #[test]
    fn test_gguf_inspector_valid_v3_synthetic_header() {
        let payload = create_synthetic_gguf("qwen2", "Qwen2.5-0.5B-Instruct", 4096, 896, 24);
        let metadata = GgufInspector::inspect_bytes(&payload).unwrap();

        assert_eq!(metadata.version, 3);
        assert_eq!(metadata.tensor_count, 140);
        assert_eq!(metadata.kv_count, 5);
        assert_eq!(metadata.architecture.as_deref(), Some("qwen2"));
        assert_eq!(
            metadata.model_name.as_deref(),
            Some("Qwen2.5-0.5B-Instruct")
        );
        assert_eq!(metadata.context_length, Some(4096));
        assert_eq!(metadata.embedding_length, Some(896));
        assert_eq!(metadata.block_count, Some(24));
    }

    #[test]
    fn test_gguf_inspector_corrupt_files_rejection() {
        // Corrupt magic bytes
        let mut bad_magic = create_synthetic_gguf("qwen2", "test", 2048, 512, 12);
        bad_magic[0] = b'X';
        bad_magic[1] = b'Y';
        assert!(matches!(
            GgufInspector::inspect_bytes(&bad_magic),
            Err(LlmError::GgufParseError(_))
        ));

        // Truncated file (less than 4 bytes)
        assert!(matches!(
            GgufInspector::inspect_bytes(b"GG"),
            Err(LlmError::GgufParseError(_))
        ));

        // Unsupported version (e.g. version 1)
        use byteorder::{LittleEndian, WriteBytesExt};
        let mut bad_version = create_synthetic_gguf("qwen2", "test", 2048, 512, 12);
        // Overwrite version field (offset 4)
        (&mut bad_version[4..8])
            .write_u32::<LittleEndian>(1)
            .unwrap();
        assert!(matches!(
            GgufInspector::inspect_bytes(&bad_version),
            Err(LlmError::GgufParseError(_))
        ));
    }

    #[test]
    fn test_local_model_downloader_atomic_ingestion_and_resumability() {
        let dir = tempfile::tempdir().unwrap();
        let downloader = LocalModelDownloader::new();

        // Generate synthetic payload
        let payload = b"header-and-weights-binary-payload-for-local-model";
        let mut verifier = Sha256Verifier::new();
        verifier.update(payload);
        let sha256 = verifier.finalize_hex();

        let model = ModelDescriptor {
            id: "test-model-nano".to_string(),
            name: "Test Model Nano".to_string(),
            tier: ModelTier::Nano0_5B,
            parameter_count: "0.1B".to_string(),
            quantization: QuantizationType::Q4_K_M,
            file_size_bytes: payload.len() as u64,
            sha256: sha256.clone(),
            default_context_window: 2048,
            license: "Apache-2.0".to_string(),
            download_urls: vec!["https://example.com/test.gguf".to_string()],
            description: "Test synthetic model".to_string(),
        };

        let mut progress_events = Vec::new();
        let mut on_progress = |p| progress_events.push(p);
        let target_file = downloader
            .ingest_model(&model, dir.path(), payload, Some(&mut on_progress))
            .unwrap();

        assert!(target_file.exists());
        assert_eq!(
            target_file.extension().and_then(|s| s.to_str()),
            Some("gguf")
        );
        // .part file must no longer exist after atomic rename
        assert!(!dir.path().join("test-model-nano.gguf.part").exists());
        assert!(!progress_events.is_empty());
        assert_eq!(
            progress_events.last().unwrap().status,
            DownloadStatus::Completed
        );

        // Corrupted payload must be rejected and .part deleted
        let corrupt_payload = b"tampered-payload";
        let corrupt_res = downloader.ingest_model(&model, dir.path(), corrupt_payload, None);
        assert!(matches!(
            corrupt_res,
            Err(LlmError::ChecksumMismatch { .. })
        ));
        assert!(!dir.path().join("test-model-nano.gguf.part").exists());

        // Test resume offset computation
        let part_file = dir.path().join("resume-test.part");
        std::fs::write(&part_file, b"0123456789").unwrap();
        let offset = downloader.compute_resume_offset(&part_file);
        assert_eq!(offset, 10);
    }
}
