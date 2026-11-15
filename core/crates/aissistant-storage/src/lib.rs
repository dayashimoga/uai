//! SQLite WAL persistence engine, schema migrations, typed settings, and tamper-evident audit log.

#![forbid(unsafe_code)]

pub mod audit_store;
pub mod error;
pub mod migration;
pub mod pool;
pub mod settings_store;

pub use audit_store::AuditStore;
pub use error::StorageError;
pub use migration::{Migration, MigrationRunner};
pub use pool::SqliteDb;
pub use settings_store::SettingsStore;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_core::{AuditEntryId, SkillId};
    use aissistant_security::{
        Actor, AuditEntry, AuditExecutionStatus, PolicyDecision, GENESIS_PREV_HASH,
    };

    #[test]
    fn test_storage_subsystem_end_to_end() {
        let db = SqliteDb::open_in_memory().unwrap();
        let settings = SettingsStore::new(db.clone());
        let audit = AuditStore::new(db);

        // Store and retrieve a setting
        settings
            .set("assistant.name", &"Jarvis".to_string())
            .unwrap();
        let name: Option<String> = settings.get("assistant.name").unwrap();
        assert_eq!(name, Some("Jarvis".to_string()));

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
    }
}
