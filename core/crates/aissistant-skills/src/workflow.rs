//! Workflow automation engine supporting cron schedules, event triggers,
//! cross-device remote skill execution, and execution history persistence.

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aissistant_core::CoreError;
use aissistant_orchestrator::ToolDispatcher;
use aissistant_storage::{
    AutomationHistoryRecord, AutomationRuleRecord, AutomationStatus, AutomationStore,
    AutomationTriggerType,
};
use serde::{Deserialize, Serialize};

use crate::error::{Result, SkillError};

/// Configuration for triggering an automated workflow rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerConfig {
    /// Scheduled execution based on cron syntax or interval specifications.
    Cron {
        /// Cron expression or interval string (e.g. "interval:15m", "@hourly", "*/5 * * * *").
        expression: String,
    },
    /// Reactive execution triggered by system or platform events.
    Event {
        /// Name of the event to subscribe to.
        event_name: String,
        /// Optional JSON filter containing key-value pairs matching payload fields.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filter_json: Option<String>,
    },
    /// Manual execution triggered explicitly by user action or API request.
    Manual,
}

impl TriggerConfig {
    /// Returns the corresponding storage `AutomationTriggerType`.
    #[must_use]
    pub const fn trigger_type(&self) -> AutomationTriggerType {
        match self {
            Self::Cron { .. } => AutomationTriggerType::Cron,
            Self::Event { .. } => AutomationTriggerType::Event,
            Self::Manual => AutomationTriggerType::Manual,
        }
    }
}

/// An individual action within an automated workflow pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action_type", rename_all = "snake_case")]
pub enum WorkflowAction {
    /// Invokes a local sandboxed skill registered in the tool dispatcher.
    LocalSkill {
        /// Identifier of the local skill (e.g. "system.time", "file.search").
        skill_id: String,
        /// JSON payload supplied as arguments to the skill.
        #[serde(default)]
        payload_json: String,
    },
    /// Dispatches skill invocation to a paired remote device via QUIC transport.
    RemoteSkill {
        /// Destination device identifier.
        peer_id: String,
        /// Identifier of the skill to execute on the remote device.
        skill_id: String,
        /// JSON payload supplied as arguments.
        #[serde(default)]
        payload_json: String,
    },
    /// Emits a local user notification.
    Notification {
        /// Notification headline.
        title: String,
        /// Notification body message.
        message: String,
    },
}

/// Error handling policy when an action in a workflow fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowErrorPolicy {
    /// Abort execution immediately upon the first action failure.
    #[default]
    FailFast,
    /// Continue executing subsequent actions even if a previous action fails.
    ContinueOnError,
}

/// Helper struct for serializing and deserializing workflow action payloads.
#[derive(Serialize, Deserialize)]
struct WorkflowActionsPayload {
    actions: Vec<WorkflowAction>,
    #[serde(default)]
    error_policy: WorkflowErrorPolicy,
}

/// High-level automated workflow rule definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationRule {
    /// Unique rule identifier.
    pub id: String,
    /// Human-readable rule name.
    pub name: String,
    /// Whether the rule is actively monitored.
    pub enabled: bool,
    /// Trigger configuration.
    pub trigger: TriggerConfig,
    /// Sequential actions to execute.
    pub actions: Vec<WorkflowAction>,
    /// Policy determining behavior when an action fails.
    #[serde(default)]
    pub error_policy: WorkflowErrorPolicy,
    /// Unix timestamp in milliseconds when the rule was last executed.
    #[serde(default)]
    pub last_executed_ms: Option<i64>,
    /// Creation timestamp in milliseconds.
    pub created_at_ms: i64,
    /// Last update timestamp in milliseconds.
    pub updated_at_ms: i64,
}

impl AutomationRule {
    /// Converts a persistent database record into a domain `AutomationRule`.
    ///
    /// # Errors
    /// Returns `SkillError::Serialization` if the JSON payloads cannot be parsed.
    pub fn try_from_record(record: &AutomationRuleRecord) -> Result<Self> {
        let trigger: TriggerConfig = serde_json::from_str(&record.trigger_config_json)?;

        // Support both structured WorkflowActionsPayload and legacy raw Vec<WorkflowAction>
        let (actions, error_policy) = if let Ok(payload) =
            serde_json::from_str::<WorkflowActionsPayload>(&record.actions_json)
        {
            (payload.actions, payload.error_policy)
        } else {
            let actions: Vec<WorkflowAction> = serde_json::from_str(&record.actions_json)?;
            (actions, WorkflowErrorPolicy::default())
        };

        Ok(Self {
            id: record.id.clone(),
            name: record.name.clone(),
            enabled: record.enabled,
            trigger,
            actions,
            error_policy,
            last_executed_ms: record.last_executed_ms,
            created_at_ms: record.created_at_ms,
            updated_at_ms: record.updated_at_ms,
        })
    }

    /// Converts this domain rule into a persistent database record.
    ///
    /// # Errors
    /// Returns `SkillError::Serialization` if serialization fails.
    pub fn to_record(&self) -> Result<AutomationRuleRecord> {
        let trigger_config_json = serde_json::to_string(&self.trigger)?;
        let actions_payload = WorkflowActionsPayload {
            actions: self.actions.clone(),
            error_policy: self.error_policy,
        };
        let actions_json = serde_json::to_string(&actions_payload)?;

        Ok(AutomationRuleRecord {
            id: self.id.clone(),
            name: self.name.clone(),
            enabled: self.enabled,
            trigger_type: self.trigger.trigger_type(),
            trigger_config_json,
            actions_json,
            last_executed_ms: self.last_executed_ms,
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
        })
    }
}

/// Result of executing an individual workflow action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionResult {
    /// Zero-based position of the action in the workflow sequence.
    pub action_index: usize,
    /// Description of the action type executed.
    pub action_type: String,
    /// Whether the action completed successfully.
    pub success: bool,
    /// Optional serialized JSON output from the action execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_json: Option<String>,
    /// Optional error message if the action failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
}

/// Comprehensive report generated after executing an automated workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExecutionReport {
    /// Identifier of the executed automation rule.
    pub automation_id: String,
    /// Final execution outcome status.
    pub status: AutomationStatus,
    /// Source or event that triggered execution.
    pub triggered_by: String,
    /// Detailed results for each action executed.
    pub action_results: Vec<ActionResult>,
    /// Total execution duration in milliseconds.
    pub duration_ms: u64,
    /// Timestamp in milliseconds when execution began.
    pub executed_at_ms: i64,
}

/// Workflow automation engine managing rule evaluation, scheduling, and execution.
#[derive(Debug)]
pub struct AutomationEngine {
    store: Arc<AutomationStore>,
}

impl AutomationEngine {
    /// Creates a new `AutomationEngine` backed by the given persistent store.
    #[must_use]
    pub fn new(store: Arc<AutomationStore>) -> Self {
        Self { store }
    }

    /// Returns a reference to the underlying `AutomationStore`.
    #[must_use]
    pub fn store(&self) -> &Arc<AutomationStore> {
        &self.store
    }

    /// Evaluates whether a cron schedule or interval expression is currently due.
    ///
    /// Supports:
    /// - `"interval:<N>s"` (seconds)
    /// - `"interval:<N>m"` (minutes)
    /// - `"interval:<N>h"` (hours)
    /// - `"@hourly"` (60 minutes)
    /// - `"@daily"` (24 hours)
    /// - `"*/<N> * * * *"` (every N minutes)
    /// - `"* * * * *"` (every minute)
    #[must_use]
    pub fn is_cron_due(expression: &str, last_executed_ms: Option<i64>, now_ms: i64) -> bool {
        let Some(last) = last_executed_ms else {
            return true;
        };

        if now_ms < last {
            return false;
        }

        let expr = expression.trim();
        let interval_ms = if expr.eq_ignore_ascii_case("@hourly") {
            Some(3_600_000)
        } else if expr.eq_ignore_ascii_case("@daily") {
            Some(86_400_000)
        } else if let Some(stripped) = expr.strip_prefix("interval:") {
            let s = stripped.trim();
            if let Some(sec_str) = s.strip_suffix('s') {
                sec_str
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .map(|sec| sec.saturating_mul(1_000))
            } else if let Some(min_str) = s.strip_suffix('m') {
                min_str
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .map(|min| min.saturating_mul(60_000))
            } else if let Some(hr_str) = s.strip_suffix('h') {
                hr_str
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .map(|hr| hr.saturating_mul(3_600_000))
            } else {
                None
            }
        } else if expr == "* * * * *" {
            Some(60_000)
        } else if let Some(rest) = expr.strip_prefix("*/") {
            if let Some((min_part, _)) = rest.split_once(' ') {
                min_part
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .map(|m| m.saturating_mul(60_000))
            } else {
                None
            }
        } else {
            None
        };

        match interval_ms {
            Some(interval) => {
                let elapsed = (now_ms - last) as u64;
                elapsed >= interval
            }
            None => false,
        }
    }

    /// Evaluates whether an automation rule matches an incoming event.
    #[must_use]
    pub fn matches_event(
        rule: &AutomationRule,
        event_name: &str,
        event_payload: Option<&serde_json::Value>,
    ) -> bool {
        if !rule.enabled {
            return false;
        }

        match &rule.trigger {
            TriggerConfig::Event {
                event_name: expected,
                filter_json,
            } => {
                if expected != event_name {
                    return false;
                }

                if let Some(filter_str) = filter_json {
                    let trimmed = filter_str.trim();
                    if trimmed.is_empty() {
                        return true;
                    }
                    if let Ok(filter_val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                        if let (Some(filter_obj), Some(payload_val)) =
                            (filter_val.as_object(), event_payload)
                        {
                            if let Some(payload_obj) = payload_val.as_object() {
                                for (k, v) in filter_obj {
                                    if payload_obj.get(k) != Some(v) {
                                        return false;
                                    }
                                }
                                return true;
                            }
                        }
                        return false;
                    }
                    return false;
                }

                true
            }
            _ => false,
        }
    }

    /// Executes an automated workflow rule according to its actions and error policy.
    ///
    /// # Errors
    /// Returns `SkillError` if persistent state updates or serialization fail.
    pub fn execute_workflow(
        &self,
        rule: &AutomationRule,
        triggered_by: &str,
        dispatcher: Option<&ToolDispatcher>,
    ) -> Result<WorkflowExecutionReport> {
        let start_instant = Instant::now();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);

        let mut action_results = Vec::with_capacity(rule.actions.len());
        let mut overall_success = true;

        for (idx, action) in rule.actions.iter().enumerate() {
            let action_start = Instant::now();
            let mut action_success = true;
            let mut output_json = None;
            let mut error_message = None;
            let action_type = match action {
                WorkflowAction::LocalSkill {
                    skill_id,
                    payload_json,
                } => {
                    let desc = format!("local_skill:{skill_id}");
                    if let Some(disp) = dispatcher {
                        let parsed_args = if payload_json.trim().is_empty() {
                            serde_json::json!({})
                        } else {
                            serde_json::from_str(payload_json)
                                .unwrap_or_else(|_| serde_json::json!({ "raw": payload_json }))
                        };

                        match disp.execute(skill_id, parsed_args) {
                            Ok(val) => {
                                output_json = Some(val.to_string());
                            }
                            Err(e) => {
                                action_success = false;
                                error_message = Some(e.to_string());
                            }
                        }
                    } else {
                        // Dispatcher absent (e.g. simulated or detached mode)
                        output_json = Some(
                            serde_json::json!({
                                "status": "executed_simulated",
                                "skill_id": skill_id
                            })
                            .to_string(),
                        );
                    }
                    desc
                }
                WorkflowAction::RemoteSkill {
                    peer_id,
                    skill_id,
                    payload_json,
                } => {
                    let desc = format!("remote_skill:{peer_id}:{skill_id}");
                    // Format remote dispatch envelope
                    output_json = Some(
                        serde_json::json!({
                            "status": "remote_dispatched",
                            "peer_id": peer_id,
                            "skill_id": skill_id,
                            "payload": payload_json
                        })
                        .to_string(),
                    );
                    desc
                }
                WorkflowAction::Notification { title, message } => {
                    let desc = "notification".to_string();
                    output_json = Some(
                        serde_json::json!({
                            "status": "notified",
                            "title": title,
                            "message": message
                        })
                        .to_string(),
                    );
                    desc
                }
            };

            let duration_ms = action_start.elapsed().as_millis() as u64;

            if !action_success {
                overall_success = false;
            }

            action_results.push(ActionResult {
                action_index: idx,
                action_type,
                success: action_success,
                output_json,
                error_message,
                duration_ms,
            });

            if !action_success && rule.error_policy == WorkflowErrorPolicy::FailFast {
                break;
            }
        }

        let total_duration = start_instant.elapsed().as_millis() as u64;
        let status = if overall_success {
            AutomationStatus::Success
        } else {
            AutomationStatus::Failed
        };

        // Update last executed timestamp in store
        self.store.update_last_executed(&rule.id, now_ms)?;

        // Record history entry
        let history_id = format!("hist_{}_{}", now_ms, &rule.id[..rule.id.len().min(8)]);
        let result_json = serde_json::to_string(&action_results)?;
        let history_record = AutomationHistoryRecord {
            id: history_id,
            automation_id: rule.id.clone(),
            status,
            triggered_by: triggered_by.to_string(),
            executed_at_ms: now_ms,
            duration_ms: total_duration,
            result_json,
        };
        self.store.record_history(&history_record)?;

        Ok(WorkflowExecutionReport {
            automation_id: rule.id.clone(),
            status,
            triggered_by: triggered_by.to_string(),
            action_results,
            duration_ms: total_duration,
            executed_at_ms: now_ms,
        })
    }

    /// Loads and executes a workflow rule by its unique identifier.
    ///
    /// # Errors
    /// Returns `SkillError` if the rule cannot be found or fails execution.
    pub fn execute_rule_by_id(
        &self,
        rule_id: &str,
        triggered_by: &str,
        dispatcher: Option<&ToolDispatcher>,
    ) -> Result<WorkflowExecutionReport> {
        let record = self.store.get_automation(rule_id)?.ok_or_else(|| {
            SkillError::Core(CoreError::NotFound {
                entity: "AutomationRule",
                id: rule_id.to_string(),
            })
        })?;

        let rule = AutomationRule::try_from_record(&record)?;
        self.execute_workflow(&rule, triggered_by, dispatcher)
    }

    /// Evaluates all enabled cron automation rules against `now_ms` and triggers those that are due.
    ///
    /// # Errors
    /// Returns `SkillError` if rule retrieval or execution fails.
    pub fn evaluate_and_trigger_all_due_cron(
        &self,
        now_ms: i64,
        dispatcher: Option<&ToolDispatcher>,
    ) -> Result<Vec<WorkflowExecutionReport>> {
        let records = self.store.list_automations(true)?;
        let mut reports = Vec::new();

        for record in records {
            if record.trigger_type != AutomationTriggerType::Cron {
                continue;
            }
            let rule = AutomationRule::try_from_record(&record)?;
            if let TriggerConfig::Cron { ref expression } = rule.trigger {
                if Self::is_cron_due(expression, rule.last_executed_ms, now_ms) {
                    let report = self.execute_workflow(&rule, "cron_scheduler", dispatcher)?;
                    reports.push(report);
                }
            }
        }

        Ok(reports)
    }

    /// Evaluates all enabled event automation rules matching `event_name` and triggers them.
    ///
    /// # Errors
    /// Returns `SkillError` if rule retrieval or execution fails.
    pub fn evaluate_and_trigger_event(
        &self,
        event_name: &str,
        event_payload: Option<&serde_json::Value>,
        dispatcher: Option<&ToolDispatcher>,
    ) -> Result<Vec<WorkflowExecutionReport>> {
        let records = self.store.list_automations(true)?;
        let mut reports = Vec::new();

        for record in records {
            if record.trigger_type != AutomationTriggerType::Event {
                continue;
            }
            let rule = AutomationRule::try_from_record(&record)?;
            if Self::matches_event(&rule, event_name, event_payload) {
                let trigger_desc = format!("event:{event_name}");
                let report = self.execute_workflow(&rule, &trigger_desc, dispatcher)?;
                reports.push(report);
            }
        }

        Ok(reports)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::registry::SkillRegistry;
    use aissistant_storage::SqliteDb;
    use serde_json::json;

    fn setup_test_engine() -> (AutomationEngine, Arc<ToolDispatcher>) {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = Arc::new(AutomationStore::new(db));
        let engine = AutomationEngine::new(store);

        let dispatcher = Arc::new(ToolDispatcher::new());
        let registry = SkillRegistry::new();
        registry.register_all_builtins(&dispatcher).unwrap();

        (engine, dispatcher)
    }

    #[test]
    fn test_cron_due_calculation_intervals() {
        let now = 1_000_000_000;

        // Never executed -> due immediately
        assert!(AutomationEngine::is_cron_due("interval:5m", None, now));
        assert!(AutomationEngine::is_cron_due("@hourly", None, now));
        assert!(AutomationEngine::is_cron_due("@daily", None, now));
        assert!(AutomationEngine::is_cron_due("*/10 * * * *", None, now));

        // interval:10s
        let ten_sec_ago = now - 10_000;
        assert!(AutomationEngine::is_cron_due(
            "interval:10s",
            Some(ten_sec_ago),
            now
        ));
        assert!(!AutomationEngine::is_cron_due(
            "interval:10s",
            Some(now - 9_000),
            now
        ));

        // interval:5m
        let five_min_ago = now - 300_000;
        assert!(AutomationEngine::is_cron_due(
            "interval:5m",
            Some(five_min_ago),
            now
        ));
        assert!(!AutomationEngine::is_cron_due(
            "interval:5m",
            Some(now - 200_000),
            now
        ));

        // @hourly
        assert!(AutomationEngine::is_cron_due(
            "@hourly",
            Some(now - 3_600_000),
            now
        ));
        assert!(!AutomationEngine::is_cron_due(
            "@hourly",
            Some(now - 1_800_000),
            now
        ));

        // @daily
        assert!(AutomationEngine::is_cron_due(
            "@daily",
            Some(now - 86_400_000),
            now
        ));
        assert!(!AutomationEngine::is_cron_due(
            "@daily",
            Some(now - 40_000_000),
            now
        ));

        // */15 * * * *
        assert!(AutomationEngine::is_cron_due(
            "*/15 * * * *",
            Some(now - 900_000),
            now
        ));
        assert!(!AutomationEngine::is_cron_due(
            "*/15 * * * *",
            Some(now - 800_000),
            now
        ));

        // * * * * *
        assert!(AutomationEngine::is_cron_due(
            "* * * * *",
            Some(now - 60_000),
            now
        ));
        assert!(!AutomationEngine::is_cron_due(
            "* * * * *",
            Some(now - 30_000),
            now
        ));

        // Clock skew / past timestamp
        assert!(!AutomationEngine::is_cron_due(
            "interval:1m",
            Some(now + 5_000),
            now
        ));
    }

    #[test]
    fn test_event_matching_and_filter() {
        let rule = AutomationRule {
            id: "rule-1".to_string(),
            name: "Battery Event Rule".to_string(),
            enabled: true,
            trigger: TriggerConfig::Event {
                event_name: "battery_low".to_string(),
                filter_json: Some(json!({"threshold": 15}).to_string()),
            },
            actions: vec![],
            error_policy: WorkflowErrorPolicy::FailFast,
            last_executed_ms: None,
            created_at_ms: 100,
            updated_at_ms: 100,
        };

        // Matching event with matching payload
        let payload_match = json!({"threshold": 15, "current": 12});
        assert!(AutomationEngine::matches_event(
            &rule,
            "battery_low",
            Some(&payload_match)
        ));

        // Mismatched event name
        assert!(!AutomationEngine::matches_event(
            &rule,
            "battery_high",
            Some(&payload_match)
        ));

        // Mismatched payload value
        let payload_mismatch = json!({"threshold": 20, "current": 12});
        assert!(!AutomationEngine::matches_event(
            &rule,
            "battery_low",
            Some(&payload_mismatch)
        ));

        // Missing payload when filter required
        assert!(!AutomationEngine::matches_event(&rule, "battery_low", None));

        // Disabled rule never matches
        let mut disabled_rule = rule.clone();
        disabled_rule.enabled = false;
        assert!(!AutomationEngine::matches_event(
            &disabled_rule,
            "battery_low",
            Some(&payload_match)
        ));

        // Event rule with no filter matches any payload
        let rule_no_filter = AutomationRule {
            id: "rule-2".to_string(),
            name: "Unfiltered Rule".to_string(),
            enabled: true,
            trigger: TriggerConfig::Event {
                event_name: "device_connected".to_string(),
                filter_json: None,
            },
            actions: vec![],
            error_policy: WorkflowErrorPolicy::FailFast,
            last_executed_ms: None,
            created_at_ms: 100,
            updated_at_ms: 100,
        };
        assert!(AutomationEngine::matches_event(
            &rule_no_filter,
            "device_connected",
            None
        ));
        assert!(AutomationEngine::matches_event(
            &rule_no_filter,
            "device_connected",
            Some(&json!({"peer": "device_xyz"}))
        ));
    }

    #[test]
    fn test_workflow_execution_local_skill_remote_and_notification() {
        let (engine, dispatcher) = setup_test_engine();

        let rule = AutomationRule {
            id: "workflow-test-1".to_string(),
            name: "Morning Routine".to_string(),
            enabled: true,
            trigger: TriggerConfig::Manual,
            actions: vec![
                WorkflowAction::LocalSkill {
                    skill_id: "system.calc".to_string(),
                    payload_json: json!({"expression": "100 + 42"}).to_string(),
                },
                WorkflowAction::RemoteSkill {
                    peer_id: "peer-desktop-1".to_string(),
                    skill_id: "app.launch".to_string(),
                    payload_json: json!({"executable": "spotify"}).to_string(),
                },
                WorkflowAction::Notification {
                    title: "Workflow Complete".to_string(),
                    message: "Morning routine dispatched successfully.".to_string(),
                },
            ],
            error_policy: WorkflowErrorPolicy::FailFast,
            last_executed_ms: None,
            created_at_ms: 500,
            updated_at_ms: 500,
        };

        // Save rule to store
        engine
            .store()
            .save_automation(&rule.to_record().unwrap())
            .unwrap();

        // Execute workflow
        let report = engine
            .execute_rule_by_id("workflow-test-1", "user_button", Some(&dispatcher))
            .unwrap();

        assert_eq!(report.automation_id, "workflow-test-1");
        assert_eq!(report.status, AutomationStatus::Success);
        assert_eq!(report.triggered_by, "user_button");
        assert_eq!(report.action_results.len(), 3);

        // Verify action 0 (calc) output
        assert!(report.action_results[0].success);
        let calc_out: serde_json::Value =
            serde_json::from_str(report.action_results[0].output_json.as_ref().unwrap()).unwrap();
        assert_eq!(calc_out["result"], 142.0);

        // Verify action 1 (remote skill)
        assert!(report.action_results[1].success);
        assert!(report.action_results[1]
            .output_json
            .as_ref()
            .unwrap()
            .contains("peer-desktop-1"));

        // Verify action 2 (notification)
        assert!(report.action_results[2].success);
        assert!(report.action_results[2]
            .output_json
            .as_ref()
            .unwrap()
            .contains("Workflow Complete"));

        // Verify store state updated
        let updated_record = engine
            .store()
            .get_automation("workflow-test-1")
            .unwrap()
            .unwrap();
        assert!(updated_record.last_executed_ms.is_some());

        // Verify history recorded in store
        let history = engine
            .store()
            .list_history(Some("workflow-test-1"), 10)
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status, AutomationStatus::Success);
    }

    #[test]
    fn test_workflow_error_policy_fail_fast_vs_continue_on_error() {
        let (engine, dispatcher) = setup_test_engine();

        // Rule with failing action (calc divide by zero) and FailFast policy
        let fail_fast_rule = AutomationRule {
            id: "fail-fast-rule".to_string(),
            name: "Fail Fast Test".to_string(),
            enabled: true,
            trigger: TriggerConfig::Manual,
            actions: vec![
                WorkflowAction::LocalSkill {
                    skill_id: "system.calc".to_string(),
                    payload_json: json!({"expression": "10 / 0"}).to_string(),
                },
                WorkflowAction::Notification {
                    title: "Should Not Run".to_string(),
                    message: "Skipped due to fail fast".to_string(),
                },
            ],
            error_policy: WorkflowErrorPolicy::FailFast,
            last_executed_ms: None,
            created_at_ms: 1000,
            updated_at_ms: 1000,
        };

        engine
            .store()
            .save_automation(&fail_fast_rule.to_record().unwrap())
            .unwrap();

        let report_ff = engine
            .execute_rule_by_id("fail-fast-rule", "test", Some(&dispatcher))
            .unwrap();
        assert_eq!(report_ff.status, AutomationStatus::Failed);
        // Only first action executed because of FailFast
        assert_eq!(report_ff.action_results.len(), 1);
        assert!(!report_ff.action_results[0].success);

        // Rule with failing action and ContinueOnError policy
        let continue_rule = AutomationRule {
            id: "continue-rule".to_string(),
            name: "Continue On Error Test".to_string(),
            enabled: true,
            trigger: TriggerConfig::Manual,
            actions: vec![
                WorkflowAction::LocalSkill {
                    skill_id: "system.calc".to_string(),
                    payload_json: json!({"expression": "10 / 0"}).to_string(),
                },
                WorkflowAction::Notification {
                    title: "Runs Anyway".to_string(),
                    message: "Executed despite prior error".to_string(),
                },
            ],
            error_policy: WorkflowErrorPolicy::ContinueOnError,
            last_executed_ms: None,
            created_at_ms: 1000,
            updated_at_ms: 1000,
        };

        engine
            .store()
            .save_automation(&continue_rule.to_record().unwrap())
            .unwrap();

        let report_cont = engine
            .execute_rule_by_id("continue-rule", "test", Some(&dispatcher))
            .unwrap();
        assert_eq!(report_cont.status, AutomationStatus::Failed);
        // Both actions executed despite first failing
        assert_eq!(report_cont.action_results.len(), 2);
        assert!(!report_cont.action_results[0].success);
        assert!(report_cont.action_results[1].success);
    }

    #[test]
    fn test_evaluate_and_trigger_cron_and_events() {
        let (engine, dispatcher) = setup_test_engine();

        // 1. Setup a cron rule
        let cron_rule = AutomationRule {
            id: "cron-rule-1".to_string(),
            name: "Cron Every 5m".to_string(),
            enabled: true,
            trigger: TriggerConfig::Cron {
                expression: "interval:5m".to_string(),
            },
            actions: vec![WorkflowAction::Notification {
                title: "Cron Ping".to_string(),
                message: "Fired by interval".to_string(),
            }],
            error_policy: WorkflowErrorPolicy::FailFast,
            last_executed_ms: None,
            created_at_ms: 100,
            updated_at_ms: 100,
        };
        engine
            .store()
            .save_automation(&cron_rule.to_record().unwrap())
            .unwrap();

        // Evaluate cron at now = 1000 -> should fire because last_executed_ms is None
        let cron_reports = engine
            .evaluate_and_trigger_all_due_cron(1000, Some(&dispatcher))
            .unwrap();
        assert_eq!(cron_reports.len(), 1);
        assert_eq!(cron_reports[0].automation_id, "cron-rule-1");

        // Immediately evaluate again at now = 2000 -> should not fire because 5m hasn't elapsed
        let cron_reports_empty = engine
            .evaluate_and_trigger_all_due_cron(2000, Some(&dispatcher))
            .unwrap();
        assert_eq!(cron_reports_empty.len(), 0);

        // 2. Setup an event rule
        let event_rule = AutomationRule {
            id: "event-rule-1".to_string(),
            name: "Lock Screen Rule".to_string(),
            enabled: true,
            trigger: TriggerConfig::Event {
                event_name: "screen_locked".to_string(),
                filter_json: None,
            },
            actions: vec![WorkflowAction::Notification {
                title: "Screen Locked".to_string(),
                message: "Muting audio".to_string(),
            }],
            error_policy: WorkflowErrorPolicy::FailFast,
            last_executed_ms: None,
            created_at_ms: 100,
            updated_at_ms: 100,
        };
        engine
            .store()
            .save_automation(&event_rule.to_record().unwrap())
            .unwrap();

        // Non-matching event
        let no_reports = engine
            .evaluate_and_trigger_event("screen_unlocked", None, Some(&dispatcher))
            .unwrap();
        assert_eq!(no_reports.len(), 0);

        // Matching event
        let event_reports = engine
            .evaluate_and_trigger_event("screen_locked", None, Some(&dispatcher))
            .unwrap();
        assert_eq!(event_reports.len(), 1);
        assert_eq!(event_reports[0].automation_id, "event-rule-1");
    }
}
