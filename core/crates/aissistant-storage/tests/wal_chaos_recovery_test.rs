#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::useless_vec
)]

//! Chaos and resilience tests for SQLite WAL corruption detection, quarantine, and recovery.

use std::fs::{self, OpenOptions};
use std::io::Write;
use tempfile::tempdir;

use aissistant_storage::SqliteDb;

#[test]
fn test_wal_corruption_detection_and_automatic_recovery() {
    let tmp = tempdir().expect("Failed to create tempdir");
    let db_path = tmp.path().join("chaos_wal_test.db");

    // Phase 1: Create database, populate initial state
    {
        let db = SqliteDb::open(&db_path).expect("Failed to open new db");
        db.with_conn(|conn| {
            conn.execute(
                "CREATE TABLE test_chaos (id INTEGER PRIMARY KEY, payload TEXT NOT NULL);",
                [],
            )?;
            for i in 1..=20 {
                conn.execute(
                    "INSERT INTO test_chaos (id, payload) VALUES (?1, ?2);",
                    rusqlite::params![i, format!("record_{i}")],
                )?;
            }
            Ok(())
        })
        .expect("Failed to insert initial records");

        assert!(db.check_integrity().expect("Integrity check failed"));
    }

    // Identify WAL file path
    let mut wal_path = db_path.as_os_str().to_os_string();
    wal_path.push("-wal");
    let wal_file = std::path::PathBuf::from(wal_path);

    // If WAL file was checkpointed away, write an active uncheckpointed transaction
    if !wal_file.exists() {
        let db = SqliteDb::open(&db_path).expect("Reopen db");
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO test_chaos (id, payload) VALUES (999, 'uncheckpointed');",
                [],
            )?;
            Ok(())
        })
        .expect("Insert in WAL");
    }

    // Guarantee WAL exists by creating/corrupting it with corrupted magic bytes
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&wal_file)
        .expect("Open WAL for corruption");
    // Write 512 bytes of invalid garbage (destroys WAL header)
    let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0xFF, 0xAA, 0x55];
    file.write_all(&garbage.repeat(64))
        .expect("Write corrupted WAL header");
    file.flush().expect("Flush corrupted WAL");
    drop(file);

    assert!(
        wal_file.exists(),
        "Corrupt WAL file must exist before recovery"
    );

    // Phase 2: Open database with corrupted WAL.
    // SqliteDb::open must automatically detect the corruption, quarantine the corrupt WAL,
    // and successfully open the database.
    let recovered_db = SqliteDb::open(&db_path).expect("SqliteDb::open must recover gracefully");

    // Check integrity after recovery
    let is_intact = recovered_db
        .check_integrity()
        .expect("Check integrity after recovery");
    assert!(is_intact, "Recovered database must pass integrity check");

    // Verify reading from table succeeds
    let count: i64 = recovered_db
        .with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM test_chaos;", [], |row| row.get(0))
                .map_err(aissistant_storage::StorageError::Sqlite)
        })
        .expect("Query recovered table");
    assert!(count >= 20, "Original committed records must be intact");

    // Verify inserting new records into recovered database succeeds
    recovered_db
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO test_chaos (id, payload) VALUES (1001, 'recovered_insert');",
                [],
            )?;
            Ok(())
        })
        .expect("Insert into recovered db must succeed");

    // Verify quarantined file was created in directory
    let mut found_quarantine = false;
    for entry in fs::read_dir(tmp.path()).expect("Read dir") {
        let entry = entry.expect("Dir entry");
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains("-wal.corrupt.") {
            found_quarantine = true;
            break;
        }
    }
    assert!(
        found_quarantine,
        "A quarantined corrupt WAL backup must exist in the directory"
    );
}

#[test]
fn test_manual_recover_corrupt_wal() {
    let tmp = tempdir().expect("Failed to create tempdir");
    let db_path = tmp.path().join("manual_recover.db");

    // Create DB in scope so connection is cleanly dropped
    {
        let _db = SqliteDb::open(&db_path).expect("Open db");
    }

    let mut wal_os = db_path.as_os_str().to_os_string();
    wal_os.push("-wal");
    let wal_path = std::path::PathBuf::from(wal_os);

    let mut shm_os = db_path.as_os_str().to_os_string();
    shm_os.push("-shm");
    let shm_path = std::path::PathBuf::from(shm_os);

    fs::write(&wal_path, b"corrupted wal header bytes").expect("Write wal");
    fs::write(&shm_path, b"corrupted shm bytes").expect("Write shm");

    let quarantined = SqliteDb::recover_corrupt_wal(&db_path).expect("Recover corrupt WAL");
    assert!(quarantined);
    assert!(!wal_path.exists());
    assert!(!shm_path.exists());
}
