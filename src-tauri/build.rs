fn main() {
    tauri_build::build();

    // A demo build bakes its key in with option_env!, which cargo cannot see
    // as an input on its own. Without these, flipping between a demo build and
    // a normal one reuses the cached crate and ships the wrong thing.
    println!("cargo:rerun-if-env-changed=JUNO_DEMO_ANTHROPIC_KEY");
    println!("cargo:rerun-if-env-changed=JUNO_DEMO_COHORT");

    emit_build_identity();

    // Note: Icon copying is no longer needed since we use embedded icon data
    // This eliminates file system dependencies and debug folder complexity

    // Add rpath for Swift runtime so @rpath/libswift_Concurrency.dylib resolves.
    // The screencapturekit crate emits this in its own build.rs, but
    // cargo:rustc-link-arg only applies to bin targets — library crate link args
    // don't propagate to the final binary, so we must set it here.
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

        // Also add the Command Line Tools / Xcode Swift runtime path
        if let Ok(output) = std::process::Command::new("xcode-select")
            .arg("-p")
            .output()
        {
            if output.status.success() {
                let dev_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                println!(
                    "cargo:rustc-link-arg=-Wl,-rpath,{}/usr/lib/swift/macosx",
                    dev_path
                );
            }
        }
    }
}

/// Compile in which build this is, so a bug report can name a commit instead
/// of a date. Every field degrades to a placeholder rather than failing the
/// build: git is missing in some release environments, and a build that cannot
/// describe itself is still better than no build.
fn emit_build_identity() {
    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git").args(args).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8(out.stdout).ok()?.trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    };

    // Commit count: monotonic along a line of development, and it maps back to
    // exactly one commit. The short sha rides along because two branches can
    // reach the same count.
    let number = git(&["rev-list", "--count", "HEAD"]).unwrap_or_else(|| "0".to_string());
    let commit = git(&["rev-parse", "--short=8", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    let branch =
        git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    // Any uncommitted change means the source is not what the commit says it
    // is, and the build should admit that rather than claim a clean sha.
    let dirty = git(&["status", "--porcelain"]).is_some();

    let built_at = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=JUNO_BUILD_NUMBER={number}");
    println!("cargo:rustc-env=JUNO_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=JUNO_BUILD_BRANCH={branch}");
    println!("cargo:rustc-env=JUNO_BUILD_DIRTY={dirty}");
    println!("cargo:rustc-env=JUNO_BUILT_AT={built_at}");

    // Without these, cargo caches the values above and every later build keeps
    // reporting the commit that happened to be checked out the first time.
    // `--git-path` is what makes this work in a worktree, where `.git` is a
    // file pointing elsewhere rather than a directory.
    for path in ["HEAD", "index"] {
        if let Some(resolved) = git(&["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={resolved}");
        }
    }
}
