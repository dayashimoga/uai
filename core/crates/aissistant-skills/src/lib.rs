//! Extensible skill framework, sandboxed executor, built-in safe utility skills,
//! and jail-constrained file management skills.
//!
//! Conforms to strict risk tiers, path canonicalization, symlink escape prevention,
//! and BLAKE3 hash-chained audit logging requirements.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod error;
pub mod file;
pub mod registry;
pub mod sandbox;
pub mod utility;

pub use error::{Result, SkillError};
pub use file::{
    FileArchiveSkill, FileChecksumSkill, FileDeleteSkill, FileMoveSkill, FileReadSkill,
    FileSearchSkill, FileWriteSkill, PathJail,
};
pub use registry::SkillRegistry;
pub use sandbox::{SandboxedSkill, DEFAULT_SKILL_TIMEOUT_MS};
pub use utility::{CalcSkill, ConvertSkill, TelemetrySkill, TimeSkill, TimerSkill};

#[cfg(test)]
mod tests {
    use super::*;
    use aissistant_orchestrator::ToolDispatcher;
    use aissistant_security::RiskTier;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_skill_registry_all_builtins_registration_and_execution() {
        let registry = SkillRegistry::new();
        let dispatcher = ToolDispatcher::new();

        registry.register_all_builtins(&dispatcher).unwrap();

        // 1. system.time
        let time_tool = dispatcher.get("system.time").unwrap().unwrap();
        assert_eq!(time_tool.name(), "system.time");
        assert_eq!(time_tool.risk_tier(), RiskTier::ReadOnly);
        let res = dispatcher
            .execute("system.time", json!({"format": "full"}))
            .unwrap();
        assert!(res.to_string().contains("iso8601"));

        // 2. system.timer
        let timer_tool = dispatcher.get("system.timer").unwrap().unwrap();
        assert_eq!(timer_tool.name(), "system.timer");
        assert_eq!(timer_tool.risk_tier(), RiskTier::StateModifying);
        let start_res = dispatcher
            .execute(
                "system.timer",
                json!({
                    "action": "set",
                    "duration_seconds": 60,
                    "name": "pasta"
                }),
            )
            .unwrap();
        assert!(start_res.to_string().contains("pasta"));

        // 3. system.calc
        let calc_tool = dispatcher.get("system.calc").unwrap().unwrap();
        assert_eq!(calc_tool.name(), "system.calc");
        assert_eq!(calc_tool.risk_tier(), RiskTier::ReadOnly);
        let calc_res = dispatcher
            .execute("system.calc", json!({"expression": "3 * (4 + 6) / 2"}))
            .unwrap();
        assert!(calc_res.to_string().contains("15"));

        // 4. system.convert
        let convert_tool = dispatcher.get("system.convert").unwrap().unwrap();
        assert_eq!(convert_tool.name(), "system.convert");
        assert_eq!(convert_tool.risk_tier(), RiskTier::ReadOnly);
        let conv_res = dispatcher
            .execute(
                "system.convert",
                json!({
                    "value": 100.0,
                    "from_unit": "celsius",
                    "to_unit": "fahrenheit"
                }),
            )
            .unwrap();
        assert!(conv_res.to_string().contains("212"));

        // 5. system.telemetry
        let telemetry_tool = dispatcher.get("system.telemetry").unwrap().unwrap();
        assert_eq!(telemetry_tool.name(), "system.telemetry");
        assert_eq!(telemetry_tool.risk_tier(), RiskTier::ReadOnly);
        let telem_res = dispatcher
            .execute("system.telemetry", json!({"metric": "all"}))
            .unwrap();
        assert!(telem_res.to_string().contains("total_bytes"));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_skill_registry_file_skills_execution_and_risk_tiers() {
        let temp = tempdir().unwrap();
        let jail_root = temp.path().to_path_buf();
        let jail = Arc::new(PathJail::new(vec![jail_root.clone()]));

        let registry = SkillRegistry::new();
        let dispatcher = ToolDispatcher::new();
        registry
            .register_all_file_skills(&dispatcher, Some(jail))
            .unwrap();

        // Check risk tiers
        assert_eq!(
            dispatcher.get("file.search").unwrap().unwrap().risk_tier(),
            RiskTier::ReadOnly
        );
        assert_eq!(
            dispatcher.get("file.read").unwrap().unwrap().risk_tier(),
            RiskTier::ReadOnly
        );
        assert_eq!(
            dispatcher
                .get("file.checksum")
                .unwrap()
                .unwrap()
                .risk_tier(),
            RiskTier::ReadOnly
        );
        assert_eq!(
            dispatcher.get("file.write").unwrap().unwrap().risk_tier(),
            RiskTier::FileAndApp
        );
        assert_eq!(
            dispatcher.get("file.move").unwrap().unwrap().risk_tier(),
            RiskTier::FileAndApp
        );
        assert_eq!(
            dispatcher.get("file.archive").unwrap().unwrap().risk_tier(),
            RiskTier::FileAndApp
        );
        assert_eq!(
            dispatcher.get("file.delete").unwrap().unwrap().risk_tier(),
            RiskTier::Destructive
        );

        // 1. file.write
        let target_file = jail_root.join("hello.txt");
        let write_res = dispatcher
            .execute(
                "file.write",
                json!({
                    "path": target_file.to_str().unwrap(),
                    "content": "Hello Universal Assistant!",
                    "mode": "overwrite"
                }),
            )
            .unwrap();
        assert_eq!(write_res["bytes_written"], 26);

        // 2. file.read
        let read_res = dispatcher
            .execute(
                "file.read",
                json!({
                    "path": target_file.to_str().unwrap(),
                    "offset": 0,
                    "max_bytes": 100
                }),
            )
            .unwrap();
        assert_eq!(read_res["content"], "Hello Universal Assistant!");

        // 3. file.checksum
        let sum_res = dispatcher
            .execute(
                "file.checksum",
                json!({
                    "path": target_file.to_str().unwrap(),
                    "algorithm": "blake3"
                }),
            )
            .unwrap();
        assert!(!sum_res["digest_hex"].as_str().unwrap().is_empty());

        // 4. file.search
        let search_res = dispatcher
            .execute(
                "file.search",
                json!({
                    "path": jail_root.to_str().unwrap(),
                    "query": "hello"
                }),
            )
            .unwrap();
        assert_eq!(search_res["count"], 1);

        // 5. file.move
        let moved_file = jail_root.join("renamed.txt");
        let move_res = dispatcher
            .execute(
                "file.move",
                json!({
                    "source_path": target_file.to_str().unwrap(),
                    "destination_path": moved_file.to_str().unwrap()
                }),
            )
            .unwrap();
        assert_eq!(move_res["size_bytes"], 26);
        assert!(!target_file.exists());
        assert!(moved_file.exists());

        // 6. file.archive (create & list)
        let zip_file = jail_root.join("bundle.zip");
        let arch_create = dispatcher
            .execute(
                "file.archive",
                json!({
                    "action": "create",
                    "archive_path": zip_file.to_str().unwrap(),
                    "target_path": moved_file.to_str().unwrap()
                }),
            )
            .unwrap();
        assert_eq!(arch_create["entries_count"], 1);

        let arch_list = dispatcher
            .execute(
                "file.archive",
                json!({
                    "action": "list",
                    "archive_path": zip_file.to_str().unwrap()
                }),
            )
            .unwrap();
        assert_eq!(arch_list["entries_count"], 1);

        // 7. file.delete
        let del_res = dispatcher
            .execute(
                "file.delete",
                json!({
                    "path": moved_file.to_str().unwrap()
                }),
            )
            .unwrap();
        assert!(del_res["deleted"].as_bool().unwrap());
        assert!(!moved_file.exists());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_sandboxed_skill_dispatch_error_propagation() {
        let dispatcher = ToolDispatcher::new();
        dispatcher
            .register(Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                CalcSkill::new(),
            ))))
            .unwrap();

        // Division by zero error
        let err = dispatcher.execute("system.calc", json!({"expression": "42 / 0"}));
        assert!(err.is_err());
    }
}
