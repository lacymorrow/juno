//! # Path Gate
//!
//! The one place that answers "may this path be touched, for this purpose".
//!
//! Every file-touching `#[tauri::command]` routes through [`authorize`], which
//! takes the operation as a parameter instead of each command carrying its own
//! check. Listing a directory and reading a file have different blast radii, so
//! they get different policies here, deliberately and in one place, rather than
//! different policies arrived at by accident in seven command bodies.
//!
//! ## What this replaces
//!
//! `commands::debug_utils::validators::valid_file_path` used to guard these
//! commands with a non-empty test plus a literal `".."` substring test. It had
//! no workspace root, no credential blocklist and no size cap, and it was the
//! only check on the surface `agents::system_agent` drove (that executor was
//! deleted in #671), so an autonomous agent reached the filesystem through the laxest of the three path checks in
//! the tree. That function is gone; this module is its replacement.
//!
//! ## The three layers, and who owns which question
//!
//! - [`crate::agent::tools::path_security`] owns *resolution*: normalize the
//!   path the way the kernel would, follow symlinks, apply the credential
//!   blocklist, and require the result inside a workspace root. It is the only
//!   parser and the only blocklist.
//! - This module owns *per-operation policy* for the command surface: what a
//!   read may additionally not do that a list may, and so on.
//! - The agent tool validators in `agent::tools::basic_tools` and
//!   `agent::tools::anthropic_computer_use` own *tool-level policy* on top of
//!   the same resolution: file-extension allow and block lists that only make
//!   sense for a model-driven tool call.
//!
//! Three layers, one parser. The commands no longer have a policy of their own.

use std::path::PathBuf;

use crate::agent::tools::path_security;

/// Largest existing file [`PathOp::Read`] will admit.
///
/// A read with no ceiling is a memory-exhaustion vector and, on the agent
/// surface, an exfiltration-volume one. The agent tool path has always had a
/// cap; the command path had none.
pub const MAX_READ_BYTES: u64 = 10 * 1024 * 1024;

/// What a caller intends to do with a path.
///
/// The gate is parameterized by this rather than duplicated per command. Add a
/// variant only when the new operation's blast radius genuinely differs; the
/// match in [`authorize_within`] is exhaustive on purpose, so a new variant
/// cannot be added without stating its policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOp {
    /// Read a file's contents.
    Read,
    /// Enumerate a directory's entries.
    ///
    /// A list discloses less than a read but it is still a disclosure: it tells
    /// a caller exactly what to go after, and some file names are themselves
    /// sensitive. It gets the same blocklist and the same boundary.
    List,
    /// Create, overwrite or modify a file, possibly one that does not exist
    /// yet and whose parent directory does not exist yet either.
    Write,
    /// Remove a file.
    Delete,
}

impl PathOp {
    /// The verb used in refusal messages.
    pub fn verb(self) -> &'static str {
        match self {
            PathOp::Read => "read",
            PathOp::List => "list",
            PathOp::Write => "write",
            PathOp::Delete => "delete",
        }
    }
}

/// May this path be touched, for this purpose?
///
/// Resolves `input` against the default workspace roots and applies the policy
/// for `op`. On success the returned [`PathBuf`] is absolute, normalized and
/// symlink-resolved, and callers must perform their I/O on *that* path rather
/// than on `input`, so the path that was checked is the path that is opened.
pub fn authorize(input: &str, op: PathOp) -> Result<PathBuf, String> {
    authorize_within(input, op, &path_security::default_workspace_roots())
}

/// [`authorize`] against an explicit set of allowed roots. Tests use this; so
/// would any caller that has narrower roots than the process default.
///
/// ## Canonicalization, and paths that do not exist yet
///
/// Resolution is [`path_security::resolve_path_lenient`], which walks the
/// components from the filesystem root and canonicalizes after each existing
/// one, so symlinks are followed before the next component is applied and `..`
/// pops a real parent. Once a component is missing the rest accumulate
/// literally, already free of `.` and `..`. That is what makes a write to a
/// not-yet-existing file checkable: the deepest existing ancestor is resolved
/// for real, and the boundary test runs on the full prospective path. The
/// classic hole in this kind of fix is to give up and pass the raw string
/// through when `canonicalize` fails; nothing here does that.
pub fn authorize_within(input: &str, op: PathOp, roots: &[PathBuf]) -> Result<PathBuf, String> {
    // Resolution, credential blocklist and workspace confinement, for every
    // operation without exception.
    let resolved = path_security::resolve_within_roots(input, roots)?;

    // Per-operation policy. Exhaustive: a new PathOp must declare itself here.
    match op {
        PathOp::Read => {
            // Size ceiling, checked only when the file is really there.
            if let Ok(metadata) = std::fs::metadata(&resolved) {
                if metadata.is_file() && metadata.len() > MAX_READ_BYTES {
                    return Err(format!(
                        "Cannot {} '{}': {} bytes exceeds the {} byte limit",
                        op.verb(),
                        resolved.display(),
                        metadata.len(),
                        MAX_READ_BYTES
                    ));
                }
            }
        }
        PathOp::List => {
            // Listing something that is not a directory is a category error,
            // and silently reading it instead would be worse.
            if let Ok(metadata) = std::fs::metadata(&resolved) {
                if !metadata.is_dir() {
                    return Err(format!(
                        "Cannot {} '{}': it is not a directory",
                        op.verb(),
                        resolved.display()
                    ));
                }
            }
        }
        PathOp::Write => {
            // The target may be absent and so may its parent; the caller is
            // allowed to create both. What it may not do is write over a
            // directory.
            if let Ok(metadata) = std::fs::metadata(&resolved) {
                if metadata.is_dir() {
                    return Err(format!(
                        "Cannot {} '{}': it is a directory",
                        op.verb(),
                        resolved.display()
                    ));
                }
            }
        }
        PathOp::Delete => {
            // No policy beyond resolution and confinement. Deleting something
            // absent is the caller's business to report, not a security
            // question, and the undo path treats a missing file as success.
        }
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {}", e))
    }

    /// Every operation, so a new one cannot quietly skip a test below.
    const ALL_OPS: [PathOp; 4] = [PathOp::Read, PathOp::List, PathOp::Write, PathOp::Delete];

    // --- The credential blocklist covers every operation, list included ---

    #[test]
    fn a_credential_file_inside_an_allowed_root_is_refused_for_every_operation() {
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        // A dummy fixture, never a real key.
        let key = dir.path().join("id_rsa");
        std::fs::write(&key, "NOT A REAL KEY").unwrap_or_else(|e| panic!("write failed: {}", e));

        for op in ALL_OPS {
            let result = authorize_within(&key.to_string_lossy(), op, &roots);
            assert!(
                result.is_err(),
                "{} must be refused for a credential file, got {:?}",
                op.verb(),
                result
            );
        }
    }

    #[test]
    fn an_ssh_directory_inside_an_allowed_root_cannot_even_be_enumerated() {
        // The hole this PR closes: the file bodies were refused and the
        // directory listing was not, which hands a caller the file names.
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let ssh = dir.path().join(".ssh");
        std::fs::create_dir(&ssh).unwrap_or_else(|e| panic!("mkdir failed: {}", e));

        let result = authorize_within(&ssh.to_string_lossy(), PathOp::List, &roots);
        assert!(
            result.is_err(),
            "listing a .ssh directory must be refused, got {:?}",
            result
        );
    }

    #[test]
    fn every_blocked_family_is_refused_through_list_as_well_as_read() {
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let names = [
            "id_ed25519",
            ".netrc",
            ".npmrc",
            ".pgpass",
            ".env",
            "server.pem",
            "private.key",
            "cert.p12",
            "wallet.dat",
        ];

        for name in names {
            let path = dir.path().join(name);
            for op in [PathOp::Read, PathOp::List] {
                let result = authorize_within(&path.to_string_lossy(), op, &roots);
                assert!(
                    result.is_err(),
                    "{} of '{}' must be refused",
                    op.verb(),
                    name
                );
            }
        }
    }

    // --- Normalization, not substring matching ---

    #[test]
    fn a_relative_traversal_is_refused_after_normalization() {
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let escape = dir.path().join("sub").join("..").join("..").join("outside");

        let result = authorize_within(&escape.to_string_lossy(), PathOp::Write, &roots);
        assert!(
            result.is_err(),
            "traversal out of the root must be refused, got {:?}",
            result
        );
    }

    #[test]
    fn a_traversal_that_stays_inside_the_root_is_allowed() {
        // Proof the refusal above came from normalization and not from a
        // `".."` substring test: this path also contains `..` and is fine.
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap_or_else(|e| panic!("mkdir failed: {}", e));
        let target = sub.join("..").join("notes.txt");

        let result = authorize_within(&target.to_string_lossy(), PathOp::Write, &roots);
        assert!(
            result.is_ok(),
            "a `..` that stays inside the root must be allowed, got {:?}",
            result
        );
    }

    #[test]
    fn an_absolute_path_to_a_sensitive_location_is_refused_with_no_dots_present() {
        // The old substring check passed this unchanged: it contains no `..`.
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let result = authorize_within("/etc/hosts", PathOp::Read, &roots);
        assert!(result.is_err(), "absolute escape must be refused");
    }

    #[test]
    fn a_symlink_out_of_an_allowed_root_is_refused() {
        let inside = temp_root();
        let outside = temp_root();
        let roots = [inside.path().to_path_buf()];

        let target = outside.path().join("elsewhere.txt");
        std::fs::write(&target, "outside").unwrap_or_else(|e| panic!("write failed: {}", e));
        let link = inside.path().join("innocent.txt");
        std::os::unix::fs::symlink(&target, &link)
            .unwrap_or_else(|e| panic!("symlink failed: {}", e));

        for op in ALL_OPS {
            let result = authorize_within(&link.to_string_lossy(), op, &roots);
            assert!(
                result.is_err(),
                "{} through a symlink leaving the root must be refused, got {:?}",
                op.verb(),
                result
            );
        }
    }

    #[test]
    fn a_symlinked_directory_out_of_an_allowed_root_cannot_be_listed_or_written_through() {
        let inside = temp_root();
        let outside = temp_root();
        let roots = [inside.path().to_path_buf()];

        let link = inside.path().join("stuff");
        std::os::unix::fs::symlink(outside.path(), &link)
            .unwrap_or_else(|e| panic!("symlink failed: {}", e));

        assert!(
            authorize_within(&link.to_string_lossy(), PathOp::List, &roots).is_err(),
            "listing through a symlinked directory must be refused"
        );
        let through = link.join("new.txt");
        assert!(
            authorize_within(&through.to_string_lossy(), PathOp::Write, &roots).is_err(),
            "writing through a symlinked directory must be refused"
        );
    }

    // --- Writes to paths that do not exist yet ---

    #[test]
    fn a_write_to_a_not_yet_existing_file_in_an_allowed_root_is_permitted() {
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let fresh = dir.path().join("brand_new.txt");

        let resolved = authorize_within(&fresh.to_string_lossy(), PathOp::Write, &roots)
            .unwrap_or_else(|e| panic!("a new file in the root must be writable: {}", e));
        assert!(resolved.ends_with("brand_new.txt"));
        assert!(resolved.is_absolute());
    }

    #[test]
    fn a_write_to_a_file_under_a_not_yet_existing_directory_is_permitted() {
        // The classic hole: `canonicalize` fails for both the file and its
        // parent, and a naive fix falls back to the unchecked raw path.
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let deep = dir.path().join("a").join("b").join("c.txt");

        let resolved = authorize_within(&deep.to_string_lossy(), PathOp::Write, &roots)
            .unwrap_or_else(|e| panic!("a new nested file in the root must be writable: {}", e));
        assert!(resolved.starts_with(
            dir.path()
                .canonicalize()
                .unwrap_or_else(|e| panic!("canonicalize root failed: {}", e))
        ));
    }

    #[test]
    fn a_write_to_a_not_yet_existing_file_outside_every_root_is_refused() {
        let dir = temp_root();
        let outside = temp_root();
        let roots = [dir.path().to_path_buf()];
        let fresh = outside.path().join("a").join("b.txt");

        assert!(
            authorize_within(&fresh.to_string_lossy(), PathOp::Write, &roots).is_err(),
            "a new file outside the boundary must be refused"
        );
    }

    #[test]
    fn a_not_yet_existing_credential_file_cannot_be_written() {
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let fresh = dir.path().join("deploy").join(".env");

        assert!(
            authorize_within(&fresh.to_string_lossy(), PathOp::Write, &roots).is_err(),
            "creating a credential-shaped file must be refused too"
        );
    }

    // --- Fail-closed and per-operation policy ---

    #[test]
    fn no_workspace_root_denies_every_operation() {
        let dir = temp_root();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "x").unwrap_or_else(|e| panic!("write failed: {}", e));

        for op in ALL_OPS {
            assert!(
                authorize_within(&file.to_string_lossy(), op, &[]).is_err(),
                "{} must fail closed with no roots",
                op.verb()
            );
        }
    }

    #[test]
    fn empty_and_home_relative_inputs_are_refused() {
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        for input in ["", "   ", "~", "~/notes.txt"] {
            assert!(
                authorize_within(input, PathOp::Read, &roots).is_err(),
                "input {:?} must be refused",
                input
            );
        }
    }

    #[test]
    fn list_refuses_a_file_and_read_allows_it() {
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "hello").unwrap_or_else(|e| panic!("write failed: {}", e));

        assert!(authorize_within(&file.to_string_lossy(), PathOp::Read, &roots).is_ok());
        assert!(authorize_within(&file.to_string_lossy(), PathOp::List, &roots).is_err());
    }

    #[test]
    fn write_refuses_a_directory_and_list_allows_it() {
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap_or_else(|e| panic!("mkdir failed: {}", e));

        assert!(authorize_within(&sub.to_string_lossy(), PathOp::List, &roots).is_ok());
        assert!(authorize_within(&sub.to_string_lossy(), PathOp::Write, &roots).is_err());
    }

    #[test]
    fn read_refuses_a_file_over_the_size_cap() {
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let big = dir.path().join("big.bin");
        let file = std::fs::File::create(&big).unwrap_or_else(|e| panic!("create failed: {}", e));
        file.set_len(MAX_READ_BYTES + 1)
            .unwrap_or_else(|e| panic!("set_len failed: {}", e));
        drop(file);

        assert!(
            authorize_within(&big.to_string_lossy(), PathOp::Read, &roots).is_err(),
            "an oversized file must be refused for Read"
        );
        // The cap is a read policy, not a boundary policy: deleting it is fine.
        assert!(authorize_within(&big.to_string_lossy(), PathOp::Delete, &roots).is_ok());
    }

    #[test]
    fn the_returned_path_is_the_resolved_one_so_callers_do_their_io_on_it() {
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap_or_else(|e| panic!("mkdir failed: {}", e));
        let noisy = sub.join(".").join("..").join("sub").join("f.txt");

        let resolved = authorize_within(&noisy.to_string_lossy(), PathOp::Write, &roots)
            .unwrap_or_else(|e| panic!("resolve failed: {}", e));
        assert!(!resolved.to_string_lossy().contains(".."));
        assert_eq!(resolved.file_name(), Some(std::ffi::OsStr::new("f.txt")));
        assert!(resolved.parent().is_some());
    }
}

/// Both directions of the contract: every file-touching `#[tauri::command]`
/// routes through this gate, and every call to the gate names an operation.
///
/// This is the test that matters most, because the real target is the eighth
/// command someone adds later. It reads the command sources at compile time and
/// checks them structurally, which is the only way to assert "no command does
/// its own thing" without a runtime registry.
#[cfg(test)]
mod command_surface_contract {
    /// The files holding the file-touching commands. A new such file must be
    /// added here, which is itself the point: the list is a declaration.
    const COMMAND_SOURCES: [(&str, &str); 2] = [
        (
            "commands/filesystem.rs",
            include_str!("commands/filesystem.rs"),
        ),
        (
            "commands/text_editor.rs",
            include_str!("commands/text_editor.rs"),
        ),
    ];

    /// Calls that touch the filesystem by path. Matched with the opening paren
    /// so a mention in prose does not count.
    const PATH_TOUCHING_CALLS: [&str; 10] = [
        "fs::read_to_string(",
        "fs::write(",
        "fs::read_dir(",
        "fs::remove_file(",
        "fs::remove_dir_all(",
        "fs::create_dir_all(",
        "fs::metadata(",
        "fs::copy(",
        "OpenOptions::new(",
        "File::create(",
    ];

    const GATE_CALL: &str = "path_gate::authorize";

    /// Source with comment lines removed, so the scan sees code only.
    fn code_only(source: &str) -> String {
        source
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Each `#[tauri::command]` body in a file, paired with a best-effort name.
    fn command_blocks(source: &str) -> Vec<(String, String)> {
        let code = code_only(source);
        code.split("#[tauri::command]")
            .skip(1)
            .map(|block| {
                let name = block
                    .split("fn ")
                    .nth(1)
                    .and_then(|rest| rest.split('(').next())
                    .unwrap_or("<unnamed>")
                    .trim()
                    .to_string();
                (name, block.to_string())
            })
            .collect()
    }

    #[test]
    fn every_file_touching_command_routes_through_the_gate() {
        let mut offenders = Vec::new();

        for (file, source) in COMMAND_SOURCES {
            for (name, block) in command_blocks(source) {
                let touches: Vec<&str> = PATH_TOUCHING_CALLS
                    .iter()
                    .copied()
                    .filter(|call| block.contains(*call))
                    .collect();
                if !touches.is_empty() && !block.contains(GATE_CALL) {
                    offenders.push(format!(
                        "{}::{} calls {} without {}",
                        file,
                        name,
                        touches.join(", "),
                        GATE_CALL
                    ));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "every file-touching #[tauri::command] must route through {}:\n  {}",
            GATE_CALL,
            offenders.join("\n  ")
        );
    }

    #[test]
    fn every_gate_call_in_a_command_names_an_operation() {
        // The other direction: a call to the gate that does not pass a PathOp
        // would not compile, but a command that imports the gate and never
        // calls it would slip through the test above unnoticed.
        for (file, source) in COMMAND_SOURCES {
            let code = code_only(source);
            let gate_calls = code.matches(GATE_CALL).count();
            let op_mentions = code.matches("PathOp::").count();
            assert!(
                op_mentions >= gate_calls,
                "{} has {} gate calls but only {} PathOp mentions",
                file,
                gate_calls,
                op_mentions
            );
        }
    }

    #[test]
    fn the_command_sources_still_hold_commands() {
        // Guards the test above against silently passing because the parse
        // found nothing: if these files are renamed, this fails loudly.
        for (file, source) in COMMAND_SOURCES {
            assert!(
                !command_blocks(source).is_empty(),
                "{} has no #[tauri::command] blocks; update COMMAND_SOURCES",
                file
            );
        }
    }

    #[test]
    fn the_interim_validator_is_gone() {
        // `valid_file_path` was the non-empty-plus-".." check that guarded the
        // agent's surface. It must not come back, here or anywhere. Comments
        // are stripped first: the note left where it used to live names it on
        // purpose, so the next person knows not to re-add it.
        let debug_utils = include_str!("commands/debug_utils.rs");
        for (file, source) in COMMAND_SOURCES
            .iter()
            .copied()
            .chain([("commands/debug_utils.rs", debug_utils)])
        {
            assert!(
                !code_only(source).contains("valid_file_path"),
                "{} still references valid_file_path",
                file
            );
        }
    }
}
