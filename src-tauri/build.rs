fn main() {
    tauri_build::build();

    // Note: Icon copying is no longer needed since we use embedded icon data
    // This eliminates file system dependencies and debug folder complexity
}
