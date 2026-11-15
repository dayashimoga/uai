//! Native desktop OS automation, process management, and window control.
//!
//! Exposes platform adapters for application launching, active process inspection,
//! process lifecycle control, and desktop window focus across Windows, Linux, and macOS.

#![deny(missing_docs)]

pub mod adapter;
pub mod error;
pub mod types;

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

use std::sync::Arc;

pub use adapter::{DesktopPlatformAdapter, VirtualDesktopAdapter};
pub use error::{PlatformError, Result};
pub use types::{AppInfo, ProcessInfo, WindowBounds, WindowInfo};

#[cfg(windows)]
pub use windows::WindowsDesktopAdapter;

#[cfg(target_os = "linux")]
pub use linux::LinuxDesktopAdapter;

/// Creates the native desktop platform adapter appropriate for the current host environment.
#[must_use]
pub fn create_default_desktop_adapter() -> Arc<dyn DesktopPlatformAdapter> {
    #[cfg(windows)]
    {
        Arc::new(WindowsDesktopAdapter::new())
    }
    #[cfg(target_os = "linux")]
    {
        Arc::new(LinuxDesktopAdapter::new())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Arc::new(VirtualDesktopAdapter::new())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_desktop_adapter() {
        let adapter = create_default_desktop_adapter();
        let procs = adapter.list_processes();
        assert!(procs.is_ok());
        let procs_list = procs.unwrap();
        assert!(!procs_list.is_empty());
    }
}
