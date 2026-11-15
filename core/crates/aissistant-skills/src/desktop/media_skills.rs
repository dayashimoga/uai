//! Audio volume control and media transport skills (`volume.get`, `volume.set`, `media.transport`).

use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_platform::{DesktopPlatformAdapter, MediaCommand};
use aissistant_security::RiskTier;
use serde_json::Value;

/// Skill for reading the master audio volume and mute status.
#[derive(Debug, Clone)]
pub struct VolumeGetSkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl VolumeGetSkill {
    /// Creates a new `VolumeGetSkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for VolumeGetSkill {
    fn name(&self) -> &str {
        "volume.get"
    }

    fn description(&self) -> &str {
        "Retrieves the master audio output volume percentage and mute status."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn execute(&self, _arguments: Value) -> Result<Value, OrchestratorError> {
        let status = self.adapter.get_master_volume().map_err(|e| {
            OrchestratorError::ToolExecutionFailed {
                tool_name: "volume.get".to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(serde_json::json!({
            "volume_percent": status.volume_percent,
            "is_muted": status.is_muted,
        }))
    }
}

/// Skill for setting master audio volume or toggling mute.
#[derive(Debug, Clone)]
pub struct VolumeSetSkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl VolumeSetSkill {
    /// Creates a new `VolumeSetSkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for VolumeSetSkill {
    fn name(&self) -> &str {
        "volume.set"
    }

    fn description(&self) -> &str {
        "Sets the master audio volume percentage (0 to 100) or updates mute status."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "level": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 100,
                    "description": "Target volume percentage level (0 to 100)"
                },
                "mute": {
                    "type": "boolean",
                    "description": "Whether audio should be muted"
                }
            }
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::StateModifying
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let level = arguments
            .get("level")
            .and_then(|v| v.as_u64())
            .map(|l| l.min(100) as u8);

        let mute = arguments.get("mute").and_then(|v| v.as_bool());

        if level.is_none() && mute.is_none() {
            return Err(OrchestratorError::ToolExecutionFailed {
                tool_name: "volume.set".to_string(),
                reason: "At least one of 'level' or 'mute' must be specified".to_string(),
            });
        }

        let updated = self.adapter.set_master_volume(level, mute).map_err(|e| {
            OrchestratorError::ToolExecutionFailed {
                tool_name: "volume.set".to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(serde_json::json!({
            "volume_percent": updated.volume_percent,
            "is_muted": updated.is_muted,
        }))
    }
}

/// Skill for media transport playback control.
#[derive(Debug, Clone)]
pub struct MediaTransportSkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl MediaTransportSkill {
    /// Creates a new `MediaTransportSkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for MediaTransportSkill {
    fn name(&self) -> &str {
        "media.transport"
    }

    fn description(&self) -> &str {
        "Controls media playback transport: play, pause, play_pause, next, previous, or stop."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "enum": ["play", "pause", "play_pause", "next", "previous", "stop"],
                    "description": "Media command to dispatch"
                }
            },
            "required": ["command"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::StateModifying
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let cmd_str = arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OrchestratorError::ToolExecutionFailed {
                tool_name: "media.transport".to_string(),
                reason: "Missing required string argument 'command'".to_string(),
            })?;

        let command = match cmd_str {
            "play" => MediaCommand::Play,
            "pause" => MediaCommand::Pause,
            "play_pause" => MediaCommand::PlayPause,
            "next" => MediaCommand::Next,
            "previous" => MediaCommand::Previous,
            "stop" => MediaCommand::Stop,
            other => {
                return Err(OrchestratorError::ToolExecutionFailed {
                    tool_name: "media.transport".to_string(),
                    reason: format!("Unknown media transport command: '{other}'"),
                });
            }
        };

        self.adapter.media_transport(command).map_err(|e| {
            OrchestratorError::ToolExecutionFailed {
                tool_name: "media.transport".to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(serde_json::json!({
            "command": cmd_str,
            "status": "dispatched",
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_platform::VirtualDesktopAdapter;

    #[test]
    fn test_volume_skills() {
        let adapter = Arc::new(VirtualDesktopAdapter::new());
        let get_skill = VolumeGetSkill::new(adapter.clone());
        let set_skill = VolumeSetSkill::new(adapter);

        assert_eq!(get_skill.risk_tier(), RiskTier::ReadOnly);
        assert_eq!(set_skill.risk_tier(), RiskTier::StateModifying);

        let initial = get_skill.execute(serde_json::json!({})).unwrap();
        assert_eq!(initial["volume_percent"], 75);
        assert_eq!(initial["is_muted"], false);

        let set_res = set_skill
            .execute(serde_json::json!({ "level": 50, "mute": true }))
            .unwrap();
        assert_eq!(set_res["volume_percent"], 50);
        assert_eq!(set_res["is_muted"], true);

        let updated = get_skill.execute(serde_json::json!({})).unwrap();
        assert_eq!(updated["volume_percent"], 50);
        assert_eq!(updated["is_muted"], true);
    }

    #[test]
    fn test_media_transport_skill() {
        let adapter = Arc::new(VirtualDesktopAdapter::new());
        let skill = MediaTransportSkill::new(adapter.clone());

        assert_eq!(skill.risk_tier(), RiskTier::StateModifying);

        let res = skill
            .execute(serde_json::json!({ "command": "play_pause" }))
            .unwrap();
        assert_eq!(res["status"], "dispatched");
        assert_eq!(res["command"], "play_pause");
        assert_eq!(adapter.last_media_command(), Some(MediaCommand::PlayPause));

        let res = skill
            .execute(serde_json::json!({ "command": "next" }))
            .unwrap();
        assert_eq!(res["command"], "next");
        assert_eq!(adapter.last_media_command(), Some(MediaCommand::Next));

        let invalid = skill.execute(serde_json::json!({ "command": "fast_forward" }));
        assert!(invalid.is_err());
    }
}
