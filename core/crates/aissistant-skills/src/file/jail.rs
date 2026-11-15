//! Path jail, canonicalization, and adversarial path traversal prevention.

use std::path::{Component, Path, PathBuf};

use crate::error::{Result, SkillError};

/// Security engine that validates, canonicalizes, and constrains file paths.
#[derive(Debug, Clone)]
pub struct PathJail {
    allowed_roots: Vec<PathBuf>,
}

impl Default for PathJail {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl PathJail {
    /// Creates a new `PathJail` with optional allowed root directory restrictions.
    ///
    /// If `allowed_roots` is empty, standard system-path protections and traversal
    /// restrictions apply without confining to a single directory tree.
    #[must_use]
    pub fn new(allowed_roots: Vec<PathBuf>) -> Self {
        let canonical_roots: Vec<PathBuf> = allowed_roots
            .into_iter()
            .filter_map(|r| std::fs::canonicalize(&r).ok().or(Some(r)))
            .collect();

        Self {
            allowed_roots: canonical_roots,
        }
    }

    /// Creates an unconfined jail allowing access to standard user paths while
    /// still strictly rejecting path traversal, null bytes, and critical OS directories.
    #[must_use]
    pub fn unrestricted() -> Self {
        Self::default()
    }

    /// Validates an existing file or directory path, verifying jail confinement.
    pub fn validate_existing(&self, raw_path: &str) -> Result<PathBuf> {
        self.pre_check_raw_path(raw_path)?;

        let path = Path::new(raw_path);
        if !path.exists() {
            return Err(SkillError::InvalidArguments {
                skill_name: "file".to_string(),
                reason: format!("File or directory does not exist: '{raw_path}'"),
            });
        }

        let canonical = std::fs::canonicalize(path).map_err(SkillError::Io)?;
        self.enforce_jail_confinement(&canonical, raw_path)?;
        self.enforce_system_denylist(&canonical, raw_path)?;

        Ok(canonical)
    }

    /// Validates a target path for file creation or write (parent directory must exist and be safe).
    pub fn validate_for_write(&self, raw_path: &str) -> Result<PathBuf> {
        self.pre_check_raw_path(raw_path)?;

        let path = Path::new(raw_path);

        if path.exists() {
            return self.validate_existing(raw_path);
        }

        // For non-existent files, validate and canonicalize parent directory
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
            SkillError::InvalidArguments {
                skill_name: "file".to_string(),
                reason: format!("Invalid file name in path: '{raw_path}'"),
            }
        })?;

        self.validate_filename_components(file_name)?;

        if !parent.exists() {
            return Err(SkillError::InvalidArguments {
                skill_name: "file".to_string(),
                reason: format!(
                    "Parent directory does not exist for target: '{}'",
                    parent.display()
                ),
            });
        }

        let canonical_parent = std::fs::canonicalize(parent).map_err(SkillError::Io)?;
        let canonical_target = canonical_parent.join(file_name);

        self.enforce_jail_confinement(&canonical_target, raw_path)?;
        self.enforce_system_denylist(&canonical_target, raw_path)?;

        Ok(canonical_target)
    }

    /// Validates a relative archive entry name (preventing zip-slip attacks).
    pub fn sanitize_archive_entry_path(
        &self,
        entry_name: &str,
        target_dir: &Path,
    ) -> Result<PathBuf> {
        if entry_name.contains('\0') {
            return Err(SkillError::SecurityViolation {
                message: "Archive entry contains null byte".to_string(),
            });
        }

        let entry_path = Path::new(entry_name);

        for component in entry_path.components() {
            match component {
                Component::ParentDir => {
                    return Err(SkillError::PathEscape {
                        path: entry_name.to_string(),
                        reason: "Zip-slip parent directory traversal ('..') detected".to_string(),
                    });
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(SkillError::PathEscape {
                        path: entry_name.to_string(),
                        reason: "Zip-slip absolute root path detected".to_string(),
                    });
                }
                Component::Normal(part) => {
                    if let Some(s) = part.to_str() {
                        self.validate_filename_components(s)?;
                    }
                }
                Component::CurDir => {}
            }
        }

        let resolved = target_dir.join(entry_path);
        let canonical_target = std::fs::canonicalize(target_dir).map_err(SkillError::Io)?;

        // Ensure resolved destination starts with canonical target
        if let Ok(canon) = std::fs::canonicalize(&resolved) {
            if !canon.starts_with(&canonical_target) {
                return Err(SkillError::PathEscape {
                    path: entry_name.to_string(),
                    reason: "Archive entry escapes destination directory".to_string(),
                });
            }
        }

        Ok(resolved)
    }

    fn pre_check_raw_path(&self, raw_path: &str) -> Result<()> {
        if raw_path.is_empty() {
            return Err(SkillError::InvalidArguments {
                skill_name: "file".to_string(),
                reason: "File path cannot be empty".to_string(),
            });
        }

        if raw_path.contains('\0') {
            return Err(SkillError::SecurityViolation {
                message: format!("File path contains null byte: '{raw_path}'"),
            });
        }

        let path = Path::new(raw_path);
        for comp in path.components() {
            if let Component::Normal(os_str) = comp {
                if let Some(s) = os_str.to_str() {
                    self.validate_filename_components(s)?;
                }
            }
        }

        Ok(())
    }

    fn validate_filename_components(&self, name: &str) -> Result<()> {
        let stem = match name.split('.').next() {
            Some(s) => s.to_ascii_uppercase(),
            None => name.to_ascii_uppercase(),
        };

        let is_reserved_dos = matches!(
            stem.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        );

        if is_reserved_dos {
            return Err(SkillError::SecurityViolation {
                message: format!("Path contains reserved Windows device name: '{name}'"),
            });
        }

        Ok(())
    }

    fn enforce_jail_confinement(&self, canonical: &Path, raw_path: &str) -> Result<()> {
        if self.allowed_roots.is_empty() {
            return Ok(());
        }

        let inside_jail = self
            .allowed_roots
            .iter()
            .any(|root| canonical.starts_with(root));

        if !inside_jail {
            return Err(SkillError::PathEscape {
                path: raw_path.to_string(),
                reason: format!(
                    "Resolved path '{}' is outside permitted roots {:?}",
                    canonical.display(),
                    self.allowed_roots
                ),
            });
        }

        Ok(())
    }

    fn enforce_system_denylist(&self, canonical: &Path, raw_path: &str) -> Result<()> {
        let canon_str = canonical.to_string_lossy().to_ascii_lowercase();

        // Windows critical operating system paths
        if cfg!(windows) {
            let win_forbidden = [
                "\\windows\\system32",
                "\\windows\\syswow64",
                "\\windows\\winsxs",
                "\\program files\\windowsapps",
            ];
            for forbidden in win_forbidden {
                if canon_str.contains(forbidden) {
                    return Err(SkillError::SecurityViolation {
                        message: format!(
                            "Access to critical OS system path denied: '{raw_path}' -> '{}'",
                            canonical.display()
                        ),
                    });
                }
            }
        }

        // Unix critical system directories
        if cfg!(unix) {
            let unix_forbidden = [
                "/etc/shadow",
                "/etc/sudoers",
                "/proc",
                "/sys",
                "/dev",
                "/boot",
            ];
            for forbidden in unix_forbidden {
                if canon_str.starts_with(forbidden) {
                    return Err(SkillError::SecurityViolation {
                        message: format!(
                            "Access to critical Unix system path denied: '{raw_path}' -> '{}'",
                            canonical.display()
                        ),
                    });
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_path_jail_traversal_rejection() {
        let jail = PathJail::unrestricted();

        // Null byte injection rejected
        let err_null = jail.validate_existing("test\0bad");
        assert!(err_null.is_err());

        // Windows reserved names rejected
        let err_dos = jail.validate_for_write("C:/safe/dir/nul.txt");
        assert!(err_dos.is_err());
        let err_com = jail.validate_for_write("C:/safe/dir/COM1");
        assert!(err_com.is_err());
    }

    #[test]
    fn test_path_jail_allowed_roots_confinement() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();

        let allowed_path = dir1.path().to_path_buf();
        let file_in_dir1 = allowed_path.join("file1.txt");
        std::fs::write(&file_in_dir1, b"hello").unwrap();

        let outside_file = dir2.path().join("outside.txt");
        std::fs::write(&outside_file, b"secret").unwrap();

        let jail = PathJail::new(vec![allowed_path]);

        // Inside jail allowed
        let val1 = jail.validate_existing(file_in_dir1.to_str().unwrap());
        assert!(val1.is_ok());

        // Outside jail rejected with PathEscape
        let val2 = jail.validate_existing(outside_file.to_str().unwrap());
        assert!(matches!(val2, Err(SkillError::PathEscape { .. })));
    }

    #[test]
    fn test_zip_slip_rejection() {
        let temp = tempdir().unwrap();
        let jail = PathJail::unrestricted();

        // Attempt zip slip with parent traversal
        let res = jail.sanitize_archive_entry_path("../../../etc/passwd", temp.path());
        assert!(matches!(res, Err(SkillError::PathEscape { .. })));

        // Absolute path inside archive entry
        let res_abs = jail.sanitize_archive_entry_path("/etc/shadow", temp.path());
        assert!(matches!(res_abs, Err(SkillError::PathEscape { .. })));

        // Safe relative path
        let safe = jail.sanitize_archive_entry_path("sub/file.txt", temp.path());
        assert!(safe.is_ok());
    }
}
