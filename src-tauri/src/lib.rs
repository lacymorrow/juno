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
pub mod agents; // Multi-agent system with specialized agents
pub mod voice_control; // Added for voice control functionality
pub mod constants;


// Re-export key items for discoverability by main.rs and tauri::generate_handler
use commands::{app_url::*, core::*, element::*, filesystem::*, keyboard::*, mouse::*, providers::*, shell::*, text_editor::*, window::*, voice_control::*};
pub use anthropic::submit_query; // Re-export the submit_query command

// Import VoiceController for the new QA command
use voice_control::VoiceController;

// Added for selector parsing

// Tauri command for QA testing transcription
#[tauri::command]
async fn qa_transcribe_file(model_path: String, audio_path: String) -> Result<String, String> {
    // It's good practice to run blocking operations on a separate thread
    // if they might take a while, to avoid blocking the main Tauri async runtime.
    // For whisper model loading and transcription, this is essential.
    tokio::task::spawn_blocking(move || {
        let voice_controller = VoiceController::new(&model_path)?;
        voice_controller.transcribe_audio_file(&audio_path)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

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

    // --- Initialize VoiceController and add to AppState ---
    // TODO: Make model path configurable (e.g., via .env, config file, or UI setting)
    let model_path_env = std::env::var("VOICE_MODEL_PATH");
    let model_path = model_path_env.as_deref().unwrap_or(constants::paths::DEFAULT_MODEL_PATH);

    info!("[Setup] Attempting to initialize VoiceController with model: {}", model_path);
    match VoiceController::new(model_path) {
        Ok(voice_controller) => {
            app_state.insert(Arc::new(std::sync::Mutex::new(voice_controller)));
            info!("[Setup] VoiceController initialized and added to AppState.");
        }
        Err(e) => {
            tracing::error!("[Setup] Failed to initialize VoiceController: {}. Voice control features will be unavailable.", e);
            // Consider inserting a placeholder/disabled VoiceController or handling this state in commands
        }
    }

    // --- Tauri Application Builder ---
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
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
                let app_handle_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    let app_state_instance = app_handle_clone.state::<state::AppState>();
                    if let Some(vc_arc) = app_state_instance.get::<Arc<std::sync::Mutex<VoiceController>>>() {
                        match vc_arc.lock() {
                            Ok(mut voice_controller) => {
                                match voice_controller.toggle_dictation(app_handle_clone.clone()) {
                                    Ok(is_now_dictating) => {
                                        info!("[GlobalShortcut] Dictation toggled. New state: {}", if is_now_dictating { "ON" } else { "OFF" });
                                        if let Err(e) = app_handle_clone.emit(constants::events::DICTATION_STATE_CHANGED, is_now_dictating) {
                                            tracing::warn!("[GlobalShortcut] Failed to emit {} event: {}", constants::events::DICTATION_STATE_CHANGED, e);
                                        }
                                        // If dictation was just turned OFF, request playback
                                        if !is_now_dictating {
                                            info!("[GlobalShortcut] Dictation stopped via shortcut.");
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("[GlobalShortcut] Error toggling dictation: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("[GlobalShortcut] Failed to lock VoiceController: {}", e);
                            }
                        }
                    } else {
                        tracing::warn!("[GlobalShortcut] VoiceController not found in AppState. Cannot toggle dictation.");
                    }
                });
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
            // Voice Control Commands
            start_dictation_command,
            stop_dictation_command,
            toggle_dictation_command,
            get_dictation_status_command,
            set_developer_playback_enabled_command,
            playback_last_audio_chunk,
            // QA Test Commands from mouse.rs
            qa_test_click,
            qa_test_click_series,
            qa_test_coordinate_transformation,
            qa_test_click_visualization,
            qa_test_select_text,
            qa_test_scroll,
            qa_transcribe_file, // Add the new QA command here
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

            // Listen for audio playback test request
            if let Some(main_window) = app.get_webview_window(constants::window_labels::MAIN) {
                let app_handle_for_listener = app.handle().clone();
                let _event_id = main_window.listen(constants::events::REQUEST_AUDIO_PLAYBACK_TEST, move |_event| {
                    info!("[Event Listener] Received {} event.", constants::events::REQUEST_AUDIO_PLAYBACK_TEST);
                    let ah_clone = app_handle_for_listener.clone();
                    tauri::async_runtime::spawn(async move {
                        let app_state = ah_clone.state::<crate::state::AppState>(); // Ensure crate::state path is correct
                        match crate::commands::voice_control::playback_last_audio_chunk(app_state).await {
                            Ok(message) => info!("[Event Listener] playback_last_audio_chunk successful: {}", message),
                            Err(e) => tracing::error!("[Event Listener] playback_last_audio_chunk error: {}", e),
                        }
                    });
                });
                // Note: .map_err().unwrap() removed as .listen() returns EventId not Result
            } else {
                tracing::error!("[Setup] Main window not found, cannot listen for {} event.", constants::events::REQUEST_AUDIO_PLAYBACK_TEST);
            }

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
