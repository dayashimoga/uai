//! Central skill registry and built-in utility skill initialization.

use std::sync::Arc;

use aissistant_orchestrator::{ToolDispatcher, ToolHandler};

use crate::error::Result;
use crate::sandbox::SandboxedSkill;
use crate::utility::{CalcSkill, ConvertSkill, TelemetrySkill, TimeSkill, TimerSkill};

/// Central registry managing discovery, sandboxing, and registration of skills.
#[derive(Debug, Default, Clone, Copy)]
pub struct SkillRegistry;

impl SkillRegistry {
    /// Creates a new `SkillRegistry` instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Creates instances of all built-in utility skills wrapped in `SandboxedSkill`.
    #[must_use]
    pub fn builtin_skills() -> Vec<Arc<dyn ToolHandler>> {
        vec![
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                TimeSkill::new(),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                TimerSkill::new(),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                CalcSkill::new(),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                ConvertSkill::new(),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                TelemetrySkill::new(),
            ))),
        ]
    }

    /// Registers all built-in utility skills into the supplied tool dispatcher.
    pub fn register_all_builtins(&self, dispatcher: &ToolDispatcher) -> Result<()> {
        for skill in Self::builtin_skills() {
            dispatcher.register(skill)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_skills_registration() {
        let registry = SkillRegistry::new();
        let dispatcher = ToolDispatcher::new();

        registry.register_all_builtins(&dispatcher).unwrap();

        let defs = dispatcher.tool_definitions().unwrap();
        assert_eq!(defs.len(), 5);

        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"system.time"));
        assert!(names.contains(&"system.timer"));
        assert!(names.contains(&"system.calc"));
        assert!(names.contains(&"system.convert"));
        assert!(names.contains(&"system.telemetry"));
    }
}
