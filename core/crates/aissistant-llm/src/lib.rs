pub mod embeddings;
pub mod engine;
pub mod error;
pub mod grammar;
pub mod hardware;
pub mod models;
pub mod ocr;
pub mod translation;
pub mod vision;

pub use embeddings::{
    cosine_similarity, l2_norm, l2_normalize, EmbeddingEngine, LocalEmbeddingEngine,
    DEFAULT_EMBEDDING_DIMENSION,
};
pub use engine::{
    FinishReason, InferenceMetrics, LlmCancellationHandle, LlmEngine, LlmEngineConfig, LlmSession,
    LocalLlmEngine, SamplingParams, TokenChunk,
};
pub use error::{LlmError, Result};
pub use grammar::{
    GbnfGrammar, GbnfRule, SchemaCompiler, ToolCall, ToolCallValidator, ToolDefinition,
};
pub use hardware::{
    CpuFeatures, GpuBackend, GpuDevice, GpuProfile, HardwareProfile, HardwareProfiler,
    HardwareRecommendation, MemoryProfile, ModelTier, SystemHardwareProfiler,
};
pub use models::{
    query_model_lifecycle, register_custom_gguf, DownloadProgress, DownloadStatus, GgufInspector,
    GgufMetadata, GgufType, LocalModelDownloader, ModelCatalog, ModelDescriptor,
    ModelDownloadManager, ModelDownloader, ModelLifecycleState, QuantizationType, Sha256Verifier,
    GGUF_MAGIC,
};
pub use ocr::{OcrBoundingBox, OcrDocument, OcrTextLine, OcrWord, OfflineOcrEngine};
pub use translation::{
    Language, LanguagePair, TranslationEngine, TranslationPromptTemplate, TranslationResult,
};
pub use vision::{
    ImageBuffer, ImageFormat, ImageMetadata, ImagePatch, VisionPreprocessor,
    VisionPreprocessorConfig,
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

    #[test]
    fn test_sampling_params_validation() {
        let default_params = SamplingParams::default();
        assert!(default_params.validate().is_ok());

        // Invalid temperature (< 0 or > 2.0)
        let mut bad_temp = default_params.clone();
        bad_temp.temperature = -0.1;
        assert!(bad_temp.validate().is_err());
        bad_temp.temperature = 2.5;
        assert!(bad_temp.validate().is_err());

        // Invalid top_p (<= 0 or > 1.0)
        let mut bad_top_p = default_params.clone();
        bad_top_p.top_p = 0.0;
        assert!(bad_top_p.validate().is_err());
        bad_top_p.top_p = 1.1;
        assert!(bad_top_p.validate().is_err());

        // Invalid top_k (== 0)
        let mut bad_top_k = default_params.clone();
        bad_top_k.top_k = 0;
        assert!(bad_top_k.validate().is_err());

        // Invalid repetition penalty (< 0.5 or > 3.0)
        let mut bad_rep = default_params.clone();
        bad_rep.repetition_penalty = 0.2;
        assert!(bad_rep.validate().is_err());
        bad_rep.repetition_penalty = 3.5;
        assert!(bad_rep.validate().is_err());

        // Invalid max_tokens (== 0)
        let mut bad_tokens = default_params;
        bad_tokens.max_tokens = 0;
        assert!(bad_tokens.validate().is_err());
    }

    #[test]
    fn test_llm_session_history_and_templates() {
        let mut session = LlmSession::new("test-session-001", 128);
        assert_eq!(session.session_id, "test-session-001");
        assert_eq!(session.context_window_limit, 128);
        assert_eq!(session.prefix_hash, 0);
        assert!(!session.is_context_exceeded());

        session.add_turn("user", "Hello assistant, what is your purpose?");
        assert_ne!(session.prefix_hash, 0);
        let hash_after_turn1 = session.prefix_hash;

        session.add_turn("assistant", "I am your offline universal AI assistant.");
        assert_ne!(session.prefix_hash, hash_after_turn1);

        // ChatML formatting
        let chatml = session.format_chatml_prompt(Some("You are a helpful assistant."));
        assert!(chatml.contains("<|im_start|>system\nYou are a helpful assistant.<|im_end|>"));
        assert!(
            chatml.contains("<|im_start|>user\nHello assistant, what is your purpose?<|im_end|>")
        );
        assert!(chatml.contains(
            "<|im_start|>assistant\nI am your offline universal AI assistant.<|im_end|>"
        ));
        assert!(chatml.ends_with("<|im_start|>assistant\n"));

        // Llama 3 formatting
        let llama3 = session.format_llama3_prompt(Some("You are a helpful assistant."));
        assert!(llama3.starts_with("<|begin_of_text|>"));
        assert!(llama3.contains(
            "<|start_header_id|>system<|end_header_id|>\n\nYou are a helpful assistant.<|eot_id|>"
        ));
        assert!(llama3.contains("<|start_header_id|>user<|end_header_id|>\n\nHello assistant, what is your purpose?<|eot_id|>"));
        assert!(llama3.ends_with("<|start_header_id|>assistant<|end_header_id|>\n\n"));

        // Token count estimation
        assert!(session.estimate_token_count() > 10);
        assert!(!session.is_context_exceeded());

        // Clear resets history and hash
        session.clear();
        assert!(session.history_turns.is_empty());
        assert_eq!(session.prefix_hash, 0);
    }

    #[test]
    fn test_local_llm_engine_lifecycle_and_streaming() {
        let mut engine = LocalLlmEngine::new();
        assert!(!engine.is_loaded());
        assert_eq!(engine.active_model_id(), None);
        assert_eq!(engine.active_config(), None);

        // Cannot generate without loading
        let params = SamplingParams::default();
        let mut chunks = Vec::new();
        let res = engine.generate_stream("Test prompt", &params, None, &mut |chunk| {
            chunks.push(chunk);
            Ok(())
        });
        assert!(res.is_err());

        // Load valid configuration
        let config = LlmEngineConfig {
            model_id: "qwen2.5-0.5b-instruct-q4_k_m".to_string(),
            model_path: std::path::PathBuf::from("/models/qwen2.5-0.5b.gguf"),
            n_gpu_layers: 0,
            context_length: 2048,
            threads: 4,
            memory_budget_bytes: 1024 * 1024 * 1024,
        };
        assert!(engine.load_model(config.clone()).is_ok());
        assert!(engine.is_loaded());
        assert_eq!(
            engine.active_model_id(),
            Some("qwen2.5-0.5b-instruct-q4_k_m")
        );
        assert_eq!(engine.active_config(), Some(&config));

        // Generate streaming tokens
        let mut received_tokens = Vec::new();
        let metrics = engine
            .generate_stream("Explain offline AI", &params, None, &mut |chunk| {
                received_tokens.push(chunk);
                Ok(())
            })
            .unwrap();

        assert!(!received_tokens.is_empty());
        assert!(metrics.generated_tokens > 0);
        assert!(metrics.prompt_tokens > 0);
        assert!(metrics.tokens_per_second >= 0.0);

        // Verify last chunk has is_final == true
        let last_chunk = received_tokens.last().unwrap();
        assert!(last_chunk.is_final);
        assert!(matches!(
            last_chunk.finish_reason,
            Some(FinishReason::Length) | Some(FinishReason::Stop)
        ));

        // Unload model
        engine.unload_model();
        assert!(!engine.is_loaded());
        assert_eq!(engine.active_model_id(), None);
    }

    #[test]
    fn test_cooperative_cancellation_latency() {
        let mut engine = LocalLlmEngine::new();
        let config = LlmEngineConfig {
            model_id: "test-model".to_string(),
            model_path: std::path::PathBuf::from("/models/test.gguf"),
            n_gpu_layers: 0,
            context_length: 2048,
            threads: 2,
            memory_budget_bytes: 512 * 1024 * 1024,
        };
        engine.load_model(config).unwrap();

        let cancel_handle = LlmCancellationHandle::new();
        assert!(!cancel_handle.is_cancelled());

        // Pre-cancel to verify immediate interruption
        cancel_handle.cancel();
        assert!(cancel_handle.is_cancelled());

        let params = SamplingParams::default();
        let mut received_tokens = Vec::new();
        let metrics = engine
            .generate_stream(
                "Cancel immediately",
                &params,
                Some(&cancel_handle),
                &mut |chunk| {
                    received_tokens.push(chunk);
                    Ok(())
                },
            )
            .unwrap();

        assert_eq!(received_tokens.len(), 1);
        assert!(received_tokens[0].is_final);
        assert_eq!(
            received_tokens[0].finish_reason,
            Some(FinishReason::Cancelled)
        );
        assert_eq!(metrics.generated_tokens, 0);

        // Reset handle for reuse
        cancel_handle.reset();
        assert!(!cancel_handle.is_cancelled());
    }

    #[test]
    fn test_stop_sequence_interruption() {
        let mut engine = LocalLlmEngine::new();
        let config = LlmEngineConfig {
            model_id: "test-model".to_string(),
            model_path: std::path::PathBuf::from("/models/test.gguf"),
            n_gpu_layers: 0,
            context_length: 2048,
            threads: 2,
            memory_budget_bytes: 512 * 1024 * 1024,
        };
        engine.load_model(config).unwrap();

        // Sample sentence is: "The Universal AI Assistant operates completely offline directly on your device . ..."
        // Add "device" as a stop sequence
        let params = SamplingParams {
            stop_sequences: vec!["device".to_string()],
            ..Default::default()
        };

        let mut received_tokens = Vec::new();
        let metrics = engine
            .generate_stream("Stop at device", &params, None, &mut |chunk| {
                received_tokens.push(chunk);
                Ok(())
            })
            .unwrap();

        assert!(!received_tokens.is_empty());
        let last_chunk = received_tokens.last().unwrap();
        assert!(last_chunk.is_final);
        assert_eq!(last_chunk.finish_reason, Some(FinishReason::Stop));
        assert!(metrics.generated_tokens < 24);
    }

    #[test]
    fn test_memory_pressure_auto_unload() {
        let mut engine = LocalLlmEngine::new();
        let config = LlmEngineConfig {
            model_id: "heavy-model".to_string(),
            model_path: std::path::PathBuf::from("/models/heavy.gguf"),
            n_gpu_layers: 32,
            context_length: 4096,
            threads: 8,
            memory_budget_bytes: 4 * 1024 * 1024 * 1024,
        };
        engine.load_model(config).unwrap();
        assert!(engine.is_loaded());

        // Non-critical pressure does not unload
        engine.on_memory_pressure(false);
        assert!(engine.is_loaded());

        // Critical pressure triggers immediate unload
        engine.on_memory_pressure(true);
        assert!(!engine.is_loaded());
    }

    #[test]
    fn test_gbnf_grammar_rule_formatting_and_validation() {
        let mut grammar = GbnfGrammar::new();
        assert!(grammar.validate().is_err()); // Missing root

        grammar.set_root("ws item ws");
        grammar.add_rule("item", "\"apple\" | \"orange\"");
        grammar.add_rule("ws", "[ \\t]*");

        assert!(grammar.validate().is_ok());
        let gbnf_str = grammar.to_gbnf_string();
        assert!(gbnf_str.starts_with("root ::= ws item ws\n"));
        assert!(gbnf_str.contains("item ::= \"apple\" | \"orange\"\n"));

        // Built-in arbitrary JSON objects & booleans
        let obj_grammar = GbnfGrammar::arbitrary_json_object();
        assert!(obj_grammar.validate().is_ok());
        assert!(obj_grammar.has_rule("json_object"));
        assert!(obj_grammar.has_rule("root"));

        let bool_grammar = GbnfGrammar::boolean_response();
        assert!(bool_grammar.validate().is_ok());

        // Escape helper
        assert_eq!(
            GbnfGrammar::escape_literal("hello \"world\"\n"),
            "\"hello \\\"world\\\"\\n\""
        );
    }

    #[test]
    fn test_schema_compiler_primitive_and_enum_types() {
        let mut compiler = SchemaCompiler::new();

        // Enum string schema
        let enum_schema = serde_json::json!({
            "type": "string",
            "enum": ["high", "medium", "low"]
        });

        let grammar = compiler.compile_schema(&enum_schema).unwrap();
        assert!(grammar.validate().is_ok());
        let gbnf = grammar.to_gbnf_string();
        assert!(gbnf.contains("\"high\" | \"medium\" | \"low\""));
    }

    #[test]
    fn test_schema_compiler_nested_object_and_array() {
        let mut compiler = SchemaCompiler::new();

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "user_id": { "type": "integer" },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "active": { "type": "boolean" }
            },
            "required": ["user_id", "tags"]
        });

        let grammar = compiler.compile_schema(&schema).unwrap();
        assert!(grammar.validate().is_ok());
        let gbnf = grammar.to_gbnf_string();
        assert!(gbnf.contains("root ::="));
        assert!(gbnf.contains("\"user_id\""));
        assert!(gbnf.contains("\"tags\""));
        assert!(gbnf.contains("\"active\""));
    }

    #[test]
    fn test_schema_compiler_single_and_multi_tool() {
        let mut compiler = SchemaCompiler::new();

        let tool1 = ToolDefinition::new(
            "get_battery_level",
            "Queries host device battery level",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "refresh": { "type": "boolean" }
                },
                "required": ["refresh"]
            }),
        );

        let tool2 = ToolDefinition::new(
            "file_read",
            "Reads a file securely",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
        );

        // Single tool compilation
        let single_grammar = compiler.compile_tool(&tool1).unwrap();
        assert!(single_grammar.validate().is_ok());
        let gbnf_single = single_grammar.to_gbnf_string();
        assert!(gbnf_single.contains("get_battery_level"));
        assert!(gbnf_single.contains(r#"\"arguments\":"#));

        // Multi tool with conversational fallback
        let multi_grammar = compiler.compile_tools(&[tool1, tool2], true).unwrap();
        assert!(multi_grammar.validate().is_ok());
        let gbnf_multi = multi_grammar.to_gbnf_string();
        assert!(gbnf_multi.contains("tool_get_battery_level"));
        assert!(gbnf_multi.contains("tool_file_read"));
        assert!(gbnf_multi.contains("conversational_text"));
    }

    #[test]
    fn test_tool_call_validator_success_and_failures() {
        let tools = vec![ToolDefinition::new(
            "adjust_volume",
            "Modifies system output volume",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "level": { "type": "integer" },
                    "mode": {
                        "type": "string",
                        "enum": ["mute", "unmute", "set"]
                    }
                },
                "required": ["level", "mode"]
            }),
        )];

        // Valid invocation
        let valid_json = r#"{
            "name": "adjust_volume",
            "arguments": {
                "level": 75,
                "mode": "set"
            }
        }"#;
        let call = ToolCallValidator::parse_and_validate(valid_json, &tools).unwrap();
        assert_eq!(call.name, "adjust_volume");
        assert_eq!(call.arguments["level"], 75);
        assert_eq!(call.arguments["mode"], "set");

        // Missing required property 'mode'
        let missing_prop = r#"{
            "name": "adjust_volume",
            "arguments": {
                "level": 50
            }
        }"#;
        assert!(ToolCallValidator::parse_and_validate(missing_prop, &tools).is_err());

        // Type mismatch ('level' as string instead of integer)
        let type_mismatch = r#"{
            "name": "adjust_volume",
            "arguments": {
                "level": "loud",
                "mode": "set"
            }
        }"#;
        assert!(ToolCallValidator::parse_and_validate(type_mismatch, &tools).is_err());

        // Enum mismatch ('mode' not in ["mute", "unmute", "set"])
        let enum_mismatch = r#"{
            "name": "adjust_volume",
            "arguments": {
                "level": 50,
                "mode": "blast"
            }
        }"#;
        assert!(ToolCallValidator::parse_and_validate(enum_mismatch, &tools).is_err());

        // Unknown tool
        let unknown_tool = r#"{
            "name": "non_existent_tool",
            "arguments": {}
        }"#;
        assert!(ToolCallValidator::parse_and_validate(unknown_tool, &tools).is_err());

        // Malformed JSON
        let malformed = "not valid json";
        assert!(ToolCallValidator::parse_and_validate(malformed, &tools).is_err());
    }

    #[test]
    fn test_register_custom_gguf_and_catalog() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"GGUF_CUSTOM_BYTES_HEADER").unwrap();

        let desc = register_custom_gguf(tmp.path()).unwrap();
        assert!(desc.id.starts_with("custom-"));
        assert_eq!(desc.license, "Custom Local");
        assert_eq!(desc.file_size_bytes, 24);

        let all = ModelCatalog::all_models();
        assert!(all.iter().any(|m| m.id == desc.id));

        ModelCatalog::remove_custom_model(&desc.id);
        let all_after = ModelCatalog::all_models();
        assert!(!all_after.iter().any(|m| m.id == desc.id));
    }

    #[test]
    fn test_query_model_lifecycle_states() {
        let temp_dir = tempfile::tempdir().unwrap();
        let models = ModelCatalog::curated_models();
        let model = &models[0];

        // 1. NotDownloaded
        let state1 = query_model_lifecycle(temp_dir.path(), model, false, false);
        assert!(matches!(state1, ModelLifecycleState::NotDownloaded { .. }));

        // 2. Active in memory
        let state_active = query_model_lifecycle(temp_dir.path(), model, true, false);
        assert!(matches!(
            state_active,
            ModelLifecycleState::LoadedActive { .. }
        ));

        // 3. Downloading (.part file exists)
        let part_file = temp_dir.path().join(format!("{}.gguf.part", model.id));
        std::fs::write(&part_file, b"partial").unwrap();
        let state_dl = query_model_lifecycle(temp_dir.path(), model, false, true);
        assert!(matches!(state_dl, ModelLifecycleState::Downloading { .. }));

        // 4. Downloaded on disk (.gguf exists)
        let final_file = temp_dir.path().join(format!("{}.gguf", model.id));
        std::fs::write(&final_file, b"completed").unwrap();
        let state_disk = query_model_lifecycle(temp_dir.path(), model, false, false);
        assert!(matches!(
            state_disk,
            ModelLifecycleState::DownloadedOnDisk { .. }
        ));
    }

    #[test]
    fn test_model_download_manager_cancellation_and_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dm = ModelDownloadManager::new(temp_dir.path());
        assert_eq!(dm.models_dir(), temp_dir.path());
        assert!(!dm.is_downloading("test-model"));
        assert!(!dm.cancel_download("test-model"));
    }
}
