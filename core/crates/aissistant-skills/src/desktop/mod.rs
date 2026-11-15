//! Desktop native automation skills for application management, process inspection, window focus, clipboard, and audio/media.

pub mod app_skills;
pub mod clipboard_skills;
pub mod media_skills;
pub mod process_skills;
pub mod system_skills;
pub mod window_skills;

pub use app_skills::{AppLaunchSkill, AppListSkill};
pub use clipboard_skills::{ClipboardReadSkill, ClipboardWriteSkill};
pub use media_skills::{MediaTransportSkill, VolumeGetSkill, VolumeSetSkill};
pub use process_skills::{ProcessKillSkill, ProcessListSkill};
pub use system_skills::SystemBatterySkill;
pub use window_skills::{WindowFocusSkill, WindowListSkill};
