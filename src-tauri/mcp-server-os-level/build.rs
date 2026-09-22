fn main() {
    // The test binary links Swift-backed system frameworks that reference
    // @rpath/libswift_Concurrency.dylib. Library link args do not reach the
    // final binary (see src-tauri/build.rs, which does the same for the app),
    // so without this `cargo test --workspace` aborts in dyld before any test
    // runs. The plain form covers this crate's test binaries and does not propagate to dependents.
    #[cfg(target_os = "macos")]
    {
        println!("cargo::rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

        if let Ok(output) = std::process::Command::new("xcode-select")
            .arg("-p")
            .output()
        {
            if output.status.success() {
                let dev_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                println!(
                    "cargo::rustc-link-arg=-Wl,-rpath,{}/usr/lib/swift/macosx",
                    dev_path
                );
            }
        }
    }
}
