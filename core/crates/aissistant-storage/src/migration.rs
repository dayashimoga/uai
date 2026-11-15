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
        assert_eq!(version, 2);
        assert_eq!(MigrationRunner::current_version(&conn).unwrap(), 2);

        // Verify tables exist
        let tables_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('settings', 'security_audit_log', 'encrypted_secrets', 'schema_migrations');",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables_count, 4);

        // Second run must be a no-op
        let version_again = MigrationRunner::run_migrations(&mut conn).unwrap();
        assert_eq!(version_again, 2);
    }
}
