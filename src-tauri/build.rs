use std::env;
use std::fs;
use std::path::Path;

fn main() {
    tauri_build::build();

    // Copy icons to target directory for development builds
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = Path::new(&out_dir).ancestors().nth(3).unwrap(); // Navigate up to target directory
    let icons_target_dir = target_dir.join("icons");

    // Create icons directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&icons_target_dir) {
        println!("cargo:warning=Failed to create icons directory: {}", e);
        return;
    }

    // Copy icon files
    let source_icons_dir = Path::new("icons");
    if source_icons_dir.exists() {
        for entry in fs::read_dir(source_icons_dir).unwrap() {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "png" || ext == "ico" || ext == "icns") {
                    let dest_path = icons_target_dir.join(entry.file_name());
                    if let Err(e) = fs::copy(&path, &dest_path) {
                        println!("cargo:warning=Failed to copy {}: {}", path.display(), e);
                    }
                }
            }
        }
    }
}
