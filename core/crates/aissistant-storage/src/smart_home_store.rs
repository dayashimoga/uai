//! Persistent smart-home entity registry, local offline device state cache,
//! and protocol descriptors backed by SQLite.

use std::fmt;
use std::str::FromStr;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::StorageError;
use crate::pool::SqliteDb;

/// Canonical domain category for smart-home entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartHomeDomain {
    Light,
    Switch,
    Climate,
    Sensor,
    Cover,
    Fan,
    Lock,
    MediaPlayer,
}

impl SmartHomeDomain {
    /// Returns the static string representation of this smart home domain.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Switch => "switch",
            Self::Climate => "climate",
            Self::Sensor => "sensor",
            Self::Cover => "cover",
            Self::Fan => "fan",
            Self::Lock => "lock",
            Self::MediaPlayer => "media_player",
        }
    }
}

impl fmt::Display for SmartHomeDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for SmartHomeDomain {
    type Err = StorageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "light" => Ok(Self::Light),
            "switch" => Ok(Self::Switch),
            "climate" => Ok(Self::Climate),
            "sensor" => Ok(Self::Sensor),
            "cover" => Ok(Self::Cover),
            "fan" => Ok(Self::Fan),
            "lock" => Ok(Self::Lock),
            "media_player" => Ok(Self::MediaPlayer),
            other => Err(StorageError::InvalidData(format!(
                "Unknown smart home domain: '{other}'"
            ))),
        }
    }
}

/// Local communication protocol or integration backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartHomeProtocol {
    Mqtt,
    HomeAssistant,
    MatterLocal,
}

impl SmartHomeProtocol {
    /// Returns the static string representation of this protocol.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Mqtt => "mqtt",
            Self::HomeAssistant => "home_assistant",
            Self::MatterLocal => "matter_local",
        }
    }
}

impl fmt::Display for SmartHomeProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for SmartHomeProtocol {
    type Err = StorageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mqtt" => Ok(Self::Mqtt),
            "home_assistant" => Ok(Self::HomeAssistant),
            "matter_local" => Ok(Self::MatterLocal),
            other => Err(StorageError::InvalidData(format!(
                "Unknown smart home protocol: '{other}'"
            ))),
        }
    }
}

/// Persistent record representing a local smart-home entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartHomeEntityRecord {
    /// Unique entity identifier (e.g. "light.living_room", "switch.desk_fan").
    pub id: String,
    /// Domain category.
    pub domain: SmartHomeDomain,
    /// Friendly display name.
    pub name: String,
    /// Serialized JSON representing current entity state (e.g. {"state":"on","brightness":255}).
    pub state_json: String,
    /// Serialized JSON representing device attributes and capabilities.
    pub attributes_json: String,
    /// Local integration protocol.
    pub protocol: SmartHomeProtocol,
    /// Whether the device is currently responsive on the local network.
    pub is_available: bool,
    /// Timestamp in milliseconds of the last state update.
    pub last_updated_ms: i64,
}

/// SQLite-backed persistent repository for local smart-home entities.
#[derive(Debug, Clone)]
pub struct SmartHomeStore {
    db: SqliteDb,
}

impl SmartHomeStore {
    /// Creates a new `SmartHomeStore` wrapping the shared SQLite database.
    #[must_use]
    pub const fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    /// Validates an entity record for schema constraints and valid JSON payloads.
    fn validate_record(record: &SmartHomeEntityRecord) -> Result<(), StorageError> {
        if record.id.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "Entity ID cannot be empty".into(),
            ));
        }
        if record.name.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "Entity name cannot be empty".into(),
            ));
        }
        serde_json::from_str::<serde_json::Value>(&record.state_json).map_err(|e| {
            StorageError::InvalidData(format!("Invalid state_json for '{}': {e}", record.id))
        })?;
        serde_json::from_str::<serde_json::Value>(&record.attributes_json).map_err(|e| {
            StorageError::InvalidData(format!("Invalid attributes_json for '{}': {e}", record.id))
        })?;
        Ok(())
    }

    /// Inserts or updates a smart-home entity in the local registry.
    ///
    /// # Errors
    /// Returns `StorageError::InvalidData` if validation fails or a SQLite error occurs.
    pub fn upsert_entity(&self, record: &SmartHomeEntityRecord) -> Result<(), StorageError> {
        Self::validate_record(record)?;

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO smart_home_entities (
                    id, domain, name, state_json, attributes_json, protocol, is_available, last_updated_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    domain = excluded.domain,
                    name = excluded.name,
                    state_json = excluded.state_json,
                    attributes_json = excluded.attributes_json,
                    protocol = excluded.protocol,
                    is_available = excluded.is_available,
                    last_updated_ms = excluded.last_updated_ms;",
                params![
                    record.id,
                    record.domain.as_str(),
                    record.name,
                    record.state_json,
                    record.attributes_json,
                    record.protocol.as_str(),
                    record.is_available as i32,
                    record.last_updated_ms,
                ],
            )?;
            Ok(())
        })
    }

    /// Retrieves an entity by its unique identifier.
    ///
    /// # Errors
    /// Returns `StorageError` on SQLite query failure.
    pub fn get_entity(&self, id: &str) -> Result<Option<SmartHomeEntityRecord>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, domain, name, state_json, attributes_json, protocol, is_available, last_updated_ms
                 FROM smart_home_entities
                 WHERE id = ?1;",
            )?;

            let mut rows = stmt.query(params![id])?;
            if let Some(row) = rows.next()? {
                let domain_str: String = row.get(1)?;
                let protocol_str: String = row.get(5)?;
                let is_available_int: i32 = row.get(6)?;

                Ok(Some(SmartHomeEntityRecord {
                    id: row.get(0)?,
                    domain: SmartHomeDomain::from_str(&domain_str)?,
                    name: row.get(2)?,
                    state_json: row.get(3)?,
                    attributes_json: row.get(4)?,
                    protocol: SmartHomeProtocol::from_str(&protocol_str)?,
                    is_available: is_available_int != 0,
                    last_updated_ms: row.get(7)?,
                }))
            } else {
                Ok(None)
            }
        })
    }

    /// Lists entities, optionally filtering by domain.
    ///
    /// # Errors
    /// Returns `StorageError` on SQLite query failure.
    pub fn list_entities(
        &self,
        domain_filter: Option<SmartHomeDomain>,
    ) -> Result<Vec<SmartHomeEntityRecord>, StorageError> {
        self.db.with_conn(|conn| {
            let mut result = Vec::new();

            if let Some(dom) = domain_filter {
                let mut stmt = conn.prepare(
                    "SELECT id, domain, name, state_json, attributes_json, protocol, is_available, last_updated_ms
                     FROM smart_home_entities
                     WHERE domain = ?1
                     ORDER BY name ASC;",
                )?;
                let mut rows = stmt.query(params![dom.as_str()])?;
                while let Some(row) = rows.next()? {
                    let domain_str: String = row.get(1)?;
                    let protocol_str: String = row.get(5)?;
                    let is_available_int: i32 = row.get(6)?;

                    result.push(SmartHomeEntityRecord {
                        id: row.get(0)?,
                        domain: SmartHomeDomain::from_str(&domain_str)?,
                        name: row.get(2)?,
                        state_json: row.get(3)?,
                        attributes_json: row.get(4)?,
                        protocol: SmartHomeProtocol::from_str(&protocol_str)?,
                        is_available: is_available_int != 0,
                        last_updated_ms: row.get(7)?,
                    });
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, domain, name, state_json, attributes_json, protocol, is_available, last_updated_ms
                     FROM smart_home_entities
                     ORDER BY domain ASC, name ASC;",
                )?;
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    let domain_str: String = row.get(1)?;
                    let protocol_str: String = row.get(5)?;
                    let is_available_int: i32 = row.get(6)?;

                    result.push(SmartHomeEntityRecord {
                        id: row.get(0)?,
                        domain: SmartHomeDomain::from_str(&domain_str)?,
                        name: row.get(2)?,
                        state_json: row.get(3)?,
                        attributes_json: row.get(4)?,
                        protocol: SmartHomeProtocol::from_str(&protocol_str)?,
                        is_available: is_available_int != 0,
                        last_updated_ms: row.get(7)?,
                    });
                }
            }

            Ok(result)
        })
    }

    /// Updates the state JSON and timestamp of an existing entity.
    ///
    /// # Errors
    /// Returns `StorageError::InvalidData` if JSON is invalid, or SQLite error.
    pub fn update_state(
        &self,
        id: &str,
        state_json: &str,
        last_updated_ms: i64,
    ) -> Result<bool, StorageError> {
        serde_json::from_str::<serde_json::Value>(state_json).map_err(|e| {
            StorageError::InvalidData(format!("Invalid state_json for '{id}': {e}"))
        })?;

        self.db.with_conn(|conn| {
            let count = conn.execute(
                "UPDATE smart_home_entities
                 SET state_json = ?1, last_updated_ms = ?2
                 WHERE id = ?3;",
                params![state_json, last_updated_ms, id],
            )?;
            Ok(count > 0)
        })
    }

    /// Updates the availability flag of an existing entity.
    ///
    /// # Errors
    /// Returns `StorageError` on SQLite query failure.
    pub fn update_availability(
        &self,
        id: &str,
        is_available: bool,
        last_updated_ms: i64,
    ) -> Result<bool, StorageError> {
        self.db.with_conn(|conn| {
            let count = conn.execute(
                "UPDATE smart_home_entities
                 SET is_available = ?1, last_updated_ms = ?2
                 WHERE id = ?3;",
                params![is_available as i32, last_updated_ms, id],
            )?;
            Ok(count > 0)
        })
    }

    /// Deletes an entity from the local registry by ID.
    ///
    /// # Errors
    /// Returns `StorageError` on SQLite query failure.
    pub fn delete_entity(&self, id: &str) -> Result<bool, StorageError> {
        self.db.with_conn(|conn| {
            let count = conn.execute(
                "DELETE FROM smart_home_entities WHERE id = ?1;",
                params![id],
            )?;
            Ok(count > 0)
        })
    }

    /// Returns the total count of registered smart-home entities.
    ///
    /// # Errors
    /// Returns `StorageError` on SQLite query failure.
    pub fn count_entities(&self) -> Result<usize, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT COUNT(*) FROM smart_home_entities;")?;
            let count: i64 = stmt.query_row([], |row| row.get(0))?;
            Ok(count as usize)
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn create_test_store() -> SmartHomeStore {
        let db = SqliteDb::open_in_memory().unwrap();
        SmartHomeStore::new(db)
    }

    #[test]
    fn test_domain_and_protocol_conversions() {
        assert_eq!(SmartHomeDomain::Light.as_str(), "light");
        assert_eq!(
            SmartHomeDomain::from_str("light").unwrap(),
            SmartHomeDomain::Light
        );
        assert_eq!(
            SmartHomeDomain::from_str("climate").unwrap(),
            SmartHomeDomain::Climate
        );
        assert_eq!(
            SmartHomeDomain::from_str("switch").unwrap(),
            SmartHomeDomain::Switch
        );
        assert!(SmartHomeDomain::from_str("invalid_domain").is_err());

        assert_eq!(SmartHomeProtocol::Mqtt.as_str(), "mqtt");
        assert_eq!(
            SmartHomeProtocol::from_str("mqtt").unwrap(),
            SmartHomeProtocol::Mqtt
        );
        assert_eq!(
            SmartHomeProtocol::from_str("home_assistant").unwrap(),
            SmartHomeProtocol::HomeAssistant
        );
        assert_eq!(
            SmartHomeProtocol::from_str("matter_local").unwrap(),
            SmartHomeProtocol::MatterLocal
        );
        assert!(SmartHomeProtocol::from_str("zigbee_cloud").is_err());
    }

    #[test]
    fn test_smart_home_store_crud_lifecycle() {
        let store = create_test_store();
        assert_eq!(store.count_entities().unwrap(), 0);

        let entity1 = SmartHomeEntityRecord {
            id: "light.living_room".to_string(),
            domain: SmartHomeDomain::Light,
            name: "Living Room Light".to_string(),
            state_json: serde_json::json!({
                "state": "on",
                "brightness": 200,
                "color_temp": 3000
            })
            .to_string(),
            attributes_json: serde_json::json!({
                "supported_color_modes": ["brightness", "color_temp"],
                "friendly_name": "Living Room Light"
            })
            .to_string(),
            protocol: SmartHomeProtocol::HomeAssistant,
            is_available: true,
            last_updated_ms: 1_000,
        };

        let entity2 = SmartHomeEntityRecord {
            id: "climate.thermostat".to_string(),
            domain: SmartHomeDomain::Climate,
            name: "Main Thermostat".to_string(),
            state_json: serde_json::json!({
                "state": "heat",
                "current_temperature": 20.5,
                "target_temperature": 22.0
            })
            .to_string(),
            attributes_json: serde_json::json!({
                "hvac_modes": ["off", "heat", "cool"],
                "min_temp": 15,
                "max_temp": 30
            })
            .to_string(),
            protocol: SmartHomeProtocol::Mqtt,
            is_available: true,
            last_updated_ms: 1_200,
        };

        // 1. Upsert entities
        store.upsert_entity(&entity1).unwrap();
        store.upsert_entity(&entity2).unwrap();
        assert_eq!(store.count_entities().unwrap(), 2);

        // 2. Get by ID
        let fetched = store.get_entity("light.living_room").unwrap().unwrap();
        assert_eq!(fetched.id, "light.living_room");
        assert_eq!(fetched.name, "Living Room Light");
        assert_eq!(fetched.domain, SmartHomeDomain::Light);
        assert!(fetched.is_available);

        // 3. List all
        let all = store.list_entities(None).unwrap();
        assert_eq!(all.len(), 2);

        // 4. List by domain
        let lights = store.list_entities(Some(SmartHomeDomain::Light)).unwrap();
        assert_eq!(lights.len(), 1);
        assert_eq!(lights[0].id, "light.living_room");

        let climates = store.list_entities(Some(SmartHomeDomain::Climate)).unwrap();
        assert_eq!(climates.len(), 1);
        assert_eq!(climates[0].id, "climate.thermostat");

        let switches = store.list_entities(Some(SmartHomeDomain::Switch)).unwrap();
        assert!(switches.is_empty());

        // 5. Update state
        let new_state = serde_json::json!({
            "state": "off",
            "brightness": 0
        })
        .to_string();
        let updated = store
            .update_state("light.living_room", &new_state, 2_000)
            .unwrap();
        assert!(updated);

        let updated_entity = store.get_entity("light.living_room").unwrap().unwrap();
        assert_eq!(updated_entity.state_json, new_state);
        assert_eq!(updated_entity.last_updated_ms, 2_000);

        // 6. Update availability
        let avail_updated = store
            .update_availability("light.living_room", false, 2_500)
            .unwrap();
        assert!(avail_updated);
        let offline_entity = store.get_entity("light.living_room").unwrap().unwrap();
        assert!(!offline_entity.is_available);

        // 7. Delete entity
        let deleted = store.delete_entity("light.living_room").unwrap();
        assert!(deleted);
        assert_eq!(store.count_entities().unwrap(), 1);
        assert!(store.get_entity("light.living_room").unwrap().is_none());
    }

    #[test]
    fn test_smart_home_store_validation_errors() {
        let store = create_test_store();

        // Empty ID rejected
        let invalid_id = SmartHomeEntityRecord {
            id: "".to_string(),
            domain: SmartHomeDomain::Switch,
            name: "Switch 1".to_string(),
            state_json: "{}".to_string(),
            attributes_json: "{}".to_string(),
            protocol: SmartHomeProtocol::Mqtt,
            is_available: true,
            last_updated_ms: 100,
        };
        assert!(store.upsert_entity(&invalid_id).is_err());

        // Empty name rejected
        let invalid_name = SmartHomeEntityRecord {
            id: "switch.desk".to_string(),
            domain: SmartHomeDomain::Switch,
            name: "   ".to_string(),
            state_json: "{}".to_string(),
            attributes_json: "{}".to_string(),
            protocol: SmartHomeProtocol::Mqtt,
            is_available: true,
            last_updated_ms: 100,
        };
        assert!(store.upsert_entity(&invalid_name).is_err());

        // Invalid state JSON rejected
        let invalid_state = SmartHomeEntityRecord {
            id: "switch.desk".to_string(),
            domain: SmartHomeDomain::Switch,
            name: "Desk Switch".to_string(),
            state_json: "{not valid json}".to_string(),
            attributes_json: "{}".to_string(),
            protocol: SmartHomeProtocol::Mqtt,
            is_available: true,
            last_updated_ms: 100,
        };
        assert!(store.upsert_entity(&invalid_state).is_err());

        // Update state with bad JSON rejected
        assert!(store
            .update_state("switch.desk", "{bad json}", 200)
            .is_err());
    }
}
