#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use aissistant_llm::models::{GgufInspector, GGUF_MAGIC};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn test_arbitrary_bytes_gguf_fuzzing_never_panics(
        fuzz_data in prop::collection::vec(any::<u8>(), 0..8192)
    ) {
        // Must never panic or crash on any random byte sequences
        let _ = GgufInspector::inspect_bytes(&fuzz_data);
    }

    #[test]
    fn test_valid_magic_mutated_payload_never_panics(
        version in 0u32..5,
        tensor_count in any::<u64>(),
        metadata_kv_count in any::<u64>(),
        trailing_data in prop::collection::vec(any::<u8>(), 0..4096)
    ) {
        let mut buffer = Vec::with_capacity(24 + trailing_data.len());
        buffer.extend_from_slice(&GGUF_MAGIC);
        buffer.extend_from_slice(&version.to_le_bytes());
        buffer.extend_from_slice(&tensor_count.to_le_bytes());
        buffer.extend_from_slice(&metadata_kv_count.to_le_bytes());
        buffer.extend_from_slice(&trailing_data);

        // Even with partially valid headers, corrupted metadata should fail cleanly
        let _ = GgufInspector::inspect_bytes(&buffer);
    }

    #[test]
    fn test_short_buffer_fuzzing_never_panics(
        short_data in prop::collection::vec(any::<u8>(), 0..32)
    ) {
        let _ = GgufInspector::inspect_bytes(&short_data);
    }
}
