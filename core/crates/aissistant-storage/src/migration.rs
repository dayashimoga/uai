//! Deterministic versioned schema migration runner.

use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::StorageError;

/// Individual schema migration step.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// Monotonically increasing schema version number.
    pub version: u32,
    /// Short description of the schema modification.
    pub description: &'static str,
    /// SQL DDL payload.
    pub sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "Initial schema: settings key-value store and security audit log table",
        sql: "
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS security_audit_log (
                id TEXT PRIMARY KEY,
                timestamp_ms INTEGER NOT NULL,
                actor_json TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                arguments_redacted_json TEXT NOT NULL,
                policy_decision_json TEXT NOT NULL,
                execution_status_json TEXT NOT NULL,
                prev_hash BLOB NOT NULL,
                entry_hash BLOB NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON security_audit_log(timestamp_ms);
        ",
    },
    Migration {
        version: 2,
        description: "Encrypted secrets store for credentials and private personal memory",
        sql: "
            CREATE TABLE IF NOT EXISTS encrypted_secrets (
                key TEXT PRIMARY KEY,
                ciphertext_blob BLOB NOT NULL,
                updated_at_ms INTEGER NOT NULL
            );
        ",
    },
    Migration {
        version: 3,
        description: "Multi-session conversation persistence, turn history, and SQLite FTS5 episodic search",
        sql: "
            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                metadata_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS conversation_turns (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                turn_index INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                token_count INTEGER NOT NULL,
                tool_calls_json TEXT,
                metadata_json TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_turns_conversation ON conversation_turns(conversation_id, turn_index);
            CREATE INDEX IF NOT EXISTS idx_turns_timestamp ON conversation_turns(timestamp_ms);

            CREATE VIRTUAL TABLE IF NOT EXISTS conversation_turns_fts USING fts5(
                turn_id UNINDEXED,
                conversation_id UNINDEXED,
                role UNINDEXED,
                content,
                tokenize = 'unicode61'
            );

            CREATE TRIGGER IF NOT EXISTS trg_turns_ai AFTER INSERT ON conversation_turns
            BEGIN
                INSERT INTO conversation_turns_fts(rowid, turn_id, conversation_id, role, content)
                VALUES (new.rowid, new.id, new.conversation_id, new.role, new.content);
            END;

            CREATE TRIGGER IF NOT EXISTS trg_turns_ad AFTER DELETE ON conversation_turns
            BEGIN
                DELETE FROM conversation_turns_fts WHERE rowid = old.rowid;
            END;

            CREATE TRIGGER IF NOT EXISTS trg_turns_au AFTER UPDATE ON conversation_turns
            BEGIN
                DELETE FROM conversation_turns_fts WHERE rowid = old.rowid;
                INSERT INTO conversation_turns_fts(rowid, turn_id, conversation_id, role, content)
                VALUES (new.rowid, new.id, new.conversation_id, new.role, new.content);
            END;
        ",
    },
    Migration {
        version: 4,
        description: "Dense vector embeddings store for hybrid episodic and semantic memory retrieval",
        sql: "
            CREATE TABLE IF NOT EXISTS semantic_embeddings (
                id TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                conversation_id TEXT,
                dimension INTEGER NOT NULL,
                embedding_blob BLOB NOT NULL,
                content_snippet TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_embeddings_entity ON semantic_embeddings(entity_type, entity_id);
            CREATE INDEX IF NOT EXISTS idx_embeddings_conversation ON semantic_embeddings(conversation_id);

            CREATE TRIGGER IF NOT EXISTS trg_turns_del_embeddings AFTER DELETE ON conversation_turns
            BEGIN
                DELETE FROM semantic_embeddings WHERE entity_type = 'turn' AND entity_id = old.id;
            END;

            CREATE TRIGGER IF NOT EXISTS trg_conv_del_embeddings AFTER DELETE ON conversations
            BEGIN
                DELETE FROM semantic_embeddings WHERE conversation_id = old.id;
            END;
        ",
    },
];

/// Migration runner applying incremental DDL changes within ACID transactions.
#[derive(Debug, Clone, Copy, Default)]
pub struct MigrationRunner;

impl MigrationRunner {
    /// Returns the current schema version recorded in the database.
    pub fn current_version(conn: &Connection) -> Result<u32, StorageError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at_ms INTEGER NOT NULL,
                description TEXT NOT NULL
            );",
        )?;

        let mut stmt = conn.prepare("SELECT COALESCE(MAX(version), 0) FROM schema_migrations;")?;
        let version: u32 = stmt.query_row([], |row| row.get(0))?;
        Ok(version)
    }

    /// Evaluates and applies all unapplied migrations in sequential order.
    pub fn run_migrations(conn: &mut Connection) -> Result<u32, StorageError> {
        let current_version = Self::current_version(conn)?;
        let tx = conn.transaction()?;

        let mut latest_version = current_version;

        for migration in MIGRATIONS {
            if migration.version > current_version {
                tx.execute_batch(migration.sql)
                    .map_err(|e| StorageError::Migration {
                        version: migration.version,
                        message: format!("Failed applying migration SQL: {e}"),
                    })?;

                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);

                tx.execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms, description)
                     VALUES (?1, ?2, ?3);",
                    rusqlite::params![migration.version, now_ms, migration.description],
                )
                .map_err(|e| StorageError::Migration {
                    version: migration.version,
                    message: format!("Failed recording migration record: {e}"),
                })?;

                latest_version = migration.version;
            }
        }

        tx.commit()?;
        Ok(latest_version)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_runner_applies_v1_and_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();

        // Initial run
        let version = MigrationRunner::run_migrations(&mut conn).unwrap();
        assert_eq!(version, 4);
        assert_eq!(MigrationRunner::current_version(&conn).unwrap(), 4);

        // Verify tables exist
        let tables_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('settings', 'security_audit_log', 'encrypted_secrets', 'schema_migrations', 'conversations', 'conversation_turns', 'conversation_turns_fts', 'semantic_embeddings');",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables_count, 8);

        // Second run must be a no-op
        let version_again = MigrationRunner::run_migrations(&mut conn).unwrap();
        assert_eq!(version_again, 4);
    }
}
