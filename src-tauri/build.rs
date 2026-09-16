fn main() {
    tauri_build::build();

    // A demo build bakes its key in with option_env!, which cargo cannot see
    // as an input on its own. Without these, flipping between a demo build and
    // a normal one reuses the cached crate and ships the wrong thing.
    println!("cargo:rerun-if-env-changed=JUNO_DEMO_ANTHROPIC_KEY");
    println!("cargo:rerun-if-env-changed=JUNO_DEMO_COHORT");

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
