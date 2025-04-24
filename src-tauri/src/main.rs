#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Ensure the library name matches the `name` field in `[lib]` section of Cargo.toml
    juno_lib::run();
}
