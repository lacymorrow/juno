//! # Path Security Helpers
//!
//! Shared canonicalization and workspace-boundary enforcement for agent file
//! operations. This is the single implementation of the "resolve symlinks and
//! `..` segments, then require the result to live inside an allowed root"
//! logic that previously existed only in `basic_tools` (security audit
//! 2026-02-08, items #13/#14/#18).
//!
//! ## Used by:
//! - `basic_tools::validate_file_path` (read path canonicalization)
//! - `anthropic_computer_use::validate_file_path` (str_replace editor tool)
//! - `enhanced_coding_tools::create_file_with_content` (smart_create_file)

use std::path::{Path, PathBuf};

/// Canonicalize a path, tolerating files that do not exist yet.
///
/// For an existing path this is `std::fs::canonicalize`. For a path whose
/// final component does not exist yet (e.g. a file about to be created), the
/// parent directory is canonicalized and the file name re-appended, so `..`
/// segments and symlinks in the directory portion are still resolved.
pub fn canonicalize_lenient(full_path: &Path) -> PathBuf {
    match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // If the file doesn't exist yet, canonicalize the parent directory
            if let Some(parent) = full_path.parent() {
                match parent.canonicalize() {
                    Ok(canonical_parent) => {
                        if let Some(file_name) = full_path.file_name() {
                            canonical_parent.join(file_name)
                        } else {
                            full_path.to_path_buf()
                        }
                    }
                    Err(_) => full_path.to_path_buf(),
                }
            } else {
                full_path.to_path_buf()
            }
        }
    }
}

/// Resolve `path_str` to an absolute, canonical path and require it to live
/// inside one of the allowed `roots`.
///
/// Relative paths are resolved against the current working directory before
/// canonicalization, matching the behavior of `basic_tools`. An empty `roots`
/// slice fails closed: no boundary can be established, so access is denied.
pub fn resolve_within_roots(path_str: &str, roots: &[PathBuf]) -> Result<PathBuf, String> {
    if path_str.is_empty() {
        return Err("Empty path not allowed".to_string());
    }

    let path = PathBuf::from(path_str);
    let full_path = if path.is_absolute() {
        path
    } else {
        let current_dir = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;
        current_dir.join(&path)
    };

    let canonical_path = canonicalize_lenient(&full_path);

    if roots.is_empty() {
        return Err(
            "Access denied: No workspace boundary is available for file access".to_string(),
        );
    }

    let allowed = roots.iter().any(|root| {
        let canonical_root = canonicalize_lenient(root);
        canonical_path.starts_with(&canonical_root)
    });

    if allowed {
        Ok(canonical_path)
    } else {
        Err(format!(
            "Access denied: Path is outside the workspace boundary. Workspace: {}",
            roots
                .iter()
                .map(|r| r.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

/// Default allowed roots for agent file operations:
/// - the process working directory (the same workspace root `basic_tools`
///   uses for its boundary), and
/// - the agent's preferred output directory (`~/Juno`), where
///   `smart_create_file` places files given a bare filename.
pub fn default_workspace_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    if let Ok(juno_dir) = crate::utils::get_agent_preferred_directory() {
        roots.push(juno_dir);
    }
    roots
}

/// Resolve `path_str` against [`default_workspace_roots`]. Fails closed when
/// no workspace root can be determined.
pub fn resolve_within_default_roots(path_str: &str) -> Result<PathBuf, String> {
    resolve_within_roots(path_str, &default_workspace_roots())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {}", e))
    }

    #[test]
    fn allows_file_inside_root() {
        let dir = temp_root();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "hello").unwrap_or_else(|e| panic!("write failed: {}", e));

        let result = resolve_within_roots(&file.to_string_lossy(), &[dir.path().to_path_buf()]);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn allows_nonexistent_file_in_existing_dir_inside_root() {
        let dir = temp_root();
        let file = dir.path().join("new_file.txt");

        let result = resolve_within_roots(&file.to_string_lossy(), &[dir.path().to_path_buf()]);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn blocks_absolute_path_outside_root() {
        let dir = temp_root();
        let result = resolve_within_roots("/etc/hosts", &[dir.path().to_path_buf()]);
        assert!(result.is_err(), "absolute path outside root must be denied");
    }

    #[test]
    fn blocks_traversal_escaping_root() {
        let dir = temp_root();
        let sneaky = dir
            .path()
            .join("sub")
            .join("..")
            .join("..")
            .join("etc")
            .join("passwd");

        let result = resolve_within_roots(&sneaky.to_string_lossy(), &[dir.path().to_path_buf()]);
        assert!(
            result.is_err(),
            "traversal escaping the root must be denied"
        );
    }

    #[test]
    fn empty_roots_fail_closed() {
        let result = resolve_within_roots("/tmp/anything.txt", &[]);
        assert!(result.is_err(), "empty roots must deny access");
    }

    #[test]
    fn empty_path_rejected() {
        let dir = temp_root();
        let result = resolve_within_roots("", &[dir.path().to_path_buf()]);
        assert!(result.is_err());
    }
}
