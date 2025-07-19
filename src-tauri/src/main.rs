#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Generate the Tauri context in the main binary crate
    let context = tauri::generate_context!();
    
    // Pass the context to the library run function
    juno_lib::run(context);
}
