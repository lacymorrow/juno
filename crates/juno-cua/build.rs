fn main() {
    // Add rpath for Swift runtime so @rpath/libswift_Concurrency.dylib resolves.
    // The screencapturekit crate (via computer-use-ai-sdk) links Swift concurrency,
    // but cargo:rustc-link-arg from library crates doesn't propagate to the final
    // binary — each bin crate must set it.
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

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
