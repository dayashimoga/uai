//! Encrypted secret repository protecting sensitive credentials and personal data at rest.

use aissistant_security::AeadKey;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::StorageError;
use crate::pool::SqliteDb;

/// Symmetrically encrypted secret repository backed by SQLite.
#[derive(Debug, Clone)]
pub struct SecretStore {
    db: SqliteDb,
    key: Arc<AeadKey>,
}

impl SecretStore {
    /// Creates a new secret store instance using the database and active AEAD key.
    #[must_use]
    pub fn new(db: SqliteDb, master_key: AeadKey) -> Self {
        Self {
            db,
            key: Arc::new(master_key),
        }
    }

    /// Creates a new secret store using an existing shared `AeadKey`.
    #[must_use]
    pub const fn with_shared_key(db: SqliteDb, key: Arc<AeadKey>) -> Self {
        Self { db, key }
    }

    /// Serializes, AEAD-encrypts, and persists a secret value under the given key.
    pub fn set<T: Serialize>(&self, key: &str, secret: &T) -> Result<(), StorageError> {
        let plaintext_json = serde_json::to_vec(secret)?;
        // Authenticate the key name itself as Associated Authenticated Data (AAD)
        // to prevent ciphertext swapping across distinct keys.
        let ciphertext = self
            .key
            .encrypt(&plaintext_json, key.as_bytes())
            .map_err(|e| StorageError::Serialization {
                message: e.to_string(),
            })?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO encrypted_secrets (key, ciphertext_blob, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET
                    ciphertext_blob = excluded.ciphertext_blob,
                    updated_at_ms = excluded.updated_at_ms;",
                rusqlite::params![key, ciphertext, now_ms],
            )?;
            Ok(())
        })
    }

    /// Retrieves, decrypts, and deserializes a stored secret by key.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT ciphertext_blob FROM encrypted_secrets WHERE key = ?1;")?;
            let mut rows = stmt.query(rusqlite::params![key])?;

            if let Some(row) = rows.next()? {
                let ciphertext: Vec<u8> = row.get(0)?;
                let plaintext = self.key.decrypt(&ciphertext, key.as_bytes()).map_err(|e| {
                    StorageError::CorruptAuditChain {
                        reason: format!("Secret decryption failed for key '{key}': {e}"),
                    }
                })?;

                let value: T = serde_json::from_slice(&plaintext)?;
                Ok(Some(value))
            } else {
                Ok(None)
            }
        })
    }

    /// Deletes an encrypted secret by key. Returns true if the key existed.
    pub fn delete(&self, key: &str) -> Result<bool, StorageError> {
        self.db.with_conn(|conn| {
            let rows_affected = conn.execute(
                "DELETE FROM encrypted_secrets WHERE key = ?1;",
                rusqlite::params![key],
            )?;
            Ok(rows_affected > 0)
        })
    }

    /// Lists all registered secret keys in lexicographical order.
    pub fn list_keys(&self) -> Result<Vec<String>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT key FROM encrypted_secrets ORDER BY key ASC;")?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            let mut keys = Vec::new();
            for key in rows {
                keys.push(key?);
            }
            Ok(keys)
        })
    }

    /// Atomically rotates the master AEAD key by re-encrypting all stored records.
    pub fn rotate_key(&mut self, new_key: AeadKey) -> Result<(), StorageError> {
        let new_key = Arc::new(new_key);
        let current_keys = self.list_keys()?;

        self.db.transaction(|tx| {
            for key in &current_keys {
                let mut stmt =
                    tx.prepare("SELECT ciphertext_blob FROM encrypted_secrets WHERE key = ?1;")?;
                let ciphertext: Vec<u8> =
                    stmt.query_row(rusqlite::params![key], |row| row.get(0))?;

                let plaintext = self.key.decrypt(&ciphertext, key.as_bytes()).map_err(|e| {
                    StorageError::CorruptAuditChain {
                        reason: format!("Key rotation decryption failed for '{key}': {e}"),
                    }
                })?;

                let new_ciphertext = new_key.encrypt(&plaintext, key.as_bytes()).map_err(|e| {
                    StorageError::Serialization {
                        message: e.to_string(),
                    }
                })?;

                tx.execute(
                    "UPDATE encrypted_secrets SET ciphertext_blob = ?1 WHERE key = ?2;",
                    rusqlite::params![new_ciphertext, key],
                )?;
            }
            Ok(())
        })?;

        self.key = new_key;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct PeerCredential {
        peer_id: String,
        token: String,
    }

    #[test]
    fn test_secret_store_encryption_and_decryption() {
        let db = SqliteDb::open_in_memory().unwrap();
        let key = AeadKey::generate();
        let store = SecretStore::new(db.clone(), key);

        let cred = PeerCredential {
            peer_id: "phone_01".to_string(),
            token: "super_secret_auth_token_987".to_string(),
        };

        store.set("peer.phone_01", &cred).unwrap();

        // Verify the raw stored SQLite blob does NOT contain the plaintext token
        let raw_blob: Vec<u8> = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT ciphertext_blob FROM encrypted_secrets WHERE key = 'peer.phone_01';",
                    [],
                    |row| row.get(0),
                )
                .map_err(StorageError::Sqlite)
            })
            .unwrap();

        let raw_str = String::from_utf8_lossy(&raw_blob);
        assert!(!raw_str.contains("super_secret_auth_token_987"));

        // Decrypt and verify
        let retrieved: Option<PeerCredential> = store.get("peer.phone_01").unwrap();
        assert_eq!(retrieved, Some(cred));
    }

    #[test]
    fn test_wrong_key_fails_decryption() {
        let db = SqliteDb::open_in_memory().unwrap();
        let key1 = AeadKey::generate();
        let key2 = AeadKey::generate();

        let store1 = SecretStore::new(db.clone(), key1);
        store1
            .set("secret.key", &"secret_data".to_string())
            .unwrap();

        // Attempt retrieval with different store instantiated with key2
        let store2 = SecretStore::new(db, key2);
        let result: Result<Option<String>, StorageError> = store2.get("secret.key");
        assert!(result.is_err());
    }

    #[test]
    fn test_key_rotation() {
        let db = SqliteDb::open_in_memory().unwrap();
        let key1 = AeadKey::generate();
        let mut store = SecretStore::new(db, key1);

        store.set("secret1", &"value1".to_string()).unwrap();
        store.set("secret2", &"value2".to_string()).unwrap();

        let key2 = AeadKey::generate();
        store.rotate_key(key2).unwrap();

        assert_eq!(
            store.get::<String>("secret1").unwrap(),
            Some("value1".to_string())
        );
        assert_eq!(
            store.get::<String>("secret2").unwrap(),
            Some("value2".to_string())
        );
    }
}
