#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use aissistant_skills::file::PathJail;
use proptest::prelude::*;
use std::fs;
use tempfile::TempDir;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn test_arbitrary_string_path_fuzzing_never_escapes(raw_path in ".*") {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let canonical_root = fs::canonicalize(temp_dir.path()).expect("canonical root");
        let jail = PathJail::new(vec![canonical_root.clone()]);

        // validate_existing must never panic and must never escape jail
        if let Ok(canon) = jail.validate_existing(&raw_path) {
            prop_assert!(
                canon.starts_with(&canonical_root),
                "Existing path {:?} escaped root {:?}",
                canon,
                canonical_root
            );
        }

        // validate_for_write must never panic and must never escape jail
        if let Ok(canon) = jail.validate_for_write(&raw_path) {
            prop_assert!(
                canon.starts_with(&canonical_root),
                "Write path {:?} escaped root {:?}",
                canon,
                canonical_root
            );
        }

        // sanitize_archive_entry_path must never panic and never escape jail
        if let Ok(canon) = jail.sanitize_archive_entry_path(&raw_path, &canonical_root) {
            if let Ok(resolved_canon) = fs::canonicalize(&canon) {
                prop_assert!(
                    resolved_canon.starts_with(&canonical_root),
                    "Archive entry {:?} resolved to {:?} escaping root {:?}",
                    raw_path,
                    resolved_canon,
                    canonical_root
                );
            }
        }
    }

    #[test]
    fn test_crafted_traversal_combinations_never_escape(
        segments in prop::collection::vec(
            prop::sample::select(vec![
                "..",
                ".",
                "sub",
                "nested",
                "etc",
                "passwd",
                "windows",
                "system32",
                "cmd.exe",
                "CON",
                "PRN",
                "AUX",
                "NUL",
                "COM1",
                "LPT1",
                "/",
                "\\",
                "//",
                "\\\\",
                "\\\\?\\C:",
                "C:",
                "D:",
                "\0",
                "evil.txt",
                "normal.dat",
            ]),
            1..15,
        )
    ) {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let canonical_root = fs::canonicalize(temp_dir.path()).expect("canonical root");
        let jail = PathJail::new(vec![canonical_root.clone()]);

        // Join segments into a combined raw path
        let joined = segments.join("/");
        let backslash_joined = segments.join("\\");

        for test_path in [&joined, &backslash_joined] {
            if let Ok(canon) = jail.validate_existing(test_path) {
                prop_assert!(
                    canon.starts_with(&canonical_root),
                    "Crafted path {:?} escaped root {:?}",
                    canon,
                    canonical_root
                );
            }

            if let Ok(canon) = jail.validate_for_write(test_path) {
                prop_assert!(
                    canon.starts_with(&canonical_root),
                    "Crafted write path {:?} escaped root {:?}",
                    canon,
                    canonical_root
                );
            }

            if let Ok(canon) = jail.sanitize_archive_entry_path(test_path, &canonical_root) {
                if let Ok(resolved_canon) = fs::canonicalize(&canon) {
                    prop_assert!(
                        resolved_canon.starts_with(&canonical_root),
                        "Crafted zip entry {:?} resolved to {:?} escaping root {:?}",
                        test_path,
                        resolved_canon,
                        canonical_root
                    );
                }
            }
        }
    }

    #[test]
    fn test_zip_slip_rejections(
        prefix in prop::sample::select(vec!["../", "..\\", "/root/", "\\system32\\", "nested/../../"]),
        filename in "[a-zA-Z0-9_-]{1,10}\\.txt"
    ) {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let canonical_root = fs::canonicalize(temp_dir.path()).expect("canonical root");
        let jail = PathJail::new(vec![canonical_root.clone()]);

        let slip_path = format!("{prefix}{filename}");
        let result = jail.sanitize_archive_entry_path(&slip_path, &canonical_root);
        prop_assert!(result.is_err(), "Zip-slip path {:?} unexpectedly succeeded", slip_path);
    }
}
