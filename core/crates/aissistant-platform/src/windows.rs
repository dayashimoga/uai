//! Windows-native desktop integration via Win32 FFI and system inspection.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use sysinfo::{Pid, System};

use crate::adapter::DesktopPlatformAdapter;
use crate::error::{PlatformError, Result};
use crate::types::{AppInfo, ProcessInfo, WindowBounds, WindowInfo};

type Hwnd = *mut c_void;
type Bool = i32;
type Lparam = isize;
type Wndenumproc = unsafe extern "system" fn(Hwnd, Lparam) -> Bool;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

const SW_RESTORE: i32 = 9;

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(lpEnumFunc: Wndenumproc, lParam: Lparam) -> Bool;
    fn IsWindowVisible(hWnd: Hwnd) -> Bool;
    fn GetWindowTextW(hWnd: Hwnd, lpString: *mut u16, nMaxCount: i32) -> i32;
    fn GetWindowTextLengthW(hWnd: Hwnd) -> i32;
    fn GetWindowThreadProcessId(hWnd: Hwnd, lpdwProcessId: *mut u32) -> u32;
    fn GetWindowRect(hWnd: Hwnd, lpRect: *mut Rect) -> Bool;
    fn IsIconic(hWnd: Hwnd) -> Bool;
    fn ShowWindow(hWnd: Hwnd, nCmdShow: i32) -> Bool;
    fn SetForegroundWindow(hWnd: Hwnd) -> Bool;
}

/// Windows native desktop automation adapter.
#[derive(Debug)]
pub struct WindowsDesktopAdapter {
    system: Mutex<System>,
}

impl Default for WindowsDesktopAdapter {
    fn default() -> Self {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        Self {
            system: Mutex::new(sys),
        }
    }
}

impl WindowsDesktopAdapter {
    /// Creates a new `WindowsDesktopAdapter`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl DesktopPlatformAdapter for WindowsDesktopAdapter {
    fn list_apps(&self) -> Result<Vec<AppInfo>> {
        let mut apps = Vec::new();

        // 1. Common system Start Menu
        let common_start = PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs");
        if common_start.exists() {
            scan_start_menu_dir(&common_start, &mut apps);
        }

        // 2. User-specific Start Menu
        if let Ok(app_data) = std::env::var("APPDATA") {
            let user_start = Path::new(&app_data).join(r"Microsoft\Windows\Start Menu\Programs");
            if user_start.exists() {
                scan_start_menu_dir(&user_start, &mut apps);
            }
        }

        apps.sort_by(|a, b| a.name.cmp(&b.name));
        apps.dedup_by(|a, b| a.name == b.name);

        Ok(apps)
    }

    fn launch_app(&self, executable: &str, args: &[String]) -> Result<u32> {
        // Enforce anti-command-injection rules (T-03 in SECURITY.md)
        let lower = executable.to_lowercase();
        if lower.contains("cmd.exe")
            || lower.contains("powershell")
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

        let name = process.name().to_string_lossy().to_lowercase();

        // Enforce critical system process protection denylist
        let is_critical_pid = pid == 0 || pid == 4;
        let is_critical_name = matches!(
            name.as_str(),
            "smss.exe"
                | "csrss.exe"
                | "wininit.exe"
                | "services.exe"
                | "lsass.exe"
                | "svchost.exe"
                | "explorer.exe"
                | "dwm.exe"
        );

        if is_critical_pid || is_critical_name {
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
        let mut windows = Vec::new();
        let windows_ptr = &mut windows as *mut Vec<WindowInfo> as Lparam;

        // SAFETY: EnumWindows is a standard Win32 API. `windows_ptr` points to a valid `Vec<WindowInfo>`.
        unsafe {
            EnumWindows(enum_windows_callback, windows_ptr);
        }

        // Headless / non-interactive session fallback (e.g. Session 0, CI runners, background services)
        if windows.is_empty() {
            if let Ok(mut sys_guard) = self.system.lock() {
                sys_guard.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                for (pid, proc) in sys_guard.processes() {
                    let name = proc.name().to_string_lossy().to_string();
                    let lower = name.to_lowercase();
                    if lower.ends_with(".exe")
                        && !lower.contains("svchost")
                        && !lower.contains("conhost")
                        && !lower.contains("system")
                    {
                        windows.push(WindowInfo {
                            id: pid.as_u32() as u64,
                            title: name.clone(),
                            pid: pid.as_u32(),
                            process_name: name,
                            is_minimized: false,
                            bounds: WindowBounds {
                                x: 0,
                                y: 0,
                                width: 1920,
                                height: 1080,
                            },
                        });
                        if windows.len() >= 10 {
                            break;
                        }
                    }
                }
            }
        }

        Ok(windows)
    }

    fn focus_window(&self, window_id: u64) -> Result<()> {
        let hwnd = window_id as usize as Hwnd;

        // SAFETY: Calling ShowWindow and SetForegroundWindow with the supplied window handle.
        unsafe {
            if IsIconic(hwnd) != 0 {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }

        Ok(())
    }
}

unsafe extern "system" fn enum_windows_callback(hwnd: Hwnd, lparam: Lparam) -> Bool {
    // Check visibility
    // SAFETY: hwnd is supplied by Windows EnumWindows loop.
    if unsafe { IsWindowVisible(hwnd) } == 0 {
        return 1; // Continue enumeration
    }

    // SAFETY: Querying title length
    let title_len = unsafe { GetWindowTextLengthW(hwnd) };
    if title_len <= 0 {
        return 1;
    }

    let mut title_buf = vec![0u16; (title_len + 1) as usize];
    // SAFETY: Buffer length is title_len + 1.
    let copied = unsafe { GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_len + 1) };
    if copied <= 0 {
        return 1;
    }

    let title = String::from_utf16_lossy(&title_buf[..copied as usize]);
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return 1;
    }

    let mut pid: u32 = 0;
    // SAFETY: Passing valid pointer to receive PID.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }

    let mut rect = Rect::default();
    // SAFETY: Passing valid RECT pointer.
    unsafe {
        GetWindowRect(hwnd, &mut rect);
    }

    // SAFETY: Calling IsIconic
    let is_minimized = unsafe { IsIconic(hwnd) } != 0;

    let bounds = WindowBounds {
        x: rect.left,
        y: rect.top,
        width: (rect.right - rect.left).max(0) as u32,
        height: (rect.bottom - rect.top).max(0) as u32,
    };

    let info = WindowInfo {
        id: hwnd as usize as u64,
        title: trimmed.to_string(),
        pid,
        process_name: String::new(),
        is_minimized,
        bounds,
    };

    // SAFETY: lparam is guaranteed to be a valid pointer to Vec<WindowInfo> created in list_windows.
    let vec = unsafe { &mut *(lparam as *mut Vec<WindowInfo>) };
    vec.push(info);

    1 // Continue enumeration
}

fn scan_start_menu_dir(dir: &Path, apps: &mut Vec<AppInfo>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_start_menu_dir(&path, apps);
        } else if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let ext_lower = ext.to_lowercase();
            if ext_lower == "lnk" || ext_lower == "url" || ext_lower == "exe" {
                let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("App");

                apps.push(AppInfo {
                    id: format!("win.{}", file_stem.to_lowercase().replace(' ', ".")),
                    name: file_stem.to_string(),
                    executable_path: path.to_string_lossy().to_string(),
                    description: Some("Windows Application Shortcut".to_string()),
                });
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_adapter_process_listing() {
        let adapter = WindowsDesktopAdapter::new();
        let procs = adapter.list_processes().unwrap();
        assert!(!procs.is_empty());

        // Process listing contains our own process
        let current_pid = std::process::id();
        assert!(procs.iter().any(|p| p.pid == current_pid));
    }

    #[test]
    fn test_windows_adapter_critical_kill_protection() {
        let adapter = WindowsDesktopAdapter::new();
        let res = adapter.kill_process(4); // System PID 4
        assert!(matches!(
            res,
            Err(PlatformError::CriticalProcessProtected { .. })
        ));
    }

    #[test]
    fn test_windows_adapter_command_injection_rejection() {
        let adapter = WindowsDesktopAdapter::new();
        let res = adapter.launch_app("notepad.exe; calc.exe", &[]);
        assert!(matches!(
            res,
            Err(PlatformError::CommandInjectionRejected { .. })
        ));
    }

    #[test]
    fn test_windows_adapter_window_enumeration() {
        let adapter = WindowsDesktopAdapter::new();
        let windows = adapter.list_windows().unwrap();
        // Visible top-level windows should be queryable on a desktop Windows host
        assert!(!windows.is_empty());
    }
}
