//! Trait definition and virtual reference implementation for desktop platform adapters.

use std::sync::RwLock;

use crate::error::{PlatformError, Result};
use crate::types::{
    AppInfo, BatteryStatus, MediaCommand, ProcessInfo, VolumeStatus, WindowBounds, WindowInfo,
};

/// Common contract for platform-native application and desktop window operations.
pub trait DesktopPlatformAdapter: std::fmt::Debug + Send + Sync {
    /// Discovers and lists registered desktop applications.
    fn list_apps(&self) -> Result<Vec<AppInfo>>;

    /// Spawns a process directly with arguments without invoking a shell interpreter.
    fn launch_app(&self, executable: &str, args: &[String]) -> Result<u32>;

    /// Lists active operating system processes with memory and CPU usage metrics.
    fn list_processes(&self) -> Result<Vec<ProcessInfo>>;

    /// Terminates an active process by PID, subject to critical process protections.
    fn kill_process(&self, pid: u32) -> Result<()>;

    /// Enumerates visible top-level desktop windows.
    fn list_windows(&self) -> Result<Vec<WindowInfo>>;

    /// Brings the specified window to the foreground and assigns input focus.
    fn focus_window(&self, window_id: u64) -> Result<()>;

    /// Reads text currently stored on the operating system clipboard.
    fn read_clipboard(&self) -> Result<String>;

    /// Writes text to the operating system clipboard.
    fn write_clipboard(&self, text: &str) -> Result<()>;

    /// Queries the current master audio output volume and mute status.
    fn get_master_volume(&self) -> Result<VolumeStatus>;

    /// Sets the master audio volume level (0 to 100) or updates mute status.
    fn set_master_volume(&self, level: Option<u8>, mute: Option<bool>) -> Result<VolumeStatus>;

    /// Dispatches a media playback transport command (play, pause, next, etc.).
    fn media_transport(&self, command: MediaCommand) -> Result<()>;

    /// Queries host battery charge and power source telemetry.
    fn get_battery_status(&self) -> Result<BatteryStatus>;
}

/// Hermetic in-memory desktop adapter for testing and non-desktop environments.
#[derive(Debug)]
pub struct VirtualDesktopAdapter {
    apps: RwLock<Vec<AppInfo>>,
    processes: RwLock<Vec<ProcessInfo>>,
    windows: RwLock<Vec<WindowInfo>>,
    focused_window_id: RwLock<Option<u64>>,
    next_pid: RwLock<u32>,
    clipboard: RwLock<String>,
    volume: RwLock<VolumeStatus>,
    last_media_cmd: RwLock<Option<MediaCommand>>,
    battery: RwLock<BatteryStatus>,
}

impl Default for VirtualDesktopAdapter {
    fn default() -> Self {
        Self {
            apps: RwLock::new(vec![
                AppInfo {
                    id: "virtual.terminal".to_string(),
                    name: "Terminal".to_string(),
                    executable_path: "/bin/terminal".to_string(),
                    description: Some("System Terminal Emulator".to_string()),
                },
                AppInfo {
                    id: "virtual.editor".to_string(),
                    name: "Text Editor".to_string(),
                    executable_path: "/bin/editor".to_string(),
                    description: Some("Text and code editor".to_string()),
                },
            ]),
            processes: RwLock::new(vec![
                ProcessInfo {
                    pid: 1,
                    name: "systemd".to_string(),
                    cpu_usage_percent: 0.1,
                    memory_bytes: 32 * 1024 * 1024,
                    executable_path: Some("/sbin/init".to_string()),
                },
                ProcessInfo {
                    pid: 42,
                    name: "editor".to_string(),
                    cpu_usage_percent: 2.5,
                    memory_bytes: 120 * 1024 * 1024,
                    executable_path: Some("/bin/editor".to_string()),
                },
            ]),
            windows: RwLock::new(vec![WindowInfo {
                id: 1001,
                title: "Editor - main.rs".to_string(),
                pid: 42,
                process_name: "editor".to_string(),
                is_minimized: false,
                bounds: WindowBounds {
                    x: 100,
                    y: 100,
                    width: 1024,
                    height: 768,
                },
            }]),
            focused_window_id: RwLock::new(Some(1001)),
            next_pid: RwLock::new(100),
            clipboard: RwLock::new("initial virtual clipboard text".to_string()),
            volume: RwLock::new(VolumeStatus {
                volume_percent: 75,
                is_muted: false,
            }),
            last_media_cmd: RwLock::new(None),
            battery: RwLock::new(BatteryStatus {
                percentage: Some(85),
                is_charging: true,
                on_ac_power: true,
            }),
        }
    }
}

impl VirtualDesktopAdapter {
    /// Creates a new `VirtualDesktopAdapter` pre-populated with standard dummy items.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the currently focused window ID.
    #[must_use]
    pub fn focused_window(&self) -> Option<u64> {
        self.focused_window_id.read().ok().and_then(|g| *g)
    }

    /// Returns the most recently dispatched media command.
    #[must_use]
    pub fn last_media_command(&self) -> Option<MediaCommand> {
        self.last_media_cmd.read().ok().and_then(|g| *g)
    }
}

impl DesktopPlatformAdapter for VirtualDesktopAdapter {
    fn list_apps(&self) -> Result<Vec<AppInfo>> {
        let guard = self
            .apps
            .read()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        Ok(guard.clone())
    }

    fn launch_app(&self, executable: &str, _args: &[String]) -> Result<u32> {
        if executable.contains("cmd.exe") || executable.contains("sh") || executable.contains(';') {
            return Err(PlatformError::CommandInjectionRejected {
                command: executable.to_string(),
                reason: "Direct shell invocation or command chaining forbidden".to_string(),
            });
        }

        let mut next_pid = self
            .next_pid
            .write()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        let pid = *next_pid;
        *next_pid += 1;

        let mut procs = self
            .processes
            .write()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        procs.push(ProcessInfo {
            pid,
            name: executable.to_string(),
            cpu_usage_percent: 1.0,
            memory_bytes: 64 * 1024 * 1024,
            executable_path: Some(executable.to_string()),
        });

        Ok(pid)
    }

    fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        let guard = self
            .processes
            .read()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        Ok(guard.clone())
    }

    fn kill_process(&self, pid: u32) -> Result<()> {
        if pid == 0 || pid == 1 || pid == 4 {
            return Err(PlatformError::CriticalProcessProtected {
                pid,
                name: "Critical OS System Process".to_string(),
            });
        }

        let mut procs = self
            .processes
            .write()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        let original_len = procs.len();
        procs.retain(|p| p.pid != pid);

        if procs.len() == original_len {
            return Err(PlatformError::ProcessNotFound { pid });
        }

        Ok(())
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        let guard = self
            .windows
            .read()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        Ok(guard.clone())
    }

    fn focus_window(&self, window_id: u64) -> Result<()> {
        let windows = self
            .windows
            .read()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        if !windows.iter().any(|w| w.id == window_id) {
            return Err(PlatformError::WindowNotFound { window_id });
        }

        let mut focus = self
            .focused_window_id
            .write()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        *focus = Some(window_id);
        Ok(())
    }

    fn read_clipboard(&self) -> Result<String> {
        let guard = self
            .clipboard
            .read()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        Ok(guard.clone())
    }

    fn write_clipboard(&self, text: &str) -> Result<()> {
        let mut guard = self
            .clipboard
            .write()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        *guard = text.to_string();
        Ok(())
    }

    fn get_master_volume(&self) -> Result<VolumeStatus> {
        let guard = self
            .volume
            .read()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        Ok(*guard)
    }

    fn set_master_volume(&self, level: Option<u8>, mute: Option<bool>) -> Result<VolumeStatus> {
        let mut guard = self
            .volume
            .write()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        if let Some(l) = level {
            guard.volume_percent = l.min(100);
        }
        if let Some(m) = mute {
            guard.is_muted = m;
        }
        Ok(*guard)
    }

    fn media_transport(&self, command: MediaCommand) -> Result<()> {
        let mut guard = self
            .last_media_cmd
            .write()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        *guard = Some(command);
        Ok(())
    }

    fn get_battery_status(&self) -> Result<BatteryStatus> {
        let guard = self
            .battery
            .read()
            .map_err(|_| PlatformError::Internal("Lock poisoned".to_string()))?;
        Ok(guard.clone())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_adapter_lifecycle() {
        let adapter = VirtualDesktopAdapter::new();

        // 1. list apps
        let apps = adapter.list_apps().unwrap();
        assert_eq!(apps.len(), 2);

        // 2. launch app
        let pid = adapter.launch_app("new_app", &[]).unwrap();
        assert!(pid >= 100);

        // 3. list processes
        let procs = adapter.list_processes().unwrap();
        assert!(procs.iter().any(|p| p.pid == pid));

        // 4. kill protected process rejected
        let err_kill = adapter.kill_process(1);
        assert!(matches!(
            err_kill,
            Err(PlatformError::CriticalProcessProtected { .. })
        ));

        // 5. kill created process
        adapter.kill_process(pid).unwrap();
        let procs_after = adapter.list_processes().unwrap();
        assert!(!procs_after.iter().any(|p| p.pid == pid));

        // 6. window focus
        adapter.focus_window(1001).unwrap();
        assert_eq!(adapter.focused_window(), Some(1001));

        let err_win = adapter.focus_window(9999);
        assert!(matches!(err_win, Err(PlatformError::WindowNotFound { .. })));

        // 7. clipboard read & write
        adapter.write_clipboard("updated text").unwrap();
        assert_eq!(adapter.read_clipboard().unwrap(), "updated text");

        // 8. volume get & set
        let vol = adapter.set_master_volume(Some(50), Some(true)).unwrap();
        assert_eq!(vol.volume_percent, 50);
        assert!(vol.is_muted);

        // 9. media transport
        adapter.media_transport(MediaCommand::PlayPause).unwrap();
        assert_eq!(adapter.last_media_command(), Some(MediaCommand::PlayPause));

        // 10. battery status
        let battery = adapter.get_battery_status().unwrap();
        assert_eq!(battery.percentage, Some(85));
        assert!(battery.on_ac_power);
    }
}
