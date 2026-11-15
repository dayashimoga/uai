//! User persona profile, voice configurations, and explicit preference storage backed by SQLite.

use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::StorageError;
use crate::pool::SqliteDb;

/// Assistant personality and conversation tone directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonaTone {
    Balanced,
    Concise,
    Technical,
    Casual,
    Academic,
}

impl PersonaTone {
    /// Returns the static string representation of this tone.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Concise => "concise",
            Self::Technical => "technical",
            Self::Casual => "casual",
            Self::Academic => "academic",
        }
    }

    /// Returns human-readable system prompt guidance corresponding to this tone.
    #[must_use]
    pub const fn system_instruction(&self) -> &'static str {
        match self {
            Self::Balanced => "Adopt a helpful, balanced, neutral, and courteous tone.",
            Self::Concise => {
                "Be extremely concise, brief, and direct without pleasantries or conversational filler."
            }
            Self::Technical => {
                "Adopt a rigorous, technically precise engineering tone with code snippets and accurate terminology."
            }
            Self::Casual => "Adopt a warm, conversational, friendly, and approachable tone.",
            Self::Academic => {
                "Adopt an in-depth, scholarly, analytical, and well-reasoned tone with systematic structuring."
            }
        }
    }
}

impl fmt::Display for PersonaTone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for PersonaTone {
    type Err = StorageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "balanced" => Ok(Self::Balanced),
            "concise" => Ok(Self::Concise),
            "technical" => Ok(Self::Technical),
            "casual" => Ok(Self::Casual),
            "academic" => Ok(Self::Academic),
            other => Err(StorageError::InvalidData(format!(
                "Unknown persona tone: '{other}'"
            ))),
        }
    }
}

/// Response verbosity and explanation depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseVerbosity {
    Concise,
    Normal,
    Detailed,
}

impl ResponseVerbosity {
    /// Returns the static string representation of this verbosity level.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Concise => "concise",
            Self::Normal => "normal",
            Self::Detailed => "detailed",
        }
    }

    /// Returns human-readable system prompt guidance corresponding to this verbosity level.
    #[must_use]
    pub const fn system_instruction(&self) -> &'static str {
        match self {
            Self::Concise => {
                "Keep responses strictly to the point, answering in 1-3 sentences or compact bullet points."
            }
            Self::Normal => "Provide complete and balanced answers with adequate explanation.",
            Self::Detailed => {
                "Provide comprehensive, deeply explanatory answers covering edge cases and architecture."
            }
        }
    }
}

impl fmt::Display for ResponseVerbosity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ResponseVerbosity {
    type Err = StorageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "concise" => Ok(Self::Concise),
            "normal" => Ok(Self::Normal),
            "detailed" => Ok(Self::Detailed),
            other => Err(StorageError::InvalidData(format!(
                "Unknown response verbosity: '{other}'"
            ))),
        }
    }
}

/// User persona profile record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPersona {
    pub id: String,
    pub name: String,
    pub tone: PersonaTone,
    pub verbosity: ResponseVerbosity,
    pub voice_id: String,
    pub speaking_rate: f32,
    pub pitch: f32,
    pub system_prompt_override: Option<String>,
    pub is_active: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl Default for UserPersona {
    fn default() -> Self {
        Self {
            id: "persona_default".to_string(),
            name: "Default Assistant".to_string(),
            tone: PersonaTone::Balanced,
            verbosity: ResponseVerbosity::Normal,
            voice_id: "vits_piper_en_ryan".to_string(),
            speaking_rate: 1.0,
            pitch: 1.0,
            system_prompt_override: None,
            is_active: true,
            created_at_ms: 0,
            updated_at_ms: 0,
        }
    }
}

/// Explicit key-value user preference entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserPreference {
    pub key: String,
    pub value_json: String,
    pub category: String,
    pub updated_at_ms: i64,
}

/// Repository for managing user personas and explicit user preferences.
#[derive(Debug, Clone)]
pub struct PersonaStore {
    db: SqliteDb,
}

impl PersonaStore {
    /// Creates a new persona store using the provided database handle.
    #[must_use]
    pub const fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    /// Retrieves the currently active user persona, if one is selected.
    pub fn get_active_persona(&self) -> Result<Option<UserPersona>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, tone, verbosity, voice_id, speaking_rate, pitch,
                        system_prompt_override, is_active, created_at_ms, updated_at_ms
                 FROM user_personas
                 WHERE is_active = 1
                 LIMIT 1;",
            )?;
            let mut rows = stmt.query([])?;
            if let Some(row) = rows.next()? {
                let tone_str: String = row.get(2)?;
                let verbosity_str: String = row.get(3)?;
                let is_active_int: i64 = row.get(8)?;

                Ok(Some(UserPersona {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    tone: PersonaTone::from_str(&tone_str)?,
                    verbosity: ResponseVerbosity::from_str(&verbosity_str)?,
                    voice_id: row.get(4)?,
                    speaking_rate: row.get(5)?,
                    pitch: row.get(6)?,
                    system_prompt_override: row.get(7)?,
                    is_active: is_active_int != 0,
                    created_at_ms: row.get(9)?,
                    updated_at_ms: row.get(10)?,
                }))
            } else {
                Ok(None)
            }
        })
    }

    /// Retrieves a persona by its unique ID.
    pub fn get_persona(&self, id: &str) -> Result<Option<UserPersona>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, tone, verbosity, voice_id, speaking_rate, pitch,
                        system_prompt_override, is_active, created_at_ms, updated_at_ms
                 FROM user_personas
                 WHERE id = ?1;",
            )?;
            let mut rows = stmt.query(params![id])?;
            if let Some(row) = rows.next()? {
                let tone_str: String = row.get(2)?;
                let verbosity_str: String = row.get(3)?;
                let is_active_int: i64 = row.get(8)?;

                Ok(Some(UserPersona {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    tone: PersonaTone::from_str(&tone_str)?,
                    verbosity: ResponseVerbosity::from_str(&verbosity_str)?,
                    voice_id: row.get(4)?,
                    speaking_rate: row.get(5)?,
                    pitch: row.get(6)?,
                    system_prompt_override: row.get(7)?,
                    is_active: is_active_int != 0,
                    created_at_ms: row.get(9)?,
                    updated_at_ms: row.get(10)?,
                }))
            } else {
                Ok(None)
            }
        })
    }

    /// Lists all stored user personas ordered by active status then name.
    pub fn list_personas(&self) -> Result<Vec<UserPersona>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, tone, verbosity, voice_id, speaking_rate, pitch,
                        system_prompt_override, is_active, created_at_ms, updated_at_ms
                 FROM user_personas
                 ORDER BY is_active DESC, name ASC;",
            )?;
            let mut rows = stmt.query([])?;
            let mut result = Vec::new();

            while let Some(row) = rows.next()? {
                let tone_str: String = row.get(2)?;
                let verbosity_str: String = row.get(3)?;
                let is_active_int: i64 = row.get(8)?;

                result.push(UserPersona {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    tone: PersonaTone::from_str(&tone_str)?,
                    verbosity: ResponseVerbosity::from_str(&verbosity_str)?,
                    voice_id: row.get(4)?,
                    speaking_rate: row.get(5)?,
                    pitch: row.get(6)?,
                    system_prompt_override: row.get(7)?,
                    is_active: is_active_int != 0,
                    created_at_ms: row.get(9)?,
                    updated_at_ms: row.get(10)?,
                });
            }

            Ok(result)
        })
    }

    /// Saves or updates a user persona. If `persona.is_active` is true, other personas are deactivated.
    pub fn save_persona(&self, persona: &UserPersona) -> Result<(), StorageError> {
        if persona.id.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "Persona ID cannot be empty".to_string(),
            ));
        }
        if persona.name.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "Persona name cannot be empty".to_string(),
            ));
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let created_at = if persona.created_at_ms > 0 {
            persona.created_at_ms
        } else {
            now_ms
        };

        self.db.transaction(|tx| {
            if persona.is_active {
                tx.execute(
                    "UPDATE user_personas SET is_active = 0 WHERE id != ?1;",
                    params![persona.id],
                )?;
            }

            tx.execute(
                "INSERT INTO user_personas (
                    id, name, tone, verbosity, voice_id, speaking_rate, pitch,
                    system_prompt_override, is_active, created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    tone = excluded.tone,
                    verbosity = excluded.verbosity,
                    voice_id = excluded.voice_id,
                    speaking_rate = excluded.speaking_rate,
                    pitch = excluded.pitch,
                    system_prompt_override = excluded.system_prompt_override,
                    is_active = excluded.is_active,
                    updated_at_ms = excluded.updated_at_ms;",
                params![
                    persona.id,
                    persona.name,
                    persona.tone.as_str(),
                    persona.verbosity.as_str(),
                    persona.voice_id,
                    persona.speaking_rate,
                    persona.pitch,
                    persona.system_prompt_override,
                    if persona.is_active { 1 } else { 0 },
                    created_at,
                    now_ms,
                ],
            )?;

            Ok(())
        })
    }

    /// Sets the specified persona as active, deactivating all others.
    pub fn set_active_persona(&self, id: &str) -> Result<(), StorageError> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        self.db.transaction(|tx| {
            let exists: bool = tx.query_row(
                "SELECT COUNT(*) > 0 FROM user_personas WHERE id = ?1;",
                params![id],
                |row| row.get(0),
            )?;

            if !exists {
                return Err(StorageError::NotFound {
                    key: format!("persona:{id}"),
                });
            }

            tx.execute("UPDATE user_personas SET is_active = 0;", [])?;
            tx.execute(
                "UPDATE user_personas SET is_active = 1, updated_at_ms = ?1 WHERE id = ?2;",
                params![now_ms, id],
            )?;

            Ok(())
        })
    }

    /// Deletes a user persona by ID. Default persona or currently active persona cannot be deleted.
    pub fn delete_persona(&self, id: &str) -> Result<bool, StorageError> {
        if id == "persona_default" {
            return Err(StorageError::InvalidData(
                "Cannot delete default system persona".to_string(),
            ));
        }

        self.db.with_conn(|conn| {
            let is_active: Option<bool> = conn
                .query_row(
                    "SELECT is_active FROM user_personas WHERE id = ?1;",
                    params![id],
                    |row| {
                        let active_int: i64 = row.get(0)?;
                        Ok(active_int != 0)
                    },
                )
                .ok();

            match is_active {
                Some(true) => Err(StorageError::InvalidData(
                    "Cannot delete currently active persona. Activate another persona first."
                        .to_string(),
                )),
                Some(false) => {
                    let affected =
                        conn.execute("DELETE FROM user_personas WHERE id = ?1;", params![id])?;
                    Ok(affected > 0)
                }
                None => Ok(false),
            }
        })
    }

    /// Retrieves an explicit user preference by key.
    pub fn get_preference(&self, key: &str) -> Result<Option<UserPreference>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT key, value_json, category, updated_at_ms
                 FROM user_preferences
                 WHERE key = ?1;",
            )?;
            let mut rows = stmt.query(params![key])?;
            if let Some(row) = rows.next()? {
                Ok(Some(UserPreference {
                    key: row.get(0)?,
                    value_json: row.get(1)?,
                    category: row.get(2)?,
                    updated_at_ms: row.get(3)?,
                }))
            } else {
                Ok(None)
            }
        })
    }

    /// Sets or updates an explicit user preference.
    pub fn set_preference(
        &self,
        key: &str,
        value_json: &str,
        category: &str,
    ) -> Result<(), StorageError> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO user_preferences (key, value_json, category, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(key) DO UPDATE SET
                    value_json = excluded.value_json,
                    category = excluded.category,
                    updated_at_ms = excluded.updated_at_ms;",
                params![key, value_json, category, now_ms],
            )?;
            Ok(())
        })
    }

    /// Lists user preferences, optionally filtered by category.
    pub fn list_preferences(
        &self,
        category: Option<&str>,
    ) -> Result<Vec<UserPreference>, StorageError> {
        self.db.with_conn(|conn| {
            let mut result = Vec::new();
            if let Some(cat) = category {
                let mut stmt = conn.prepare(
                    "SELECT key, value_json, category, updated_at_ms
                     FROM user_preferences
                     WHERE category = ?1
                     ORDER BY key ASC;",
                )?;
                let mut rows = stmt.query(params![cat])?;
                while let Some(row) = rows.next()? {
                    result.push(UserPreference {
                        key: row.get(0)?,
                        value_json: row.get(1)?,
                        category: row.get(2)?,
                        updated_at_ms: row.get(3)?,
                    });
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT key, value_json, category, updated_at_ms
                     FROM user_preferences
                     ORDER BY category ASC, key ASC;",
                )?;
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    result.push(UserPreference {
                        key: row.get(0)?,
                        value_json: row.get(1)?,
                        category: row.get(2)?,
                        updated_at_ms: row.get(3)?,
                    });
                }
            }
            Ok(result)
        })
    }

    /// Deletes an explicit user preference. Returns true if the key existed.
    pub fn delete_preference(&self, key: &str) -> Result<bool, StorageError> {
        self.db.with_conn(|conn| {
            let affected =
                conn.execute("DELETE FROM user_preferences WHERE key = ?1;", params![key])?;
            Ok(affected > 0)
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_tone_and_verbosity_serde_and_instructions() {
        let tone = PersonaTone::from_str("technical").unwrap();
        assert_eq!(tone, PersonaTone::Technical);
        assert_eq!(tone.as_str(), "technical");
        assert!(tone.system_instruction().contains("rigorous"));
        assert_eq!(format!("{tone}"), "technical");

        assert!(PersonaTone::from_str("nonexistent").is_err());

        let verbosity = ResponseVerbosity::from_str("concise").unwrap();
        assert_eq!(verbosity, ResponseVerbosity::Concise);
        assert_eq!(verbosity.as_str(), "concise");
        assert!(verbosity
            .system_instruction()
            .contains("strictly to the point"));
        assert_eq!(format!("{verbosity}"), "concise");

        assert!(ResponseVerbosity::from_str("invalid_depth").is_err());
    }

    #[test]
    fn test_default_persona_seeding_and_retrieval() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = PersonaStore::new(db);

        let active = store.get_active_persona().unwrap();
        assert!(active.is_some());
        let persona = active.unwrap();
        assert_eq!(persona.id, "persona_default");
        assert_eq!(persona.name, "Default Assistant");
        assert_eq!(persona.tone, PersonaTone::Balanced);
        assert_eq!(persona.verbosity, ResponseVerbosity::Normal);
        assert!(persona.is_active);
    }

    #[test]
    fn test_persona_crud_and_exclusivity() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = PersonaStore::new(db);

        let custom = UserPersona {
            id: "persona_code_expert".to_string(),
            name: "Code Expert".to_string(),
            tone: PersonaTone::Technical,
            verbosity: ResponseVerbosity::Detailed,
            voice_id: "vits_piper_en_ryan".to_string(),
            speaking_rate: 1.1,
            pitch: 0.95,
            system_prompt_override: Some("Always prefer Rust and functional patterns.".to_string()),
            is_active: true,
            created_at_ms: 0,
            updated_at_ms: 0,
        };

        store.save_persona(&custom).unwrap();

        // Check that custom is active and default was deactivated
        let active = store.get_active_persona().unwrap().unwrap();
        assert_eq!(active.id, "persona_code_expert");
        assert_eq!(active.speaking_rate, 1.1);

        let default_persona = store.get_persona("persona_default").unwrap().unwrap();
        assert!(!default_persona.is_active);

        // Switch back to default
        store.set_active_persona("persona_default").unwrap();
        let active_again = store.get_active_persona().unwrap().unwrap();
        assert_eq!(active_again.id, "persona_default");

        // Now custom is inactive, delete it
        let deleted = store.delete_persona("persona_code_expert").unwrap();
        assert!(deleted);
        assert!(store.get_persona("persona_code_expert").unwrap().is_none());
    }

    #[test]
    fn test_cannot_delete_default_or_active_persona() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = PersonaStore::new(db);

        // Cannot delete default
        let res = store.delete_persona("persona_default");
        assert!(res.is_err());

        // Create inactive custom persona
        let custom = UserPersona {
            id: "persona_temp".to_string(),
            name: "Temporary".to_string(),
            tone: PersonaTone::Casual,
            verbosity: ResponseVerbosity::Concise,
            voice_id: "voice1".to_string(),
            speaking_rate: 1.0,
            pitch: 1.0,
            system_prompt_override: None,
            is_active: false,
            created_at_ms: 0,
            updated_at_ms: 0,
        };
        store.save_persona(&custom).unwrap();

        // Make it active
        store.set_active_persona("persona_temp").unwrap();
        // Trying to delete active persona fails
        let res2 = store.delete_persona("persona_temp");
        assert!(res2.is_err());

        // Deleting non-existent persona returns Ok(false)
        let res3 = store.delete_persona("non_existent").unwrap();
        assert!(!res3);
    }

    #[test]
    fn test_persona_validation_errors() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = PersonaStore::new(db);

        let invalid_id = UserPersona {
            id: "   ".to_string(),
            ..Default::default()
        };
        assert!(store.save_persona(&invalid_id).is_err());

        let invalid_name = UserPersona {
            id: "p1".to_string(),
            name: "".to_string(),
            ..Default::default()
        };
        assert!(store.save_persona(&invalid_name).is_err());

        // Set active on non-existent persona returns NotFound
        assert!(store.set_active_persona("ghost_id").is_err());
    }

    #[test]
    fn test_user_preferences_crud_and_categories() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = PersonaStore::new(db);

        store
            .set_preference("editor.indent", "4", "editor")
            .unwrap();
        store
            .set_preference("editor.theme", "\"dracula\"", "editor")
            .unwrap();
        store
            .set_preference("ui.locale", "\"en_US\"", "localization")
            .unwrap();

        let pref = store.get_preference("editor.indent").unwrap().unwrap();
        assert_eq!(pref.value_json, "4");
        assert_eq!(pref.category, "editor");

        // List by category
        let editor_prefs = store.list_preferences(Some("editor")).unwrap();
        assert_eq!(editor_prefs.len(), 2);
        assert_eq!(editor_prefs[0].key, "editor.indent");
        assert_eq!(editor_prefs[1].key, "editor.theme");

        // List all
        let all_prefs = store.list_preferences(None).unwrap();
        assert_eq!(all_prefs.len(), 3);

        // Delete
        let deleted = store.delete_preference("editor.indent").unwrap();
        assert!(deleted);
        assert!(store.get_preference("editor.indent").unwrap().is_none());

        let not_found_del = store.delete_preference("editor.indent").unwrap();
        assert!(!not_found_del);
    }
}
