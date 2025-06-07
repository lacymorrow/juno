#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Import necessary external crates and standard library items
use clap::Parser;
use computer_use_ai_sdk::Desktop;
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use tauri::{
    Manager, // WindowEvent, // Removed WindowEvent
    menu::{MenuItemKind, Menu, PredefinedMenuItem, SubmenuBuilder}, // Added PredefinedMenuItem, SubmenuBuilder
    tray::{TrayIconEvent, MouseButton, MouseButtonState, TrayIconBuilder}, // Ensured TrayIconBuilder
    image::Image as TauriImage, // Use tauri::image::Image, aliased
    AppHandle, // Keep AppHandle
    Emitter, // Import Emitter trait for .emit()
    Listener, // Added Listener for .listen()
    WebviewWindow, // Keep WebviewWindow
    Wry, // Keep Wry if needed elsewhere, remove if not
};
use tauri_plugin_global_shortcut::{Shortcut, Code, ShortcutState, Modifiers as ShortcutModifiers}; // Use ShortcutState, remove ShortcutEvent, Add Modifiers
use tracing_subscriber::{fmt, EnvFilter}; // Add fmt and EnvFilter
use tracing::{info, warn, error}; // Import logging macros
use std::sync::Mutex; // Added for VoiceController state access

// macOS specific imports
#[cfg(target_os = "macos")]
use {
    cocoa::{
        appkit::{NSWindow, NSWindowCollectionBehavior},
        base::{id as cocoa_id, nil, YES, NO, BOOL},
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
pub mod constants;
pub mod dictation_monitor; // Module for intelligent dictation input handling

// Embed tray icon data directly in the binary - no file system dependencies
const TRAY_ICON_DATA: &[u8] = include_bytes!("../icons/32x32.png");

/// Parse a shortcut string into a Shortcut object
/// Examples: "Alt+D" -> Shortcut, "Option+Space" -> Shortcut
pub fn parse_shortcut_string(shortcut_str: &str) -> Option<Shortcut> {
    let parts: Vec<&str> = shortcut_str.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers = ShortcutModifiers::empty();
    let key_part = parts.last()?;

    // Parse modifiers
    for part in &parts[..parts.len() - 1] {
        match part.to_lowercase().as_str() {
            "alt" | "option" => modifiers |= ShortcutModifiers::ALT,
            "cmd" | "command" => modifiers |= ShortcutModifiers::META,
            "ctrl" | "control" => modifiers |= ShortcutModifiers::CONTROL,
            "shift" => modifiers |= ShortcutModifiers::SHIFT,
            _ => {
                warn!("Unknown modifier: {}", part);
                return None;
            }
        }
    }

    // Parse the main key
    let code = match key_part.to_lowercase().as_str() {
        "a" => Code::KeyA,
        "b" => Code::KeyB,
        "c" => Code::KeyC,
        "d" => Code::KeyD,
        "e" => Code::KeyE,
        "f" => Code::KeyF,
        "g" => Code::KeyG,
        "h" => Code::KeyH,
        "i" => Code::KeyI,
        "j" => Code::KeyJ,
        "k" => Code::KeyK,
        "l" => Code::KeyL,
        "m" => Code::KeyM,
        "n" => Code::KeyN,
        "o" => Code::KeyO,
        "p" => Code::KeyP,
        "q" => Code::KeyQ,
        "r" => Code::KeyR,
        "s" => Code::KeyS,
        "t" => Code::KeyT,
        "u" => Code::KeyU,
        "v" => Code::KeyV,
        "w" => Code::KeyW,
        "x" => Code::KeyX,
        "y" => Code::KeyY,
        "z" => Code::KeyZ,
        "space" => Code::Space,
        "escape" | "esc" => Code::Escape,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "," => Code::Comma,
        _ => {
            warn!("Unknown key: {}", key_part);
            return None;
        }
    };

    let final_modifiers = if modifiers.is_empty() { None } else { Some(modifiers) };
    Some(Shortcut::new(final_modifiers, code))
}

// Re-export key items for discoverability by main.rs and tauri::generate_handler
use commands::{app_url::*, core::*, dictation::*, element::*, filesystem::*, floating_bar::*, keyboard::*, mouse::*, permissions::*, providers::*, shell::*, text_editor::*, window::*, orchestrator::*, sound::*};
pub use anthropic::submit_query; // Re-export the submit_query command

// Import dictation reset commands
use crate::commands::dictation_reset::{force_reset_dictation_transcription, get_dictation_transcription_status};

// Import tool configuration commands explicitly
use crate::commands::{
    get_tool_configurations,
    get_tool_config,
    set_tool_enabled,
    set_tool_category_enabled,
    get_enabled_tools,
    is_tool_enabled,
    reset_tool_configuration,
    get_tool_configuration_summary,
};

// Import keyboard shortcuts commands explicitly
use crate::commands::{
    get_keyboard_shortcuts,
    set_keyboard_shortcut,
    set_keyboard_shortcuts,
    reset_keyboard_shortcuts,
};

// Added for selector parsing

// Old BarStateChangeEventPayload removed - now using floating bar manager



#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize enhanced tracing with Slack/Apple Messages style formatting
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .with_target(false) // Hide target module names for cleaner output
        .with_thread_ids(false) // Hide thread IDs for cleaner output
        .with_ansi(true) // Enable colors for better readability
        .compact() // Use compact format instead of full
        .init();
    dotenv().ok();
    let cli = cli::Cli::parse();

    // --- Initialize Desktop Automation Engine --- (Moved before CLI handling)
    let desktop_instance_result = Desktop::new_with_auto_redirect(false, true, true);
    let desktop_instance = match desktop_instance_result {
        Ok(instance) => {
            tracing::info!("Desktop Automation Engine initialized successfully with auto-redirect");
            Some(instance)
        },
        Err(e) => {
            tracing::warn!("Failed to initialize Desktop Automation Engine: {}", e);
            tracing::info!("App will start with limited functionality - desktop automation features will be disabled");
            tracing::info!("System Settings should have opened automatically if permissions are needed");
            None
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
    if cli::runner::handle_cli_commands(&cli, desktop_instance.as_ref()) {
        return; // Exit early if a CLI command was handled
    }

    // --- Proceed with Tauri Application Launch if no CLI command was run ---
    println!("No CLI commands detected or tests requiring exit, launching Tauri application...");

    // Create desktop_arc only if we have a valid instance
    let desktop_arc = desktop_instance.map(|instance| Arc::new(instance));

    // Create the AppState with optional desktop instance
    let app_state = state::AppState::new(desktop_arc);

    // Initialize shell state
    commands::shell::init_shell_state(&app_state);

    // --- Tauri Application Builder ---
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_voice_transcription::init()) // Add the voice transcription plugin
        .plugin(tauri_plugin_process::init()) // Add the process plugin for app restart
        .plugin(tauri_plugin_store::Builder::default().build()) // Add the store plugin for persistent data
        .plugin        (tauri_plugin_global_shortcut::Builder::new().with_handler(|app: &AppHandle, shortcut: &Shortcut, event| {
            println!("[GlobalShortcut Triggered] Shortcut: {:?}, State: {:?}", shortcut, event.state());

            let app_state = app.state::<state::AppState>();

            // Get current keyboard shortcuts from state
            let current_shortcuts = match app_state.keyboard_shortcuts.lock() {
                Ok(shortcuts) => shortcuts.clone(),
                Err(e) => {
                    error!("Failed to get keyboard shortcuts: {}", e);
                    return; // Exit early if we can't get shortcuts
                }
            };

            // Create shortcut objects from current configuration
            let escape_shortcut = Shortcut::new(None, Code::Escape);
            let dictation_toggle_shortcut = parse_shortcut_string(&current_shortcuts.agent_mode_toggle);
            let dictation_input_shortcut = parse_shortcut_string(&current_shortcuts.dictation_input);

            if shortcut == &escape_shortcut && event.state() == ShortcutState::Pressed {
                println!("[GlobalShortcut] Escape pressed! Signaling agent stop and checking for active transcription.");
                let app_state_instance = app.state::<state::AppState>();

                // Cancel any running AI agent
                app_state_instance.signal_cancel();
                info!("[GlobalShortcut] Agent cancellation signal sent via Escape.");

                // Stop any active TTS
                crate::tts::stop_speech();
                info!("[GlobalShortcut] TTS stop signal sent via Escape.");

                // Check if dictation mode is active and cancel it if so
                let is_dictation_active = app_state_instance.dictation_active.lock()
                    .map(|active| *active)
                    .unwrap_or(false);

                if is_dictation_active {
                    info!("[GlobalShortcut] Dictation mode is active - cancelling transcription");

                    // Emit dictation cancellation event
                    if let Err(e) = app.emit("dictation-transcription-cancel", ()) {
                        error!("[GlobalShortcut] Failed to emit dictation-transcription-cancel: {}", e);
                    }

                    // Force stop voice controller
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        // Try to stop the voice transcription plugin
                        if let Some(controller_state) = app_clone.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
                            if let Err(e) = tauri_plugin_voice_transcription::commands::stop_dictation(
                                app_clone.clone(),
                                controller_state
                            ).await {
                                error!("[GlobalShortcut] Failed to stop voice transcription: {}", e);
                            } else {
                                info!("[GlobalShortcut] Voice transcription stopped successfully via Escape");
                            }
                        } else {
                            warn!("[GlobalShortcut] Voice controller not available - cannot stop transcription");
                        }

                        // Clean up app state
                        let app_state = app_clone.state::<state::AppState>();
                        if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                            *dictation_active = false;
                        }

                        // Update floating bar manager
                        if let Err(e) = app_clone.emit("dictation-active", false) {
                            error!("[GlobalShortcut] Failed to emit dictation-active event: {}", e);
                        }

                        // Reset dictation input monitor state
                        crate::dictation_monitor::force_reset_dictation_input_state().await;
                    });
                }

                // Always check if there's any voice activity to stop (handles both Agent Mode and edge cases)
                if let Some(voice_controller_state) = app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
                    if let Ok(voice_controller) = voice_controller_state.lock() {
                        if voice_controller.is_dictating() {
                            info!("[GlobalShortcut] Voice controller is active - stopping transcription");
                            drop(voice_controller); // Release the lock before async operation

                            let app_clone = app.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Some(controller_state) = app_clone.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
                                    if let Err(e) = tauri_plugin_voice_transcription::commands::stop_dictation(
                                        app_clone.clone(),
                                        controller_state
                                    ).await {
                                        error!("[GlobalShortcut] Failed to stop voice controller: {}", e);
                                    } else {
                                        info!("[GlobalShortcut] Voice controller stopped successfully via Escape");
                                    }
                                } else {
                                    warn!("[GlobalShortcut] Voice controller not available - cannot stop voice controller");
                                }
                            });
                        }
                    }
                }

                // Emit agent stopping event for any running AI agents
                if let Err(e) = app.emit(constants::events::AGENT_STOPPING, ()) {
                    eprintln!("[GlobalShortcut Error] Failed to emit {} event: {}", constants::events::AGENT_STOPPING, e);
                }
            }

            // Handle dictation toggle shortcut (Alt+D / Option+D)
            if let Some(ref toggle_shortcut) = dictation_toggle_shortcut {
                if shortcut == toggle_shortcut && event.state() == ShortcutState::Pressed {
                    info!("[GlobalShortcut] Dictation toggle shortcut ({:?}) pressed.", shortcut);
                    // Emit an event for the frontend to handle
                    if let Err(e) = app.emit("toggle-dictation-request", ()) {
                        tracing::error!("[GlobalShortcut] Failed to emit toggle-dictation-request event: {}", e);
                    }
                }
            }

            // Handle dictation input shortcut (Alt+Space / Option+Space)
            if let Some(ref input_shortcut) = dictation_input_shortcut {
                if shortcut == input_shortcut {
                    // Handle dictation input with timing logic
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if event.state() == ShortcutState::Pressed {
                            crate::dictation_monitor::on_dictation_input_pressed().await;
                        } else if event.state() == ShortcutState::Released {
                            crate::dictation_monitor::on_dictation_input_released(&app_clone).await;
                        }
                    });
                }
            }
        }).build())
        .manage(app_state) // Manage the AppState
        .invoke_handler(tauri::generate_handler![
            // Use re-exported commands
            list_apps,
            check_server_status,
            submit_query,
            anthropic::clear_conversation_history, // Add conversation history clearing
            commands::test_system_context, // Test system context gathering
            // Orchestrator Commands
            submit_orchestrated_query,
            get_orchestrator_status,
            configure_orchestrator,
            get_task_history,
            get_active_tasks,
            get_agent_capabilities,
            anthropic::cleanup_browser, // Add browser cleanup function
            tts::invoke_tts, // Use the main invoke_tts command for Tauri
            tts::set_tts_provider_command, // Added for TTS provider selection
            tts::get_tts_provider_command, // Added for TTS provider selection
            tts::stop_tts, // Added for stopping TTS via escape key
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
            get_agent_mode,
            set_agent_mode,
            // Dictation Settings Commands
            get_dictation_clipboard_enabled,
            set_dictation_clipboard_enabled,
            // Dictation Reset Commands
            force_reset_dictation_transcription,
            get_dictation_transcription_status,
            // Permissions Commands
            check_permissions_status,
            request_accessibility_permission,
            open_system_preferences,
            start_permissions_monitoring,
            stop_permissions_monitoring,
            // Enhanced Permissions Commands with Auto-Redirect
            check_permissions_status_with_auto_redirect,
            request_accessibility_permission_with_auto_redirect,
            open_system_settings_enhanced,
            restart_app_after_permissions,
            prompt_app_restart_after_permissions,
            check_restart_needed_after_permissions,
            // QA Test Commands from mouse.rs
            qa_test_click,
            qa_test_click_series,
            qa_test_coordinate_transformation,
            qa_test_click_visualization,
            qa_test_select_text,
            qa_test_scroll,
            // Sound Commands
            play_sound_by_type,
            play_sound_file,
            play_notification_sound,
            play_success_sound,
            play_error_sound,
            play_alert_sound,
            get_available_sounds,
            get_sound_enabled,
            set_sound_enabled,
            // Tool Configuration Commands
            get_tool_configurations,
            get_tool_config,
            set_tool_enabled,
            set_tool_category_enabled,
            get_enabled_tools,
            is_tool_enabled,
            reset_tool_configuration,
            get_tool_configuration_summary,
            // Floating Bar Commands
            floating_bar_click,
            floating_bar_focus_change,
            floating_bar_input_blur,
            floating_bar_input_change,
            floating_bar_submit,
            // Keyboard Shortcuts Commands
            get_keyboard_shortcuts,
            set_keyboard_shortcut,
            set_keyboard_shortcuts,
            reset_keyboard_shortcuts,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // --- Load Keyboard Shortcuts from Configuration ---
            let shortcuts_app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let app_state = shortcuts_app_handle.state::<state::AppState>();
                if let Err(e) = crate::commands::shortcuts::load_shortcuts_from_store(&shortcuts_app_handle, &*app_state).await {
                    tracing::warn!("Failed to load keyboard shortcuts from store: {}", e);
                    tracing::info!("Using default keyboard shortcuts");
                }
            });

            // --- Setup Application Menu ---
            // Juno Application Menu
            let about_menu_item = tauri::menu::MenuItemBuilder::new("About Juno")
                .id(constants::app_menu_ids::ABOUT)
                .build(app)?;

            let check_updates_menu_item = tauri::menu::MenuItemBuilder::new("Check for Updates...")
                .id(constants::app_menu_ids::CHECK_FOR_UPDATES)
                .build(app)?;

            let settings_menu_item = tauri::menu::MenuItemBuilder::new("Settings...")
                .id(constants::app_menu_ids::SETTINGS)
                .accelerator("CmdOrCtrl+,")
                .build(app)?;

            let app_submenu = tauri::menu::SubmenuBuilder::new(app, "Juno")
                .item(&about_menu_item)
                .separator()
                .item(&check_updates_menu_item)
                .separator()
                .item(&settings_menu_item)
                .separator()
                .services()
                .separator()
                .hide()
                .hide_others()
                .quit()
                .build()?;

            // File Menu
            let new_chat_menu_item = tauri::menu::MenuItemBuilder::new("New Chat")
                .id(constants::app_menu_ids::NEW_CHAT)
                .accelerator("CmdOrCtrl+N")
                .build(app)?;

            let clear_history_menu_item = tauri::menu::MenuItemBuilder::new("Clear History")
                .id(constants::app_menu_ids::CLEAR_HISTORY)
                .accelerator("CmdOrCtrl+Shift+Delete")
                .build(app)?;

            let import_chat_menu_item = tauri::menu::MenuItemBuilder::new("Import Chat...")
                .id(constants::app_menu_ids::IMPORT_CHAT)
                .accelerator("CmdOrCtrl+O")
                .build(app)?;

            let export_chat_menu_item = tauri::menu::MenuItemBuilder::new("Export Chat...")
                .id(constants::app_menu_ids::EXPORT_CHAT)
                .accelerator("CmdOrCtrl+S")
                .build(app)?;

            let file_submenu = SubmenuBuilder::new(app, "File")
                .item(&new_chat_menu_item)
                .separator()
                .item(&clear_history_menu_item)
                .separator()
                .item(&import_chat_menu_item)
                .item(&export_chat_menu_item)
                .build()?;

            // Edit Menu with standard keyboard shortcuts
            let edit_submenu = SubmenuBuilder::new(app, "Edit")
                .item(&PredefinedMenuItem::undo(app, None)?)
                .item(&PredefinedMenuItem::redo(app, None)?)
                .separator()
                .item(&PredefinedMenuItem::cut(app, None)?)
                .item(&PredefinedMenuItem::copy(app, None)?)
                .item(&PredefinedMenuItem::paste(app, None)?)
                .separator()
                .item(&PredefinedMenuItem::select_all(app, None)?)
                .build()?;

            // View Menu
            let toggle_floating_bar_menu_item = tauri::menu::MenuItemBuilder::new("Toggle Floating Bar")
                .id(constants::app_menu_ids::TOGGLE_FLOATING_BAR)
                .accelerator("CmdOrCtrl+B")
                .build(app)?;

            let toggle_dev_panel_menu_item = tauri::menu::MenuItemBuilder::new("Toggle Developer Panel")
                .id(constants::app_menu_ids::TOGGLE_DEV_PANEL)
                .accelerator("CmdOrCtrl+Shift+D")
                .build(app)?;

            let show_devtools_menu_item = tauri::menu::MenuItemBuilder::new("Developer Tools")
                .id(constants::app_menu_ids::SHOW_DEVTOOLS)
                .accelerator("CmdOrCtrl+Alt+I")
                .build(app)?;

            let show_permissions_menu_item = tauri::menu::MenuItemBuilder::new("Permissions...")
                .id(constants::app_menu_ids::SHOW_PERMISSIONS)
                .build(app)?;

            let toggle_fullscreen_menu_item = tauri::menu::MenuItemBuilder::new("Toggle Full Screen")
                .id(constants::app_menu_ids::TOGGLE_FULLSCREEN)
                .accelerator("CmdOrCtrl+Ctrl+F")
                .build(app)?;

            let view_submenu = SubmenuBuilder::new(app, "View")
                .item(&toggle_floating_bar_menu_item)
                .item(&toggle_dev_panel_menu_item)
                .separator()
                .item(&show_devtools_menu_item)
                .item(&show_permissions_menu_item)
                .separator()
                .item(&toggle_fullscreen_menu_item)
                .build()?;

            // Window Menu
            let minimize_menu_item = tauri::menu::MenuItemBuilder::new("Minimize")
                .id(constants::app_menu_ids::MINIMIZE)
                .accelerator("CmdOrCtrl+M")
                .build(app)?;

            let zoom_menu_item = tauri::menu::MenuItemBuilder::new("Zoom")
                .id(constants::app_menu_ids::ZOOM)
                .build(app)?;

            let bring_all_to_front_menu_item = tauri::menu::MenuItemBuilder::new("Bring All to Front")
                .id(constants::app_menu_ids::BRING_ALL_TO_FRONT)
                .build(app)?;

            let window_submenu = SubmenuBuilder::new(app, "Window")
                .item(&minimize_menu_item)
                .item(&zoom_menu_item)
                .separator()
                .item(&bring_all_to_front_menu_item)
                .build()?;

            // Help Menu
            let help_menu_item = tauri::menu::MenuItemBuilder::new("Juno Help")
                .id(constants::app_menu_ids::HELP)
                .accelerator("CmdOrCtrl+?")
                .build(app)?;

            let keyboard_shortcuts_menu_item = tauri::menu::MenuItemBuilder::new("Keyboard Shortcuts")
                .id(constants::app_menu_ids::KEYBOARD_SHORTCUTS)
                .accelerator("CmdOrCtrl+/")
                .build(app)?;

            let send_feedback_menu_item = tauri::menu::MenuItemBuilder::new("Send Feedback...")
                .id(constants::app_menu_ids::SEND_FEEDBACK)
                .build(app)?;

            let report_issue_menu_item = tauri::menu::MenuItemBuilder::new("Report Issue...")
                .id(constants::app_menu_ids::REPORT_ISSUE)
                .build(app)?;

            let visit_website_menu_item = tauri::menu::MenuItemBuilder::new("Visit Website")
                .id(constants::app_menu_ids::VISIT_WEBSITE)
                .build(app)?;

            let help_submenu = SubmenuBuilder::new(app, "Help")
                .item(&help_menu_item)
                .item(&keyboard_shortcuts_menu_item)
                .separator()
                .item(&send_feedback_menu_item)
                .item(&report_issue_menu_item)
                .separator()
                .item(&visit_website_menu_item)
                .build()?;

            let app_menu = tauri::menu::MenuBuilder::new(app)
                .items(&[&app_submenu, &file_submenu, &edit_submenu, &view_submenu, &window_submenu, &help_submenu])
                .build()?;

            app.set_menu(app_menu)?;

            // Listen for menu events
            let app_handle_for_menu = app_handle.clone();
            app.on_menu_event(move |_app, event| {
                match event.id().as_ref() {
                    // Juno Menu
                    constants::app_menu_ids::ABOUT => {
                        info!("[Menu] About menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit("about-requested", ()) {
                            tracing::error!("[Menu] Failed to emit about event: {}", e);
                        }
                    }
                    constants::app_menu_ids::CHECK_FOR_UPDATES => {
                        info!("[Menu] Check for Updates menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::UPDATE_CHECK_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit update check event: {}", e);
                        }
                    }
                    constants::app_menu_ids::SETTINGS => {
                        info!("[Menu] Settings menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::SETTINGS_REQUESTED, "/settings") {
                            tracing::error!("[Menu] Failed to emit settings event: {}", e);
                        }
                    }

                    // File Menu
                    constants::app_menu_ids::NEW_CHAT => {
                        info!("[Menu] New Chat menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::NEW_CHAT_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit new chat event: {}", e);
                        }
                    }
                    constants::app_menu_ids::CLEAR_HISTORY => {
                        info!("[Menu] Clear History menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::CLEAR_HISTORY_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit clear history event: {}", e);
                        }
                    }
                    constants::app_menu_ids::IMPORT_CHAT => {
                        info!("[Menu] Import Chat menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::IMPORT_CHAT_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit import chat event: {}", e);
                        }
                    }
                    constants::app_menu_ids::EXPORT_CHAT => {
                        info!("[Menu] Export Chat menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::EXPORT_CHAT_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit export chat event: {}", e);
                        }
                    }

                    // View Menu
                    constants::app_menu_ids::TOGGLE_FLOATING_BAR => {
                        info!("[Menu] Toggle Floating Bar menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::TOGGLE_FLOATING_BAR_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit toggle floating bar event: {}", e);
                        }
                    }
                    constants::app_menu_ids::TOGGLE_DEV_PANEL => {
                        info!("[Menu] Toggle Dev Panel menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::TOGGLE_DEV_PANEL_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit toggle dev panel event: {}", e);
                        }
                    }
                    constants::app_menu_ids::SHOW_DEVTOOLS => {
                        info!("[Menu] Developer Tools menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::DEVTOOLS_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit devtools event: {}", e);
                        }
                    }
                    constants::app_menu_ids::SHOW_PERMISSIONS => {
                        info!("[Menu] Permissions menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::PERMISSIONS_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit permissions event: {}", e);
                        }
                    }
                    constants::app_menu_ids::TOGGLE_FULLSCREEN => {
                        info!("[Menu] Toggle Fullscreen menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::TOGGLE_FULLSCREEN_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit toggle fullscreen event: {}", e);
                        }
                    }

                    // Window Menu
                    constants::app_menu_ids::MINIMIZE => {
                        info!("[Menu] Minimize menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::MINIMIZE_WINDOW_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit minimize event: {}", e);
                        }
                    }
                    constants::app_menu_ids::ZOOM => {
                        info!("[Menu] Zoom menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::ZOOM_WINDOW_REQUESTED, ()) {
                            tracing::error!("[Menu] Failed to emit zoom event: {}", e);
                        }
                    }
                    constants::app_menu_ids::BRING_ALL_TO_FRONT => {
                        info!("[Menu] Bring All to Front menu item clicked");
                        // This is handled automatically by macOS for most cases
                        info!("[Menu] Bring All to Front executed");
                    }

                    // Help Menu
                    constants::app_menu_ids::HELP => {
                        info!("[Menu] Help menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::HELP_REQUESTED, "general") {
                            tracing::error!("[Menu] Failed to emit help event: {}", e);
                        }
                    }
                    constants::app_menu_ids::KEYBOARD_SHORTCUTS => {
                        info!("[Menu] Keyboard Shortcuts menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::HELP_REQUESTED, "shortcuts") {
                            tracing::error!("[Menu] Failed to emit keyboard shortcuts event: {}", e);
                        }
                    }
                    constants::app_menu_ids::SEND_FEEDBACK => {
                        info!("[Menu] Send Feedback menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::FEEDBACK_REQUESTED, "feedback") {
                            tracing::error!("[Menu] Failed to emit feedback event: {}", e);
                        }
                    }
                    constants::app_menu_ids::REPORT_ISSUE => {
                        info!("[Menu] Report Issue menu item clicked");
                        if let Err(e) = app_handle_for_menu.emit(constants::events::FEEDBACK_REQUESTED, "issue") {
                            tracing::error!("[Menu] Failed to emit report issue event: {}", e);
                        }
                    }
                    constants::app_menu_ids::VISIT_WEBSITE => {
                        info!("[Menu] Visit Website menu item clicked");
                        // Open website in default browser
                        if let Err(e) = open::that("https://github.com/juno-ai") {
                            tracing::error!("[Menu] Failed to open website: {}", e);
                        }
                    }

                    _ => {
                        info!("[Menu] Unhandled menu event: {:?}", event.id());
                    }
                }
            });

            // --- Setup Enhanced Tray Icon ---
            let tray_app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                // Load the embedded icon data - no file system dependencies
                let loaded_tauri_icon = match image::load_from_memory(TRAY_ICON_DATA) {
                    Ok(dynamic_image) => {
                        let width = dynamic_image.width();
                        let height = dynamic_image.height();
                        let rgba_image = dynamic_image.to_rgba8();
                        let bytes = rgba_image.into_raw();
                        let img = TauriImage::new_owned(bytes, width, height);
                        Some(img)
                    },
                    Err(e) => {
                        eprintln!("[Tray Setup Error] Failed to load embedded tray icon: {}", e);
                        None
                    }
                };

                // Create enhanced tray menu with better organization
                let show_main_window_item = MenuItemKind::MenuItem(tauri::menu::MenuItem::with_id(&tray_app_handle, constants::tray_menu_ids::SHOW_MAIN_WINDOW, "Show Juno", true, None::<&str>).unwrap());
                let new_chat_item = MenuItemKind::MenuItem(tauri::menu::MenuItem::with_id(&tray_app_handle, constants::tray_menu_ids::NEW_CHAT, "New Chat", true, None::<&str>).unwrap());
                let toggle_item = MenuItemKind::MenuItem(tauri::menu::MenuItem::with_id(&tray_app_handle, constants::tray_menu_ids::TOGGLE_FLOATING_BAR, "Toggle Floating Bar", true, None::<&str>).unwrap());
                let devtools_item = MenuItemKind::MenuItem(tauri::menu::MenuItem::with_id(&tray_app_handle, constants::tray_menu_ids::SHOW_DEVTOOLS, "Developer Tools", true, None::<&str>).unwrap());
                let settings_item = MenuItemKind::MenuItem(tauri::menu::MenuItem::with_id(&tray_app_handle, constants::tray_menu_ids::SETTINGS, "Settings...", true, None::<&str>).unwrap());
                let quit_item = MenuItemKind::MenuItem(tauri::menu::MenuItem::with_id(&tray_app_handle, constants::tray_menu_ids::QUIT, "Quit Juno", true, None::<&str>).unwrap());

                let tray_menu = Menu::with_items(&tray_app_handle, &[
                    &show_main_window_item,
                    &new_chat_item,
                    &MenuItemKind::Predefined(tauri::menu::PredefinedMenuItem::separator(&tray_app_handle).unwrap()),
                    &toggle_item,
                    &devtools_item,
                    &MenuItemKind::Predefined(tauri::menu::PredefinedMenuItem::separator(&tray_app_handle).unwrap()),
                    &settings_item,
                    &MenuItemKind::Predefined(tauri::menu::PredefinedMenuItem::separator(&tray_app_handle).unwrap()),
                    &quit_item,
                ]).map_err(|e| eprintln!("[Tray Setup Error] Failed to create tray menu: {}", e)).ok();

                let mut tray_builder = TrayIconBuilder::new()
                    .on_menu_event(move |app_handle, event| {
                        match event.id().as_ref() {
                            constants::tray_menu_ids::QUIT => {
                                println!("[Tray Menu] Quit requested.");
                                app_handle.exit(0);
                            }
                            constants::tray_menu_ids::SHOW_MAIN_WINDOW => {
                                println!("[Tray Menu] Show main window requested.");
                                if let Some(window) = app_handle.get_webview_window(constants::window_labels::MAIN) {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                    let _ = window.unminimize();
                                } else {
                                    eprintln!("[Tray Menu Error] Main window not found.");
                                }
                            }
                            constants::tray_menu_ids::NEW_CHAT => {
                                println!("[Tray Menu] New chat requested.");
                                if let Err(e) = app_handle.emit(constants::events::NEW_CHAT_REQUESTED, ()) {
                                    tracing::error!("[Tray Menu] Failed to emit new chat event: {}", e);
                                }
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
                            constants::tray_menu_ids::SHOW_DEVTOOLS => {
                                info!("[Tray Menu] Developer Tools menu item clicked");
                                if let Err(e) = app_handle.emit(constants::events::DEVTOOLS_REQUESTED, ()) {
                                    tracing::error!("[Tray Menu] Failed to emit devtools-requested event: {}", e);
                                }
                            }
                            constants::tray_menu_ids::SETTINGS => {
                                info!("[Tray Menu] Settings menu item clicked");
                                if let Err(e) = app_handle.emit(constants::events::SETTINGS_REQUESTED, "/settings") {
                                    tracing::error!("[Tray Menu] Failed to emit settings-requested event: {}", e);
                                }
                            }
                            _ => {
                                println!("[Tray Menu] Unhandled tray menu event: {:?}", event.id());
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

            // --- Old bar-state-changed listener removed - now handled by floating bar manager ---

            // --- macOS Specific Setup for Floating Bar ---
            #[cfg(target_os = "macos")]
            {
                info!("Applying macOS specific setup...");
                if let Some(window) = app_handle.get_webview_window(constants::window_labels::FLOATING_BAR) {
                    info!("Found floating-bar for macOS setup.");
                    // --- Apply Standard Window Styling ---
                    match window.ns_window() {
                        Ok(ns_window_ptr) => {
                            let ns_window = ns_window_ptr as cocoa_id;
                            unsafe {
                                // Keep window floating above others - Use integer value for Floating level
                                ns_window.setLevel_(5); // kCGFloatingWindowLevelKey is typically 5
                                // Allow clicks to pass through transparent areas
                                ns_window.setOpaque_(NO);
                                ns_window.setHasShadow_(NO); // Optional: remove shadow if desired
                                // Keep it visible across spaces
                                ns_window.setCollectionBehavior_(
                                    NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces |
                                    NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary | // Keeps it stationary during space switching
                                    NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle // Exclude from Cmd+` cycle
                                );

                                // Set initial ignore state based on visibility (handled by tray logic, but good initial state)
                                if !window.is_visible().unwrap_or(false) {
                                     #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
                                     let _: BOOL = msg_send![ns_window, setIgnoresMouseEvents: YES];
                                     info!("macOS Setup: Floating bar initially hidden, ignoring mouse events.");
                                } else {
                                     #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
                                     let _: BOOL = msg_send![ns_window, setIgnoresMouseEvents: NO];
                                     info!("macOS Setup: Floating bar initially visible, accepting mouse events.");
                                }
                                info!("macOS standard styling applied to floating-bar.");
                            }
                        }
                        Err(e) => {
                            eprintln!("Error getting NSWindow for styling floating-bar: {}", e);
                        }
                    }
                     // --- Setup Mouse Tracking ---
                    macos_tracking::setup_tracking_area(&window, app_handle.clone());

                } else {
                    eprintln!("Warning: floating-bar window not found during macOS specific setup.");
                }
            }
            // --- End macOS Specific Setup ---

            // --- Play Application Boot Sound ---
            let app_handle_for_boot_sound = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Small delay to ensure UI is ready
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

                let state = app_handle_for_boot_sound.state::<crate::state::AppState>();
                let app_handle_clone = app_handle_for_boot_sound.clone();
                if let Err(e) = crate::commands::sound::play_notification_sound(app_handle_clone, state).await {
                    warn!("Failed to play boot sound: {}", e);
                } else {
                    info!("Boot sound played successfully from backend");
                }
            });
            // --- End Boot Sound ---

            // --- Initialize Multi-Agent Orchestrator ---
            let app_handle_for_orchestrator = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = commands::orchestrator::init_orchestrator_with_app_handle(app_handle_for_orchestrator).await {
                    tracing::error!("[Setup] Failed to initialize orchestrator system: {}", e);
                } else {
                    tracing::info!("[Setup] Multi-agent orchestrator system initialized successfully");
                }
            });

            // --- Initialize Floating Bar Manager ---
            let app_handle_for_bar_manager = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                commands::floating_bar::initialize_bar_manager(app_handle_for_bar_manager).await;
                tracing::info!("[Setup] Floating bar manager initialized successfully");
            });

            let app_handle_shortcuts = app.handle().clone(); // Use a new clone for shortcuts
            tauri::async_runtime::spawn(async move {
                let state = app_handle_shortcuts.state::<state::AppState>();

                // Load keyboard shortcuts from configuration store
                if let Err(e) = commands::shortcuts::load_shortcuts_from_store(&app_handle_shortcuts, &state).await {
                    warn!("[GlobalShortcut] Failed to load shortcuts from file: {}", e);
                }

                // Register keyboard shortcuts based on current configuration
                if let Err(e) = commands::shortcuts::update_global_shortcuts(&app_handle_shortcuts, &state).await {
                    error!("[GlobalShortcut] Failed to register shortcuts: {}", e);
                }

                info!("[GlobalShortcut] Keyboard shortcuts initialized from configuration");

                // Initialize dictation input monitoring system
                if let Err(e) = crate::dictation_monitor::init_dictation_input_monitoring(app_handle_shortcuts.clone()).await {
                    tracing::error!("[Setup] Failed to initialize dictation input monitoring: {}", e);
                } else {
                    info!("[Setup] Dictation input monitoring system initialized successfully");
                }
            });

            // Listen for dictation started events from the plugin
            let app_handle_for_listener = app.handle().clone();
            app.listen("voice-transcription:dictation-started", move |event| {
                info!("[Event] Received voice-transcription:dictation-started event");

                // Update floating bar manager
                let app_handle_clone = app_handle_for_listener.clone();
                tauri::async_runtime::spawn(async move {
                    commands::floating_bar::handle_dictation_started(&app_handle_clone).await;
                });

                // Rebroadcast the event as app-dictation-started for backward compatibility
                if let Err(e) = app_handle_for_listener.emit("app-dictation-started", event.payload()) {
                    tracing::error!("[Event] Failed to rebroadcast dictation-started event: {}", e);
                }
            });

            // Listen for partial result events from the plugin
            let app_handle_for_listener = app.handle().clone();
            app.listen("voice-transcription:partial-result", move |event| {
                info!("[Event] Received voice-transcription:partial-result event: {:?}", event.payload());

                // Extract partial text and update floating bar manager
                let payload_str = event.payload();
                if let Ok(payload_json) = serde_json::from_str::<serde_json::Value>(payload_str) {
                    if let Some(text_value) = payload_json.get("text") {
                        if let Some(text) = text_value.as_str() {
                            let app_handle_clone = app_handle_for_listener.clone();
                            let partial_text = text.to_string();
                            tauri::async_runtime::spawn(async move {
                                commands::floating_bar::handle_dictation_partial(&app_handle_clone, partial_text).await;
                            });
                        }
                    }
                }

                // Rebroadcast the event as app-dictation-partial-result for backward compatibility
                if let Err(e) = app_handle_for_listener.emit("app-dictation-partial-result", event.payload()) {
                    tracing::error!("[Event] Failed to rebroadcast partial-result event: {}", e);
                }
            });

            // Listen for final result events from the plugin (for AI Agent Mode only)
            let app_handle_for_listener = app.handle().clone();
            app.listen("voice-transcription:final-result", move |event| {
                info!("[Event] Received voice-transcription:final-result event: {:?}", event.payload());

                // Check if Dictation Mode is active - if so, skip AI agent processing
                // This prevents AI processing when user is doing immediate voice-to-text
                let app_state = app_handle_for_listener.state::<state::AppState>();
                let is_dictation_active = app_state.dictation_active.lock()
                    .map(|active| *active)
                    .unwrap_or(false);

                // Extract text from payload for floating bar manager
                let payload_str = event.payload();
                let extracted_text = match serde_json::from_str::<serde_json::Value>(payload_str) {
                    Ok(payload_json) => {
                        payload_json.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                    }
                    Err(_) => None,
                };

                if is_dictation_active {
                    info!("[Event] Processing final result for Dictation Mode");

                    // Update floating bar manager for dictation mode completion
                    let app_handle_clone = app_handle_for_listener.clone();
                    tauri::async_runtime::spawn(async move {
                        commands::floating_bar::handle_dictation_finished(&app_handle_clone, None).await;
                    });
                    return; // Exit early, let dictation handler process this
                }

                info!("[Event] Processing final result for AI Agent Mode");

                // Update floating bar manager for agent mode query
                if let Some(text) = &extracted_text {
                    let app_handle_clone = app_handle_for_listener.clone();
                    let query_text = text.clone();
                    tauri::async_runtime::spawn(async move {
                        commands::floating_bar::handle_dictation_finished(&app_handle_clone, Some(query_text)).await;
                    });
                }

                // Transform the payload from { "text": "..." } to { "query": "..." } format expected by frontend
                match serde_json::from_str::<serde_json::Value>(payload_str) {
                    Ok(payload_json) => {
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

            // Listen for dictation transcription start events (immediate)
            let app_handle_for_dictation_start = app.handle().clone();
            app.listen("dictation-transcription-start", move |_event| {
                info!("[Event] Received dictation-transcription-start event - starting immediate transcription");

                // Start dictation using the voice transcription plugin command
                let app_handle_clone = app_handle_for_dictation_start.clone();
                tauri::async_runtime::spawn(async move {
                    // Use the plugin command to start dictation only if controller exists
                    match app_handle_clone.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
                        Some(controller_state) => {
                            match tauri_plugin_voice_transcription::commands::start_dictation(
                                app_handle_clone.clone(),
                                controller_state
                            ).await {
                                Ok(()) => {
                                    info!("[Dictation Mode] Started immediate transcription successfully");
                                    // Mark this as Dictation Mode in AppState
                                    let app_state = app_handle_clone.state::<state::AppState>();
                                    if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                                        *dictation_active = true;
                                    }

                                    // Update floating bar manager
                                    let app_handle_for_bar = app_handle_clone.clone();
                                    tauri::async_runtime::spawn(async move {
                                        commands::floating_bar::handle_dictation_mode_change(&app_handle_for_bar, true).await;
                                    });

                                    if let Err(e) = app_handle_clone.emit("dictation-active", true) {
                                        tracing::error!("[Dictation Mode] Failed to emit dictation-active event: {}", e);
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("[Dictation Mode] Failed to start transcription: {}", e);

                                    // Clean up state if start failed
                                    let app_state = app_handle_clone.state::<state::AppState>();
                                    if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                                        *dictation_active = false;
                                    }

                                    // Update floating bar manager
                                    let app_handle_for_bar = app_handle_clone.clone();
                                    tauri::async_runtime::spawn(async move {
                                        commands::floating_bar::handle_dictation_mode_change(&app_handle_for_bar, false).await;
                                    });

                                    // Reset dictation input monitor state
                                    crate::dictation_monitor::force_reset_dictation_input_state().await;

                                    // Emit failure event to UI
                                    if let Err(e) = app_handle_clone.emit("dictation-active", false) {
                                        tracing::error!("[Dictation Mode] Failed to emit dictation-active event after start failure: {}", e);
                                    }
                                }
                            }
                        }
                        None => {
                            tracing::warn!("[Dictation Mode] Voice controller not available - cannot start transcription");

                            // Clean up state since start failed
                            let app_state = app_handle_clone.state::<state::AppState>();
                            if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                                *dictation_active = false;
                            }

                            // Update floating bar manager
                            let app_handle_for_bar = app_handle_clone.clone();
                            tauri::async_runtime::spawn(async move {
                                commands::floating_bar::handle_dictation_mode_change(&app_handle_for_bar, false).await;
                            });

                            // Reset dictation input monitor state
                            crate::dictation_monitor::force_reset_dictation_input_state().await;

                            // Emit failure event to UI
                            if let Err(e) = app_handle_clone.emit("dictation-active", false) {
                                tracing::error!("[Dictation Mode] Failed to emit dictation-active event after unavailable voice controller: {}", e);
                            }
                        }
                    }
                });
            });

            // Listen for Dictation Mode commitment events (threshold reached)
            let _app_handle_for_dictation_committed = app.handle().clone();
            app.listen("dictation-committed", move |_event| {
                info!("[Event] Received dictation-committed event - threshold reached");
                // This event indicates the user has held dictation input long enough to commit to dictation
                // We can use this for additional UI feedback if needed
                // The transcription is already running, so we just acknowledge the commitment
            });

            // Listen for dictation transcription cancellation events (released before threshold)
            let app_handle_for_dictation_cancel = app.handle().clone();
            app.listen("dictation-transcription-cancel", move |_event| {
                info!("[Event] Received dictation-transcription-cancel event - cancelling transcription");

                // Stop dictation and discard results
                let app_handle_clone = app_handle_for_dictation_cancel.clone();
                tauri::async_runtime::spawn(async move {
                    // Use the plugin command to stop dictation only if controller exists
                    match app_handle_clone.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
                        Some(controller_state) => {
                            match tauri_plugin_voice_transcription::commands::stop_dictation(
                                app_handle_clone.clone(),
                                controller_state
                            ).await {
                                Ok(_) => {
                                    info!("[Dictation Mode] Cancelled transcription successfully");
                                }
                                Err(e) => {
                                    tracing::error!("[Dictation Mode] Failed to cancel transcription: {}", e);
                                }
                            }
                        }
                        None => {
                            tracing::warn!("[Dictation Mode] Voice controller not available - cannot cancel transcription");
                        }
                    }

                    // Always clean up state regardless of stop_dictation result
                    let app_state = app_handle_clone.state::<state::AppState>();
                    if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                        *dictation_active = false;
                    }

                    // Update floating bar manager
                    let app_handle_for_bar = app_handle_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        commands::floating_bar::handle_dictation_mode_change(&app_handle_for_bar, false).await;
                    });

                    if let Err(e) = app_handle_clone.emit("dictation-active", false) {
                        tracing::error!("[Dictation Mode] Failed to emit dictation-active event: {}", e);
                    }
                });
            });

            // Listen for Dictation Mode stop events (normal completion)
            let app_handle_for_dictation_stop = app.handle().clone();
            app.listen("dictation-stop", move |_event| {
                info!("[Event] Received dictation-stop event - completing dictation normally");

                // Stop dictation using the voice transcription plugin command
                let app_handle_clone = app_handle_for_dictation_stop.clone();
                tauri::async_runtime::spawn(async move {
                    // Use the plugin command to stop dictation only if controller exists
                    match app_handle_clone.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
                        Some(controller_state) => {
                            match tauri_plugin_voice_transcription::commands::stop_dictation(
                                app_handle_clone.clone(),
                                controller_state
                            ).await {
                                Ok(_) => {
                                    info!("[Dictation Mode] Completed dictation successfully");
                                }
                                Err(e) => {
                                    tracing::error!("[Dictation Mode] Failed to stop dictation: {}", e);
                                }
                            }
                        }
                        None => {
                            tracing::warn!("[Dictation Mode] Voice controller not available - cannot stop dictation");
                        }
                    }

                    // Always clean up state regardless of stop_dictation result
                    let app_state = app_handle_clone.state::<state::AppState>();
                    if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                        *dictation_active = false;
                    }

                    // Update floating bar manager
                    let app_handle_for_bar = app_handle_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        commands::floating_bar::handle_dictation_mode_change(&app_handle_for_bar, false).await;
                    });

                    if let Err(e) = app_handle_clone.emit("dictation-active", false) {
                        tracing::error!("[Dictation Mode] Failed to emit dictation-active event: {}", e);
                    }
                });
            });

            // Listen for force stop events (timeout/stuck transcription)
            let app_handle_for_force_stop = app.handle().clone();
            app.listen("dictation-transcription-force-stop", move |_event| {
                warn!("[Event] Received dictation-transcription-force-stop event - emergency cleanup");

                let app_handle_clone = app_handle_for_force_stop.clone();
                tauri::async_runtime::spawn(async move {
                    // Force stop the voice controller with timeout only if it exists
                    match app_handle_clone.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
                        Some(controller_state) => {
                            let stop_with_timeout = tokio::time::timeout(
                                std::time::Duration::from_secs(2),
                                tauri_plugin_voice_transcription::commands::stop_dictation(
                                    app_handle_clone.clone(),
                                    controller_state
                                )
                            );

                            match stop_with_timeout.await {
                                Ok(Ok(_)) => {
                                    info!("[Dictation Mode] Force stop completed successfully");
                                }
                                Ok(Err(e)) => {
                                    error!("[Dictation Mode] Force stop failed: {}", e);
                                }
                                Err(_) => {
                                    error!("[Dictation Mode] Force stop timed out - controller may be deadlocked");
                                }
                            }
                        }
                        None => {
                            warn!("[Dictation Mode] Voice controller not available - cannot force stop");
                        }
                    }

                    // Force clean up state
                    let app_state = app_handle_clone.state::<state::AppState>();
                    if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                        *dictation_active = false;
                    }

                    // Update floating bar manager
                    let app_handle_for_bar = app_handle_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        commands::floating_bar::handle_dictation_mode_change(&app_handle_for_bar, false).await;
                    });

                    if let Err(e) = app_handle_clone.emit("dictation-active", false) {
                        error!("[Dictation Mode] Failed to emit dictation-active event: {}", e);
                    }
                });
            });

            // Listen for force cleanup events (stuck state recovery)
            let app_handle_for_force_cleanup = app.handle().clone();
            app.listen("dictation-transcription-force-cleanup", move |_event| {
                warn!("[Event] Received dictation-transcription-force-cleanup event - recovering stuck state");

                let app_handle_clone = app_handle_for_force_cleanup.clone();
                tauri::async_runtime::spawn(async move {
                    // Reset dictation input monitor state
                    crate::dictation_monitor::force_reset_dictation_input_state().await;

                    // Force clean up app state
                    let app_state = app_handle_clone.state::<state::AppState>();
                    if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                        *dictation_active = false;
                    }

                    // Update floating bar manager
                    let app_handle_for_bar = app_handle_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        commands::floating_bar::handle_dictation_mode_change(&app_handle_for_bar, false).await;
                    });

                    // Emit cleanup complete event
                    if let Err(e) = app_handle_clone.emit("dictation-active", false) {
                        error!("[Dictation Mode] Failed to emit dictation-active event: {}", e);
                    }

                    info!("[Dictation Mode] Force cleanup completed");
                });
            });

            // Listen for voice transcription final results specifically for Dictation Mode
            let app_handle_for_dictation_result = app.handle().clone();
            app.listen("voice-transcription:final-result", move |event| {
                // Check if we're in Dictation Mode and handle immediate typing
                let app_handle_clone = app_handle_for_dictation_result.clone();
                tauri::async_runtime::spawn(async move {
                    let app_state = app_handle_clone.state::<state::AppState>();

                    // Check if Dictation Mode is active
                    let is_dictation_active = app_state.dictation_active.lock()
                        .map(|active| *active)
                        .unwrap_or(false);

                    if is_dictation_active {
                        info!("[Dictation Mode] Processing final result for immediate typing");

                        // Parse the transcription result
                        let payload_str = event.payload();
                        match serde_json::from_str::<serde_json::Value>(payload_str) {
                            Ok(payload_json) => {
                                if let Some(text_value) = payload_json.get("text") {
                                    if let Some(text) = text_value.as_str() {
                                        // Only type if the text is not empty and not just whitespace
                                        let trimmed_text = text.trim();
                                        if !trimmed_text.is_empty() {
                                            // Check if clipboard saving is enabled
                                            let clipboard_enabled = app_state.dictation_clipboard_enabled.lock()
                                                .map(|enabled| *enabled)
                                                .unwrap_or(true); // Default to true if lock fails

                                            // Store to clipboard if enabled
                                            if clipboard_enabled {
                                                match crate::commands::core::dev_set_clipboard(
                                                    trimmed_text.to_string(),
                                                    app_state.clone()
                                                ).await {
                                                    Ok(()) => {
                                                        info!("[Dictation Mode] Successfully stored text to clipboard: '{}'", trimmed_text);
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("[Dictation Mode] Failed to store text to clipboard: {}", e);
                                                    }
                                                }
                                            } else {
                                                info!("[Dictation Mode] Clipboard saving is disabled, skipping clipboard storage");
                                            }

                                            // Then type the transcribed text immediately using the computer use tools
                                            match crate::commands::keyboard::dev_global_type_text(
                                                trimmed_text.to_string(),
                                                app_state.clone()
                                            ).await {
                                                Ok(()) => {
                                                    info!("[Dictation Mode] Successfully typed text: '{}'", trimmed_text);
                                                }
                                                Err(e) => {
                                                    tracing::error!("[Dictation Mode] Failed to type transcribed text: {}", e);
                                                }
                                            }
                                        } else {
                                            info!("[Dictation Mode] Transcribed text was empty or whitespace only, skipping typing");
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("[Dictation Mode] Failed to parse final-result payload: {}", e);
                            }
                        }

                        // Reset Dictation Mode state after processing
                        if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                            *dictation_active = false;
                        }

                        // Emit state change event for UI
                        if let Err(e) = app_handle_clone.emit("dictation-active", false) {
                            error!("[Dictation Mode] Failed to emit dictation-active event after final result: {}", e);
                        }
                    }
                });
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
