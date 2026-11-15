//! Central skill registry and built-in utility, file, and desktop skill initialization.

use std::sync::Arc;

use aissistant_orchestrator::{ToolDispatcher, ToolHandler};
use aissistant_platform::{create_default_desktop_adapter, DesktopPlatformAdapter};

use crate::desktop::{
    AppLaunchSkill, AppListSkill, ClipboardReadSkill, ClipboardWriteSkill, MediaTransportSkill,
    ProcessKillSkill, ProcessListSkill, SystemBatterySkill, VolumeGetSkill, VolumeSetSkill,
    WindowFocusSkill, WindowListSkill,
};
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

    /// Creates instances of all desktop OS automation skills wrapped in `SandboxedSkill`.
    #[must_use]
    pub fn desktop_skills(
        adapter: Option<Arc<dyn DesktopPlatformAdapter>>,
    ) -> Vec<Arc<dyn ToolHandler>> {
        let active_adapter = adapter.unwrap_or_else(create_default_desktop_adapter);
        vec![
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                AppListSkill::new(active_adapter.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                AppLaunchSkill::new(active_adapter.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                ProcessListSkill::new(active_adapter.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                ProcessKillSkill::new(active_adapter.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                WindowListSkill::new(active_adapter.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                WindowFocusSkill::new(active_adapter.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                ClipboardReadSkill::new(active_adapter.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                ClipboardWriteSkill::new(active_adapter.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                VolumeGetSkill::new(active_adapter.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                VolumeSetSkill::new(active_adapter.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                MediaTransportSkill::new(active_adapter.clone()),
            ))),
            Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                SystemBatterySkill::new(active_adapter),
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

    /// Registers all desktop native automation skills into the supplied tool dispatcher.
    pub fn register_all_desktop_skills(
        &self,
        dispatcher: &ToolDispatcher,
        adapter: Option<Arc<dyn DesktopPlatformAdapter>>,
    ) -> Result<()> {
        for skill in Self::desktop_skills(adapter) {
            dispatcher.register(skill)?;
        }
        Ok(())
    }

    /// Registers all built-in utility, file management, and desktop skills into the tool dispatcher.
    pub fn register_all(
        &self,
        dispatcher: &ToolDispatcher,
        jail: Option<Arc<PathJail>>,
        adapter: Option<Arc<dyn DesktopPlatformAdapter>>,
    ) -> Result<()> {
        self.register_all_builtins(dispatcher)?;
        self.register_all_file_skills(dispatcher, jail)?;
        self.register_all_desktop_skills(dispatcher, adapter)?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_platform::VirtualDesktopAdapter;

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
    fn test_desktop_skills_registration() {
        let registry = SkillRegistry::new();
        let dispatcher = ToolDispatcher::new();
        let v_adapter = Arc::new(VirtualDesktopAdapter::new());

        registry
            .register_all_desktop_skills(&dispatcher, Some(v_adapter))
            .unwrap();

        let defs = dispatcher.tool_definitions().unwrap();
        assert_eq!(defs.len(), 12);

        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"app.list"));
        assert!(names.contains(&"app.launch"));
        assert!(names.contains(&"process.list"));
        assert!(names.contains(&"process.kill"));
        assert!(names.contains(&"window.list"));
        assert!(names.contains(&"window.focus"));
        assert!(names.contains(&"clipboard.read"));
        assert!(names.contains(&"clipboard.write"));
        assert!(names.contains(&"volume.get"));
        assert!(names.contains(&"volume.set"));
        assert!(names.contains(&"media.transport"));
        assert!(names.contains(&"system.battery"));
    }

    #[test]
    fn test_register_all_combined() {
        let registry = SkillRegistry::new();
        let dispatcher = ToolDispatcher::new();
        let v_adapter = Arc::new(VirtualDesktopAdapter::new());

        registry
            .register_all(&dispatcher, None, Some(v_adapter))
            .unwrap();

        let defs = dispatcher.tool_definitions().unwrap();
        assert_eq!(defs.len(), 24);
    }
}
