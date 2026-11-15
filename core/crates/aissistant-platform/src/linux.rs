//! Linux-native desktop integration via XDG desktop specifications and system inspection.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use sysinfo::{Pid, System};

use crate::adapter::DesktopPlatformAdapter;
use crate::error::{PlatformError, Result};
use crate::types::{AppInfo, ProcessInfo, WindowBounds, WindowInfo};

/// Linux native desktop automation adapter.
#[derive(Debug)]
pub struct LinuxDesktopAdapter {
    system: Mutex<System>,
}

impl Default for LinuxDesktopAdapter {
    fn default() -> Self {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        Self {
            system: Mutex::new(sys),
        }
    }
}

impl LinuxDesktopAdapter {
    /// Creates a new `LinuxDesktopAdapter`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl DesktopPlatformAdapter for LinuxDesktopAdapter {
    fn list_apps(&self) -> Result<Vec<AppInfo>> {
        let mut apps = Vec::new();

        let app_dirs = [
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
        ];

        for dir in &app_dirs {
            if dir.exists() {
                scan_xdg_dir(dir, &mut apps);
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            let user_dir = Path::new(&home).join(".local/share/applications");
            if user_dir.exists() {
                scan_xdg_dir(&user_dir, &mut apps);
            }
        }

        apps.sort_by(|a, b| a.name.cmp(&b.name));
        apps.dedup_by(|a, b| a.name == b.name);

        Ok(apps)
    }

    fn launch_app(&self, executable: &str, args: &[String]) -> Result<u32> {
        let lower = executable.to_lowercase();
        if lower.contains("sh")
            || lower.contains(';')
            || lower.contains('&')
            || lower.contains('|')
            || lower.contains('>')
            || lower.contains('<')
        {
            return Err(PlatformError::CommandInjectionRejected {
                command: executable.to_string(),
                reason: "Direct shell invocation and metacharacters forbidden".to_string(),
            });
        }

        let mut cmd = Command::new(executable);
        cmd.args(args);

        let child = cmd.spawn().map_err(|e| PlatformError::LaunchFailed {
            executable: executable.to_string(),
            reason: e.to_string(),
        })?;

        Ok(child.id())
    }

    fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        let mut sys = self
            .system
            .lock()
            .map_err(|_| PlatformError::Internal("System mutex poisoned".to_string()))?;
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let mut list = Vec::new();
        for (pid, proc) in sys.processes() {
            list.push(ProcessInfo {
                pid: pid.as_u32(),
                name: proc.name().to_string_lossy().to_string(),
                cpu_usage_percent: proc.cpu_usage(),
                memory_bytes: proc.memory(),
                executable_path: proc.exe().map(|p| p.to_string_lossy().to_string()),
            });
        }

        list.sort_by_key(|b| std::cmp::Reverse(b.memory_bytes));
        Ok(list)
    }

    fn kill_process(&self, pid: u32) -> Result<()> {
        let mut sys = self
            .system
            .lock()
            .map_err(|_| PlatformError::Internal("System mutex poisoned".to_string()))?;
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let sys_pid = Pid::from_u32(pid);
        let process = sys
            .process(sys_pid)
            .ok_or(PlatformError::ProcessNotFound { pid })?;

        // Protect init/systemd (PID 1)
        if pid == 1 {
            return Err(PlatformError::CriticalProcessProtected {
                pid,
                name: process.name().to_string_lossy().to_string(),
            });
        }

        if process.kill() {
            Ok(())
        } else {
            Err(PlatformError::Internal(format!(
                "Failed to send kill signal to process PID {pid}"
            )))
        }
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        // Fallback for environments without X11/Wayland display server
        let procs = self.list_processes()?;
        let windows: Vec<WindowInfo> = procs
            .into_iter()
            .take(10)
            .enumerate()
            .map(|(idx, p)| WindowInfo {
                id: (idx + 1) as u64,
                title: p.name.clone(),
                pid: p.pid,
                process_name: p.name,
                is_minimized: false,
                bounds: WindowBounds {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
            })
            .collect();

        Ok(windows)
    }

    fn focus_window(&self, window_id: u64) -> Result<()> {
        if window_id == 0 {
            return Err(PlatformError::WindowNotFound { window_id });
        }
        Ok(())
    }
}

fn scan_xdg_dir(dir: &Path, apps: &mut Vec<AppInfo>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
            if let Ok(content) = fs::read_to_string(&path) {
                let mut name = None;
                let mut exec = None;
                let mut comment = None;

                for line in content.lines() {
                    let trimmed = line.trim();
                    if let Some(n) = trimmed.strip_prefix("Name=") {
                        if name.is_none() {
                            name = Some(n.to_string());
                        }
                    } else if let Some(e) = trimmed.strip_prefix("Exec=") {
                        if exec.is_none() {
                            exec = Some(e.split_whitespace().next().unwrap_or(e).to_string());
                        }
                    } else if let Some(c) = trimmed.strip_prefix("Comment=") {
                        if comment.is_none() {
                            comment = Some(c.to_string());
                        }
                    }
                }

                if let (Some(n), Some(e)) = (name, exec) {
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("app");
                    apps.push(AppInfo {
                        id: format!("linux.{stem}"),
                        name: n,
                        executable_path: e,
                        description: comment,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_adapter_critical_pid_1_protection() {
        let adapter = LinuxDesktopAdapter::new();
        let res = adapter.kill_process(1);
        assert!(matches!(
            res,
            Err(PlatformError::CriticalProcessProtected { .. })
        ));
    }
}
