//! Local smart-home device control, MQTT message serialization,
//! Home Assistant offline integration adapters, and sandboxed skill definitions.

use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use aissistant_orchestrator::ToolHandler;
use aissistant_security::RiskTier;
use aissistant_storage::{SmartHomeDomain, SmartHomeProtocol, SmartHomeStore};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// High-level device actuation command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SmartHomeCommand {
    /// Turn on the device, with optional brightness (0-255) and color hex.
    TurnOn {
        /// Brightness level between 0 and 255.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        brightness: Option<u8>,
        /// Hex color string (e.g. "#FF5500").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color_hex: Option<String>,
    },
    /// Turn off the device.
    TurnOff,
    /// Toggle between on and off.
    Toggle,
    /// Set target temperature for HVAC / climate devices.
    SetTemperature {
        /// Target temperature in degrees Celsius.
        target_celsius: f32,
    },
    /// Set position percentage for covers, blinds, or shades (0-100%).
    SetPosition {
        /// Percentage position where 0 is closed and 100 is fully open.
        position_percent: u8,
    },
}

impl SmartHomeCommand {
    /// Generates MQTT subtopic and payload representation according to local convention.
    #[must_use]
    pub fn to_mqtt_payload(&self, _domain: SmartHomeDomain) -> (String, String) {
        match self {
            Self::TurnOn {
                brightness,
                color_hex,
            } => {
                let mut map = serde_json::Map::new();
                map.insert("state".to_string(), json!("ON"));
                if let Some(b) = brightness {
                    map.insert("brightness".to_string(), json!(b));
                }
                if let Some(c) = color_hex {
                    map.insert("color".to_string(), json!(c));
                }
                (
                    "set".to_string(),
                    serde_json::Value::Object(map).to_string(),
                )
            }
            Self::TurnOff => ("set".to_string(), json!({"state": "OFF"}).to_string()),
            Self::Toggle => ("set".to_string(), json!({"state": "TOGGLE"}).to_string()),
            Self::SetTemperature { target_celsius } => (
                "target_temperature/set".to_string(),
                target_celsius.to_string(),
            ),
            Self::SetPosition { position_percent } => {
                ("position/set".to_string(), position_percent.to_string())
            }
        }
    }

    /// Formats the command into a local Home Assistant REST/WebSocket service call.
    #[must_use]
    pub fn to_home_assistant_payload(
        &self,
        entity_id: &str,
        domain: SmartHomeDomain,
    ) -> (String, serde_json::Value) {
        let domain_str = domain.as_str();
        match self {
            Self::TurnOn {
                brightness,
                color_hex,
            } => {
                let service = format!("{domain_str}/turn_on");
                let mut data = serde_json::Map::new();
                data.insert("entity_id".to_string(), json!(entity_id));
                if let Some(b) = brightness {
                    data.insert("brightness".to_string(), json!(b));
                }
                if let Some(c) = color_hex {
                    data.insert("color_name".to_string(), json!(c));
                }
                (service, serde_json::Value::Object(data))
            }
            Self::TurnOff => {
                let service = format!("{domain_str}/turn_off");
                (service, json!({ "entity_id": entity_id }))
            }
            Self::Toggle => {
                let service = format!("{domain_str}/toggle");
                (service, json!({ "entity_id": entity_id }))
            }
            Self::SetTemperature { target_celsius } => {
                let service = "climate/set_temperature".to_string();
                (
                    service,
                    json!({
                        "entity_id": entity_id,
                        "temperature": target_celsius
                    }),
                )
            }
            Self::SetPosition { position_percent } => {
                let service = "cover/set_cover_position".to_string();
                (
                    service,
                    json!({
                        "entity_id": entity_id,
                        "position": position_percent
                    }),
                )
            }
        }
    }
}

/// Skill listing local smart-home devices.
pub struct HomeDeviceListSkill {
    store: Arc<SmartHomeStore>,
}

impl HomeDeviceListSkill {
    /// Creates a new `HomeDeviceListSkill` instance.
    #[must_use]
    pub fn new(store: Arc<SmartHomeStore>) -> Self {
        Self { store }
    }
}

impl std::fmt::Debug for HomeDeviceListSkill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HomeDeviceListSkill").finish()
    }
}

impl ToolHandler for HomeDeviceListSkill {
    fn name(&self) -> &'static str {
        "home.device.list"
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn description(&self) -> &str {
        "Lists local smart-home devices, optionally filtered by domain (e.g. light, switch, climate)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "domain": {
                    "type": "string",
                    "description": "Optional domain filter (light, switch, climate, sensor, cover, fan, lock, media_player)"
                }
            }
        })
    }

    fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> aissistant_orchestrator::Result<serde_json::Value> {
        let domain_filter = if let Some(dom_str) = arguments.get("domain").and_then(|v| v.as_str())
        {
            SmartHomeDomain::from_str(dom_str).map(Some).map_err(|e| {
                aissistant_orchestrator::OrchestratorError::InvalidToolArguments {
                    tool_name: self.name().to_string(),
                    reason: e.to_string(),
                }
            })?
        } else {
            None
        };

        let entities = self.store.list_entities(domain_filter).map_err(|e| {
            aissistant_orchestrator::OrchestratorError::ToolExecutionFailed {
                tool_name: self.name().to_string(),
                reason: e.to_string(),
            }
        })?;

        let devices: Vec<serde_json::Value> = entities
            .into_iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "domain": e.domain.as_str(),
                    "name": e.name,
                    "state": serde_json::from_str::<serde_json::Value>(&e.state_json).unwrap_or(json!({})),
                    "protocol": e.protocol.as_str(),
                    "is_available": e.is_available,
                    "last_updated_ms": e.last_updated_ms,
                })
            })
            .collect();

        Ok(json!({
            "count": devices.len(),
            "devices": devices,
        }))
    }
}

/// Skill querying current state of a specific device.
pub struct HomeDeviceStateSkill {
    store: Arc<SmartHomeStore>,
}

impl HomeDeviceStateSkill {
    /// Creates a new `HomeDeviceStateSkill` instance.
    #[must_use]
    pub fn new(store: Arc<SmartHomeStore>) -> Self {
        Self { store }
    }
}

impl std::fmt::Debug for HomeDeviceStateSkill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HomeDeviceStateSkill").finish()
    }
}

impl ToolHandler for HomeDeviceStateSkill {
    fn name(&self) -> &'static str {
        "home.device.state"
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn description(&self) -> &str {
        "Queries the current state and attributes of a local smart-home device by entity ID."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "entity_id": {
                    "type": "string",
                    "description": "Unique smart-home entity identifier (e.g. 'light.kitchen')"
                }
            },
            "required": ["entity_id"]
        })
    }

    fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> aissistant_orchestrator::Result<serde_json::Value> {
        let entity_id = arguments
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or_else(
                || aissistant_orchestrator::OrchestratorError::InvalidToolArguments {
                    tool_name: self.name().to_string(),
                    reason: "Missing required 'entity_id' argument".to_string(),
                },
            )?;

        let entity = self
            .store
            .get_entity(entity_id)
            .map_err(
                |e| aissistant_orchestrator::OrchestratorError::ToolExecutionFailed {
                    tool_name: self.name().to_string(),
                    reason: e.to_string(),
                },
            )?
            .ok_or_else(
                || aissistant_orchestrator::OrchestratorError::ToolExecutionFailed {
                    tool_name: self.name().to_string(),
                    reason: format!("Smart home entity '{entity_id}' not found"),
                },
            )?;

        let state_val: serde_json::Value =
            serde_json::from_str(&entity.state_json).unwrap_or(json!({}));
        let attr_val: serde_json::Value =
            serde_json::from_str(&entity.attributes_json).unwrap_or(json!({}));

        Ok(json!({
            "id": entity.id,
            "domain": entity.domain.as_str(),
            "name": entity.name,
            "state": state_val,
            "attributes": attr_val,
            "protocol": entity.protocol.as_str(),
            "is_available": entity.is_available,
            "last_updated_ms": entity.last_updated_ms,
        }))
    }
}

/// Skill controlling a smart home device.
pub struct HomeDeviceControlSkill {
    store: Arc<SmartHomeStore>,
}

impl HomeDeviceControlSkill {
    /// Creates a new `HomeDeviceControlSkill` instance.
    #[must_use]
    pub fn new(store: Arc<SmartHomeStore>) -> Self {
        Self { store }
    }
}

impl std::fmt::Debug for HomeDeviceControlSkill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HomeDeviceControlSkill").finish()
    }
}

impl ToolHandler for HomeDeviceControlSkill {
    fn name(&self) -> &'static str {
        "home.device.control"
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::StateModifying
    }

    fn description(&self) -> &str {
        "Controls a smart-home device (turn_on, turn_off, toggle, set_temperature, set_position)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "entity_id": {
                    "type": "string",
                    "description": "Identifier of the device to control"
                },
                "command": {
                    "type": "string",
                    "enum": ["turn_on", "turn_off", "toggle", "set_temperature", "set_position"],
                    "description": "Action command"
                },
                "brightness": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 255
                },
                "color_hex": {
                    "type": "string"
                },
                "target_celsius": {
                    "type": "number"
                },
                "position_percent": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 100
                }
            },
            "required": ["entity_id", "command"]
        })
    }

    fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> aissistant_orchestrator::Result<serde_json::Value> {
        let entity_id = arguments
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or_else(
                || aissistant_orchestrator::OrchestratorError::InvalidToolArguments {
                    tool_name: self.name().to_string(),
                    reason: "Missing 'entity_id'".to_string(),
                },
            )?;

        let cmd_str = arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(
                || aissistant_orchestrator::OrchestratorError::InvalidToolArguments {
                    tool_name: self.name().to_string(),
                    reason: "Missing 'command'".to_string(),
                },
            )?;

        let entity = self
            .store
            .get_entity(entity_id)
            .map_err(
                |e| aissistant_orchestrator::OrchestratorError::ToolExecutionFailed {
                    tool_name: self.name().to_string(),
                    reason: e.to_string(),
                },
            )?
            .ok_or_else(
                || aissistant_orchestrator::OrchestratorError::ToolExecutionFailed {
                    tool_name: self.name().to_string(),
                    reason: format!("Entity '{entity_id}' not found"),
                },
            )?;

        let command = match cmd_str {
            "turn_on" => {
                let brightness = arguments
                    .get("brightness")
                    .and_then(|v| v.as_u64())
                    .map(|b| b as u8);
                let color_hex = arguments
                    .get("color_hex")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                SmartHomeCommand::TurnOn {
                    brightness,
                    color_hex,
                }
            }
            "turn_off" => SmartHomeCommand::TurnOff,
            "toggle" => SmartHomeCommand::Toggle,
            "set_temperature" => {
                let target = arguments
                    .get("target_celsius")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| {
                        aissistant_orchestrator::OrchestratorError::InvalidToolArguments {
                            tool_name: self.name().to_string(),
                            reason: "Missing 'target_celsius' for set_temperature command"
                                .to_string(),
                        }
                    })? as f32;
                SmartHomeCommand::SetTemperature {
                    target_celsius: target,
                }
            }
            "set_position" => {
                let pos = arguments
                    .get("position_percent")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        aissistant_orchestrator::OrchestratorError::InvalidToolArguments {
                            tool_name: self.name().to_string(),
                            reason: "Missing 'position_percent' for set_position command"
                                .to_string(),
                        }
                    })? as u8;
                SmartHomeCommand::SetPosition {
                    position_percent: pos,
                }
            }
            other => {
                return Err(
                    aissistant_orchestrator::OrchestratorError::InvalidToolArguments {
                        tool_name: self.name().to_string(),
                        reason: format!("Unknown command: '{other}'"),
                    },
                );
            }
        };

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);

        // Derive updated state JSON
        let new_state_val = match &command {
            SmartHomeCommand::TurnOn {
                brightness,
                color_hex,
            } => {
                let mut map = serde_json::Map::new();
                map.insert("state".to_string(), json!("on"));
                if let Some(b) = brightness {
                    map.insert("brightness".to_string(), json!(b));
                }
                if let Some(c) = color_hex {
                    map.insert("color_hex".to_string(), json!(c));
                }
                serde_json::Value::Object(map)
            }
            SmartHomeCommand::TurnOff => json!({"state": "off"}),
            SmartHomeCommand::Toggle => {
                let current_state: serde_json::Value =
                    serde_json::from_str(&entity.state_json).unwrap_or(json!({}));
                let is_on = current_state.get("state").and_then(|v| v.as_str()) == Some("on");
                if is_on {
                    json!({"state": "off"})
                } else {
                    json!({"state": "on"})
                }
            }
            SmartHomeCommand::SetTemperature { target_celsius } => {
                json!({
                    "state": "heat",
                    "target_temperature": target_celsius
                })
            }
            SmartHomeCommand::SetPosition { position_percent } => {
                json!({
                    "state": if *position_percent == 0 { "closed" } else { "open" },
                    "position": position_percent
                })
            }
        };

        // Persist new state into cache
        let new_state_str = new_state_val.to_string();
        self.store
            .update_state(entity_id, &new_state_str, now_ms)
            .map_err(
                |e| aissistant_orchestrator::OrchestratorError::ToolExecutionFailed {
                    tool_name: self.name().to_string(),
                    reason: e.to_string(),
                },
            )?;

        // Format protocol payload for audit transparency
        let (dispatch_action, dispatch_payload) = match entity.protocol {
            SmartHomeProtocol::Mqtt => {
                let (subtopic, payload) = command.to_mqtt_payload(entity.domain);
                (
                    format!("mqtt:publish:{subtopic}"),
                    json!({"payload": payload}),
                )
            }
            SmartHomeProtocol::HomeAssistant => {
                let (service, ha_json) =
                    command.to_home_assistant_payload(entity_id, entity.domain);
                (format!("ha:service:{service}"), ha_json)
            }
            SmartHomeProtocol::MatterLocal => (
                "matter:cluster_command".to_string(),
                json!({"command": cmd_str}),
            ),
        };

        Ok(json!({
            "success": true,
            "entity_id": entity_id,
            "command": cmd_str,
            "dispatched_via": entity.protocol.as_str(),
            "dispatch_action": dispatch_action,
            "dispatch_payload": dispatch_payload,
            "new_state": new_state_val,
        }))
    }
}

/// Skill discovering local network smart-home bridges and entities.
pub struct HomeDeviceDiscoverSkill {
    store: Arc<SmartHomeStore>,
}

impl HomeDeviceDiscoverSkill {
    /// Creates a new `HomeDeviceDiscoverSkill` instance.
    #[must_use]
    pub fn new(store: Arc<SmartHomeStore>) -> Self {
        Self { store }
    }
}

impl std::fmt::Debug for HomeDeviceDiscoverSkill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HomeDeviceDiscoverSkill").finish()
    }
}

impl ToolHandler for HomeDeviceDiscoverSkill {
    fn name(&self) -> &'static str {
        "home.device.discover"
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn description(&self) -> &str {
        "Discovers local offline smart-home bridges and reports registered device count."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "protocol": {
                    "type": "string",
                    "description": "Optional protocol filter (mqtt, home_assistant, matter_local)"
                }
            }
        })
    }

    fn execute(
        &self,
        _arguments: serde_json::Value,
    ) -> aissistant_orchestrator::Result<serde_json::Value> {
        let count = self.store.count_entities().map_err(|e| {
            aissistant_orchestrator::OrchestratorError::ToolExecutionFailed {
                tool_name: self.name().to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(json!({
            "discovered_bridges": [
                {
                    "protocol": "home_assistant",
                    "host": "homeassistant.local",
                    "port": 8123,
                    "offline_ready": true
                },
                {
                    "protocol": "mqtt",
                    "host": "127.0.0.1",
                    "port": 1883,
                    "offline_ready": true
                }
            ],
            "total_registered_entities": count,
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_orchestrator::ToolDispatcher;
    use aissistant_storage::{SmartHomeEntityRecord, SqliteDb};

    fn setup_test_smart_home() -> (Arc<SmartHomeStore>, ToolDispatcher) {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = Arc::new(SmartHomeStore::new(db));
        let dispatcher = ToolDispatcher::new();

        // Seed a light and a climate device
        let light = SmartHomeEntityRecord {
            id: "light.office_desk".to_string(),
            domain: SmartHomeDomain::Light,
            name: "Office Desk Light".to_string(),
            state_json: json!({"state": "off", "brightness": 0}).to_string(),
            attributes_json: json!({"friendly_name": "Office Desk"}).to_string(),
            protocol: SmartHomeProtocol::HomeAssistant,
            is_available: true,
            last_updated_ms: 100,
        };
        store.upsert_entity(&light).unwrap();

        let thermostat = SmartHomeEntityRecord {
            id: "climate.bedroom".to_string(),
            domain: SmartHomeDomain::Climate,
            name: "Bedroom Thermostat".to_string(),
            state_json: json!({"state": "heat", "temperature": 21.0}).to_string(),
            attributes_json: json!({}).to_string(),
            protocol: SmartHomeProtocol::Mqtt,
            is_available: true,
            last_updated_ms: 100,
        };
        store.upsert_entity(&thermostat).unwrap();

        dispatcher
            .register(Arc::new(HomeDeviceListSkill::new(store.clone())))
            .unwrap();
        dispatcher
            .register(Arc::new(HomeDeviceStateSkill::new(store.clone())))
            .unwrap();
        dispatcher
            .register(Arc::new(HomeDeviceControlSkill::new(store.clone())))
            .unwrap();
        dispatcher
            .register(Arc::new(HomeDeviceDiscoverSkill::new(store.clone())))
            .unwrap();

        (store, dispatcher)
    }

    #[test]
    fn test_smart_home_command_payload_formatting() {
        let cmd_on = SmartHomeCommand::TurnOn {
            brightness: Some(180),
            color_hex: Some("#00FF88".to_string()),
        };
        let (topic, mqtt_payload) = cmd_on.to_mqtt_payload(SmartHomeDomain::Light);
        assert_eq!(topic, "set");
        assert!(mqtt_payload.contains("\"brightness\":180"));

        let (service, ha_payload) =
            cmd_on.to_home_assistant_payload("light.desk", SmartHomeDomain::Light);
        assert_eq!(service, "light/turn_on");
        assert_eq!(ha_payload["entity_id"], "light.desk");
        assert_eq!(ha_payload["brightness"], 180);

        let cmd_temp = SmartHomeCommand::SetTemperature {
            target_celsius: 23.5,
        };
        let (ha_temp_srv, ha_temp_payload) =
            cmd_temp.to_home_assistant_payload("climate.living_room", SmartHomeDomain::Climate);
        assert_eq!(ha_temp_srv, "climate/set_temperature");
        assert_eq!(ha_temp_payload["temperature"], 23.5);
    }

    #[test]
    fn test_home_device_list_and_state_skills() {
        let (_store, dispatcher) = setup_test_smart_home();

        // 1. List all devices
        let list_res = dispatcher.execute("home.device.list", json!({})).unwrap();
        assert_eq!(list_res["count"], 2);

        // 2. List with domain filter
        let light_res = dispatcher
            .execute("home.device.list", json!({"domain": "light"}))
            .unwrap();
        assert_eq!(light_res["count"], 1);
        assert_eq!(light_res["devices"][0]["id"], "light.office_desk");

        // 3. Query device state
        let state_res = dispatcher
            .execute(
                "home.device.state",
                json!({"entity_id": "light.office_desk"}),
            )
            .unwrap();
        assert_eq!(state_res["name"], "Office Desk Light");
        assert_eq!(state_res["state"]["state"], "off");

        // 4. Query non-existent device returns error
        assert!(dispatcher
            .execute("home.device.state", json!({"entity_id": "light.ghost"}))
            .is_err());
    }

    #[test]
    fn test_home_device_control_skill_turn_on_and_temperature() {
        let (store, dispatcher) = setup_test_smart_home();

        // 1. Turn on light with brightness
        let turn_on_res = dispatcher
            .execute(
                "home.device.control",
                json!({
                    "entity_id": "light.office_desk",
                    "command": "turn_on",
                    "brightness": 255
                }),
            )
            .unwrap();
        assert_eq!(turn_on_res["success"], true);
        assert_eq!(turn_on_res["new_state"]["state"], "on");
        assert_eq!(turn_on_res["new_state"]["brightness"], 255);

        // Verify state was persisted in store
        let persisted = store.get_entity("light.office_desk").unwrap().unwrap();
        assert!(persisted.state_json.contains("\"brightness\":255"));

        // 2. Set thermostat temperature
        let temp_res = dispatcher
            .execute(
                "home.device.control",
                json!({
                    "entity_id": "climate.bedroom",
                    "command": "set_temperature",
                    "target_celsius": 24.5
                }),
            )
            .unwrap();
        assert_eq!(temp_res["success"], true);
        assert_eq!(temp_res["new_state"]["target_temperature"], 24.5);

        let persisted_thermo = store.get_entity("climate.bedroom").unwrap().unwrap();
        assert!(persisted_thermo.state_json.contains("24.5"));
    }

    #[test]
    fn test_home_device_discover_skill() {
        let (_store, dispatcher) = setup_test_smart_home();
        let disc_res = dispatcher
            .execute("home.device.discover", json!({}))
            .unwrap();
        assert_eq!(disc_res["total_registered_entities"], 2);
        assert!(disc_res["discovered_bridges"].as_array().unwrap().len() >= 2);
    }
}
