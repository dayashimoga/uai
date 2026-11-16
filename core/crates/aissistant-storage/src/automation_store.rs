//! Persistent workflow automation rules, event triggers, and execution history backed by SQLite.

use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::StorageError;
use crate::pool::SqliteDb;

/// Trigger category for workflow automation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutomationTriggerType {
    Cron,
    Event,
    Manual,
}

impl AutomationTriggerType {
    /// Returns the static string representation of this trigger type.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Cron => "cron",
            Self::Event => "event",
            Self::Manual => "manual",
        }
    }
}

impl fmt::Display for AutomationTriggerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for AutomationTriggerType {
    type Err = StorageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "cron" => Ok(Self::Cron),
            "event" => Ok(Self::Event),
            "manual" => Ok(Self::Manual),
            other => Err(StorageError::InvalidData(format!(
                "Unknown automation trigger type: '{other}'"
            ))),
        }
    }
}

/// Execution outcome status for workflow history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutomationStatus {
    Success,
    Failed,
    Cancelled,
}

impl AutomationStatus {
    /// Returns the static string representation of this execution status.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl fmt::Display for AutomationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for AutomationStatus {
    type Err = StorageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "success" => Ok(Self::Success),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(StorageError::InvalidData(format!(
                "Unknown automation status: '{other}'"
            ))),
        }
    }
}

/// Persistent record defining an automated workflow rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationRuleRecord {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub trigger_type: AutomationTriggerType,
    pub trigger_config_json: String,
    pub actions_json: String,
    pub last_executed_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Record documenting a historical execution of an automation rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationHistoryRecord {
    pub id: String,
    pub automation_id: String,
    pub status: AutomationStatus,
    pub triggered_by: String,
    pub executed_at_ms: i64,
    pub duration_ms: u64,
    pub result_json: String,
}

/// Repository for persistent automations and execution log management.
#[derive(Debug, Clone)]
pub struct AutomationStore {
    db: SqliteDb,
}

impl AutomationStore {
    /// Creates a new automation store instance wrapping the SQLite connection pool.
    #[must_use]
    pub const fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    /// Retrieves an automation rule by ID.
    pub fn get_automation(&self, id: &str) -> Result<Option<AutomationRuleRecord>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, enabled, trigger_type, trigger_config_json, actions_json,
                        last_executed_ms, created_at_ms, updated_at_ms
                 FROM automations
                 WHERE id = ?1;",
            )?;

            let mut rows = stmt.query(params![id])?;
            if let Some(row) = rows.next()? {
                let enabled_int: i64 = row.get(2)?;
                let trigger_type_str: String = row.get(3)?;

                Ok(Some(AutomationRuleRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    enabled: enabled_int != 0,
                    trigger_type: AutomationTriggerType::from_str(&trigger_type_str)?,
                    trigger_config_json: row.get(4)?,
                    actions_json: row.get(5)?,
                    last_executed_ms: row.get(6)?,
                    created_at_ms: row.get(7)?,
                    updated_at_ms: row.get(8)?,
                }))
            } else {
                Ok(None)
            }
        })
    }

    /// Lists automation rules, optionally filtering for enabled rules only.
    pub fn list_automations(
        &self,
        enabled_only: bool,
    ) -> Result<Vec<AutomationRuleRecord>, StorageError> {
        self.db.with_conn(|conn| {
            let mut result = Vec::new();

            if enabled_only {
                let mut stmt = conn.prepare(
                    "SELECT id, name, enabled, trigger_type, trigger_config_json, actions_json,
                            last_executed_ms, created_at_ms, updated_at_ms
                     FROM automations
                     WHERE enabled = 1
                     ORDER BY name ASC;",
                )?;
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    let trigger_type_str: String = row.get(3)?;
                    result.push(AutomationRuleRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        enabled: true,
                        trigger_type: AutomationTriggerType::from_str(&trigger_type_str)?,
                        trigger_config_json: row.get(4)?,
                        actions_json: row.get(5)?,
                        last_executed_ms: row.get(6)?,
                        created_at_ms: row.get(7)?,
                        updated_at_ms: row.get(8)?,
                    });
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, name, enabled, trigger_type, trigger_config_json, actions_json,
                            last_executed_ms, created_at_ms, updated_at_ms
                     FROM automations
                     ORDER BY enabled DESC, name ASC;",
                )?;
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    let enabled_int: i64 = row.get(2)?;
                    let trigger_type_str: String = row.get(3)?;
                    result.push(AutomationRuleRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        enabled: enabled_int != 0,
                        trigger_type: AutomationTriggerType::from_str(&trigger_type_str)?,
                        trigger_config_json: row.get(4)?,
                        actions_json: row.get(5)?,
                        last_executed_ms: row.get(6)?,
                        created_at_ms: row.get(7)?,
                        updated_at_ms: row.get(8)?,
                    });
                }
            }

            Ok(result)
        })
    }

    /// Inserts or updates an automation rule.
    pub fn save_automation(&self, rule: &AutomationRuleRecord) -> Result<(), StorageError> {
        if rule.id.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "Automation ID cannot be empty".to_string(),
            ));
        }
        if rule.name.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "Automation name cannot be empty".to_string(),
            ));
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let created_at = if rule.created_at_ms > 0 {
            rule.created_at_ms
        } else {
            now_ms
        };

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO automations (
                    id, name, enabled, trigger_type, trigger_config_json, actions_json,
                    last_executed_ms, created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    enabled = excluded.enabled,
                    trigger_type = excluded.trigger_type,
                    trigger_config_json = excluded.trigger_config_json,
                    actions_json = excluded.actions_json,
                    last_executed_ms = excluded.last_executed_ms,
                    updated_at_ms = excluded.updated_at_ms;",
                params![
                    rule.id,
                    rule.name,
                    if rule.enabled { 1 } else { 0 },
                    rule.trigger_type.as_str(),
                    rule.trigger_config_json,
                    rule.actions_json,
                    rule.last_executed_ms,
                    created_at,
                    now_ms,
                ],
            )?;
            Ok(())
        })
    }

    /// Toggles the enabled state of an automation rule. Returns true if updated.
    pub fn toggle_automation(&self, id: &str, enabled: bool) -> Result<bool, StorageError> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        self.db.with_conn(|conn| {
            let affected = conn.execute(
                "UPDATE automations SET enabled = ?1, updated_at_ms = ?2 WHERE id = ?3;",
                params![if enabled { 1 } else { 0 }, now_ms, id],
            )?;
            Ok(affected > 0)
        })
    }

    /// Updates the last executed timestamp of an automation rule.
    pub fn update_last_executed(&self, id: &str, executed_at_ms: i64) -> Result<(), StorageError> {
        self.db.with_conn(|conn| {
            let affected = conn.execute(
                "UPDATE automations SET last_executed_ms = ?1, updated_at_ms = ?1 WHERE id = ?2;",
                params![executed_at_ms, id],
            )?;
            if affected == 0 {
                return Err(StorageError::NotFound {
                    key: format!("automation:{id}"),
                });
            }
            Ok(())
        })
    }

    /// Deletes an automation rule and cascades to its execution history.
    pub fn delete_automation(&self, id: &str) -> Result<bool, StorageError> {
        self.db.with_conn(|conn| {
            let affected = conn.execute("DELETE FROM automations WHERE id = ?1;", params![id])?;
            Ok(affected > 0)
        })
    }

    /// Appends an execution log record to history.
    pub fn record_history(&self, record: &AutomationHistoryRecord) -> Result<(), StorageError> {
        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO automation_history (
                    id, automation_id, status, triggered_by, executed_at_ms, duration_ms, result_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
                params![
                    record.id,
                    record.automation_id,
                    record.status.as_str(),
                    record.triggered_by,
                    record.executed_at_ms,
                    record.duration_ms as i64,
                    record.result_json,
                ],
            )?;
            Ok(())
        })
    }

    /// Lists execution history records, optionally filtered by automation rule ID.
    pub fn list_history(
        &self,
        automation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AutomationHistoryRecord>, StorageError> {
        let max_rows = if limit == 0 { 50 } else { limit };

        self.db.with_conn(|conn| {
            let mut result = Vec::new();

            if let Some(auto_id) = automation_id {
                let mut stmt = conn.prepare(
                    "SELECT id, automation_id, status, triggered_by, executed_at_ms, duration_ms, result_json
                     FROM automation_history
                     WHERE automation_id = ?1
                     ORDER BY executed_at_ms DESC
                     LIMIT ?2;",
                )?;
                let mut rows = stmt.query(params![auto_id, max_rows as i64])?;
                while let Some(row) = rows.next()? {
                    let status_str: String = row.get(2)?;
                    let dur: i64 = row.get(5)?;
                    result.push(AutomationHistoryRecord {
                        id: row.get(0)?,
                        automation_id: row.get(1)?,
                        status: AutomationStatus::from_str(&status_str)?,
                        triggered_by: row.get(3)?,
                        executed_at_ms: row.get(4)?,
                        duration_ms: dur.max(0) as u64,
                        result_json: row.get(6)?,
                    });
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, automation_id, status, triggered_by, executed_at_ms, duration_ms, result_json
                     FROM automation_history
                     ORDER BY executed_at_ms DESC
                     LIMIT ?1;",
                )?;
                let mut rows = stmt.query(params![max_rows as i64])?;
                while let Some(row) = rows.next()? {
                    let status_str: String = row.get(2)?;
                    let dur: i64 = row.get(5)?;
                    result.push(AutomationHistoryRecord {
                        id: row.get(0)?,
                        automation_id: row.get(1)?,
                        status: AutomationStatus::from_str(&status_str)?,
                        triggered_by: row.get(3)?,
                        executed_at_ms: row.get(4)?,
                        duration_ms: dur.max(0) as u64,
                        result_json: row.get(6)?,
                    });
                }
            }

            Ok(result)
        })
    }

    /// Clears execution history, optionally for a specific automation rule. Returns rows deleted.
    pub fn clear_history(&self, automation_id: Option<&str>) -> Result<usize, StorageError> {
        self.db.with_conn(|conn| {
            let rows = if let Some(auto_id) = automation_id {
                conn.execute(
                    "DELETE FROM automation_history WHERE automation_id = ?1;",
                    params![auto_id],
                )?
            } else {
                conn.execute("DELETE FROM automation_history;", [])?
            };
            Ok(rows)
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_automation_enums_serde_and_parsing() {
        assert_eq!(AutomationTriggerType::Cron.as_str(), "cron");
        assert_eq!(AutomationTriggerType::Event.as_str(), "event");
        assert_eq!(AutomationTriggerType::Manual.as_str(), "manual");
        assert_eq!(
            AutomationTriggerType::from_str("cron").unwrap(),
            AutomationTriggerType::Cron
        );
        assert_eq!(
            AutomationTriggerType::from_str("event").unwrap(),
            AutomationTriggerType::Event
        );
        assert!(AutomationTriggerType::from_str("invalid_type").is_err());

        assert_eq!(AutomationStatus::Success.as_str(), "success");
        assert_eq!(AutomationStatus::Failed.as_str(), "failed");
        assert_eq!(AutomationStatus::Cancelled.as_str(), "cancelled");
        assert_eq!(
            AutomationStatus::from_str("success").unwrap(),
            AutomationStatus::Success
        );
        assert!(AutomationStatus::from_str("pending").is_err());
    }

    #[test]
    fn test_automation_store_crud_and_history() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = AutomationStore::new(db);

        let rule = AutomationRuleRecord {
            id: "auto_morning_brief".to_string(),
            name: "Morning Briefing".to_string(),
            enabled: true,
            trigger_type: AutomationTriggerType::Cron,
            trigger_config_json: r#"{"expression": "0 8 * * *"}"#.to_string(),
            actions_json: r#"[{"type": "notification", "message": "Good morning!"}]"#.to_string(),
            last_executed_ms: None,
            created_at_ms: 0,
            updated_at_ms: 0,
        };

        // 1. Save rule
        store.save_automation(&rule).unwrap();

        // 2. Query rule
        let fetched = store.get_automation("auto_morning_brief").unwrap();
        assert!(fetched.is_some());
        let f = fetched.unwrap();
        assert_eq!(f.name, "Morning Briefing");
        assert!(f.enabled);
        assert_eq!(f.trigger_type, AutomationTriggerType::Cron);

        // 3. Toggle rule
        let toggled = store
            .toggle_automation("auto_morning_brief", false)
            .unwrap();
        assert!(toggled);
        let disabled_rule = store.get_automation("auto_morning_brief").unwrap().unwrap();
        assert!(!disabled_rule.enabled);

        let enabled_list = store.list_automations(true).unwrap();
        assert!(enabled_list.is_empty());

        let all_list = store.list_automations(false).unwrap();
        assert_eq!(all_list.len(), 1);

        // 4. Update last executed
        store
            .update_last_executed("auto_morning_brief", 1731680000000)
            .unwrap();
        let updated = store.get_automation("auto_morning_brief").unwrap().unwrap();
        assert_eq!(updated.last_executed_ms, Some(1731680000000));

        // 5. Append history
        let hist1 = AutomationHistoryRecord {
            id: "hist_01".to_string(),
            automation_id: "auto_morning_brief".to_string(),
            status: AutomationStatus::Success,
            triggered_by: "cron:0 8 * * *".to_string(),
            executed_at_ms: 1731680000000,
            duration_ms: 45,
            result_json: r#"{"status": "delivered"}"#.to_string(),
        };
        let hist2 = AutomationHistoryRecord {
            id: "hist_02".to_string(),
            automation_id: "auto_morning_brief".to_string(),
            status: AutomationStatus::Failed,
            triggered_by: "manual".to_string(),
            executed_at_ms: 1731680005000,
            duration_ms: 12,
            result_json: r#"{"error": "network unreachable"}"#.to_string(),
        };
        store.record_history(&hist1).unwrap();
        store.record_history(&hist2).unwrap();

        let history_entries = store.list_history(Some("auto_morning_brief"), 10).unwrap();
        assert_eq!(history_entries.len(), 2);
        assert_eq!(history_entries[0].id, "hist_02"); // DESC order
        assert_eq!(history_entries[1].id, "hist_01");

        // 6. Delete rule cascades to history
        let deleted = store.delete_automation("auto_morning_brief").unwrap();
        assert!(deleted);
        assert!(store
            .get_automation("auto_morning_brief")
            .unwrap()
            .is_none());

        let remaining_history = store.list_history(Some("auto_morning_brief"), 10).unwrap();
        assert!(remaining_history.is_empty());
    }

    #[test]
    fn test_automation_store_validation_and_errors() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = AutomationStore::new(db);

        let empty_id = AutomationRuleRecord {
            id: "".to_string(),
            name: "Test".to_string(),
            enabled: true,
            trigger_type: AutomationTriggerType::Manual,
            trigger_config_json: "{}".to_string(),
            actions_json: "[]".to_string(),
            last_executed_ms: None,
            created_at_ms: 0,
            updated_at_ms: 0,
        };
        assert!(store.save_automation(&empty_id).is_err());

        let empty_name = AutomationRuleRecord {
            id: "rule1".to_string(),
            name: "   ".to_string(),
            enabled: true,
            trigger_type: AutomationTriggerType::Manual,
            trigger_config_json: "{}".to_string(),
            actions_json: "[]".to_string(),
            last_executed_ms: None,
            created_at_ms: 0,
            updated_at_ms: 0,
        };
        assert!(store.save_automation(&empty_name).is_err());

        // Update last executed on non-existent rule
        assert!(store.update_last_executed("non_existent", 1000).is_err());
    }
}
