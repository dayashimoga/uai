//! Data types representing applications, operating system processes, and desktop windows.

use serde::{Deserialize, Serialize};

/// Discovered desktop application metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    /// Unique application identifier (e.g. filename or reverse-DNS slug).
    pub id: String,
    /// Display name of the application.
    pub name: String,
    /// Absolute path or launch command for the executable.
    pub executable_path: String,
    /// Optional human-readable description.
    pub description: Option<String>,
}

/// Active operating system process metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// Operating system process ID (PID).
    pub pid: u32,
    /// Process name (e.g. `chrome.exe` or `code`).
    pub name: String,
    /// Current CPU usage percentage (0.0 to 100.0 * num_cores).
    pub cpu_usage_percent: f32,
    /// Resident memory consumption in bytes.
    pub memory_bytes: u64,
    /// Absolute path of the executable image if accessible.
    pub executable_path: Option<String>,
}

/// Rectangle bounding coordinates for a desktop window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowBounds {
    /// Horizontal screen offset in pixels.
    pub x: i32,
    /// Vertical screen offset in pixels.
    pub y: i32,
    /// Window width in pixels.
    pub width: u32,
    /// Window height in pixels.
    pub height: u32,
}

/// Visible top-level desktop application window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Platform-specific native window handle or numeric identifier.
    pub id: u64,
    /// Window title bar text.
    pub title: String,
    /// Owning process PID.
    pub pid: u32,
    /// Name of the process owning the window.
    pub process_name: String,
    /// Whether the window is currently minimized.
    pub is_minimized: bool,
    /// Window position and dimensions.
    pub bounds: WindowBounds,
}
