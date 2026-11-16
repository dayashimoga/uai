//! Thread-safe SQLite connection pool and WAL initialization engine.

use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

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
    /// If an existing WAL or SHM file is corrupted (e.g. following sudden power loss or crash),
    /// automatically quarantines the corrupted WAL file and restores operational database access.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Pre-validate existing WAL file header if present
        let mut wal_os = p.as_os_str().to_os_string();
        wal_os.push("-wal");
        let wal_path = PathBuf::from(wal_os);
        if wal_path.exists() && !Self::is_wal_header_valid(&wal_path) {
            let _ = Self::recover_corrupt_wal(p);
        }

        match Self::open_internal(p) {
            Ok(db) => Ok(db),
            Err(err) => {
                // If opening failed due to corruption / invalid database, attempt WAL recovery
                if Self::is_corruption_error(&err) {
                    if let Ok(recovered) = Self::recover_corrupt_wal(p) {
                        if recovered {
                            return Self::open_internal(p);
                        }
                    }
                }
                Err(err)
            }
        }
    }

    /// Validates if an existing WAL file header has the required 32-byte SQLite WAL magic and structure.
    pub fn is_wal_header_valid(wal_path: &Path) -> bool {
        if !wal_path.exists() {
            return true;
        }

        let Ok(metadata) = std::fs::metadata(wal_path) else {
            return false;
        };

        // If file exists but is smaller than the 32-byte WAL header, it is truncated/corrupt
        if metadata.len() < 32 {
            return false;
        }

        let Ok(mut file) = std::fs::File::open(wal_path) else {
            return false;
        };

        use std::io::Read;
        let mut header = [0u8; 32];
        if file.read_exact(&mut header).is_err() {
            return false;
        }

        let magic_be = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let magic_le = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let is_magic_valid = magic_be == 0x377f0682
            || magic_be == 0x377f0683
            || magic_le == 0x377f0682
            || magic_le == 0x377f0683;

        if !is_magic_valid {
            return false;
        }

        // Validate page size (bytes 8..12)
        let page_size = if magic_be == 0x377f0682 || magic_be == 0x377f0683 {
            u32::from_be_bytes([header[8], header[9], header[10], header[11]])
        } else {
            u32::from_le_bytes([header[8], header[9], header[10], header[11]])
        };

        // SQLite page sizes must be a power of 2 between 512 and 65536
        (512..=65536).contains(&page_size) && page_size.is_power_of_two()
    }

    /// Internal file opener and schema migrator.
    fn open_internal(path: &Path) -> Result<Self, StorageError> {
        let mut conn = Connection::open(path)?;
        Self::configure_pragmas(&mut conn, true)?;
        MigrationRunner::run_migrations(&mut conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Checks if a storage error indicates corruption or invalid database header.
    fn is_corruption_error(err: &StorageError) -> bool {
        match err {
            StorageError::Sqlite(sql_err) => {
                let msg = sql_err.to_string().to_lowercase();
                msg.contains("malformed")
                    || msg.contains("not a database")
                    || msg.contains("corrupt")
                    || msg.contains("disk i/o error")
            }
            _ => false,
        }
    }

    /// Quarantines any existing `-wal` and `-shm` files to `<path>-wal.corrupt.<timestamp>`
    /// so the primary database can be safely opened and recovered without crashing.
    pub fn recover_corrupt_wal(path: impl AsRef<Path>) -> Result<bool, StorageError> {
        let p = path.as_ref();
        let mut wal_os = p.as_os_str().to_os_string();
        wal_os.push("-wal");
        let wal_path = PathBuf::from(wal_os);

        let mut shm_os = p.as_os_str().to_os_string();
        shm_os.push("-shm");
        let shm_path = PathBuf::from(shm_os);

        let mut quarantined = false;
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        if wal_path.exists() {
            let mut corrupt_os = wal_path.as_os_str().to_os_string();
            corrupt_os.push(format!(".corrupt.{ts}"));
            let corrupt_path = PathBuf::from(corrupt_os);
            if std::fs::rename(&wal_path, &corrupt_path).is_ok()
                || std::fs::remove_file(&wal_path).is_ok()
            {
                quarantined = true;
            }
        }

        if shm_path.exists() {
            let _ = std::fs::remove_file(&shm_path);
            quarantined = true;
        }

        Ok(quarantined)
    }

    /// Runs SQLite `PRAGMA quick_check;` to verify database structural integrity.
    /// Returns `Ok(true)` if integrity check passes with "ok", or `Ok(false)` otherwise.
    pub fn check_integrity(&self) -> Result<bool, StorageError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("PRAGMA quick_check;")?;
            let status: String = stmt.query_row([], |row| row.get(0))?;
            Ok(status.eq_ignore_ascii_case("ok"))
        })
    }

    /// Runs SQLite `PRAGMA integrity_check;` to perform exhaustive structural validation.
    pub fn check_full_integrity(&self) -> Result<bool, StorageError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("PRAGMA integrity_check;")?;
            let status: String = stmt.query_row([], |row| row.get(0))?;
            Ok(status.eq_ignore_ascii_case("ok"))
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

    #[test]
    fn test_check_integrity() {
        let db = SqliteDb::open_in_memory().unwrap();
        assert!(db.check_integrity().unwrap());
        assert!(db.check_full_integrity().unwrap());
    }
}
