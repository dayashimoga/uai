//! SQLite WAL persistence engine, schema migrations, typed settings, keystore providers, and encrypted secret store.

#![forbid(unsafe_code)]

pub mod audit_store;
pub mod conversation_store;
pub mod device_registry;
pub mod error;
pub mod hybrid_search;
pub mod keystore;
pub mod migration;
pub mod pool;
pub mod secret_store;
pub mod settings_store;
pub mod vector_store;

pub use audit_store::AuditStore;
pub use conversation_store::{
    sanitize_fts5_query, Conversation, ConversationMetadata, ConversationStore, ConversationTurn,
    EpisodicSearchResult, ExportedConversation, TurnMetadata, TurnRole,
};
pub use device_registry::DeviceRegistry;
pub use error::StorageError;
pub use hybrid_search::{HybridSearchEngine, HybridSearchResult, RrfConfig};
pub use keystore::{
    derive_key_from_secret, FileBoundKeystore, InMemoryKeystore, KeystoreProvider, OsKeystore,
};
pub use migration::{Migration, MigrationRunner};
pub use pool::SqliteDb;
pub use secret_store::SecretStore;
pub use settings_store::SettingsStore;
pub use vector_store::{
    blob_to_vec, cosine_similarity, l2_norm, vec_to_blob, VectorMemoryStore, VectorRecord,
    VectorSearchResult,
};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_core::{AuditEntryId, SkillId};
    use aissistant_security::{
        Actor, AeadKey, AuditEntry, AuditExecutionStatus, PolicyDecision, GENESIS_PREV_HASH,
    };

    #[test]
    fn test_storage_subsystem_end_to_end() {
        let db = SqliteDb::open_in_memory().unwrap();
        let settings = SettingsStore::new(db.clone());
        let audit = AuditStore::new(db.clone());
        let master_key = AeadKey::generate();
        let secrets = SecretStore::new(db.clone(), master_key);

        // Store and retrieve a setting
        settings
            .set("assistant.name", &"Jarvis".to_string())
            .unwrap();
        let name: Option<String> = settings.get("assistant.name").unwrap();
        assert_eq!(name, Some("Jarvis".to_string()));

        // Store and retrieve an encrypted secret
        secrets
            .set("wifi.password", &"ultra_secure_pass_123".to_string())
            .unwrap();
        let pass: Option<String> = secrets.get("wifi.password").unwrap();
        assert_eq!(pass, Some("ultra_secure_pass_123".to_string()));

        // Append and verify audit chain
        let entry = AuditEntry::create(
            AuditEntryId::from_sequence(1),
            12345,
            Actor::LocalUser,
            SkillId::new("system.time").unwrap(),
            "{}",
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
            GENESIS_PREV_HASH,
        );
        audit.append(&entry).unwrap();
        assert_eq!(audit.verify_integrity().unwrap(), entry.entry_hash);

        // Store conversation, add turn, and search episodic memory
        let conversations = ConversationStore::new(db.clone());
        let conv = conversations
            .create_conversation("Integration Test", None)
            .unwrap();
        let turn = ConversationTurn {
            id: aissistant_core::TurnId::from_sequence(1),
            conversation_id: conv.id.clone(),
            turn_index: 0,
            role: TurnRole::User,
            content: "Remember to buy milk and eggs tomorrow".to_string(),
            timestamp_ms: 123456,
            token_count: 8,
            tool_calls: None,
            metadata: TurnMetadata::default(),
        };
        conversations.add_turn(&turn).unwrap();

        let search = conversations.search_episodic_memory("milk", 5).unwrap();
        assert_eq!(search.len(), 1);
        assert!(search[0].snippet.contains("<mark>milk</mark>"));

        // Store vector embedding and run hybrid search
        let vectors = VectorMemoryStore::new(db);
        vectors
            .store_embedding(&VectorRecord {
                id: "vec_integ_01".to_string(),
                entity_type: "turn".to_string(),
                entity_id: turn.id.as_str().to_string(),
                conversation_id: Some(conv.id.as_str().to_string()),
                dimension: 2,
                embedding: vec![1.0, 0.0],
                content_snippet: turn.content.clone(),
                created_at_ms: 123456,
            })
            .unwrap();

        let hybrid = HybridSearchEngine::new(conversations, vectors);
        let hybrid_results = hybrid.search("milk", &[1.0, 0.0], 5, None, None).unwrap();
        assert_eq!(hybrid_results.len(), 1);
        assert_eq!(hybrid_results[0].turn_id, turn.id);
        assert!(hybrid_results[0].bm25_rank.is_some());
        assert!(hybrid_results[0].vector_rank.is_some());
    }
}
