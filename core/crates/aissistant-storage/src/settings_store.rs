//! Persistent key-value settings store backed by SQLite.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::StorageError;
use crate::pool::SqliteDb;

/// High-level typed settings repository.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    db: SqliteDb,
}

impl SettingsStore {
    /// Creates a new settings store using the provided database handle.
    #[must_use]
    pub const fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    /// Retrieves and deserializes a stored configuration value by key.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT value_json FROM settings WHERE key = ?1;")?;
            let mut rows = stmt.query(rusqlite::params![key])?;

            if let Some(row) = rows.next()? {
                let json_str: String = row.get(0)?;
                let value: T = serde_json::from_str(&json_str)?;
                Ok(Some(value))
            } else {
                Ok(None)
            }
        })
    }

    /// Serializes and stores a configuration value under the given key.
    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), StorageError> {
        let json_str = serde_json::to_string(value)?;
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO settings (key, value_json, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET
                    value_json = excluded.value_json,
                    updated_at_ms = excluded.updated_at_ms;",
                rusqlite::params![key, json_str, now_ms],
            )?;
            Ok(())
        })
    }

    /// Deletes a key from the settings store. Returns true if the key existed.
    pub fn delete(&self, key: &str) -> Result<bool, StorageError> {
        self.db.with_conn(|conn| {
            let rows_affected = conn.execute(
                "DELETE FROM settings WHERE key = ?1;",
                rusqlite::params![key],
            )?;
            Ok(rows_affected > 0)
        })
    }

    /// Lists all registered settings keys in lexicographical order.
    pub fn list_keys(&self) -> Result<Vec<String>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT key FROM settings ORDER BY key ASC;")?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            let mut keys = Vec::new();
            for key in rows {
                keys.push(key?);
            }
            Ok(keys)
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_core::VoiceConfig;

    #[test]
    fn test_settings_store_lifecycle() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = SettingsStore::new(db);

        // Missing key
        let initial: Option<VoiceConfig> = store.get("voice_settings").unwrap();
        assert!(initial.is_none());

        // Set value
        let voice = VoiceConfig {
            wake_word_sensitivity: 0.85,
            ..VoiceConfig::default()
        };
        store.set("voice_settings", &voice).unwrap();

        // Get value
        let retrieved: Option<VoiceConfig> = store.get("voice_settings").unwrap();
        assert_eq!(retrieved, Some(voice));

        // Update value
        store.set("app_theme", &"dark".to_string()).unwrap();

        let keys = store.list_keys().unwrap();
        assert_eq!(keys, vec!["app_theme", "voice_settings"]);

        // Delete key
        let deleted = store.delete("app_theme").unwrap();
        assert!(deleted);
        let deleted_again = store.delete("app_theme").unwrap();
        assert!(!deleted_again);
    }
}
