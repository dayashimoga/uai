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
}
