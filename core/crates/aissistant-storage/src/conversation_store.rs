//! Multi-session conversation persistence, turn history, and SQLite FTS5 episodic search.

use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use aissistant_core::{ConversationId, TurnId};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::StorageError;
use crate::pool::SqliteDb;

/// Interaction role within a conversation turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnRole {
    User,
    Assistant,
    System,
    Tool,
}

impl TurnRole {
    /// Returns the static string slice of this role.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Tool => "tool",
        }
    }
}

impl FromStr for TurnRole {
    type Err = StorageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "system" => Ok(Self::System),
            "tool" => Ok(Self::Tool),
            _ => Err(StorageError::Serialization {
                message: format!("Unknown turn role: '{s}'"),
            }),
        }
    }
}

impl std::fmt::Display for TurnRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Optional metadata associated with a conversation session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConversationMetadata {
    /// Whether the conversation is pinned to the top of the session list.
    #[serde(default)]
    pub pinned: bool,
    /// User-defined or AI-extracted categorical tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// High-level conversation summary or gist.
    #[serde(default)]
    pub summary: Option<String>,
    /// ID of the model used in this conversation.
    #[serde(default)]
    pub model_id: Option<String>,
    /// System prompt active for this session.
    #[serde(default)]
    pub system_prompt: Option<String>,
}

/// Persistent multi-turn conversation session record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    /// Unique identifier for the conversation session.
    pub id: ConversationId,
    /// Human-readable title or generated topic.
    pub title: String,
    /// Unix millisecond timestamp when the conversation was initiated.
    pub created_at_ms: i64,
    /// Unix millisecond timestamp of the most recent turn or edit.
    pub updated_at_ms: i64,
    /// Structured session metadata.
    pub metadata: ConversationMetadata,
}

/// Execution telemetry and generation metadata for an individual turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TurnMetadata {
    /// Model checkpoint used to generate the assistant response.
    #[serde(default)]
    pub model_id: Option<String>,
    /// End-to-end inference latency in milliseconds.
    #[serde(default)]
    pub latency_ms: Option<u64>,
    /// LLM completion termination condition (`stop`, `length`, `tool_call`).
    #[serde(default)]
    pub finish_reason: Option<String>,
    /// Spoken audio duration in milliseconds if synthesized by TTS.
    #[serde(default)]
    pub audio_duration_ms: Option<u64>,
}

/// Structured message record representing a single turn in a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationTurn {
    /// Unique identifier for this turn.
    pub id: TurnId,
    /// Parent conversation identifier.
    pub conversation_id: ConversationId,
    /// Zero-based sequential order within the parent conversation.
    pub turn_index: u32,
    /// Role of the message producer (`user`, `assistant`, `system`, `tool`).
    pub role: TurnRole,
    /// Natural-language text content of the turn.
    pub content: String,
    /// Unix millisecond timestamp of when the turn occurred.
    pub timestamp_ms: i64,
    /// Estimated or exact token count consumed by this turn.
    pub token_count: usize,
    /// Optional tool invocations emitted or received in this turn.
    pub tool_calls: Option<serde_json::Value>,
    /// Execution telemetry metadata.
    pub metadata: TurnMetadata,
}

/// Result item returned from SQLite FTS5 episodic memory search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpisodicSearchResult {
    /// Turn identifier containing the match.
    pub turn_id: TurnId,
    /// Parent conversation identifier.
    pub conversation_id: ConversationId,
    /// Turn index within the parent conversation.
    pub turn_index: u32,
    /// Turn role (`user`, `assistant`, etc.).
    pub role: TurnRole,
    /// Full message content of the matching turn.
    pub content: String,
    /// Unix millisecond timestamp of the turn.
    pub timestamp_ms: i64,
    /// Positive BM25 relevance score (higher is more relevant).
    pub bm25_score: f64,
    /// Formatted snippet with `<mark>` tags around matched terms.
    pub snippet: String,
}

/// Complete exported conversation bundle containing metadata and turns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportedConversation {
    /// Conversation header and metadata.
    pub conversation: Conversation,
    /// Ordered list of turns.
    pub turns: Vec<ConversationTurn>,
}

/// Thread-safe conversation persistence and SQLite FTS5 episodic search engine.
#[derive(Debug, Clone)]
pub struct ConversationStore {
    db: SqliteDb,
}

impl ConversationStore {
    /// Creates a new `ConversationStore` backed by the given SQLite database handle.
    #[must_use]
    pub fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    /// Creates and persists a new conversation session.
    pub fn create_conversation(
        &self,
        title: &str,
        metadata: Option<ConversationMetadata>,
    ) -> Result<Conversation, StorageError> {
        let id = ConversationId::generate();
        let now_ms = current_time_ms();
        let meta = metadata.unwrap_or_default();
        let meta_json = serde_json::to_string(&meta)?;

        let conversation = Conversation {
            id: id.clone(),
            title: title.trim().to_string(),
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            metadata: meta,
        };

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO conversations (id, title, created_at_ms, updated_at_ms, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5);",
                params![
                    conversation.id.as_str(),
                    conversation.title,
                    conversation.created_at_ms,
                    conversation.updated_at_ms,
                    meta_json
                ],
            )?;
            Ok(())
        })?;

        Ok(conversation)
    }

    /// Retrieves a conversation by its identifier.
    pub fn get_conversation(
        &self,
        id: &ConversationId,
    ) -> Result<Option<Conversation>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, title, created_at_ms, updated_at_ms, metadata_json
                 FROM conversations WHERE id = ?1;",
            )?;

            let mut rows = stmt.query(params![id.as_str()])?;
            if let Some(row) = rows.next()? {
                let raw_id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let created_at_ms: i64 = row.get(2)?;
                let updated_at_ms: i64 = row.get(3)?;
                let meta_json: String = row.get(4)?;
                let metadata: ConversationMetadata = serde_json::from_str(&meta_json)?;

                Ok(Some(Conversation {
                    id: ConversationId::new(raw_id)?,
                    title,
                    created_at_ms,
                    updated_at_ms,
                    metadata,
                }))
            } else {
                Ok(None)
            }
        })
    }

    /// Lists conversations ordered by most recently updated first.
    pub fn list_conversations(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Conversation>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, title, created_at_ms, updated_at_ms, metadata_json
                 FROM conversations
                 ORDER BY updated_at_ms DESC
                 LIMIT ?1 OFFSET ?2;",
            )?;

            let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
                let raw_id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let created_at_ms: i64 = row.get(2)?;
                let updated_at_ms: i64 = row.get(3)?;
                let meta_json: String = row.get(4)?;
                Ok((raw_id, title, created_at_ms, updated_at_ms, meta_json))
            })?;

            let mut conversations = Vec::new();
            for item in rows {
                let (raw_id, title, created_at_ms, updated_at_ms, meta_json) = item?;
                let metadata: ConversationMetadata =
                    serde_json::from_str(&meta_json).map_err(|e| StorageError::Serialization {
                        message: e.to_string(),
                    })?;
                conversations.push(Conversation {
                    id: ConversationId::new(raw_id)?,
                    title,
                    created_at_ms,
                    updated_at_ms,
                    metadata,
                });
            }

            Ok(conversations)
        })
    }

    /// Updates the title of an existing conversation.
    pub fn update_conversation_title(
        &self,
        id: &ConversationId,
        new_title: &str,
    ) -> Result<(), StorageError> {
        let now_ms = current_time_ms();
        let rows_affected = self.db.with_conn(|conn| {
            let count = conn.execute(
                "UPDATE conversations SET title = ?1, updated_at_ms = ?2 WHERE id = ?3;",
                params![new_title.trim(), now_ms, id.as_str()],
            )?;
            Ok(count)
        })?;

        if rows_affected == 0 {
            return Err(StorageError::ConversationNotFound { id: id.to_string() });
        }

        Ok(())
    }

    /// Updates the metadata of an existing conversation.
    pub fn update_conversation_metadata(
        &self,
        id: &ConversationId,
        metadata: &ConversationMetadata,
    ) -> Result<(), StorageError> {
        let now_ms = current_time_ms();
        let meta_json = serde_json::to_string(metadata)?;

        let rows_affected = self.db.with_conn(|conn| {
            let count = conn.execute(
                "UPDATE conversations SET metadata_json = ?1, updated_at_ms = ?2 WHERE id = ?3;",
                params![meta_json, now_ms, id.as_str()],
            )?;
            Ok(count)
        })?;

        if rows_affected == 0 {
            return Err(StorageError::ConversationNotFound { id: id.to_string() });
        }

        Ok(())
    }

    /// Deletes a conversation and cascades deletion to all associated turns and FTS index entries.
    pub fn delete_conversation(&self, id: &ConversationId) -> Result<(), StorageError> {
        let rows_affected = self.db.with_conn(|conn| {
            let count = conn.execute(
                "DELETE FROM conversations WHERE id = ?1;",
                params![id.as_str()],
            )?;
            Ok(count)
        })?;

        if rows_affected == 0 {
            return Err(StorageError::ConversationNotFound { id: id.to_string() });
        }

        Ok(())
    }

    /// Appends a new turn to an existing conversation and updates the conversation's `updated_at_ms`.
    pub fn add_turn(&self, turn: &ConversationTurn) -> Result<(), StorageError> {
        let tool_calls_json = turn
            .tool_calls
            .as_ref()
            .map(|tc| serde_json::to_string(tc).map_err(StorageError::from))
            .transpose()?;

        let meta_json = serde_json::to_string(&turn.metadata)?;

        self.db.transaction(|tx| {
            // Verify parent conversation exists
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1);",
                params![turn.conversation_id.as_str()],
                |row| row.get(0),
            )?;

            if !exists {
                return Err(StorageError::ConversationNotFound {
                    id: turn.conversation_id.to_string(),
                });
            }

            // Insert turn (triggers automatic FTS5 index insertion)
            tx.execute(
                "INSERT INTO conversation_turns (
                    id, conversation_id, turn_index, role, content,
                    timestamp_ms, token_count, tool_calls_json, metadata_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
                params![
                    turn.id.as_str(),
                    turn.conversation_id.as_str(),
                    turn.turn_index,
                    turn.role.as_str(),
                    turn.content,
                    turn.timestamp_ms,
                    turn.token_count as i64,
                    tool_calls_json,
                    meta_json
                ],
            )?;

            // Update parent conversation updated_at_ms
            tx.execute(
                "UPDATE conversations SET updated_at_ms = ?1 WHERE id = ?2;",
                params![turn.timestamp_ms, turn.conversation_id.as_str()],
            )?;

            Ok(())
        })
    }

    /// Retrieves an individual turn by its identifier.
    pub fn get_turn(&self, id: &TurnId) -> Result<Option<ConversationTurn>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, conversation_id, turn_index, role, content,
                        timestamp_ms, token_count, tool_calls_json, metadata_json
                 FROM conversation_turns WHERE id = ?1;",
            )?;

            let mut rows = stmt.query(params![id.as_str()])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row_to_turn(row)?))
            } else {
                Ok(None)
            }
        })
    }

    /// Lists all turns belonging to a conversation ordered by `turn_index` ascending.
    pub fn list_turns(
        &self,
        conversation_id: &ConversationId,
        limit: Option<usize>,
    ) -> Result<Vec<ConversationTurn>, StorageError> {
        self.db.with_conn(|conn| {
            let sql = if let Some(lim) = limit {
                format!(
                    "SELECT id, conversation_id, turn_index, role, content,
                            timestamp_ms, token_count, tool_calls_json, metadata_json
                     FROM conversation_turns
                     WHERE conversation_id = ?1
                     ORDER BY turn_index ASC
                     LIMIT {lim};"
                )
            } else {
                "SELECT id, conversation_id, turn_index, role, content,
                        timestamp_ms, token_count, tool_calls_json, metadata_json
                 FROM conversation_turns
                 WHERE conversation_id = ?1
                 ORDER BY turn_index ASC;"
                    .to_string()
            };

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![conversation_id.as_str()], |row| {
                row_to_turn_tuple(row)
            })?;

            let mut turns = Vec::new();
            for item in rows {
                turns.push(tuple_to_turn(item?)?);
            }

            Ok(turns)
        })
    }

    /// Deletes an individual turn by identifier (trigger automatically removes it from FTS5 index).
    pub fn delete_turn(&self, id: &TurnId) -> Result<(), StorageError> {
        let rows_affected = self.db.with_conn(|conn| {
            let count = conn.execute(
                "DELETE FROM conversation_turns WHERE id = ?1;",
                params![id.as_str()],
            )?;
            Ok(count)
        })?;

        if rows_affected == 0 {
            return Err(StorageError::TurnNotFound { id: id.to_string() });
        }

        Ok(())
    }

    /// Performs full-text episodic memory search across all stored conversations.
    pub fn search_episodic_memory(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EpisodicSearchResult>, StorageError> {
        let sanitized = sanitize_fts5_query(query);
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }

        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT 
                    t.id, 
                    t.conversation_id, 
                    t.turn_index, 
                    t.role, 
                    t.content, 
                    t.timestamp_ms, 
                    bm25(conversation_turns_fts) AS rank,
                    snippet(conversation_turns_fts, 3, '<mark>', '</mark>', '...', 24) AS snippet
                 FROM conversation_turns_fts
                 JOIN conversation_turns t ON t.rowid = conversation_turns_fts.rowid
                 WHERE conversation_turns_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2;",
            )?;

            let rows = stmt.query_map(params![sanitized, limit as i64], |row| {
                let turn_id: String = row.get(0)?;
                let conv_id: String = row.get(1)?;
                let turn_index: u32 = row.get(2)?;
                let role_str: String = row.get(3)?;
                let content: String = row.get(4)?;
                let timestamp_ms: i64 = row.get(5)?;
                let raw_rank: f64 = row.get(6)?;
                let snippet: String = row.get(7)?;

                Ok((
                    turn_id,
                    conv_id,
                    turn_index,
                    role_str,
                    content,
                    timestamp_ms,
                    raw_rank,
                    snippet,
                ))
            })?;

            let mut results = Vec::new();
            for item in rows {
                let (
                    turn_id,
                    conv_id,
                    turn_index,
                    role_str,
                    content,
                    timestamp_ms,
                    raw_rank,
                    snippet,
                ) = item?;
                let role = TurnRole::from_str(&role_str)?;
                // SQLite FTS5 bm25() returns negative values where more negative is more relevant;
                // invert to positive score for intuitive consumer ranking.
                let bm25_score = (-raw_rank).max(0.0);

                results.push(EpisodicSearchResult {
                    turn_id: TurnId::new(turn_id)?,
                    conversation_id: ConversationId::new(conv_id)?,
                    turn_index,
                    role,
                    content,
                    timestamp_ms,
                    bm25_score,
                    snippet,
                });
            }

            Ok(results)
        })
    }

    /// Performs full-text episodic memory search restricted to a single conversation session.
    pub fn search_episodic_memory_in_conversation(
        &self,
        conversation_id: &ConversationId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EpisodicSearchResult>, StorageError> {
        let sanitized = sanitize_fts5_query(query);
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }

        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT 
                    t.id, 
                    t.conversation_id, 
                    t.turn_index, 
                    t.role, 
                    t.content, 
                    t.timestamp_ms, 
                    bm25(conversation_turns_fts) AS rank,
                    snippet(conversation_turns_fts, 3, '<mark>', '</mark>', '...', 24) AS snippet
                 FROM conversation_turns_fts
                 JOIN conversation_turns t ON t.rowid = conversation_turns_fts.rowid
                 WHERE conversation_turns_fts MATCH ?1 AND t.conversation_id = ?2
                 ORDER BY rank
                 LIMIT ?3;",
            )?;

            let rows = stmt.query_map(
                params![sanitized, conversation_id.as_str(), limit as i64],
                |row| {
                    let turn_id: String = row.get(0)?;
                    let conv_id: String = row.get(1)?;
                    let turn_index: u32 = row.get(2)?;
                    let role_str: String = row.get(3)?;
                    let content: String = row.get(4)?;
                    let timestamp_ms: i64 = row.get(5)?;
                    let raw_rank: f64 = row.get(6)?;
                    let snippet: String = row.get(7)?;

                    Ok((
                        turn_id,
                        conv_id,
                        turn_index,
                        role_str,
                        content,
                        timestamp_ms,
                        raw_rank,
                        snippet,
                    ))
                },
            )?;

            let mut results = Vec::new();
            for item in rows {
                let (
                    turn_id,
                    conv_id,
                    turn_index,
                    role_str,
                    content,
                    timestamp_ms,
                    raw_rank,
                    snippet,
                ) = item?;
                let role = TurnRole::from_str(&role_str)?;
                let bm25_score = (-raw_rank).max(0.0);

                results.push(EpisodicSearchResult {
                    turn_id: TurnId::new(turn_id)?,
                    conversation_id: ConversationId::new(conv_id)?,
                    turn_index,
                    role,
                    content,
                    timestamp_ms,
                    bm25_score,
                    snippet,
                });
            }

            Ok(results)
        })
    }

    /// Exports a conversation and all its constituent turns into a structured JSON string.
    pub fn export_conversation_json(&self, id: &ConversationId) -> Result<String, StorageError> {
        let conversation = self
            .get_conversation(id)?
            .ok_or_else(|| StorageError::ConversationNotFound { id: id.to_string() })?;

        let turns = self.list_turns(id, None)?;
        let export = ExportedConversation {
            conversation,
            turns,
        };

        serde_json::to_string_pretty(&export).map_err(StorageError::from)
    }

    /// Permanently deletes all conversations, turns, and indexed memories across the entire database.
    pub fn purge_all_conversations(&self) -> Result<(), StorageError> {
        self.db.transaction(|tx| {
            tx.execute("DELETE FROM conversations;", [])?;
            tx.execute("DELETE FROM conversation_turns;", [])?;
            tx.execute("DELETE FROM conversation_turns_fts;", [])?;
            Ok(())
        })
    }
}

/// Helper extracting current system time in Unix epoch milliseconds.
fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Helper converting an SQLite row into a `ConversationTurn`.
fn row_to_turn(row: &rusqlite::Row<'_>) -> Result<ConversationTurn, StorageError> {
    let (
        turn_id,
        conv_id,
        turn_index,
        role_str,
        content,
        timestamp_ms,
        token_count,
        tool_calls_json,
        meta_json,
    ) = row_to_turn_tuple(row)?;
    tuple_to_turn((
        turn_id,
        conv_id,
        turn_index,
        role_str,
        content,
        timestamp_ms,
        token_count,
        tool_calls_json,
        meta_json,
    ))
}

type TurnTuple = (
    String,
    String,
    u32,
    String,
    String,
    i64,
    i64,
    Option<String>,
    String,
);

fn row_to_turn_tuple(row: &rusqlite::Row<'_>) -> Result<TurnTuple, rusqlite::Error> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
    ))
}

fn tuple_to_turn(tuple: TurnTuple) -> Result<ConversationTurn, StorageError> {
    let (
        turn_id,
        conv_id,
        turn_index,
        role_str,
        content,
        timestamp_ms,
        token_count,
        tool_calls_json,
        meta_json,
    ) = tuple;
    let role = TurnRole::from_str(&role_str)?;
    let tool_calls = match tool_calls_json {
        Some(s) if !s.trim().is_empty() => Some(serde_json::from_str(&s)?),
        _ => None,
    };
    let metadata: TurnMetadata = serde_json::from_str(&meta_json)?;

    Ok(ConversationTurn {
        id: TurnId::new(turn_id)?,
        conversation_id: ConversationId::new(conv_id)?,
        turn_index,
        role,
        content,
        timestamp_ms,
        token_count: token_count as usize,
        tool_calls,
        metadata,
    })
}

/// Sanitizes natural-language search queries into safe SQLite FTS5 MATCH expressions.
///
/// Splits search queries into alphanumeric/underscore/hyphen tokens and wraps each as a quoted
/// prefix token (`"token"*`). This ensures that special punctuation, control characters, and bare
/// SQL keywords (such as `AND`, `OR`, `NOT`) never cause syntax errors.
#[must_use]
pub fn sanitize_fts5_query(query: &str) -> String {
    let mut tokens = Vec::new();

    for raw_term in query.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-') {
        let trimmed = raw_term.trim();
        if !trimmed.is_empty() {
            tokens.push(format!("\"{trimmed}\"*"));
        }
    }

    tokens.join(" ")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_crud_lifecycle() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = ConversationStore::new(db);

        // 1. Create conversation
        let conv = store
            .create_conversation(
                "Local AI Architecture",
                Some(ConversationMetadata {
                    pinned: true,
                    tags: vec!["architecture".to_string(), "offline".to_string()],
                    summary: Some("Discussion on local speech and inference pipeline".to_string()),
                    model_id: Some("qwen2.5-1.5b".to_string()),
                    system_prompt: Some("You are a helpful assistant.".to_string()),
                }),
            )
            .unwrap();

        assert_eq!(conv.title, "Local AI Architecture");
        assert!(conv.metadata.pinned);
        assert_eq!(conv.metadata.tags.len(), 2);

        // 2. Retrieve conversation
        let fetched = store.get_conversation(&conv.id).unwrap().unwrap();
        assert_eq!(fetched.id, conv.id);
        assert_eq!(fetched.title, "Local AI Architecture");
        assert_eq!(fetched.metadata.model_id, Some("qwen2.5-1.5b".to_string()));

        // 3. Update title
        store
            .update_conversation_title(&conv.id, "Local AI Architecture v2")
            .unwrap();
        let updated = store.get_conversation(&conv.id).unwrap().unwrap();
        assert_eq!(updated.title, "Local AI Architecture v2");

        // 4. Update metadata
        let mut new_meta = updated.metadata.clone();
        new_meta.pinned = false;
        store
            .update_conversation_metadata(&conv.id, &new_meta)
            .unwrap();
        let unpinned = store.get_conversation(&conv.id).unwrap().unwrap();
        assert!(!unpinned.metadata.pinned);

        // 5. List conversations
        let list = store.list_conversations(10, 0).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, conv.id);

        // 6. Delete conversation
        store.delete_conversation(&conv.id).unwrap();
        let deleted = store.get_conversation(&conv.id).unwrap();
        assert!(deleted.is_none());

        // Deleting non-existent conversation returns error
        let err = store.delete_conversation(&conv.id).unwrap_err();
        assert!(matches!(err, StorageError::ConversationNotFound { .. }));
    }

    #[test]
    fn test_conversation_turns_and_cascade_deletion() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = ConversationStore::new(db);

        let conv = store.create_conversation("Turn Test", None).unwrap();

        let turn1 = ConversationTurn {
            id: TurnId::from_sequence(1),
            conversation_id: conv.id.clone(),
            turn_index: 0,
            role: TurnRole::User,
            content: "What is SQLite WAL mode?".to_string(),
            timestamp_ms: 1000,
            token_count: 6,
            tool_calls: None,
            metadata: TurnMetadata::default(),
        };

        let turn2 = ConversationTurn {
            id: TurnId::from_sequence(2),
            conversation_id: conv.id.clone(),
            turn_index: 1,
            role: TurnRole::Assistant,
            content: "WAL stands for Write-Ahead Logging, which enables concurrent reads."
                .to_string(),
            timestamp_ms: 2000,
            token_count: 12,
            tool_calls: None,
            metadata: TurnMetadata {
                model_id: Some("qwen2.5".to_string()),
                latency_ms: Some(150),
                finish_reason: Some("stop".to_string()),
                audio_duration_ms: None,
            },
        };

        // Add turns
        store.add_turn(&turn1).unwrap();
        store.add_turn(&turn2).unwrap();

        // Verify conversation updated_at_ms was updated to turn2's timestamp
        let conv_updated = store.get_conversation(&conv.id).unwrap().unwrap();
        assert_eq!(conv_updated.updated_at_ms, 2000);

        // List turns
        let turns = store.list_turns(&conv.id, None).unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].content, "What is SQLite WAL mode?");
        assert_eq!(
            turns[1].content,
            "WAL stands for Write-Ahead Logging, which enables concurrent reads."
        );

        // Individual turn get
        let fetched_turn = store.get_turn(&turn1.id).unwrap().unwrap();
        assert_eq!(fetched_turn.content, turn1.content);

        // Delete turn 1
        store.delete_turn(&turn1.id).unwrap();
        let remaining = store.list_turns(&conv.id, None).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, turn2.id);

        // Cascade delete: deleting conversation removes turn2 and FTS entries
        store.delete_conversation(&conv.id).unwrap();
        let turns_after_del = store.list_turns(&conv.id, None).unwrap();
        assert!(turns_after_del.is_empty());
    }

    #[test]
    fn test_sqlite_fts5_episodic_search() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = ConversationStore::new(db);

        let conv1 = store.create_conversation("Rust Systems", None).unwrap();
        let conv2 = store.create_conversation("Cooking Recipes", None).unwrap();

        // Populate conversation 1 with systems programming turns
        store
            .add_turn(&ConversationTurn {
                id: TurnId::new("turn_rust_01").unwrap(),
                conversation_id: conv1.id.clone(),
                turn_index: 0,
                role: TurnRole::User,
                content: "How does the borrow checker prevent data races in Rust?".to_string(),
                timestamp_ms: 1000,
                token_count: 10,
                tool_calls: None,
                metadata: TurnMetadata::default(),
            })
            .unwrap();

        store
            .add_turn(&ConversationTurn {
                id: TurnId::new("turn_rust_02").unwrap(),
                conversation_id: conv1.id.clone(),
                turn_index: 1,
                role: TurnRole::Assistant,
                content: "Rust enforces compile-time ownership rules, ensuring either one mutable reference or multiple immutable references.".to_string(),
                timestamp_ms: 2000,
                token_count: 16,
                tool_calls: None,
                metadata: TurnMetadata::default(),
            })
            .unwrap();

        // Populate conversation 2 with culinary turns
        store
            .add_turn(&ConversationTurn {
                id: TurnId::new("turn_cook_01").unwrap(),
                conversation_id: conv2.id.clone(),
                turn_index: 0,
                role: TurnRole::User,
                content: "Give me an authentic Italian recipe for carbonara pasta.".to_string(),
                timestamp_ms: 3000,
                token_count: 9,
                tool_calls: None,
                metadata: TurnMetadata::default(),
            })
            .unwrap();

        // Search for "borrow checker" across all conversations
        let results = store.search_episodic_memory("borrow checker", 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].turn_id.as_str(), "turn_rust_01");
        assert!(results[0].bm25_score > 0.0);
        assert!(results[0].snippet.contains("<mark>borrow</mark>"));

        // Search for "ownership rules"
        let rust_results = store.search_episodic_memory("ownership", 5).unwrap();
        assert_eq!(rust_results.len(), 1);
        assert_eq!(rust_results[0].turn_id.as_str(), "turn_rust_02");

        // Search for "carbonara recipe"
        let pasta_results = store.search_episodic_memory("carbonara", 5).unwrap();
        assert_eq!(pasta_results.len(), 1);
        assert_eq!(pasta_results[0].turn_id.as_str(), "turn_cook_01");

        // Scoped search within conversation 1 for "carbonara" must return 0 results
        let scoped_empty = store
            .search_episodic_memory_in_conversation(&conv1.id, "carbonara", 5)
            .unwrap();
        assert!(scoped_empty.is_empty());

        // Scoped search within conversation 2 for "carbonara" returns 1 result
        let scoped_hit = store
            .search_episodic_memory_in_conversation(&conv2.id, "carbonara", 5)
            .unwrap();
        assert_eq!(scoped_hit.len(), 1);
        assert_eq!(scoped_hit[0].turn_id.as_str(), "turn_cook_01");
    }

    #[test]
    fn test_fts5_query_sanitization_resilience() {
        // Unclosed quotes and punctuation must not crash or trigger SQL syntax errors
        assert_eq!(sanitize_fts5_query("\"unclosed"), "\"unclosed\"*");
        assert_eq!(
            sanitize_fts5_query("hello: world*"),
            "\"hello\"* \"world\"*"
        );
        assert_eq!(
            sanitize_fts5_query("OR AND NOT"),
            "\"OR\"* \"AND\"* \"NOT\"*"
        );
        assert_eq!(sanitize_fts5_query("   "), "");
        assert_eq!(sanitize_fts5_query("+++"), "");

        let db = SqliteDb::open_in_memory().unwrap();
        let store = ConversationStore::new(db);
        let conv = store.create_conversation("Punctuation Test", None).unwrap();

        store
            .add_turn(&ConversationTurn {
                id: TurnId::from_sequence(1),
                conversation_id: conv.id.clone(),
                turn_index: 0,
                role: TurnRole::User,
                content: "Testing C++20 and Python 3.11 with SQLite's FTS5 engine!".to_string(),
                timestamp_ms: 1000,
                token_count: 10,
                tool_calls: None,
                metadata: TurnMetadata::default(),
            })
            .unwrap();

        // Search with punctuation-heavy queries
        let res1 = store.search_episodic_memory("C++20", 5).unwrap();
        assert_eq!(res1.len(), 1);

        let res2 = store.search_episodic_memory("FTS5*", 5).unwrap();
        assert_eq!(res2.len(), 1);

        // Completely empty / invalid punctuation query returns empty list without error
        let empty_res = store.search_episodic_memory("!@#$%^&*()", 5).unwrap();
        assert!(empty_res.is_empty());
    }

    #[test]
    fn test_memory_transparency_json_export_and_purge() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = ConversationStore::new(db);

        let conv = store
            .create_conversation(
                "Exportable Session",
                Some(ConversationMetadata {
                    pinned: true,
                    tags: vec!["privacy".to_string()],
                    summary: None,
                    model_id: Some("mistral-7b".to_string()),
                    system_prompt: None,
                }),
            )
            .unwrap();

        store
            .add_turn(&ConversationTurn {
                id: TurnId::from_sequence(10),
                conversation_id: conv.id.clone(),
                turn_index: 0,
                role: TurnRole::User,
                content: "Export this conversation please.".to_string(),
                timestamp_ms: 5000,
                token_count: 5,
                tool_calls: None,
                metadata: TurnMetadata::default(),
            })
            .unwrap();

        // Export to JSON
        let json_str = store.export_conversation_json(&conv.id).unwrap();
        assert!(json_str.contains("Exportable Session"));
        assert!(json_str.contains("Export this conversation please."));
        assert!(json_str.contains("mistral-7b"));

        // Verify JSON deserializes cleanly into ExportedConversation
        let parsed: ExportedConversation = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.conversation.id, conv.id);
        assert_eq!(parsed.turns.len(), 1);
        assert_eq!(parsed.turns[0].content, "Export this conversation please.");

        // Purge all conversations
        store.purge_all_conversations().unwrap();

        let convs_after = store.list_conversations(10, 0).unwrap();
        assert!(convs_after.is_empty());

        let turns_after = store.list_turns(&conv.id, None).unwrap();
        assert!(turns_after.is_empty());

        let search_after = store.search_episodic_memory("Export", 5).unwrap();
        assert!(search_after.is_empty());
    }

    #[test]
    fn test_tool_calls_serialization_and_retrieval() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = ConversationStore::new(db);

        let conv = store.create_conversation("Tool Test", None).unwrap();

        let tool_payload = serde_json::json!([
            {
                "tool": "file.read",
                "arguments": { "path": "/etc/hosts" }
            }
        ]);

        let turn = ConversationTurn {
            id: TurnId::from_sequence(42),
            conversation_id: conv.id.clone(),
            turn_index: 0,
            role: TurnRole::Assistant,
            content: "Reading file /etc/hosts for you.".to_string(),
            timestamp_ms: 3000,
            token_count: 14,
            tool_calls: Some(tool_payload.clone()),
            metadata: TurnMetadata {
                model_id: Some("tool-model".to_string()),
                latency_ms: Some(250),
                finish_reason: Some("tool_call".to_string()),
                audio_duration_ms: None,
            },
        };

        store.add_turn(&turn).unwrap();

        let retrieved = store.get_turn(&turn.id).unwrap().unwrap();
        assert_eq!(retrieved.tool_calls, Some(tool_payload));
        assert_eq!(
            retrieved.metadata.finish_reason.as_deref(),
            Some("tool_call")
        );
    }

    #[test]
    fn test_error_branches_and_turn_fts_cleanup() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = ConversationStore::new(db);

        // Attempting to add turn to non-existent conversation fails
        let bogus_conv_id = ConversationId::new("non_existent").unwrap();
        let orphan_turn = ConversationTurn {
            id: TurnId::from_sequence(99),
            conversation_id: bogus_conv_id.clone(),
            turn_index: 0,
            role: TurnRole::User,
            content: "Should fail".to_string(),
            timestamp_ms: 100,
            token_count: 2,
            tool_calls: None,
            metadata: TurnMetadata::default(),
        };

        let err = store.add_turn(&orphan_turn).unwrap_err();
        assert!(matches!(
            err,
            StorageError::ConversationNotFound { ref id } if id == bogus_conv_id.as_str()
        ));

        // Updating title or metadata of non-existent conversation fails
        assert!(matches!(
            store
                .update_conversation_title(&bogus_conv_id, "New Title")
                .unwrap_err(),
            StorageError::ConversationNotFound { .. }
        ));

        assert!(matches!(
            store
                .update_conversation_metadata(&bogus_conv_id, &ConversationMetadata::default())
                .unwrap_err(),
            StorageError::ConversationNotFound { .. }
        ));

        // Create valid conversation and turn
        let conv = store.create_conversation("Cleanup Test", None).unwrap();
        let turn = ConversationTurn {
            id: TurnId::from_sequence(101),
            conversation_id: conv.id.clone(),
            turn_index: 0,
            role: TurnRole::User,
            content: "Ephemeral secret key xyzzy".to_string(),
            timestamp_ms: 200,
            token_count: 5,
            tool_calls: None,
            metadata: TurnMetadata::default(),
        };

        store.add_turn(&turn).unwrap();

        // Verify FTS search finds it
        let hits_before = store.search_episodic_memory("xyzzy", 5).unwrap();
        assert_eq!(hits_before.len(), 1);

        // Delete turn
        store.delete_turn(&turn.id).unwrap();

        // Verify FTS trigger removed it from FTS index
        let hits_after = store.search_episodic_memory("xyzzy", 5).unwrap();
        assert!(hits_after.is_empty());

        // Deleting non-existent turn returns TurnNotFound
        assert!(matches!(
            store.delete_turn(&turn.id).unwrap_err(),
            StorageError::TurnNotFound { .. }
        ));
    }

    #[test]
    fn test_turn_role_conversions() {
        assert_eq!(TurnRole::User.as_str(), "user");
        assert_eq!(TurnRole::Assistant.as_str(), "assistant");
        assert_eq!(TurnRole::System.as_str(), "system");
        assert_eq!(TurnRole::Tool.as_str(), "tool");

        assert_eq!(TurnRole::from_str("USER").unwrap(), TurnRole::User);
        assert_eq!(
            TurnRole::from_str("assistant").unwrap(),
            TurnRole::Assistant
        );
        assert_eq!(TurnRole::from_str("System").unwrap(), TurnRole::System);
        assert_eq!(TurnRole::from_str("tool").unwrap(), TurnRole::Tool);

        assert!(TurnRole::from_str("invalid_role").is_err());
        assert_eq!(format!("{}", TurnRole::User), "user");
    }
}
