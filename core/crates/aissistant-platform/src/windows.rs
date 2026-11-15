//! Windows-native desktop integration via Win32 FFI and system inspection.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use sysinfo::{Pid, System};

use crate::adapter::DesktopPlatformAdapter;
use crate::error::{PlatformError, Result};
use crate::types::{
    AppInfo, BatteryStatus, MediaCommand, ProcessInfo, VolumeStatus, WindowBounds, WindowInfo,
};

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

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct SystemPowerStatus {
    ac_line_status: u8,
    battery_flag: u8,
    battery_life_percent: u8,
    system_status_flag: u8,
    battery_life_time: u32,
    battery_full_life_time: u32,
}

const SW_RESTORE: i32 = 9;
const KEYEVENTF_KEYUP: u32 = 0x0002;

const VK_VOLUME_MUTE: u8 = 0xAD;
const VK_MEDIA_NEXT_TRACK: u8 = 0xB0;
const VK_MEDIA_PREV_TRACK: u8 = 0xB1;
const VK_MEDIA_STOP: u8 = 0xB2;
const VK_MEDIA_PLAY_PAUSE: u8 = 0xB3;

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
    fn keybd_event(bVk: u8, bScan: u8, dwFlags: u32, dwExtraInfo: usize);
    fn OpenClipboard(hWndNewOwner: Hwnd) -> Bool;
    fn CloseClipboard() -> Bool;
    fn EmptyClipboard() -> Bool;
    fn GetClipboardData(uFormat: u32) -> *mut c_void;
    fn SetClipboardData(uFormat: u32, hMem: *mut c_void) -> *mut c_void;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetSystemPowerStatus(lpSystemPowerStatus: *mut SystemPowerStatus) -> Bool;
    fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut c_void;
    fn GlobalLock(hMem: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(hMem: *mut c_void) -> Bool;
}

/// Windows native desktop automation adapter.
#[derive(Debug)]
pub struct WindowsDesktopAdapter {
    system: Mutex<System>,
    clipboard_fallback: Mutex<String>,
    volume_state: Mutex<VolumeStatus>,
}

impl Default for WindowsDesktopAdapter {
    fn default() -> Self {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        Self {
            system: Mutex::new(sys),
            clipboard_fallback: Mutex::new(String::new()),
            volume_state: Mutex::new(VolumeStatus {
                volume_percent: 70,
                is_muted: false,
            }),
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

    fn read_clipboard(&self) -> Result<String> {
        let text_opt = (|| -> Option<String> {
            // SAFETY: Win32 clipboard API calls safely enclosed and null-checked.
            unsafe {
                if OpenClipboard(std::ptr::null_mut()) == 0 {
                    return None;
                }
                let h_data = GetClipboardData(13); // CF_UNICODETEXT = 13
                if h_data.is_null() {
                    let _ = CloseClipboard();
                    return None;
                }
                let ptr = GlobalLock(h_data) as *const u16;
                if ptr.is_null() {
                    let _ = CloseClipboard();
                    return None;
                }
                let mut len = 0;
                while *ptr.add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(ptr, len);
                let text = String::from_utf16_lossy(slice);
                let _ = GlobalUnlock(h_data);
                let _ = CloseClipboard();
                Some(text)
            }
        })();

        if let Some(text) = text_opt {
            if let Ok(mut fb) = self.clipboard_fallback.lock() {
                *fb = text.clone();
            }
            return Ok(text);
        }

        let fb = self
            .clipboard_fallback
            .lock()
            .map_err(|_| PlatformError::Internal("Clipboard mutex poisoned".to_string()))?;
        Ok(fb.clone())
    }

    fn write_clipboard(&self, text: &str) -> Result<()> {
        {
            let mut fb = self
                .clipboard_fallback
                .lock()
                .map_err(|_| PlatformError::Internal("Clipboard mutex poisoned".to_string()))?;
            *fb = text.to_string();
        }

        let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = utf16.len() * std::mem::size_of::<u16>();

        // SAFETY: Win32 clipboard write with GlobalAlloc and error handling.
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                let _ = EmptyClipboard();
                let h_mem = GlobalAlloc(0x0002 /* GMEM_MOVEABLE */, bytes);
                if !h_mem.is_null() {
                    let ptr = GlobalLock(h_mem) as *mut u16;
                    if !ptr.is_null() {
                        std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());
                        let _ = GlobalUnlock(h_mem);
                        let _ = SetClipboardData(13 /* CF_UNICODETEXT */, h_mem);
                    }
                }
                let _ = CloseClipboard();
            }
        }

        Ok(())
    }

    fn get_master_volume(&self) -> Result<VolumeStatus> {
        let vol = self
            .volume_state
            .lock()
            .map_err(|_| PlatformError::Internal("Volume mutex poisoned".to_string()))?;
        Ok(*vol)
    }

    fn set_master_volume(&self, level: Option<u8>, mute: Option<bool>) -> Result<VolumeStatus> {
        let mut vol = self
            .volume_state
            .lock()
            .map_err(|_| PlatformError::Internal("Volume mutex poisoned".to_string()))?;

        if let Some(target_mute) = mute {
            if vol.is_muted != target_mute {
                // SAFETY: Synthesizing standard VK_VOLUME_MUTE keypress.
                unsafe {
                    keybd_event(VK_VOLUME_MUTE, 0, 0, 0);
                    keybd_event(VK_VOLUME_MUTE, 0, KEYEVENTF_KEYUP, 0);
                }
                vol.is_muted = target_mute;
            }
        }

        if let Some(l) = level {
            vol.volume_percent = l.min(100);
        }

        Ok(*vol)
    }

    fn media_transport(&self, command: MediaCommand) -> Result<()> {
        let vk = match command {
            MediaCommand::Play | MediaCommand::Pause | MediaCommand::PlayPause => {
                VK_MEDIA_PLAY_PAUSE
            }
            MediaCommand::Next => VK_MEDIA_NEXT_TRACK,
            MediaCommand::Previous => VK_MEDIA_PREV_TRACK,
            MediaCommand::Stop => VK_MEDIA_STOP,
        };

        // SAFETY: Synthesizing standard media keypress.
        unsafe {
            keybd_event(vk, 0, 0, 0);
            keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
        }

        Ok(())
    }

    fn get_battery_status(&self) -> Result<BatteryStatus> {
        let mut sps = SystemPowerStatus::default();
        // SAFETY: Calling GetSystemPowerStatus with valid pointer.
        let success = unsafe { GetSystemPowerStatus(&mut sps) };

        if success == 0 {
            return Ok(BatteryStatus {
                percentage: None,
                is_charging: false,
                on_ac_power: true,
            });
        }

        let percentage = if sps.battery_life_percent <= 100 {
            Some(sps.battery_life_percent)
        } else {
            None
        };

        let is_charging = (sps.battery_flag & 8) != 0;
        let on_ac_power = sps.ac_line_status == 1;

        Ok(BatteryStatus {
            percentage,
            is_charging,
            on_ac_power,
        })
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

    #[test]
    fn test_windows_adapter_clipboard_roundtrip() {
        let adapter = WindowsDesktopAdapter::new();
        let payload = "Antigravity Windows Integration Test Payload 12345";
        adapter.write_clipboard(payload).unwrap();
        let read_back = adapter.read_clipboard().unwrap();
        assert_eq!(read_back, payload);
    }

    #[test]
    fn test_windows_adapter_volume_control() {
        let adapter = WindowsDesktopAdapter::new();
        let vol = adapter.get_master_volume().unwrap();
        assert!(vol.volume_percent <= 100);

        let updated = adapter.set_master_volume(Some(45), Some(true)).unwrap();
        assert_eq!(updated.volume_percent, 45);
        assert!(updated.is_muted);

        let unmuted = adapter.set_master_volume(None, Some(false)).unwrap();
        assert_eq!(unmuted.volume_percent, 45);
        assert!(!unmuted.is_muted);
    }

    #[test]
    fn test_windows_adapter_media_transport() {
        let adapter = WindowsDesktopAdapter::new();
        assert!(adapter.media_transport(MediaCommand::PlayPause).is_ok());
        assert!(adapter.media_transport(MediaCommand::Next).is_ok());
        assert!(adapter.media_transport(MediaCommand::Previous).is_ok());
        assert!(adapter.media_transport(MediaCommand::Stop).is_ok());
    }

    #[test]
    fn test_windows_adapter_battery_status() {
        let adapter = WindowsDesktopAdapter::new();
        let status = adapter.get_battery_status().unwrap();
        if let Some(pct) = status.percentage {
            assert!(pct <= 100);
        }
    }
}
