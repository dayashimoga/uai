//! Local dense embedding generation and vector similarity math.

use crate::error::Result;

/// Default embedding dimension for lightweight local sentence models.
pub const DEFAULT_EMBEDDING_DIMENSION: usize = 384;

/// Trait defining offline dense embedding extraction from text.
pub trait EmbeddingEngine: Send + Sync {
    /// Extracts a normalized dense embedding vector for the given text.
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Batch embeds multiple text strings sequentially.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// The fixed vector dimension produced by this embedding engine.
    fn dimension(&self) -> usize;
}

/// Computes the Euclidean (L2) norm of a float vector.
#[must_use]
pub fn l2_norm(vector: &[f32]) -> f32 {
    let sum_sq: f32 = vector.iter().map(|x| x * x).sum();
    sum_sq.sqrt()
}

/// In-place scales a float vector to unit length (Euclidean L2 norm = 1.0).
pub fn l2_normalize(vector: &mut [f32]) {
    let norm = l2_norm(vector);
    if norm > 1e-12 {
        let inv_norm = 1.0 / norm;
        for x in vector.iter_mut() {
            *x *= inv_norm;
        }
    }
}

/// Computes the cosine similarity between two float vectors.
///
/// Returns a value in the range `[-1.0, 1.0]`. If either vector has near-zero magnitude
/// or their dimensions do not match, returns `0.0`.
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a = l2_norm(a);
    let norm_b = l2_norm(b);

    if norm_a < 1e-12 || norm_b < 1e-12 {
        return 0.0;
    }

    let similarity = dot / (norm_a * norm_b);
    similarity.clamp(-1.0, 1.0)
}

/// Deterministic, offline local embedding engine producing normalized dense vectors.
///
/// Uses feature hashing over word tokens and character n-grams (3-grams, 4-grams)
/// with signed random projections and inverse position weighting, producing
/// L2-normalized vectors compatible with cosine similarity indexing.
#[derive(Debug, Clone)]
pub struct LocalEmbeddingEngine {
    dimension: usize,
}

impl Default for LocalEmbeddingEngine {
    fn default() -> Self {
        Self::new(DEFAULT_EMBEDDING_DIMENSION)
    }
}

impl LocalEmbeddingEngine {
    /// Creates a new local embedding engine with the specified vector dimension.
    #[must_use]
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension: dimension.max(16),
        }
    }

    /// Hashes a token string into an index within `[0, dimension)`.
    fn hash_to_index(&self, token: &str, salt: u32) -> usize {
        let mut h: u32 = 0x811c9dc5 ^ salt;
        for byte in token.bytes() {
            h ^= u32::from(byte);
            h = h.wrapping_mul(0x01000193);
        }
        (h as usize) % self.dimension
    }

    /// Computes a +1 or -1 sign for random projection feature hashing.
    fn hash_to_sign(&self, token: &str, salt: u32) -> f32 {
        let mut h: u32 = 0x9e3779b9 ^ salt;
        for byte in token.bytes() {
            h ^= u32::from(byte);
            h = h.wrapping_mul(0x85ebca6b);
        }
        if (h & 1) == 0 {
            1.0
        } else {
            -1.0
        }
    }
}

impl EmbeddingEngine for LocalEmbeddingEngine {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut vector = vec![0.0f32; self.dimension];
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(vector);
        }

        let words: Vec<&str> = trimmed
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|w| !w.is_empty())
            .collect();

        if words.is_empty() {
            return Ok(vector);
        }

        let total_words = words.len() as f32;

        for (i, word) in words.iter().enumerate() {
            let lower = word.to_ascii_lowercase();
            // Position discount (earlier tokens hold slightly higher weight)
            let pos_weight = 1.0 + (1.0 - (i as f32 / total_words)) * 0.5;

            // 1. Whole word projection
            let idx1 = self.hash_to_index(&lower, 0x1234);
            let sign1 = self.hash_to_sign(&lower, 0x1234);
            vector[idx1] += sign1 * pos_weight * 2.0;

            let idx2 = self.hash_to_index(&lower, 0x5678);
            let sign2 = self.hash_to_sign(&lower, 0x5678);
            vector[idx2] += sign2 * pos_weight * 1.5;

            // 2. Character n-grams (3-grams and 4-grams for subword similarity)
            let chars: Vec<char> = lower.chars().collect();
            if chars.len() >= 3 {
                for window_size in [3, 4] {
                    if chars.len() >= window_size {
                        for window in chars.windows(window_size) {
                            let ngram: String = window.iter().collect();
                            let ngram_idx = self.hash_to_index(&ngram, 0x9abc);
                            let ngram_sign = self.hash_to_sign(&ngram, 0x9abc);
                            vector[ngram_idx] += ngram_sign * pos_weight * 0.75;
                        }
                    }
                }
            }
        }

        // L2-normalize to unit sphere
        l2_normalize(&mut vector);

        Ok(vector)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_norm_and_normalization() {
        let mut v = vec![3.0, 4.0];
        assert!((l2_norm(&v) - 5.0).abs() < 1e-6);

        l2_normalize(&mut v);
        assert!((l2_norm(&v) - 1.0).abs() < 1e-6);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);

        // Near-zero vector normalization does not panic or divide by zero
        let mut zero_vec = vec![0.0, 0.0, 0.0];
        l2_normalize(&mut zero_vec);
        assert_eq!(zero_vec, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_cosine_similarity_properties() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];
        let d = vec![-1.0, 0.0, 0.0];

        // Identical vectors = 1.0
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        // Orthogonal vectors = 0.0
        assert!(cosine_similarity(&a, &c).abs() < 1e-6);

        // Opposite vectors = -1.0
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 1e-6);

        // Dimension mismatch returns 0.0
        let shorter = vec![1.0, 0.0];
        assert_eq!(cosine_similarity(&a, &shorter), 0.0);

        // Empty vectors return 0.0
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn test_local_embedding_engine_dimension_and_determinism() {
        let engine = LocalEmbeddingEngine::new(384);
        assert_eq!(engine.dimension(), 384);

        let v1 = engine.embed("Local AI Assistant architecture").unwrap();
        assert_eq!(v1.len(), 384);
        assert!((l2_norm(&v1) - 1.0).abs() < 1e-5);

        // Identical text must produce identical vector
        let v2 = engine.embed("Local AI Assistant architecture").unwrap();
        assert_eq!(v1, v2);

        // Cosine similarity of identical text is 1.0
        assert!((cosine_similarity(&v1, &v2) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_local_embedding_engine_semantic_relatedness() {
        let engine = LocalEmbeddingEngine::default();

        let v_rust1 = engine
            .embed("Writing concurrent Rust code with async tokio")
            .unwrap();
        let v_rust2 = engine
            .embed("Concurrent systems programming in modern Rust")
            .unwrap();
        let v_cooking = engine
            .embed("Delicious homemade Italian pizza dough recipe")
            .unwrap();

        let sim_rust = cosine_similarity(&v_rust1, &v_rust2);
        let sim_diff = cosine_similarity(&v_rust1, &v_cooking);

        // Rust texts must be significantly more similar than Rust vs Cooking
        assert!(
            sim_rust > 0.4,
            "Expected high similarity between related Rust texts, got {sim_rust}"
        );
        assert!(
            sim_diff < 0.2,
            "Expected low similarity between unrelated texts, got {sim_diff}"
        );
        assert!(sim_rust > sim_diff);
    }

    #[test]
    fn test_batch_embedding() {
        let engine = LocalEmbeddingEngine::new(128);
        let texts = [
            "Hello world",
            "Offline speech recognition",
            "Machine learning",
        ];
        let batch = engine.embed_batch(&texts).unwrap();

        assert_eq!(batch.len(), 3);
        for v in &batch {
            assert_eq!(v.len(), 128);
            assert!((l2_norm(v) - 1.0).abs() < 1e-5);
        }
    }
}
