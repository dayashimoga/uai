//! Desktop native automation skills for application management, process inspection, and window focus.

pub mod app_skills;
pub mod process_skills;
pub mod window_skills;

pub use app_skills::{AppLaunchSkill, AppListSkill};
pub use process_skills::{ProcessKillSkill, ProcessListSkill};
pub use window_skills::{WindowFocusSkill, WindowListSkill};
