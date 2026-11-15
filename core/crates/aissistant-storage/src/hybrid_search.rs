//! Reciprocal Rank Fusion (RRF) hybrid search combining SQLite FTS5 BM25 and dense vector similarity.

use std::collections::HashMap;

use aissistant_core::{ConversationId, TurnId};
use serde::{Deserialize, Serialize};

use crate::conversation_store::ConversationStore;
use crate::error::StorageError;
use crate::vector_store::VectorMemoryStore;

/// Hyperparameters for Reciprocal Rank Fusion (RRF).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RrfConfig {
    /// Smoothing constant (standard default is 60.0).
    pub k: f64,
    /// Weight applied to the BM25 keyword ranker.
    pub bm25_weight: f64,
    /// Weight applied to the dense vector cosine ranker.
    pub vector_weight: f64,
}

impl Default for RrfConfig {
    fn default() -> Self {
        Self {
            k: 60.0,
            bm25_weight: 1.0,
            vector_weight: 1.0,
        }
    }
}

/// Unified search result item combining sparse BM25 and dense vector rank signals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HybridSearchResult {
    /// Turn identifier for the matching conversation turn.
    pub turn_id: TurnId,
    /// Parent conversation identifier.
    pub conversation_id: ConversationId,
    /// Reciprocal Rank Fusion score (higher indicates superior combined relevance).
    pub rrf_score: f64,
    /// 1-based rank from SQLite FTS5 BM25 search (if matched).
    pub bm25_rank: Option<usize>,
    /// 1-based rank from dense vector cosine search (if matched).
    pub vector_rank: Option<usize>,
    /// Cosine similarity from dense vector search (if matched).
    pub cosine_similarity: Option<f32>,
    /// Text content of the turn.
    pub content: String,
    /// Highlight snippet (with `<mark>` tags if matched by BM25).
    pub snippet: String,
}

/// Intermediate candidate accumulator during reciprocal rank fusion.
struct CandidateAccumulator {
    turn_id: TurnId,
    conversation_id: ConversationId,
    rrf_score: f64,
    bm25_rank: Option<usize>,
    vector_rank: Option<usize>,
    cosine_similarity: Option<f32>,
    content: String,
    snippet: String,
}

/// Hybrid search engine combining SQLite FTS5 episodic keyword search with dense vector similarity.
#[derive(Debug, Clone)]
pub struct HybridSearchEngine {
    conversation_store: ConversationStore,
    vector_store: VectorMemoryStore,
}

impl HybridSearchEngine {
    /// Creates a new `HybridSearchEngine` wrapping conversation and vector stores.
    #[must_use]
    pub fn new(conversation_store: ConversationStore, vector_store: VectorMemoryStore) -> Self {
        Self {
            conversation_store,
            vector_store,
        }
    }

    /// Performs hybrid search using Reciprocal Rank Fusion.
    pub fn search(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        limit: usize,
        config: Option<RrfConfig>,
        conversation_id: Option<&ConversationId>,
    ) -> Result<Vec<HybridSearchResult>, StorageError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let cfg = config.unwrap_or_default();
        let candidate_pool_limit = limit.max(25);

        // 1. Retrieve BM25 keyword candidates from FTS5
        let bm25_candidates = if let Some(cid) = conversation_id {
            self.conversation_store
                .search_episodic_memory_in_conversation(cid, query_text, candidate_pool_limit)?
        } else {
            self.conversation_store
                .search_episodic_memory(query_text, candidate_pool_limit)?
        };

        // 2. Retrieve dense vector candidates via cosine similarity
        let cid_str = conversation_id.map(|c| c.as_str());
        let vector_candidates = if !query_embedding.is_empty() {
            self.vector_store.search_vector_similarity(
                query_embedding,
                candidate_pool_limit,
                0.0,
                cid_str,
            )?
        } else {
            Vec::new()
        };

        // 3. Fuse candidates using Reciprocal Rank Fusion (RRF)
        let mut map: HashMap<String, CandidateAccumulator> = HashMap::new();

        // Accumulate BM25 signals
        for (i, item) in bm25_candidates.into_iter().enumerate() {
            let rank = i + 1; // 1-based rank
            let score_contrib = cfg.bm25_weight / (cfg.k + rank as f64);
            let key = item.turn_id.as_str().to_string();

            map.entry(key)
                .and_modify(|acc| {
                    acc.rrf_score += score_contrib;
                    acc.bm25_rank = Some(rank);
                    if acc.snippet.is_empty() {
                        acc.snippet = item.snippet.clone();
                    }
                })
                .or_insert_with(|| CandidateAccumulator {
                    turn_id: item.turn_id,
                    conversation_id: item.conversation_id,
                    rrf_score: score_contrib,
                    bm25_rank: Some(rank),
                    vector_rank: None,
                    cosine_similarity: None,
                    content: item.content,
                    snippet: item.snippet,
                });
        }

        // Accumulate Vector signals
        for (i, item) in vector_candidates.into_iter().enumerate() {
            let rank = i + 1; // 1-based rank
            let score_contrib = cfg.vector_weight / (cfg.k + rank as f64);
            let key = item.record.entity_id.clone();

            if let Some(acc) = map.get_mut(&key) {
                acc.rrf_score += score_contrib;
                acc.vector_rank = Some(rank);
                acc.cosine_similarity = Some(item.similarity);
            } else {
                let turn_id = match TurnId::new(&item.record.entity_id) {
                    Ok(tid) => tid,
                    Err(_) => continue,
                };
                let conv_id = match item.record.conversation_id.as_ref() {
                    Some(cid) => match ConversationId::new(cid) {
                        Ok(c) => c,
                        Err(_) => continue,
                    },
                    None => continue,
                };

                map.insert(
                    key,
                    CandidateAccumulator {
                        turn_id,
                        conversation_id: conv_id,
                        rrf_score: score_contrib,
                        bm25_rank: None,
                        vector_rank: Some(rank),
                        cosine_similarity: Some(item.similarity),
                        content: item.record.content_snippet.clone(),
                        snippet: item.record.content_snippet,
                    },
                );
            }
        }

        // 4. Sort candidates by fused RRF score descending
        let mut results: Vec<HybridSearchResult> = map
            .into_values()
            .map(|acc| HybridSearchResult {
                turn_id: acc.turn_id,
                conversation_id: acc.conversation_id,
                rrf_score: acc.rrf_score,
                bm25_rank: acc.bm25_rank,
                vector_rank: acc.vector_rank,
                cosine_similarity: acc.cosine_similarity,
                content: acc.content,
                snippet: acc.snippet,
            })
            .collect();

        results.sort_by(|a, b| {
            b.rrf_score
                .partial_cmp(&a.rrf_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results.truncate(limit);
        Ok(results)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::conversation_store::{ConversationTurn, TurnMetadata, TurnRole};
    use crate::pool::SqliteDb;
    use crate::vector_store::VectorRecord;

    #[test]
    fn test_hybrid_search_rrf_dual_hit_boost() {
        let db = SqliteDb::open_in_memory().unwrap();
        let conv_store = ConversationStore::new(db.clone());
        let vec_store = VectorMemoryStore::new(db);
        let hybrid = HybridSearchEngine::new(conv_store.clone(), vec_store.clone());

        let conv = conv_store.create_conversation("Hybrid Test", None).unwrap();

        // Turn 1: matches both keyword ("database") and semantic vector
        let turn1 = ConversationTurn {
            id: TurnId::new("turn_db_01").unwrap(),
            conversation_id: conv.id.clone(),
            turn_index: 0,
            role: TurnRole::Assistant,
            content: "SQLite is a zero-configuration transactional database engine.".to_string(),
            timestamp_ms: 1000,
            token_count: 8,
            tool_calls: None,
            metadata: TurnMetadata::default(),
        };

        // Turn 2: matches keyword ("database") only, orthogonal vector
        let turn2 = ConversationTurn {
            id: TurnId::new("turn_db_02").unwrap(),
            conversation_id: conv.id.clone(),
            turn_index: 1,
            role: TurnRole::Assistant,
            content: "Relational database systems organize data into structured tables."
                .to_string(),
            timestamp_ms: 2000,
            token_count: 8,
            tool_calls: None,
            metadata: TurnMetadata::default(),
        };

        // Turn 3: matches semantic vector only (synonyms / concepts without literal keyword)
        let turn3 = ConversationTurn {
            id: TurnId::new("turn_db_03").unwrap(),
            conversation_id: conv.id.clone(),
            turn_index: 2,
            role: TurnRole::Assistant,
            content: "Persistent local disk storage with ACID transactions.".to_string(),
            timestamp_ms: 3000,
            token_count: 8,
            tool_calls: None,
            metadata: TurnMetadata::default(),
        };

        conv_store.add_turn(&turn1).unwrap();
        conv_store.add_turn(&turn2).unwrap();
        conv_store.add_turn(&turn3).unwrap();

        // Index vectors (3-dimensional for testing)
        // Query will be [1.0, 0.0, 0.0]
        // Turn 1 vector: [1.0, 0.0, 0.0] -> sim 1.0 (vector rank 1)
        vec_store
            .store_embedding(&VectorRecord {
                id: "vec_01".to_string(),
                entity_type: "turn".to_string(),
                entity_id: turn1.id.as_str().to_string(),
                conversation_id: Some(conv.id.as_str().to_string()),
                dimension: 3,
                embedding: vec![1.0, 0.0, 0.0],
                content_snippet: turn1.content.clone(),
                created_at_ms: 1000,
            })
            .unwrap();

        // Turn 2 vector: [0.0, 0.0, 1.0] -> sim 0.0 (no vector match / low)
        vec_store
            .store_embedding(&VectorRecord {
                id: "vec_02".to_string(),
                entity_type: "turn".to_string(),
                entity_id: turn2.id.as_str().to_string(),
                conversation_id: Some(conv.id.as_str().to_string()),
                dimension: 3,
                embedding: vec![0.0, 0.0, 1.0],
                content_snippet: turn2.content.clone(),
                created_at_ms: 2000,
            })
            .unwrap();

        // Turn 3 vector: [0.95, 0.31, 0.0] -> sim 0.95 (vector rank 2)
        vec_store
            .store_embedding(&VectorRecord {
                id: "vec_03".to_string(),
                entity_type: "turn".to_string(),
                entity_id: turn3.id.as_str().to_string(),
                conversation_id: Some(conv.id.as_str().to_string()),
                dimension: 3,
                embedding: vec![0.95, 0.31, 0.0],
                content_snippet: turn3.content.clone(),
                created_at_ms: 3000,
            })
            .unwrap();

        // Hybrid search for "database" with query vector [1.0, 0.0, 0.0]
        let query_vec = vec![1.0, 0.0, 0.0];
        let results = hybrid
            .search("database", &query_vec, 5, None, None)
            .unwrap();

        assert!(!results.is_empty());

        // Turn 1 matched BOTH BM25 ("database") and Vector (sim 1.0)
        // Therefore RRF fusion MUST rank Turn 1 first with the highest score
        assert_eq!(results[0].turn_id.as_str(), "turn_db_01");
        assert!(results[0].bm25_rank.is_some());
        assert!(results[0].vector_rank.is_some());
        assert!((results[0].cosine_similarity.unwrap() - 1.0).abs() < 1e-4);

        // Verify dual-hit score is strictly greater than single-hit scores
        if results.len() > 1 {
            assert!(results[0].rrf_score > results[1].rrf_score);
        }
    }
}
