#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Import necessary external crates and standard library items
use clap::Parser;
use computer_use_ai_sdk::Desktop;
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use tauri::{
    Manager, // WindowEvent, // Removed WindowEvent
    menu::{MenuItemKind, Menu}, // Removed Menu, MenuItemBuilder, PredefinedMenuItem // Added Menu
    tray::{TrayIconEvent, MouseButton, MouseButtonState, TrayIconBuilder}, // Ensured TrayIconBuilder
    image::Image as TauriImage, // Use tauri::image::Image, aliased
    AppHandle, // Keep AppHandle
    Emitter, // Import Emitter trait for .emit()
    Listener, // Added Listener for .listen()
    WebviewWindow, // Keep WebviewWindow
    Wry, // Keep Wry if needed elsewhere, remove if not
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, Code, ShortcutState, Modifiers as ShortcutModifiers}; // Use ShortcutState, remove ShortcutEvent, Add Modifiers
use tracing_subscriber::{fmt, EnvFilter}; // Add fmt and EnvFilter
use tracing::info; // Import the info macro
use serde::Deserialize; // Added for deserializing payload struct

// macOS specific imports
#[cfg(target_os = "macos")]
use {
    cocoa::{
        appkit::{NSWindow}, // Removed NSWindowCollectionBehavior
        base::{id as cocoa_id, nil}, // Removed YES, NO, BOOL
        foundation::{NSRect},
    },
    objc::{class, msg_send, runtime::{Class, Object, Sel}, sel, sel_impl, declare::ClassDecl},
};

// Declare modules
pub mod tts;
pub mod state;
pub mod anthropic;
pub mod tools;
pub mod commands;
pub mod cli;
pub mod utils;
pub mod agent;
pub mod constants;

// Re-export key items for discoverability by main.rs and tauri::generate_handler
use commands::{app_url::*, core::*, element::*, filesystem::*, keyboard::*, mouse::*, providers::*, shell::*, text_editor::*, window::*};
pub use anthropic::submit_query; // Re-export the submit_query command

// Added for selector parsing

// Define a struct for the expected payload of bar-state-changed event
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BarStateChangeEventPayload {
    new_state: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Explicitly initialize tracing with INFO level by default
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();
    dotenv().ok();
    let cli = cli::Cli::parse();

    // --- Initialize Desktop Automation Engine --- (Moved before CLI handling)
    let desktop_instance_result = Desktop::new(false, true);
    let desktop_instance = match desktop_instance_result {
        Ok(instance) => instance,
        Err(e) => {
            eprintln!("FATAL: Failed to initialize Desktop Automation Engine: {}", e);
            tracing::error!("Failed to initialize Desktop Automation Engine: {}", e);
            std::process::exit(1);
        }
    };

    // --- Initialize Provider Settings ---
    if let Err(e) = agent::providers::factory::BrainFactory::init() {
        tracing::warn!("Failed to initialize AI provider settings: {}", e);
        tracing::info!("Continuing with environment variables or fallback defaults");
    } else {
        tracing::info!("Provider settings initialized from configuration");
    }

    // --- Handle CLI Commands ---
    // If handle_cli_commands returns true, it means a command was executed
    // and the application should exit.
    if cli::runner::handle_cli_commands(&cli, &desktop_instance) {
        return; // Exit early if a CLI command was handled
    }

    // --- Proceed with Tauri Application Launch if no CLI command was run ---
    println!("No CLI commands detected or tests requiring exit, launching Tauri application...");
    let desktop_arc = Arc::new(desktop_instance);

    // Create the AppState
    let app_state = state::AppState::new(desktop_arc.clone());

    // Initialize shell state
    commands::shell::init_shell_state(&app_state);

    // --- Tauri Application Builder ---
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_voice_transcription::init()) // Add the voice transcription plugin
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app: &AppHandle, shortcut: &Shortcut, event| {
            println!("[GlobalShortcut Triggered] Shortcut: {:?}, State: {:?}", shortcut, event.state());

            // Define the specific shortcuts we're interested in
            let escape_shortcut = Shortcut::new(None, Code::Escape);
            // TODO: Make the dictation shortcut configurable
            let dictation_toggle_shortcut = Shortcut::new(Some(ShortcutModifiers::ALT), Code::KeyD);


            if shortcut == &escape_shortcut && event.state() == ShortcutState::Pressed {
                println!("[GlobalShortcut] Escape pressed! Signaling agent stop.");
                let app_state_instance = app.state::<state::AppState>();
                app_state_instance.signal_cancel();
                info!("[GlobalShortcut] Agent cancellation signal sent via Escape.");
                if let Err(e) = app.emit(constants::events::AGENT_STOPPING, ()) {
                    eprintln!("[GlobalShortcut Error] Failed to emit {} event: {}", constants::events::AGENT_STOPPING, e);
                }
            } else if shortcut == &dictation_toggle_shortcut && event.state() == ShortcutState::Pressed {
                info!("[GlobalShortcut] Dictation toggle shortcut ({:?}) pressed.", shortcut);
                // Emit an event for the frontend to handle
                if let Err(e) = app.emit("toggle-dictation-request", ()) {
                    tracing::error!("[GlobalShortcut] Failed to emit toggle-dictation-request event: {}", e);
                }
            }
        }).build())
        .manage(app_state) // Manage the AppState
        .invoke_handler(tauri::generate_handler![
            // Use re-exported commands
            list_apps,
            check_server_status,
            submit_query,
            anthropic::cleanup_browser, // Add browser cleanup function
            tts::invoke_tts, // Use the main invoke_tts command for Tauri
            tts::set_tts_provider_command, // Added for TTS provider selection
            tts::get_tts_provider_command, // Added for TTS provider selection
            capture_screenshot_command,
            dev_get_focused_element_info,
            capture_element_screenshot_command,
            dev_click_focused_element,
            dev_type_text,
            dev_press_key,
            dev_open_application,
            dev_open_url,
            dev_scroll_window,
            dev_global_type_text,
            dev_get_clipboard,
            dev_set_clipboard,
            dev_hold_key,
            dev_release_key,
            dev_wait,
            dev_find_element_by_selector,
            dev_click_element_by_selector,
            dev_get_window_list,
            dev_get_selected_text,
            dev_get_window_info,
            dev_focus_window,
            dev_triple_click,
            dev_mouse_move,
            dev_left_mouse_down,
            dev_left_mouse_up,
            dev_left_click,
            dev_left_click_drag,
            dev_right_click,
            dev_middle_click,
            dev_double_click,
            dev_get_cursor_position,
            dev_test_click_visualization,
            dev_bash_command,
            dev_list_files,
            dev_get_file_content,
            dev_set_file_content,
            // Text Editor Commands
            dev_text_editor_view,
            dev_text_editor_create,
            dev_text_editor_str_replace,
            dev_text_editor_insert,
            dev_text_editor_undo_edit,
            // Provider Management Commands
            get_providers,
            get_active_provider,
            set_active_provider,
            get_provider_settings,
            update_provider_api_key,
            update_provider_model,
            update_provider_max_tokens,
            update_provider_temperature,
            update_provider_system_prompt,
            // QA Test Commands from mouse.rs
            qa_test_click,
            qa_test_click_series,
            qa_test_coordinate_transformation,
            qa_test_click_visualization,
            qa_test_select_text,
            qa_test_scroll,
            // App Life Cycle
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // --- Setup Tray Icon ---
            let tray_app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                // Load the icon
                let icon_path = tray_app_handle.path().resolve("icons/32x32.png", tauri::path::BaseDirectory::Resource).unwrap_or_else(|_| {
                    eprintln!("[Tray Setup Error] Failed to resolve icon path. Using a placeholder or default mechanism if available.");
                    std::path::PathBuf::from("icons/32x32.png")
                });

                let loaded_tauri_icon = match image::open(&icon_path) { // Use the `image` crate to open
                    Ok(dynamic_image) => {
                        let width = dynamic_image.width();
                        let height = dynamic_image.height();
                        let rgba_image = dynamic_image.to_rgba8();
                        let bytes = rgba_image.into_raw();
                        // Assuming TauriImage::new returns TauriImage directly (or panics on error)
                        // based on `-> Self` in its signature from earlier compiler messages.
                        // Use new_owned to move the bytes and avoid lifetime issues.
                        let img = TauriImage::new_owned(bytes, width, height);
                        Some(img)
                    },
                    Err(e) => {
                        eprintln!("[Tray Setup Error] Failed to load image from path {:?} using 'image' crate: {}", icon_path, e);
                        None
                    }
                };

                // Create a simple menu
                let quit_item = MenuItemKind::MenuItem(tauri::menu::MenuItem::with_id(&tray_app_handle, constants::tray_menu_ids::QUIT, "Quit Juno", true, None::<&str>).unwrap());
                let toggle_item = MenuItemKind::MenuItem(tauri::menu::MenuItem::with_id(&tray_app_handle, constants::tray_menu_ids::TOGGLE_FLOATING_BAR, "Toggle Floating Bar", true, None::<&str>).unwrap());
                let tray_menu = Menu::with_items(&tray_app_handle, &[
                    &quit_item,
                    &MenuItemKind::Predefined(tauri::menu::PredefinedMenuItem::separator(&tray_app_handle).unwrap()),
                    &toggle_item,
                ]).map_err(|e| eprintln!("[Tray Setup Error] Failed to create tray menu: {}", e)).ok();


                let mut tray_builder = TrayIconBuilder::new()
                    .on_menu_event(move |app_handle, event| {
                        match event.id().as_ref() {
                            constants::tray_menu_ids::QUIT => {
                                println!("[Tray Menu] Quit requested.");
                                app_handle.exit(0);
                            }
                            constants::tray_menu_ids::TOGGLE_FLOATING_BAR => {
                                println!("[Tray Menu] Toggle floating bar requested.");
                                if let Some(window) = app_handle.get_webview_window(constants::window_labels::FLOATING_BAR) {
                                    match window.is_visible() {
                                        Ok(true) => {
                                            let _ = window.hide();
                                            if let Err(e) = window.set_ignore_cursor_events(true) {
                                                eprintln!("[Tray Error] Failed to set ignore cursor events to true: {}", e);
                                            }
                                        }
                                        Ok(false) => {
                                            if let Err(e) = window.set_ignore_cursor_events(false) {
                                                eprintln!("[Tray Error] Failed to set ignore cursor events to false: {}", e);
                                            }
                                            let _ = window.show();
                                            let _ = window.set_focus();
                                        }
                                        Err(e) => eprintln!("[Tray Menu Error] Checking floating bar visibility: {}", e),
                                    }
                                } else {
                                    eprintln!("[Tray Menu Error] Floating bar window not found for toggle.");
                                }
                            }
                            _ => {
                                println!("[Tray Menu] Unhandled event: {:?}", event.id());
                            }
                        }
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            println!("[Tray Icon] Left click detected.");
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window(constants::window_labels::FLOATING_BAR) {
                                match window.is_visible() {
                                    Ok(true) => {
                                        window.hide().unwrap();
                                        if let Err(e) = window.set_ignore_cursor_events(true) {
                                            eprintln!("[Tray Error] Failed to set ignore cursor events to true: {}", e);
                                        }
                                        println!("[Tray Icon] Floating bar hidden and ignoring clicks.");
                                    },
                                    Ok(false) => {
                                        if let Err(e) = window.set_ignore_cursor_events(false) {
                                            eprintln!("[Tray Error] Failed to set ignore cursor events to false: {}", e);
                                        }
                                        window.show().unwrap();
                                        window.set_focus().unwrap();
                                        println!("[Tray Icon] Floating bar shown, focused, and accepting clicks.");
                                    },
                                    Err(e) => eprintln!("[Tray Icon Error] checking floating bar visibility: {}", e),
                                }
                            } else {
                                 eprintln!("[Tray Icon Error] Floating bar window not found on left click.");
                            }
                        }
                    });

                if let Some(icon_image) = loaded_tauri_icon {
                    tray_builder = tray_builder.icon(icon_image);
                }

                if let Some(menu) = tray_menu {
                    tray_builder = tray_builder.menu(&menu);
                }

                match tray_builder.build(&tray_app_handle) {
                    Ok(_) => println!("[Tray Setup] Tray icon configured successfully."),
                    Err(e) => eprintln!("[Tray Setup Error] Failed to build tray icon: {}", e),
                }
            });
            // --- End of Tray Icon Setup ---

            // --- Setup Floating Bar State Listener ---
            if let Some(floating_bar_window) = app.get_webview_window(constants::window_labels::FLOATING_BAR) {
                let app_handle_for_listener = app.handle().clone(); // Clone AppHandle for the listener
                floating_bar_window.listen(constants::events::BAR_STATE_CHANGED, move |event| {
                    info!("[Event: {}] Received raw: {:?}", constants::events::BAR_STATE_CHANGED, event.payload());
                    let payload_str = event.payload(); // Assuming this is &str as per compiler error
                    match serde_json::from_str::<BarStateChangeEventPayload>(payload_str) {
                        Ok(parsed_payload) => {
                            let new_state_str = &parsed_payload.new_state;
                            // Get AppState from the AppHandle inside the closure
                            let app_state = app_handle_for_listener.state::<state::AppState>();
                            // Clone the Arc for the bar_ui_state to extend its lifetime
                            let bar_ui_state_arc = app_state.bar_ui_state.clone();
                            let lock_result = bar_ui_state_arc.lock(); // Assign lock result to a variable
                            match lock_result { // Match on the result
                                Ok(mut bar_state_guard) => {
                                    *bar_state_guard = new_state_str.to_string();
                                    info!("[AppState Update] bar_ui_state updated to: {}", new_state_str);
                                }
                                Err(e) => {
                                    tracing::error!("[Event: {}] Failed to lock AppState.bar_ui_state: {}", constants::events::BAR_STATE_CHANGED, e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("[Event: {}] Failed to parse payload into BarStateChangeEventPayload: {}. Payload: {}", constants::events::BAR_STATE_CHANGED, e, payload_str);
                        }
                    }
                });
                info!("[Setup] Listener for '{}' event attached to floating-bar window.", constants::events::BAR_STATE_CHANGED);
            } else {
                tracing::error!("[Setup] Floating-bar window not found, cannot listen for {} event.", constants::events::BAR_STATE_CHANGED);
            }

            // --- End of Floating Bar State Listener Setup ---

            let app_handle_shortcuts = app.handle().clone(); // Use a new clone for shortcuts
            tauri::async_runtime::spawn(async move {
                // Register Escape shortcut
                if let Err(e) = app_handle_shortcuts.global_shortcut().register("Escape") {
                    eprintln!("[GlobalShortcut Error] Failed to register Escape shortcut: {}", e);
                }

                // Register Dictation Toggle Shortcut
                let dictation_shortcut_str = if cfg!(target_os = "macos") { "Option+D" } else { "Alt+D" };
                if let Err(e) = app_handle_shortcuts.global_shortcut().register(dictation_shortcut_str) {
                     eprintln!("[GlobalShortcut Error] Failed to register {} shortcut: {}", dictation_shortcut_str, e);
                }
            });

            // Listen for dictation started events from the plugin
            let app_handle_for_listener = app.handle().clone();
            app.listen("voice-transcription:dictation-started", move |event| {
                info!("[Event] Received voice-transcription:dictation-started event");
                // Rebroadcast the event as app-dictation-started for backward compatibility
                if let Err(e) = app_handle_for_listener.emit("app-dictation-started", event.payload()) {
                    tracing::error!("[Event] Failed to rebroadcast dictation-started event: {}", e);
                }
            });

            // Listen for partial result events from the plugin
            let app_handle_for_listener = app.handle().clone();
            app.listen("voice-transcription:partial-result", move |event| {
                info!("[Event] Received voice-transcription:partial-result event: {:?}", event.payload());
                // Rebroadcast the event as app-dictation-partial-result for backward compatibility
                if let Err(e) = app_handle_for_listener.emit("app-dictation-partial-result", event.payload()) {
                    tracing::error!("[Event] Failed to rebroadcast partial-result event: {}", e);
                }
            });

            // Listen for final result events from the plugin
            let app_handle_for_listener = app.handle().clone();
            app.listen("voice-transcription:final-result", move |event| {
                info!("[Event] Received voice-transcription:final-result event: {:?}", event.payload());

                // Transform the payload from { "text": "..." } to { "query": "..." } format expected by frontend
                let payload_str = event.payload();
                match serde_json::from_str::<serde_json::Value>(payload_str) {
                    Ok(mut payload_json) => {
                        if let Some(text_value) = payload_json.get("text") {
                            // Transform { "text": "..." } to { "query": "..." }
                            let transformed_payload = serde_json::json!({
                                "query": text_value
                            });
                            if let Err(e) = app_handle_for_listener.emit("app-dictation-finished", transformed_payload) {
                                tracing::error!("[Event] Failed to rebroadcast final-result event: {}", e);
                            }
                        } else {
                            tracing::error!("[Event] No 'text' field found in final-result payload: {}", payload_str);
                        }
                    }
                    Err(e) => {
                        tracing::error!("[Event] Failed to parse final-result payload as JSON: {}, payload: {}", e, payload_str);
                        // Fallback: emit with original payload
                        if let Err(e) = app_handle_for_listener.emit("app-dictation-finished", event.payload()) {
                            tracing::error!("[Event] Failed to rebroadcast final-result event (fallback): {}", e);
                        }
                    }
                }
            });

            // Listen for dictation stopped events from the plugin
            let app_handle_for_listener = app.handle().clone();
            app.listen("voice-transcription:dictation-stopped", move |event| {
                info!("[Event] Received voice-transcription:dictation-stopped event");
                // Rebroadcast the event as app-dictation-stopped for backward compatibility
                if let Err(e) = app_handle_for_listener.emit("app-dictation-stopped", event.payload()) {
                    tracing::error!("[Event] Failed to rebroadcast dictation-stopped event: {}", e);
                }
            });

            Ok(())
        });

    builder
        .run(tauri::generate_context!()) // Use context relative to lib.rs now
        .expect("error while running tauri application");
}

// Unit tests module
#[cfg(test)]
mod tests {


    #[test]
    fn test_focused_element_info_placeholder() {
        // This test is a placeholder and needs a proper implementation
        // For now, it just asserts true to ensure the test runner picks it up
        assert!(true, "Placeholder test for focused_element_info");
    }
}

// --- Define macOS specific constants and delegate ---
#[cfg(target_os = "macos")]
mod macos_tracking {
    use super::*; // Import items from parent module (like AppHandle, cocoa types etc.)
    use std::sync::Mutex; // Use std::sync::Mutex for interior mutability safely

    // Constants for NSTrackingAreaOptions
    const NS_TRACKING_MOUSE_ENTERED_AND_EXITED: u64 = 0x01;
    const NS_TRACKING_ACTIVE_ALWAYS: u64 = 0x80;
    const TRACKING_OPTIONS: u64 = NS_TRACKING_MOUSE_ENTERED_AND_EXITED | NS_TRACKING_ACTIVE_ALWAYS;

    // Static storage for the AppHandle, wrapped for thread safety
    static APP_HANDLE: Mutex<Option<AppHandle>> = Mutex::new(None);

    // Delegate implementation
    extern "C" fn mouse_entered(_this: &Object, _cmd: Sel, _event: cocoa_id) {
        info!("[Tracking Delegate] Mouse Entered");
        if let Some(handle) = APP_HANDLE.lock().unwrap().as_ref() {
             if let Some(window) = handle.get_webview_window(constants::window_labels::FLOATING_BAR) {
                let _ = window.emit("mouse-entered-window", ()); // Emit specific event
                 info!("[Tracking Delegate] Emitted mouse-entered-window");
             } else {
                  eprintln!("[Tracking Delegate Error] Floating bar window not found for mouse_entered emit.");
             }
        }
    }

    extern "C" fn mouse_exited(_this: &Object, _cmd: Sel, _event: cocoa_id) {
         info!("[Tracking Delegate] Mouse Exited");
         if let Some(handle) = APP_HANDLE.lock().unwrap().as_ref() {
             if let Some(window) = handle.get_webview_window(constants::window_labels::FLOATING_BAR) {
                let _ = window.emit("mouse-left-window", ()); // Emit specific event
                 info!("[Tracking Delegate] Emitted mouse-left-window");
             } else {
                 eprintln!("[Tracking Delegate Error] Floating bar window not found for mouse_exited emit.");
             }
         }
    }

    pub fn setup_tracking_area(window: &WebviewWindow<Wry>, app_handle: AppHandle) {
        info!("Setting up macOS tracking area for floating-bar...");
        // Store the AppHandle statically
        *APP_HANDLE.lock().unwrap() = Some(app_handle.clone());

        let ns_window = match window.ns_window() {
            Ok(ptr) => ptr as cocoa_id,
            Err(e) => {
                eprintln!("Failed to get NSWindow for tracking area setup: {}", e);
                return;
            }
        };

        unsafe {
            let view = ns_window.contentView();
            if view == nil {
                eprintln!("Failed to get contentView for tracking area setup.");
                return;
            }

            let delegate_class_name = "MouseTrackingDelegate";
            let mut delegate_class = Class::get(delegate_class_name);

            // Declare class only if it doesn't exist yet
            if delegate_class.is_none() {
                 info!("Declaring MouseTrackingDelegate class...");
                 #[allow(unexpected_cfgs)] // Allow cfg from class! macro
                let superclass = class!(NSObject);
                let mut decl = ClassDecl::new(delegate_class_name, superclass).unwrap();

                // Add mouseEntered: method
                #[allow(unexpected_cfgs)] // Allow cfg from sel! macro
                decl.add_method(
                    sel!(mouseEntered:),
                    mouse_entered as extern "C" fn(&Object, Sel, cocoa_id),
                );

                // Add mouseExited: method
                #[allow(unexpected_cfgs)] // Allow cfg from sel! macro
                decl.add_method(
                    sel!(mouseExited:),
                    mouse_exited as extern "C" fn(&Object, Sel, cocoa_id),
                );

                delegate_class = Some(decl.register());
                 info!("MouseTrackingDelegate class registered.");
            }

            #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
            let delegate: cocoa_id = msg_send![delegate_class.unwrap(), new];
             info!("MouseTrackingDelegate instance created: {:?}", delegate);

            // Keep the delegate alive. Leaking it here is simpler than complex lifetime management.
            let _ = Box::leak(Box::new(delegate)); // Box the delegate and leak it

            #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
            let bounds: NSRect = msg_send![view, bounds];
             info!("Got view bounds for tracking area.");

            #[allow(unexpected_cfgs)] // Allow cfg from msg_send and class! macros
            let tracking_area: cocoa_id = msg_send![class!(NSTrackingArea), alloc];
            #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
            let tracking_area_ptr: cocoa_id = msg_send![
                tracking_area,
                initWithRect: bounds
                options: TRACKING_OPTIONS
                owner: delegate // Use the delegate instance as the owner
                userInfo: nil
            ];
             info!("NSTrackingArea created: {:?}", tracking_area_ptr);

            #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
            let _: () = msg_send![view, addTrackingArea: tracking_area_ptr];
            #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
            let _: () = msg_send![tracking_area_ptr, release]; // Release after adding (view retains it)
            // Note: Do not release the delegate here, it's leaked via Box::leak

             info!("NSTrackingArea added to view.");
        }
    }
}
