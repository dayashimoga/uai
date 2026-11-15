//! Citation-grounded offline knowledge retriever combining SQLite FTS5 BM25 and dense vector similarity.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::document_store::{estimate_token_count, DocumentStore};
use crate::error::StorageError;
use crate::hybrid_search::RrfConfig;
use crate::vector_store::VectorMemoryStore;

/// Grounded knowledge citation resulting from hybrid document chunk retrieval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeCitation {
    /// Unique chunk identifier (`chk_<doc_suffix>_<index>`).
    pub chunk_id: String,
    /// Parent document identifier (`doc_<hex>`).
    pub document_id: String,
    /// Canonical file or resource path.
    pub path: String,
    /// Display file name.
    pub file_name: String,
    /// Zero-based sequential chunk index within the document.
    pub chunk_index: usize,
    /// Combined Reciprocal Rank Fusion relevance score.
    pub rrf_score: f64,
    /// 1-based rank from BM25 FTS5 lexical search (if matched).
    pub bm25_rank: Option<usize>,
    /// 1-based rank from dense vector cosine search (if matched).
    pub vector_rank: Option<usize>,
    /// Cosine similarity from dense vector search (if matched).
    pub cosine_similarity: Option<f32>,
    /// Full text content of the chunk.
    pub content: String,
    /// Highlighted snippet with `<mark>` tags.
    pub snippet: String,
}

/// Structured RAG prompt context generated from grounded citations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeRagPromptContext {
    /// Ordered citations included within the token budget.
    pub citations: Vec<KnowledgeCitation>,
    /// Formatted prompt block ready to inject into the system prompt or conversation context.
    pub prompt_block: String,
    /// Estimated token count of the prompt block.
    pub estimated_tokens: usize,
}

/// Intermediate candidate accumulator during reciprocal rank fusion for document chunks.
struct KnowledgeCandidateAccumulator {
    chunk_id: String,
    document_id: String,
    path: String,
    file_name: String,
    chunk_index: usize,
    rrf_score: f64,
    bm25_rank: Option<usize>,
    vector_rank: Option<usize>,
    cosine_similarity: Option<f32>,
    content: String,
    snippet: String,
}

/// Offline hybrid knowledge retriever combining lexical FTS5 search and dense vector k-NN search.
#[derive(Debug, Clone)]
pub struct KnowledgeRetriever {
    document_store: DocumentStore,
    vector_store: VectorMemoryStore,
}

impl KnowledgeRetriever {
    /// Creates a new `KnowledgeRetriever` backed by document and vector stores.
    #[must_use]
    pub fn new(document_store: DocumentStore, vector_store: VectorMemoryStore) -> Self {
        Self {
            document_store,
            vector_store,
        }
    }

    /// Performs hybrid search combining BM25 lexical search and dense vector search via Reciprocal Rank Fusion.
    pub fn search(
        &self,
        query_text: &str,
        query_embedding: Option<&[f32]>,
        limit: usize,
        config: Option<RrfConfig>,
    ) -> Result<Vec<KnowledgeCitation>, StorageError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let cfg = config.unwrap_or_default();
        let candidate_pool_limit = limit.max(25);

        // 1. Lexical BM25 candidates from FTS5
        let bm25_candidates = self
            .document_store
            .search_chunks_fts(query_text, candidate_pool_limit)?;

        // 2. Dense vector candidates via cosine similarity
        let vector_candidates = if let Some(embedding) = query_embedding {
            if !embedding.is_empty() {
                self.vector_store.search_vector_similarity_by_entity(
                    embedding,
                    candidate_pool_limit,
                    0.0,
                    "doc_chunk",
                )?
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // 3. Reciprocal Rank Fusion (RRF)
        let mut map: HashMap<String, KnowledgeCandidateAccumulator> = HashMap::new();

        // Accumulate BM25 keyword signals
        for (i, item) in bm25_candidates.into_iter().enumerate() {
            let rank = i + 1;
            let score_contrib = cfg.bm25_weight / (cfg.k + rank as f64);
            let key = item.chunk_id.clone();

            map.entry(key)
                .and_modify(|acc| {
                    acc.rrf_score += score_contrib;
                    acc.bm25_rank = Some(rank);
                    if acc.snippet.is_empty() {
                        acc.snippet = item.snippet.clone();
                    }
                })
                .or_insert_with(|| KnowledgeCandidateAccumulator {
                    chunk_id: item.chunk_id,
                    document_id: item.document_id,
                    path: item.path,
                    file_name: item.file_name,
                    chunk_index: item.chunk_index,
                    rrf_score: score_contrib,
                    bm25_rank: Some(rank),
                    vector_rank: None,
                    cosine_similarity: None,
                    content: item.content,
                    snippet: item.snippet,
                });
        }

        // Accumulate dense vector signals
        for (i, item) in vector_candidates.into_iter().enumerate() {
            let rank = i + 1;
            let score_contrib = cfg.vector_weight / (cfg.k + rank as f64);
            let key = item.record.entity_id.clone();

            if let Some(acc) = map.get_mut(&key) {
                acc.rrf_score += score_contrib;
                acc.vector_rank = Some(rank);
                acc.cosine_similarity = Some(item.similarity);
            } else if let Ok(Some((chunk, doc))) = self.document_store.get_chunk_with_document(&key)
            {
                let snippet = if !item.record.content_snippet.is_empty() {
                    item.record.content_snippet
                } else {
                    chunk.content.chars().take(200).collect()
                };
                map.insert(
                    key.clone(),
                    KnowledgeCandidateAccumulator {
                        chunk_id: chunk.id,
                        document_id: doc.id,
                        path: doc.path,
                        file_name: doc.file_name,
                        chunk_index: chunk.chunk_index,
                        rrf_score: score_contrib,
                        bm25_rank: None,
                        vector_rank: Some(rank),
                        cosine_similarity: Some(item.similarity),
                        content: chunk.content,
                        snippet,
                    },
                );
            }
        }

        let mut results: Vec<KnowledgeCitation> = map
            .into_values()
            .map(|acc| KnowledgeCitation {
                chunk_id: acc.chunk_id,
                document_id: acc.document_id,
                path: acc.path,
                file_name: acc.file_name,
                chunk_index: acc.chunk_index,
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

    /// Formats retrieved citations into a grounded prompt context block bounded by `max_token_budget`.
    #[must_use]
    pub fn format_rag_prompt(
        &self,
        citations: &[KnowledgeCitation],
        max_token_budget: usize,
    ) -> KnowledgeRagPromptContext {
        let mut included = Vec::new();
        let mut prompt_lines = vec![
            "[Offline Knowledge Base Context]".to_string(),
            "The following citations were retrieved from offline documents to ground your response."
                .to_string(),
            "When relying on these citations, explicitly reference the source using [Doc: <file_name> #<chunk_index>]."
                .to_string(),
            String::new(),
        ];

        let mut current_tokens = estimate_token_count(&prompt_lines.join("\n"));

        for (idx, citation) in citations.iter().enumerate() {
            let block = format!(
                "--- Source [{}] (Document: {}, Path: {}, Chunk: #{}) ---\n{}\n",
                idx + 1,
                citation.file_name,
                citation.path,
                citation.chunk_index,
                citation.content
            );
            let block_tokens = estimate_token_count(&block);
            if !included.is_empty() && current_tokens + block_tokens > max_token_budget {
                break;
            }
            prompt_lines.push(block);
            current_tokens += block_tokens;
            included.push(citation.clone());
        }

        let full_text = prompt_lines.join("\n");
        let total_tokens = estimate_token_count(&full_text);

        KnowledgeRagPromptContext {
            citations: included,
            prompt_block: full_text,
            estimated_tokens: total_tokens,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::pool::SqliteDb;

    fn setup_test_retriever() -> (KnowledgeRetriever, DocumentStore, VectorMemoryStore) {
        let db = SqliteDb::open_in_memory().unwrap();
        let doc_store = DocumentStore::new(db.clone());
        let vec_store = VectorMemoryStore::new(db);
        let retriever = KnowledgeRetriever::new(doc_store.clone(), vec_store.clone());
        (retriever, doc_store, vec_store)
    }

    #[test]
    fn test_knowledge_retriever_hybrid_and_rag_prompt() {
        let (retriever, doc_store, _) = setup_test_retriever();

        let doc1_content = "Rust is a systems programming language focused on safety, speed, and concurrency. Cargo is its build tool.";

        doc_store
            .index_document(
                "docs/rust.md",
                "rust.md",
                "text/markdown",
                doc1_content,
                Some(|_: &str| Ok(vec![1.0, 0.0])),
            )
            .unwrap();

        let doc2_content = "Flutter is an open source framework by Google for building multi-platform applications from a single codebase.";

        doc_store
            .index_document(
                "docs/flutter.md",
                "flutter.md",
                "text/markdown",
                doc2_content,
                Some(|_: &str| Ok(vec![0.0, 1.0])),
            )
            .unwrap();

        // 1. Search lexical matching "concurrency"
        let results_lex = retriever.search("concurrency", None, 5, None).unwrap();
        assert_eq!(results_lex.len(), 1);
        assert_eq!(results_lex[0].file_name, "rust.md");
        assert!(results_lex[0].bm25_rank.is_some());
        assert!(results_lex[0].vector_rank.is_none());
        assert!(results_lex[0].snippet.contains("<mark>concurrency</mark>"));

        // 2. Hybrid search with vector query matching flutter embedding [0.0, 1.0]
        let results_hybrid = retriever
            .search("framework", Some(&[0.0, 1.0]), 5, None)
            .unwrap();
        assert!(!results_hybrid.is_empty());
        assert_eq!(results_hybrid[0].file_name, "flutter.md");
        assert!(results_hybrid[0].bm25_rank.is_some());
        assert!(results_hybrid[0].vector_rank.is_some());
        assert!(results_hybrid[0].cosine_similarity.unwrap() > 0.99);

        // 3. Prompt context formatting bounded by token budget
        let prompt_ctx = retriever.format_rag_prompt(&results_hybrid, 1000);
        assert_eq!(prompt_ctx.citations.len(), 2);
        assert_eq!(prompt_ctx.citations[0].file_name, "flutter.md");
        assert!(prompt_ctx
            .prompt_block
            .contains("[Offline Knowledge Base Context]"));
        assert!(prompt_ctx.prompt_block.contains("flutter.md"));
        assert!(prompt_ctx.estimated_tokens > 0);

        // 4. Test zero budget limits citations
        let tight_prompt_ctx = retriever.format_rag_prompt(&results_hybrid, 10);
        assert!(tight_prompt_ctx.citations.len() <= 1);
    }

    #[test]
    fn test_knowledge_retriever_vector_only_candidate() {
        let (retriever, doc_store, _) = setup_test_retriever();

        let doc_content = "Deep learning and neural networks power offline natural language processing and computer vision.";

        doc_store
            .index_document(
                "docs/ai.md",
                "ai.md",
                "text/markdown",
                doc_content,
                Some(|_: &str| Ok(vec![0.8, 0.6])),
            )
            .unwrap();

        // Search with query text that does NOT match any keyword in FTS, but vector matches
        let results = retriever
            .search("unmatched_keyword_xyz", Some(&[0.8, 0.6]), 5, None)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_name, "ai.md");
        assert!(results[0].bm25_rank.is_none());
        assert!(results[0].vector_rank.is_some());
        assert!(results[0].cosine_similarity.unwrap() > 0.99);
    }
}
