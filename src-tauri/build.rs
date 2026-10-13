fn main() {
    tauri_build::build();

    // A demo build bakes its key in with option_env!, which cargo cannot see
    // as an input on its own. Without these, flipping between a demo build and
    // a normal one reuses the cached crate and ships the wrong thing.
    println!("cargo:rerun-if-env-changed=JUNO_DEMO_ANTHROPIC_KEY");
    println!("cargo:rerun-if-env-changed=JUNO_DEMO_COHORT");

    emit_build_identity();
    build_foundation_models_bridge();

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

/// Compile `swift/DictationCleanup.swift`, the bridge to Apple's on-device
/// language model, into a static library and link it.
///
/// Juno targets macOS 12.3 and ships a universal binary, while the
/// FoundationModels framework exists only in the macOS 26 SDK and runs only on
/// Apple Silicon. So the framework is never a hard dependency:
///
/// - the Swift source guards every use with `canImport` and `@available`;
/// - the framework is linked WEAKLY (`-weak_framework`), so dyld leaves it
///   unresolved on macOS 12 to 25 and the app still launches;
/// - the Swift autolink entry for the framework is turned off, because an
///   autolinked framework is a strong one;
/// - when the build is for Intel, or the SDK has no FoundationModels, or the
///   real compile fails, a stub with the same C symbols is built instead
///   (`JUNO_NO_FM`), and the bridge reports "unsupported" at runtime.
///
/// A fallback prints a `cargo:warning` so a build that quietly lost the feature
/// is visible in the CI log.
fn build_foundation_models_bridge() {
    println!("cargo:rerun-if-changed=swift/DictationCleanup.swift");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let out_dir = match std::env::var("OUT_DIR") {
        Ok(dir) => dir,
        Err(_) => return,
    };
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        _ => return,
    };

    let run = |program: &str, args: &[&str]| -> Option<String> {
        let out = std::process::Command::new(program)
            .args(args)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };

    let sdk = match run("xcrun", &["--sdk", "macosx", "--show-sdk-path"]) {
        Some(sdk) => sdk,
        None => {
            println!("cargo:warning=DictationCleanup: no macOS SDK found; the bridge is not built");
            return;
        }
    };
    let sdk_has_fm = std::path::Path::new(&sdk)
        .join("System/Library/Frameworks/FoundationModels.framework")
        .exists();
    let want_fm = arch == "arm64" && sdk_has_fm;

    let lib = format!("{out_dir}/libjunofm.a");
    let target = format!("{arch}-apple-macos12.3");
    let compile = |no_fm: bool| -> bool {
        let mut cmd = std::process::Command::new("xcrun");
        cmd.args(["swiftc", "-parse-as-library", "-O", "-swift-version", "5"])
            .args(["-module-name", "JunoFM", "-target", &target, "-sdk", &sdk])
            .args(["-emit-library", "-static", "-o", &lib]);
        if no_fm {
            cmd.args(["-D", "JUNO_NO_FM"]);
        } else {
            cmd.args(["-Xfrontend", "-disable-autolink-framework"])
                .args(["-Xfrontend", "FoundationModels"]);
        }
        cmd.arg("swift/DictationCleanup.swift");
        match cmd.output() {
            Ok(out) if out.status.success() => true,
            Ok(out) => {
                for line in String::from_utf8_lossy(&out.stderr).lines().take(20) {
                    println!("cargo:warning=swiftc: {line}");
                }
                false
            }
            Err(e) => {
                println!("cargo:warning=swiftc did not start: {e}");
                false
            }
        }
    };

    let with_fm = want_fm && compile(false);
    if want_fm && !with_fm {
        println!("cargo:warning=DictationCleanup: FoundationModels build failed; using the stub");
    } else if !want_fm {
        println!(
            "cargo:warning=DictationCleanup: arch={arch}, SDK has FoundationModels={sdk_has_fm}; building the stub"
        );
    }
    if !with_fm && !compile(true) {
        // Nothing to link, and the Rust side calls these symbols: a build
        // failure, not a silent loss of the feature.
        panic!("DictationCleanup: swiftc could not build the bridge stub");
    }

    println!("cargo:rustc-link-search=native={out_dir}");
    println!("cargo:rustc-link-lib=static=junofm");
    println!("cargo:rustc-link-search=native=/usr/lib/swift");
    if let Some(swiftc) = run("xcrun", &["--find", "swiftc"]) {
        if let Some(bin_dir) = std::path::Path::new(&swiftc).parent() {
            println!(
                "cargo:rustc-link-search=native={}",
                bin_dir.join("../lib/swift/macosx").display()
            );
        }
    }
    if with_fm {
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,FoundationModels");
    }
}
