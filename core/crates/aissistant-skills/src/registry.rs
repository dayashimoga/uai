//! Central skill registry and built-in utility and file skill initialization.

use std::sync::Arc;

use aissistant_orchestrator::{ToolDispatcher, ToolHandler};

use crate::error::Result;
use crate::file::{
    FileArchiveSkill, FileChecksumSkill, FileDeleteSkill, FileMoveSkill, FileReadSkill,
    FileSearchSkill, FileWriteSkill, PathJail,
};
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

    /// Creates instances of all safe file management skills wrapped in `SandboxedSkill`.
    #[must_use]
    pub fn file_skills(jail: Option<Arc<PathJail>>) -> Vec<Arc<dyn ToolHandler>> {
        let active_jail = jail.unwrap_or_else(|| Arc::new(PathJail::default()));
        vec![
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                FileSearchSkill::with_jail(active_jail.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                FileReadSkill::with_jail(active_jail.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                FileWriteSkill::with_jail(active_jail.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                FileMoveSkill::with_jail(active_jail.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                FileDeleteSkill::with_jail(active_jail.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                FileArchiveSkill::with_jail(active_jail.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                FileChecksumSkill::with_jail(active_jail),
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

    /// Registers all file management skills into the supplied tool dispatcher.
    pub fn register_all_file_skills(
        &self,
        dispatcher: &ToolDispatcher,
        jail: Option<Arc<PathJail>>,
    ) -> Result<()> {
        for skill in Self::file_skills(jail) {
            dispatcher.register(skill)?;
        }
        Ok(())
    }

    /// Registers all built-in utility and file management skills into the tool dispatcher.
    pub fn register_all(
        &self,
        dispatcher: &ToolDispatcher,
        jail: Option<Arc<PathJail>>,
    ) -> Result<()> {
        self.register_all_builtins(dispatcher)?;
        self.register_all_file_skills(dispatcher, jail)?;
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

    #[test]
    fn test_file_skills_registration() {
        let registry = SkillRegistry::new();
        let dispatcher = ToolDispatcher::new();

        registry
            .register_all_file_skills(&dispatcher, None)
            .unwrap();

        let defs = dispatcher.tool_definitions().unwrap();
        assert_eq!(defs.len(), 7);

        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"file.search"));
        assert!(names.contains(&"file.read"));
        assert!(names.contains(&"file.write"));
        assert!(names.contains(&"file.move"));
        assert!(names.contains(&"file.delete"));
        assert!(names.contains(&"file.archive"));
        assert!(names.contains(&"file.checksum"));
    }

    #[test]
    fn test_register_all_combined() {
        let registry = SkillRegistry::new();
        let dispatcher = ToolDispatcher::new();

        registry.register_all(&dispatcher, None).unwrap();

        let defs = dispatcher.tool_definitions().unwrap();
        assert_eq!(defs.len(), 12);
    }
}
