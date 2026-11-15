//! Local document ingestion, boundary-aware semantic chunking, and FTS5 search.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::conversation_store::sanitize_fts5_query;
use crate::error::StorageError;
use crate::pool::SqliteDb;
use crate::vector_store::vec_to_blob;

/// Represents an ingested document record stored in SQLite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentRecord {
    /// Unique identifier for this document (`doc_<hex>`).
    pub id: String,
    /// Canonical file or resource path.
    pub path: String,
    /// Display file name.
    pub file_name: String,
    /// MIME type (e.g. `text/markdown`, `text/plain`, `text/x-rust`).
    pub mime_type: String,
    /// BLAKE3 256-bit hexadecimal content hash for incremental change tracking.
    pub content_hash: String,
    /// File size in bytes.
    pub file_size_bytes: u64,
    /// Number of semantic chunks extracted from this document.
    pub chunk_count: usize,
    /// Epoch timestamp in milliseconds when initially indexed.
    pub indexed_at_ms: i64,
    /// Epoch timestamp in milliseconds when last updated.
    pub updated_at_ms: i64,
}

/// An individual chunk of text extracted from an ingested document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentChunk {
    /// Unique identifier for this chunk (`chk_<doc_suffix>_<index>`).
    pub id: String,
    /// Foreign key referencing the parent `documents.id`.
    pub document_id: String,
    /// Zero-based sequential chunk index within the document.
    pub chunk_index: usize,
    /// Raw text content of the chunk.
    pub content: String,
    /// Estimated token count.
    pub token_count: usize,
    /// Character / byte offset start in the original document.
    pub byte_offset_start: usize,
    /// Character / byte offset end in the original document.
    pub byte_offset_end: usize,
}

/// Result of an indexing operation indicating whether content was newly stored, updated, or skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexingStatus {
    /// Document was newly indexed with extracted chunks.
    Indexed {
        /// Document identifier.
        document_id: String,
        /// Number of chunks created.
        chunk_count: usize,
    },
    /// Content hash matches existing document; indexing was skipped with zero work.
    SkippedUnchanged {
        /// Document identifier.
        document_id: String,
    },
    /// Existing document was modified; previous chunks and embeddings replaced.
    Updated {
        /// Document identifier.
        document_id: String,
        /// New chunk count.
        chunk_count: usize,
    },
}

/// A search hit from SQLite FTS5 BM25 search across document chunks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkSearchResult {
    /// Unique chunk identifier.
    pub chunk_id: String,
    /// Parent document identifier.
    pub document_id: String,
    /// Document path.
    pub path: String,
    /// Document display file name.
    pub file_name: String,
    /// Chunk index within document.
    pub chunk_index: usize,
    /// Chunk text content.
    pub content: String,
    /// Highlighted excerpt formatted with `<mark>` tags.
    pub snippet: String,
    /// BM25 relevance score from SQLite FTS5 (lower/negative values are better).
    pub bm25_score: f64,
}

/// Uncommitted raw chunk before assigning database IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawChunk {
    /// Extracted text content.
    pub content: String,
    /// Estimated token count.
    pub token_count: usize,
    /// Starting byte offset in original text.
    pub byte_offset_start: usize,
    /// Ending byte offset in original text.
    pub byte_offset_end: usize,
}

/// Boundary-aware text chunker splitting on paragraphs, sentences, and line breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentChunker {
    /// Target chunk size in characters (approx. 4 characters per token).
    pub target_chunk_size: usize,
    /// Character overlap between adjacent chunks.
    pub chunk_overlap: usize,
    /// Minimum chunk size required before splitting on natural punctuation.
    pub min_chunk_size: usize,
}

impl Default for DocumentChunker {
    fn default() -> Self {
        Self {
            target_chunk_size: 500,
            chunk_overlap: 100,
            min_chunk_size: 50,
        }
    }
}

impl DocumentChunker {
    /// Creates a new `DocumentChunker` with specified parameters.
    #[must_use]
    pub const fn new(
        target_chunk_size: usize,
        chunk_overlap: usize,
        min_chunk_size: usize,
    ) -> Self {
        Self {
            target_chunk_size,
            chunk_overlap,
            min_chunk_size,
        }
    }

    /// Splits text into sequential boundary-aware chunks with overlap.
    #[must_use]
    pub fn chunk_text(&self, text: &str) -> Vec<RawChunk> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        // If entire text fits within target chunk size, return single chunk
        if text.len() <= self.target_chunk_size {
            return vec![RawChunk {
                content: text.to_string(),
                token_count: estimate_token_count(text),
                byte_offset_start: 0,
                byte_offset_end: text.len(),
            }];
        }

        let mut chunks = Vec::new();
        let mut start = 0;
        let len = text.len();

        while start < len {
            // Find tentative end
            let mut end = (start + self.target_chunk_size).min(len);

            // Snap end to valid UTF-8 char boundary
            while end < len && !text.is_char_boundary(end) {
                end += 1;
            }

            if end < len {
                // Seek natural boundary in search window
                let search_window = &text[start..end];
                let natural_break = search_window
                    .rfind("\n\n")
                    .map(|pos| pos + 2)
                    .or_else(|| search_window.rfind(".\n").map(|pos| pos + 2))
                    .or_else(|| search_window.rfind(". ").map(|pos| pos + 2))
                    .or_else(|| search_window.rfind("?\n").map(|pos| pos + 2))
                    .or_else(|| search_window.rfind("? ").map(|pos| pos + 2))
                    .or_else(|| search_window.rfind("!\n").map(|pos| pos + 2))
                    .or_else(|| search_window.rfind("! ").map(|pos| pos + 2))
                    .or_else(|| search_window.rfind('\n').map(|pos| pos + 1))
                    .or_else(|| search_window.rfind(' ').map(|pos| pos + 1));

                if let Some(offset) = natural_break {
                    if offset >= self.min_chunk_size {
                        end = start + offset;
                    }
                }
            }

            while end < len && !text.is_char_boundary(end) {
                end += 1;
            }

            let slice = &text[start..end];
            let token_count = estimate_token_count(slice);
            chunks.push(RawChunk {
                content: slice.to_string(),
                token_count,
                byte_offset_start: start,
                byte_offset_end: end,
            });

            if end >= len {
                break;
            }

            // Calculate next start offset with overlap
            let mut next_start = if end > self.chunk_overlap {
                end - self.chunk_overlap
            } else {
                start + 1
            };

            if next_start <= start {
                next_start = start + 1;
            }

            while next_start < end && !text.is_char_boundary(next_start) {
                next_start += 1;
            }

            // Snap next_start forward to nearest whitespace if available
            if let Some(space_pos) = text[next_start..end].find(|c: char| c.is_whitespace()) {
                let candidate = next_start + space_pos + 1;
                if candidate < end && text.is_char_boundary(candidate) {
                    next_start = candidate;
                }
            }

            start = next_start;
        }

        chunks
    }
}

/// Estimates token count based on character length and word heuristic.
#[must_use]
pub fn estimate_token_count(text: &str) -> usize {
    let by_chars = text.len().div_ceil(4);
    let by_words = (text.split_whitespace().count() as f64 * 1.3).ceil() as usize;
    by_chars.max(by_words).max(1)
}

/// Current epoch time in milliseconds.
fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Persistent SQLite store for ingested documents, chunks, and FTS5 search index.
#[derive(Debug, Clone)]
pub struct DocumentStore {
    db: SqliteDb,
    chunker: DocumentChunker,
}

impl DocumentStore {
    /// Creates a new `DocumentStore` backed by the given SQLite database pool with default chunking.
    #[must_use]
    pub fn new(db: SqliteDb) -> Self {
        Self {
            db,
            chunker: DocumentChunker::default(),
        }
    }

    /// Creates a new `DocumentStore` with custom chunker parameters.
    #[must_use]
    pub const fn with_chunker(db: SqliteDb, chunker: DocumentChunker) -> Self {
        Self { db, chunker }
    }

    /// Ingests or updates a document by path with incremental BLAKE3 change detection.
    ///
    /// If the file content hash is identical to the existing record, indexing is skipped (`SkippedUnchanged`).
    /// If modified, previous chunks and associated vector embeddings are atomically replaced.
    /// Optionally computes and stores vector embeddings if `embed_fn` is provided.
    pub fn index_document<F>(
        &self,
        path: &str,
        file_name: &str,
        mime_type: &str,
        content: &str,
        embed_fn: Option<F>,
    ) -> Result<IndexingStatus, StorageError>
    where
        F: Fn(&str) -> Result<Vec<f32>, StorageError>,
    {
        let now_ms = current_time_ms();
        let content_hash = blake3::hash(content.as_bytes()).to_hex().to_string();
        let file_size_bytes = content.len() as u64;

        // Check for existing document
        let existing = self.get_document_by_path(path)?;
        if let Some(doc) = &existing {
            if doc.content_hash == content_hash {
                return Ok(IndexingStatus::SkippedUnchanged {
                    document_id: doc.id.clone(),
                });
            }
        }

        let is_update = existing.is_some();
        let doc_id = match &existing {
            Some(doc) => doc.id.clone(),
            None => format!("doc_{}", &blake3::hash(path.as_bytes()).to_hex()[..16]),
        };

        let raw_chunks = self.chunker.chunk_text(content);
        let chunk_count = raw_chunks.len();

        self.db.with_conn(|conn| {
            let tx = conn.transaction()?;

            // 1. Upsert document record
            tx.execute(
                "INSERT INTO documents (
                    id, path, file_name, mime_type, content_hash,
                    file_size_bytes, chunk_count, indexed_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(path) DO UPDATE SET
                    file_name = excluded.file_name,
                    mime_type = excluded.mime_type,
                    content_hash = excluded.content_hash,
                    file_size_bytes = excluded.file_size_bytes,
                    chunk_count = excluded.chunk_count,
                    updated_at_ms = excluded.updated_at_ms;",
                params![
                    doc_id,
                    path,
                    file_name,
                    mime_type,
                    content_hash,
                    file_size_bytes as i64,
                    chunk_count as i64,
                    now_ms,
                    now_ms,
                ],
            )?;

            // 2. If updating, remove old chunks (triggers automatically clean FTS and semantic embeddings)
            if is_update {
                tx.execute(
                    "DELETE FROM document_chunks WHERE document_id = ?1;",
                    params![doc_id],
                )?;
            }

            // 3. Insert chunks and optional embeddings
            for (index, raw) in raw_chunks.iter().enumerate() {
                let chunk_id = format!("chk_{}_{index}", &doc_id[4..]);

                tx.execute(
                    "INSERT INTO document_chunks (
                        id, document_id, chunk_index, content,
                        token_count, byte_offset_start, byte_offset_end
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
                    params![
                        chunk_id,
                        doc_id,
                        index as i64,
                        raw.content,
                        raw.token_count as i64,
                        raw.byte_offset_start as i64,
                        raw.byte_offset_end as i64,
                    ],
                )?;

                // If embedder provided, compute and store dense embedding vector
                if let Some(ref embed) = embed_fn {
                    let emb = embed(&raw.content)?;
                    let dimension = emb.len();
                    let blob = vec_to_blob(&emb);
                    let emb_id = format!("emb_chk_{}_{index}", &doc_id[4..]);

                    tx.execute(
                        "INSERT INTO semantic_embeddings (
                            id, entity_type, entity_id, conversation_id,
                            dimension, embedding_blob, content_snippet, created_at_ms
                         ) VALUES (?1, 'doc_chunk', ?2, NULL, ?3, ?4, ?5, ?6);",
                        params![
                            emb_id,
                            chunk_id,
                            dimension as i64,
                            blob,
                            raw.content.chars().take(200).collect::<String>(),
                            now_ms,
                        ],
                    )?;
                }
            }

            tx.commit()?;
            Ok(())
        })?;

        if is_update {
            Ok(IndexingStatus::Updated {
                document_id: doc_id,
                chunk_count,
            })
        } else {
            Ok(IndexingStatus::Indexed {
                document_id: doc_id,
                chunk_count,
            })
        }
    }

    /// Retrieves a document record by its unique identifier.
    pub fn get_document(&self, id: &str) -> Result<Option<DocumentRecord>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, path, file_name, mime_type, content_hash,
                        file_size_bytes, chunk_count, indexed_at_ms, updated_at_ms
                 FROM documents WHERE id = ?1;",
            )?;

            let mut rows = stmt.query(params![id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row_to_document(row)?))
            } else {
                Ok(None)
            }
        })
    }

    /// Retrieves a document record by its canonical file path.
    pub fn get_document_by_path(&self, path: &str) -> Result<Option<DocumentRecord>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, path, file_name, mime_type, content_hash,
                        file_size_bytes, chunk_count, indexed_at_ms, updated_at_ms
                 FROM documents WHERE path = ?1;",
            )?;

            let mut rows = stmt.query(params![path])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row_to_document(row)?))
            } else {
                Ok(None)
            }
        })
    }

    /// Lists all indexed documents ordered by updated timestamp descending.
    pub fn list_documents(&self) -> Result<Vec<DocumentRecord>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, path, file_name, mime_type, content_hash,
                        file_size_bytes, chunk_count, indexed_at_ms, updated_at_ms
                 FROM documents ORDER BY updated_at_ms DESC;",
            )?;

            let rows = stmt.query_map([], row_to_document)?;
            let mut docs = Vec::new();
            for r in rows {
                docs.push(r?);
            }
            Ok(docs)
        })
    }

    /// Returns the total count of indexed documents.
    pub fn document_count(&self) -> Result<usize, StorageError> {
        self.db.with_conn(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM documents;", [], |row| row.get(0))?;
            Ok(count as usize)
        })
    }

    /// Returns the total count of document chunks across all documents.
    pub fn chunk_count(&self) -> Result<usize, StorageError> {
        self.db.with_conn(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM document_chunks;", [], |row| {
                    row.get(0)
                })?;
            Ok(count as usize)
        })
    }

    /// Retrieves all semantic chunks belonging to a document ordered by `chunk_index`.
    pub fn get_document_chunks(&self, doc_id: &str) -> Result<Vec<DocumentChunk>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, document_id, chunk_index, content,
                        token_count, byte_offset_start, byte_offset_end
                 FROM document_chunks WHERE document_id = ?1
                 ORDER BY chunk_index ASC;",
            )?;

            let rows = stmt.query_map(params![doc_id], row_to_chunk)?;
            let mut chunks = Vec::new();
            for r in rows {
                chunks.push(r?);
            }
            Ok(chunks)
        })
    }

    /// Deletes a document by ID. Cascading triggers automatically delete associated chunks,
    /// FTS5 index rows, and dense vector embeddings.
    pub fn delete_document(&self, id: &str) -> Result<bool, StorageError> {
        self.db.with_conn(|conn| {
            let deleted = conn.execute("DELETE FROM documents WHERE id = ?1;", params![id])?;
            Ok(deleted > 0)
        })
    }

    /// Deletes a document by canonical path. Cascading triggers purge all chunks, FTS, and vectors.
    pub fn delete_document_by_path(&self, path: &str) -> Result<bool, StorageError> {
        self.db.with_conn(|conn| {
            let deleted = conn.execute("DELETE FROM documents WHERE path = ?1;", params![path])?;
            Ok(deleted > 0)
        })
    }

    /// Performs lexical SQLite FTS5 search across document chunks with BM25 ranking and snippet excerpts.
    pub fn search_chunks_fts(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ChunkSearchResult>, StorageError> {
        let sanitized = sanitize_fts5_query(query);
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }

        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT f.chunk_id, f.document_id, d.path, d.file_name, c.chunk_index,
                        c.content, snippet(document_chunks_fts, 2, '<mark>', '</mark>', '...', 16),
                        bm25(document_chunks_fts) as score
                 FROM document_chunks_fts f
                 JOIN document_chunks c ON c.id = f.chunk_id
                 JOIN documents d ON d.id = f.document_id
                 WHERE document_chunks_fts MATCH ?1
                 ORDER BY score ASC
                 LIMIT ?2;",
            )?;

            let rows = stmt.query_map(params![sanitized, limit as i64], |row| {
                Ok(ChunkSearchResult {
                    chunk_id: row.get(0)?,
                    document_id: row.get(1)?,
                    path: row.get(2)?,
                    file_name: row.get(3)?,
                    chunk_index: row.get::<_, i64>(4)? as usize,
                    content: row.get(5)?,
                    snippet: row.get(6)?,
                    bm25_score: row.get(7)?,
                })
            })?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        })
    }

    /// Retrieves a chunk along with its parent document metadata by chunk ID.
    pub fn get_chunk_with_document(
        &self,
        chunk_id: &str,
    ) -> Result<Option<(DocumentChunk, DocumentRecord)>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT c.id, c.document_id, c.chunk_index, c.content,
                        c.token_count, c.byte_offset_start, c.byte_offset_end,
                        d.id, d.path, d.file_name, d.mime_type, d.content_hash,
                        d.file_size_bytes, d.chunk_count, d.indexed_at_ms, d.updated_at_ms
                 FROM document_chunks c
                 JOIN documents d ON d.id = c.document_id
                 WHERE c.id = ?1;",
            )?;

            let mut rows = stmt.query(params![chunk_id])?;
            if let Some(row) = rows.next()? {
                let chunk = DocumentChunk {
                    id: row.get(0)?,
                    document_id: row.get(1)?,
                    chunk_index: row.get::<_, i64>(2)? as usize,
                    content: row.get(3)?,
                    token_count: row.get::<_, i64>(4)? as usize,
                    byte_offset_start: row.get::<_, i64>(5)? as usize,
                    byte_offset_end: row.get::<_, i64>(6)? as usize,
                };
                let doc = DocumentRecord {
                    id: row.get(7)?,
                    path: row.get(8)?,
                    file_name: row.get(9)?,
                    mime_type: row.get(10)?,
                    content_hash: row.get(11)?,
                    file_size_bytes: row.get::<_, i64>(12)? as u64,
                    chunk_count: row.get::<_, i64>(13)? as usize,
                    indexed_at_ms: row.get(14)?,
                    updated_at_ms: row.get(15)?,
                };
                Ok(Some((chunk, doc)))
            } else {
                Ok(None)
            }
        })
    }
}

fn row_to_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentRecord> {
    Ok(DocumentRecord {
        id: row.get(0)?,
        path: row.get(1)?,
        file_name: row.get(2)?,
        mime_type: row.get(3)?,
        content_hash: row.get(4)?,
        file_size_bytes: row.get::<_, i64>(5)? as u64,
        chunk_count: row.get::<_, i64>(6)? as usize,
        indexed_at_ms: row.get(7)?,
        updated_at_ms: row.get(8)?,
    })
}

fn row_to_chunk(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentChunk> {
    Ok(DocumentChunk {
        id: row.get(0)?,
        document_id: row.get(1)?,
        chunk_index: row.get::<_, i64>(2)? as usize,
        content: row.get(3)?,
        token_count: row.get::<_, i64>(4)? as usize,
        byte_offset_start: row.get::<_, i64>(5)? as usize,
        byte_offset_end: row.get::<_, i64>(6)? as usize,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn setup_test_store() -> DocumentStore {
        let db = SqliteDb::open_in_memory().unwrap();
        DocumentStore::new(db)
    }

    #[test]
    fn test_boundary_aware_chunking_preserves_paragraphs_and_sentences() {
        let chunker = DocumentChunker::new(55, 10, 10);
        let sample_text = "First paragraph content that explains something.\n\nSecond paragraph content with more details.\n\nThird paragraph content with final thoughts.";
        let chunks = chunker.chunk_text(sample_text);

        assert!(!chunks.is_empty());
        assert!(chunks.len() >= 3);
        assert_eq!(chunks[0].byte_offset_start, 0);
        assert!(chunks[0].content.contains("First paragraph"));
    }

    #[test]
    fn test_short_document_produces_single_chunk() {
        let chunker = DocumentChunker::default();
        let short = "A short document that fits in one chunk.";
        let chunks = chunker.chunk_text(short);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, short);
        assert_eq!(chunks[0].byte_offset_start, 0);
        assert_eq!(chunks[0].byte_offset_end, short.len());
    }

    #[test]
    fn test_index_document_lifecycle_and_incremental_skip() {
        let store = setup_test_store();
        let content = "# Project Architecture\n\nThe Universal AI Assistant runs 100% offline.\n\nIt features local LLMs, sherpa-onnx speech, and SQLite encrypted memory.";

        // Initial indexing
        let status1 = store
            .index_document::<fn(&str) -> Result<Vec<f32>, StorageError>>(
                "/docs/architecture.md",
                "architecture.md",
                "text/markdown",
                content,
                None,
            )
            .unwrap();

        match status1 {
            IndexingStatus::Indexed {
                document_id,
                chunk_count,
            } => {
                assert!(document_id.starts_with("doc_"));
                assert!(chunk_count >= 1);
            }
            _ => panic!("Expected IndexingStatus::Indexed"),
        }

        assert_eq!(store.document_count().unwrap(), 1);
        let chunks = store.chunk_count().unwrap();
        assert!(chunks >= 1);

        // Re-indexing identical content must skip with zero changes
        let status2 = store
            .index_document::<fn(&str) -> Result<Vec<f32>, StorageError>>(
                "/docs/architecture.md",
                "architecture.md",
                "text/markdown",
                content,
                None,
            )
            .unwrap();

        match status2 {
            IndexingStatus::SkippedUnchanged { document_id } => {
                assert!(document_id.starts_with("doc_"));
            }
            _ => panic!("Expected IndexingStatus::SkippedUnchanged"),
        }

        assert_eq!(store.document_count().unwrap(), 1);
        assert_eq!(store.chunk_count().unwrap(), chunks);
    }

    #[test]
    fn test_index_document_modification_updates_chunks() {
        let store = setup_test_store();
        let path = "/src/main.rs";

        let v1 = "fn main() {\n    println!(\"Hello World\");\n}";
        let v2 = "fn main() {\n    println!(\"Hello Antigravity!\");\n    // Added new line\n}";

        let res1 = store
            .index_document::<fn(&str) -> Result<Vec<f32>, StorageError>>(
                path,
                "main.rs",
                "text/x-rust",
                v1,
                None,
            )
            .unwrap();

        let doc_id = match res1 {
            IndexingStatus::Indexed { document_id, .. } => document_id,
            _ => panic!("Expected Indexed"),
        };

        // Update with modified content
        let res2 = store
            .index_document::<fn(&str) -> Result<Vec<f32>, StorageError>>(
                path,
                "main.rs",
                "text/x-rust",
                v2,
                None,
            )
            .unwrap();

        match res2 {
            IndexingStatus::Updated { document_id, .. } => {
                assert_eq!(document_id, doc_id);
            }
            _ => panic!("Expected Updated"),
        }

        let doc = store.get_document(&doc_id).unwrap().unwrap();
        assert_eq!(
            doc.content_hash,
            blake3::hash(v2.as_bytes()).to_hex().to_string()
        );
    }

    #[test]
    fn test_fts5_lexical_search_and_snippet_generation() {
        let store = setup_test_store();

        let doc1 = "Quantum computing relies on quantum bits or qubits that can exist in superpositions of states.";
        let doc2 =
            "Rust is a systems programming language focused on safety, speed, and concurrency.";

        store
            .index_document::<fn(&str) -> Result<Vec<f32>, StorageError>>(
                "/docs/quantum.txt",
                "quantum.txt",
                "text/plain",
                doc1,
                None,
            )
            .unwrap();

        store
            .index_document::<fn(&str) -> Result<Vec<f32>, StorageError>>(
                "/docs/rust.txt",
                "rust.txt",
                "text/plain",
                doc2,
                None,
            )
            .unwrap();

        let hits = store.search_chunks_fts("qubits superposition", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_name, "quantum.txt");
        assert!(hits[0].snippet.contains("<mark>"));

        let hits_rust = store.search_chunks_fts("concurrency safety", 5).unwrap();
        assert_eq!(hits_rust.len(), 1);
        assert_eq!(hits_rust[0].file_name, "rust.txt");
    }

    #[test]
    fn test_cascading_deletion_purges_chunks_fts_and_embeddings() {
        let store = setup_test_store();
        let path = "/docs/deleteme.md";
        let content = "Secret confidential data that must be purged on deletion.";

        // Mock embedding generator producing 4-dimensional normalized float vectors
        let dummy_embed =
            |_text: &str| -> Result<Vec<f32>, StorageError> { Ok(vec![0.5, 0.5, 0.5, 0.5]) };

        let status = store
            .index_document(
                path,
                "deleteme.md",
                "text/markdown",
                content,
                Some(dummy_embed),
            )
            .unwrap();

        let doc_id = match status {
            IndexingStatus::Indexed { document_id, .. } => document_id,
            _ => panic!("Expected Indexed"),
        };

        assert_eq!(store.document_count().unwrap(), 1);
        assert!(store.chunk_count().unwrap() >= 1);

        // Verify FTS hit exists
        let pre_hits = store.search_chunks_fts("confidential", 5).unwrap();
        assert_eq!(pre_hits.len(), 1);

        // Verify embedding exists in semantic_embeddings
        let emb_count: i64 = store
            .db
            .with_conn(|conn| {
                let count = conn.query_row(
                    "SELECT COUNT(*) FROM semantic_embeddings WHERE entity_type = 'doc_chunk';",
                    [],
                    |row| row.get(0),
                )?;
                Ok(count)
            })
            .unwrap();
        assert!(emb_count >= 1);

        // Delete document
        let deleted = store.delete_document(&doc_id).unwrap();
        assert!(deleted);

        // Verify 100% cascade purge
        assert_eq!(store.document_count().unwrap(), 0);
        assert_eq!(store.chunk_count().unwrap(), 0);

        let post_hits = store.search_chunks_fts("confidential", 5).unwrap();
        assert_eq!(post_hits.len(), 0);

        let post_emb_count: i64 = store
            .db
            .with_conn(|conn| {
                let count = conn.query_row(
                    "SELECT COUNT(*) FROM semantic_embeddings WHERE entity_type = 'doc_chunk';",
                    [],
                    |row| row.get(0),
                )?;
                Ok(count)
            })
            .unwrap();
        assert_eq!(post_emb_count, 0);
    }
}
