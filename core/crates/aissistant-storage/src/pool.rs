//! Thread-safe SQLite connection pool and WAL initialization engine.

use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::error::StorageError;
use crate::migration::MigrationRunner;

/// Shared thread-safe handle to a local SQLite database instance with WAL mode.
#[derive(Debug, Clone)]
pub struct SqliteDb {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteDb {
    /// Opens an in-memory SQLite database configured with WAL/foreign keys and applies migrations.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let mut conn = Connection::open_in_memory()?;
        Self::configure_pragmas(&mut conn, false)?;
        MigrationRunner::run_migrations(&mut conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Opens or creates a file-backed SQLite database at the specified path with WAL mode.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let mut conn = Connection::open(p)?;
        Self::configure_pragmas(&mut conn, true)?;
        MigrationRunner::run_migrations(&mut conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Configures mandatory SQLite pragmas for performance, durability, and concurrency.
    fn configure_pragmas(conn: &mut Connection, is_file: bool) -> Result<(), StorageError> {
        if is_file {
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;",
            )?;
        }
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA temp_store = MEMORY;",
        )?;
        Ok(())
    }

    /// Executes an operation with exclusive access to the underlying connection.
    pub fn with_conn<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut Connection) -> Result<R, StorageError>,
    {
        let mut conn = self.conn.lock().map_err(|e| {
            StorageError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::other(e.to_string()),
            )))
        })?;
        f(&mut conn)
    }

    /// Executes an operation within an ACID transaction, rolling back automatically on error.
    pub fn transaction<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> Result<R, StorageError>,
    {
        self.with_conn(|conn| {
            let tx = conn.transaction()?;
            let result = f(&tx)?;
            tx.commit()?;
            Ok(result)
        })
    }

    /// Reclaims SQLite memory by executing `PRAGMA shrink_memory` and checkpointing WAL.
    pub fn trim_memory(&self) -> Result<(), StorageError> {
        self.with_conn(|conn| {
            conn.execute_batch(
                "PRAGMA shrink_memory;
                 PRAGMA wal_checkpoint(PASSIVE);",
            )?;
            Ok(())
        })
    }

    /// Sets SQLite cache size in kibibytes (negative value) or pages (positive value).
    pub fn set_cache_size(&self, cache_size: i32) -> Result<(), StorageError> {
        self.with_conn(|conn| {
            conn.execute(&format!("PRAGMA cache_size = {cache_size};"), [])?;
            Ok(())
        })
    }

    /// Returns estimated SQLite database size on disk/memory in bytes.
    pub fn memory_used_bytes(&self) -> Result<i64, StorageError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("PRAGMA page_count;")?;
            let page_count: i64 = stmt.query_row([], |r| r.get(0))?;
            let mut stmt_size = conn.prepare("PRAGMA page_size;")?;
            let page_size: i64 = stmt_size.query_row([], |r| r.get(0))?;
            Ok(page_count.saturating_mul(page_size))
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_db_creation() {
        let db = SqliteDb::open_in_memory();
        assert!(db.is_ok());
    }

    #[test]
    fn test_file_backed_db_creation() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test_assistant.db");

        let db = SqliteDb::open(&db_path).unwrap();
        let journal_mode: String = db
            .with_conn(|conn| {
                let mut stmt = conn.prepare("PRAGMA journal_mode;")?;
                let mode: String = stmt.query_row([], |row| row.get(0))?;
                Ok(mode)
            })
            .unwrap();

        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    }

    #[test]
    fn test_transaction_rollback_on_error() {
        let db = SqliteDb::open_in_memory().unwrap();

        db.with_conn(|conn| {
            conn.execute(
                "CREATE TABLE test_tx (id INTEGER PRIMARY KEY, name TEXT);",
                [],
            )?;
            Ok(())
        })
        .unwrap();

        let tx_result: Result<(), StorageError> = db.transaction(|tx| {
            tx.execute("INSERT INTO test_tx (id, name) VALUES (1, 'item1')", [])?;
            // Simulate error
            Err(StorageError::NotFound {
                key: "fail".to_string(),
            })
        });

        assert!(tx_result.is_err());

        // Verify row was rolled back
        let count: i64 = db
            .with_conn(|conn| {
                conn.query_row("SELECT COUNT(*) FROM test_tx;", [], |row| row.get(0))
                    .map_err(StorageError::Sqlite)
            })
            .unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn test_trim_memory_and_cache_tuning() {
        let db = SqliteDb::open_in_memory().unwrap();
        assert!(db.trim_memory().is_ok());
        assert!(db.set_cache_size(-2000).is_ok());
        let mem = db.memory_used_bytes().unwrap();
        assert!(mem >= 0);
    }
}
