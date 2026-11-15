//! Persistent SQLite dense vector store and cosine similarity retrieval engine.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::StorageError;
use crate::pool::SqliteDb;

/// Serializes a slice of 32-bit floats into a little-endian binary byte vector.
#[must_use]
pub fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for &val in v {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Deserializes a little-endian binary byte slice back into a vector of 32-bit floats.
pub fn blob_to_vec(b: &[u8], expected_dim: usize) -> Result<Vec<f32>, StorageError> {
    if b.len() != expected_dim * 4 {
        return Err(StorageError::Serialization {
            message: format!(
                "Embedding blob byte length {} does not match expected dimension {} * 4 = {}",
                b.len(),
                expected_dim,
                expected_dim * 4
            ),
        });
    }

    let mut vec = Vec::with_capacity(expected_dim);
    for chunk in b.chunks_exact(4) {
        let arr: [u8; 4] = chunk.try_into().map_err(|_| StorageError::Serialization {
            message: "Invalid chunk size when reading f32".to_string(),
        })?;
        vec.push(f32::from_le_bytes(arr));
    }
    Ok(vec)
}

/// Computes the Euclidean (L2) norm of a float vector.
#[must_use]
pub fn l2_norm(vector: &[f32]) -> f32 {
    let sum_sq: f32 = vector.iter().map(|x| x * x).sum();
    sum_sq.sqrt()
}

/// Computes the cosine similarity between two float vectors.
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

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

/// A stored vector record representing a dense embedding associated with an entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorRecord {
    /// Unique identifier for this embedding entry.
    pub id: String,
    /// Entity discriminator (`turn`, `document_chunk`, `personal_fact`).
    pub entity_type: String,
    /// Identifier of the target entity (e.g. `TurnId`).
    pub entity_id: String,
    /// Optional parent conversation identifier for scoped filtering.
    pub conversation_id: Option<String>,
    /// Number of dimensions in the float vector.
    pub dimension: usize,
    /// Dense float embedding vector.
    pub embedding: Vec<f32>,
    /// Text excerpt or content snippet represented by this vector.
    pub content_snippet: String,
    /// Unix timestamp in milliseconds when this embedding was stored.
    pub created_at_ms: i64,
}

/// Search result item containing the matched record and its cosine similarity score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorSearchResult {
    /// The stored vector record.
    pub record: VectorRecord,
    /// Cosine similarity score in range `[-1.0, 1.0]` (higher is more similar).
    pub similarity: f32,
}

/// Thread-safe SQLite vector store for dense semantic embeddings.
#[derive(Debug, Clone)]
pub struct VectorMemoryStore {
    db: SqliteDb,
}

impl VectorMemoryStore {
    /// Creates a new `VectorMemoryStore` backed by the given SQLite database.
    #[must_use]
    pub fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    /// Stores or replaces an embedding record in the database.
    pub fn store_embedding(&self, record: &VectorRecord) -> Result<(), StorageError> {
        if record.embedding.len() != record.dimension {
            return Err(StorageError::Serialization {
                message: format!(
                    "Embedding vector length {} does not match recorded dimension {}",
                    record.embedding.len(),
                    record.dimension
                ),
            });
        }

        let blob = vec_to_blob(&record.embedding);

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO semantic_embeddings (
                    id, entity_type, entity_id, conversation_id,
                    dimension, embedding_blob, content_snippet, created_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    entity_type = excluded.entity_type,
                    entity_id = excluded.entity_id,
                    conversation_id = excluded.conversation_id,
                    dimension = excluded.dimension,
                    embedding_blob = excluded.embedding_blob,
                    content_snippet = excluded.content_snippet,
                    created_at_ms = excluded.created_at_ms;",
                params![
                    record.id,
                    record.entity_type,
                    record.entity_id,
                    record.conversation_id,
                    record.dimension as i64,
                    blob,
                    record.content_snippet,
                    record.created_at_ms,
                ],
            )?;
            Ok(())
        })
    }

    /// Retrieves an individual embedding record by its identifier.
    pub fn get_embedding(&self, id: &str) -> Result<Option<VectorRecord>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, entity_type, entity_id, conversation_id,
                        dimension, embedding_blob, content_snippet, created_at_ms
                 FROM semantic_embeddings WHERE id = ?1;",
            )?;

            let mut rows = stmt.query(params![id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row_to_record(row)?))
            } else {
                Ok(None)
            }
        })
    }

    /// Deletes an embedding record by identifier.
    pub fn delete_embedding(&self, id: &str) -> Result<(), StorageError> {
        self.db.with_conn(|conn| {
            conn.execute(
                "DELETE FROM semantic_embeddings WHERE id = ?1;",
                params![id],
            )?;
            Ok(())
        })
    }

    /// Deletes all embeddings belonging to a specific entity.
    pub fn delete_embeddings_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<usize, StorageError> {
        self.db.with_conn(|conn| {
            let count = conn.execute(
                "DELETE FROM semantic_embeddings WHERE entity_type = ?1 AND entity_id = ?2;",
                params![entity_type, entity_id],
            )?;
            Ok(count)
        })
    }

    /// Lists all embeddings belonging to a specific conversation.
    pub fn list_embeddings_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<VectorRecord>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, entity_type, entity_id, conversation_id,
                        dimension, embedding_blob, content_snippet, created_at_ms
                 FROM semantic_embeddings
                 WHERE conversation_id = ?1
                 ORDER BY created_at_ms ASC;",
            )?;

            let rows = stmt.query_map(params![conversation_id], row_to_record_tuple)?;

            let mut records = Vec::new();
            for item in rows {
                records.push(tuple_to_record(item?)?);
            }

            Ok(records)
        })
    }

    /// Performs a dense vector k-NN search across stored embeddings using cosine similarity.
    pub fn search_vector_similarity(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
        conversation_id: Option<&str>,
    ) -> Result<Vec<VectorSearchResult>, StorageError> {
        if query_embedding.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let records = self.db.with_conn(|conn| {
            let stmt = if let Some(cid) = conversation_id {
                let mut s = conn.prepare(
                    "SELECT id, entity_type, entity_id, conversation_id,
                            dimension, embedding_blob, content_snippet, created_at_ms
                     FROM semantic_embeddings
                     WHERE conversation_id = ?1;",
                )?;
                let rows = s.query_map(params![cid], row_to_record_tuple)?;
                let mut recs = Vec::new();
                for r in rows {
                    recs.push(tuple_to_record(r?)?);
                }
                recs
            } else {
                let mut s = conn.prepare(
                    "SELECT id, entity_type, entity_id, conversation_id,
                            dimension, embedding_blob, content_snippet, created_at_ms
                     FROM semantic_embeddings;",
                )?;
                let rows = s.query_map([], row_to_record_tuple)?;
                let mut recs = Vec::new();
                for r in rows {
                    recs.push(tuple_to_record(r?)?);
                }
                recs
            };

            Ok(stmt)
        })?;

        let mut scored: Vec<VectorSearchResult> = records
            .into_iter()
            .filter_map(|record| {
                if record.dimension != query_embedding.len() {
                    return None;
                }
                let similarity = cosine_similarity(query_embedding, &record.embedding);
                if similarity >= min_similarity {
                    Some(VectorSearchResult { record, similarity })
                } else {
                    None
                }
            })
            .collect();

        // Sort descending by similarity score
        scored.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored.truncate(limit);
        Ok(scored)
    }
}

type RecordTuple = (
    String,
    String,
    String,
    Option<String>,
    i64,
    Vec<u8>,
    String,
    i64,
);

fn row_to_record_tuple(row: &rusqlite::Row<'_>) -> Result<RecordTuple, rusqlite::Error> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
    ))
}

fn tuple_to_record(tuple: RecordTuple) -> Result<VectorRecord, StorageError> {
    let (
        id,
        entity_type,
        entity_id,
        conversation_id,
        dimension_i64,
        blob,
        content_snippet,
        created_at_ms,
    ) = tuple;
    let dimension = dimension_i64 as usize;
    let embedding = blob_to_vec(&blob, dimension)?;

    Ok(VectorRecord {
        id,
        entity_type,
        entity_id,
        conversation_id,
        dimension,
        embedding,
        content_snippet,
        created_at_ms,
    })
}

fn row_to_record(row: &rusqlite::Row<'_>) -> Result<VectorRecord, StorageError> {
    tuple_to_record(row_to_record_tuple(row)?)
}

/// Helper extracting current Unix epoch milliseconds.
#[must_use]
pub fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_vec_blob_serde_roundtrip() {
        let original = vec![1.234f32, -5.678, 0.0, 999.999];
        let blob = vec_to_blob(&original);
        assert_eq!(blob.len(), 16);

        let decoded = blob_to_vec(&blob, 4).unwrap();
        assert_eq!(original, decoded);

        // Mismatched dimension fails with error
        assert!(blob_to_vec(&blob, 5).is_err());
    }

    #[test]
    fn test_vector_memory_store_crud() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = VectorMemoryStore::new(db);

        let rec = VectorRecord {
            id: "vec_001".to_string(),
            entity_type: "turn".to_string(),
            entity_id: "turn_101".to_string(),
            conversation_id: Some("conv_999".to_string()),
            dimension: 3,
            embedding: vec![0.6, 0.8, 0.0],
            content_snippet: "Rust memory management".to_string(),
            created_at_ms: 1000,
        };

        store.store_embedding(&rec).unwrap();

        // Retrieve
        let retrieved = store.get_embedding("vec_001").unwrap().unwrap();
        assert_eq!(retrieved.id, "vec_001");
        assert_eq!(retrieved.entity_id, "turn_101");
        assert_eq!(retrieved.embedding, vec![0.6, 0.8, 0.0]);

        // List by conversation
        let list = store.list_embeddings_for_conversation("conv_999").unwrap();
        assert_eq!(list.len(), 1);

        // Delete
        store.delete_embedding("vec_001").unwrap();
        assert!(store.get_embedding("vec_001").unwrap().is_none());
    }

    #[test]
    fn test_vector_similarity_search() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = VectorMemoryStore::new(db);

        // Vector A: [1.0, 0.0, 0.0]
        store
            .store_embedding(&VectorRecord {
                id: "vec_a".to_string(),
                entity_type: "turn".to_string(),
                entity_id: "turn_a".to_string(),
                conversation_id: Some("conv_1".to_string()),
                dimension: 3,
                embedding: vec![1.0, 0.0, 0.0],
                content_snippet: "Topic A".to_string(),
                created_at_ms: 1000,
            })
            .unwrap();

        // Vector B: [1/sqrt(2), 1/sqrt(2), 0.0] (~45 degrees, similarity ~0.707)
        let frac_sqrt_2 = std::f32::consts::FRAC_1_SQRT_2;
        store
            .store_embedding(&VectorRecord {
                id: "vec_b".to_string(),
                entity_type: "turn".to_string(),
                entity_id: "turn_b".to_string(),
                conversation_id: Some("conv_1".to_string()),
                dimension: 3,
                embedding: vec![frac_sqrt_2, frac_sqrt_2, 0.0],
                content_snippet: "Topic B".to_string(),
                created_at_ms: 2000,
            })
            .unwrap();

        // Vector C: [0.0, 1.0, 0.0] (orthogonal to A, similarity 0.0)
        store
            .store_embedding(&VectorRecord {
                id: "vec_c".to_string(),
                entity_type: "turn".to_string(),
                entity_id: "turn_c".to_string(),
                conversation_id: Some("conv_2".to_string()),
                dimension: 3,
                embedding: vec![0.0, 1.0, 0.0],
                content_snippet: "Topic C".to_string(),
                created_at_ms: 3000,
            })
            .unwrap();

        // Search with query = [1.0, 0.0, 0.0]
        let query = vec![1.0, 0.0, 0.0];
        let hits = store
            .search_vector_similarity(&query, 5, 0.1, None)
            .unwrap();

        // Expect Vector A (sim 1.0) and Vector B (sim ~0.707) to exceed threshold 0.1
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].record.id, "vec_a");
        assert!((hits[0].similarity - 1.0).abs() < 1e-4);

        assert_eq!(hits[1].record.id, "vec_b");
        assert!((hits[1].similarity - frac_sqrt_2).abs() < 1e-4);

        // Scoped search in conv_2 returns empty because sim with Vector C is 0.0 (< 0.1)
        let scoped_c = store
            .search_vector_similarity(&query, 5, 0.1, Some("conv_2"))
            .unwrap();
        assert!(scoped_c.is_empty());
    }

    #[test]
    fn test_cascade_trigger_cleans_embeddings_on_turn_delete() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = VectorMemoryStore::new(db.clone());

        // Create a conversation and turn
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO conversations (id, title, created_at_ms, updated_at_ms, metadata_json)
                 VALUES ('conv_del', 'Delete Test', 100, 100, '{}');",
                [],
            )?;
            conn.execute(
                "INSERT INTO conversation_turns (id, conversation_id, turn_index, role, content, timestamp_ms, token_count, metadata_json)
                 VALUES ('turn_del', 'conv_del', 0, 'user', 'Hello delete', 100, 2, '{}');",
                [],
            )?;
            Ok(())
        })
        .unwrap();

        // Store embedding for this turn
        store
            .store_embedding(&VectorRecord {
                id: "vec_turn_del".to_string(),
                entity_type: "turn".to_string(),
                entity_id: "turn_del".to_string(),
                conversation_id: Some("conv_del".to_string()),
                dimension: 2,
                embedding: vec![1.0, 0.0],
                content_snippet: "Hello delete".to_string(),
                created_at_ms: 100,
            })
            .unwrap();

        assert!(store.get_embedding("vec_turn_del").unwrap().is_some());

        // Delete the turn directly
        db.with_conn(|conn| {
            conn.execute("DELETE FROM conversation_turns WHERE id = 'turn_del';", [])?;
            Ok(())
        })
        .unwrap();

        // Verify trigger deleted the embedding automatically
        assert!(store.get_embedding("vec_turn_del").unwrap().is_none());
    }
}
