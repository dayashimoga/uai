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

/// Current master audio output volume and mute status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeStatus {
    /// Volume percentage level (0 to 100).
    pub volume_percent: u8,
    /// Whether audio output is currently muted.
    pub is_muted: bool,
}

/// Media transport control commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaCommand {
    /// Resume playback.
    Play,
    /// Pause playback.
    Pause,
    /// Toggle between play and pause.
    PlayPause,
    /// Skip to next media item or track.
    Next,
    /// Go to previous media item or track.
    Previous,
    /// Stop media playback.
    Stop,
}

/// Host power and battery charging status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryStatus {
    /// Battery charge percentage (0 to 100), or None if host has no battery.
    pub percentage: Option<u8>,
    /// Whether the battery is currently charging.
    pub is_charging: bool,
    /// Whether the device is connected to external AC/line power.
    pub on_ac_power: bool,
}
