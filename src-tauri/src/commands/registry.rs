//! Command registry and organization system
//!
//! This module provides organized command groupings and macros to reduce
//! boilerplate in Tauri command definitions while maintaining compatibility
//! with the invoke_handler! macro.

/// Macro to generate the complete invoke_handler! call with all commands organized by category
#[macro_export]
macro_rules! generate_invoke_handler {
    () => {
        tauri::generate_handler![
            // Core System Commands
            list_apps,
            check_server_status,
            test_system_context,

            // Agent Commands
            submit_query,
            submit_orchestrated_query,
            get_orchestrator_status,
            configure_orchestrator,
            create_orchestrator_task,
            get_task_history,
            get_active_tasks,
            get_agent_capabilities,
            cancel_task,

            // MCP Integration Commands
            get_mcp_tools,
            add_mcp_server,
            remove_mcp_server,
            start_mcp_server,
            stop_mcp_server,
            get_mcp_server_statuses,
            execute_mcp_task,

            // MCP Diagnostics Commands
            get_mcp_system_diagnostics,
            force_restart_all_mcp_servers,
            check_mcp_prerequisites,
            restart_mcp_server_with_diagnostics,
            troubleshoot_mcp_issues,
            apply_mcp_quick_fixes,

            // Workflow Commands
            get_workflow_templates,
            execute_workflow_template,

            // Anthropic-specific Commands
            crate::anthropic::clear_conversation_history,
            crate::anthropic::cleanup_browser,
            crate::anthropic::handle_tts_completion,

            // Mouse Commands
            dev_right_click,
            dev_middle_click,
            dev_double_click,
            dev_triple_click,
            dev_mouse_move,
            dev_left_mouse_down,
            dev_left_mouse_up,
            dev_left_click,
            dev_left_click_drag,
            dev_get_cursor_position,
            dev_window_relative_click,
            dev_focused_window_relative_click,

            // QA Test Commands
            qa_test_click,
            qa_test_click_series,
            qa_test_coordinate_transformation,
            qa_test_click_visualization,
            qa_test_select_text,
            qa_test_scroll,

            // Production Keyboard Commands
            type_text,
            press_key,
            hold_key,
            release_key,
            global_type_text,

            // Dev Keyboard Commands (for devtools)
            dev_type_text,
            dev_press_key,
            dev_hold_key,
            dev_release_key,
            dev_global_type_text,

            // Dev Network Commands
            crate::commands::dev::check_network_connectivity,
            crate::commands::dev::test_network_error_detection,

            // Window Commands
            dev_get_window_list,
            dev_get_window_info,
            dev_focus_window,
            open_application,
            open_url,
            dev_scroll_window,

            // Clipboard Commands
            dev_get_clipboard,
            dev_set_clipboard,

            // Element Commands
            dev_get_focused_element_info,
            dev_click_focused_element,
            dev_find_element_by_selector,
            dev_click_element_by_selector,
            dev_get_selected_text,
            capture_element_screenshot_command,

            // Screenshot Commands
            capture_screenshot_command,
            capture_window_screenshot_command,
            capture_focused_window_screenshot_command,

            // File System Commands
            list_files,
            get_file_content,
            set_file_content,

            // Text Editor Commands
            dev_text_editor_view,
            dev_text_editor_create,
            dev_text_editor_str_replace,
            dev_text_editor_insert,
            dev_text_editor_undo_edit,

            // Shell Commands
            bash_command,
            dev_wait,

            // Provider Commands
                    get_providers,
        get_active_provider,
        set_active_provider,
        validate_provider_model,
        get_provider_models,
            get_provider_settings,
            update_provider_api_key,
            update_provider_model,
            update_provider_max_tokens,
            update_provider_temperature,
            update_provider_system_prompt,
            get_agent_mode,
            set_agent_mode,

            // Permissions Commands - Native APIs Only (No Password Prompts)
            check_permissions_status_native,
            get_permissions_state,
            request_accessibility_permission_native,
            request_microphone_permission_native,
            request_screen_recording_permission_native,
            request_input_monitoring_permission_native,
            test_microphone_functionality,
            open_system_preferences,
            open_system_settings_enhanced,
            start_permissions_monitoring,
            stop_permissions_monitoring,
            restart_app_after_permissions,
            prompt_app_restart_after_permissions,
            check_restart_needed_after_permissions,
            handle_restart_after_permissions,

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

            // TTS Commands
            crate::tts::invoke_tts,
            crate::tts::set_tts_provider_command,
            crate::tts::get_tts_provider_command,

            // Tool Configuration Commands
            get_tool_configurations,
            get_tool_config,
            set_tool_enabled,
            set_tool_category_enabled,
            get_enabled_tools,
            is_tool_enabled,
            reset_tool_configuration,
            get_tool_configuration_summary,

            // Dictation Commands
            get_dictation_clipboard_enabled,
            set_dictation_clipboard_enabled,
            force_reset_dictation_transcription,
            get_dictation_transcription_status,

            // Floating Bar Commands
                    floating_bar_click,
        floating_bar_focus_change,
        floating_bar_input_blur,
        floating_bar_input_change,
        floating_bar_submit,
        get_floating_bar_config,
        set_floating_bar_config,

            // Core/Miscellaneous commands (screenshots, app list, clipboard, wait)
            list_ai_providers,
            set_ai_provider,
            get_agent_execution_progress,

            // Always Listening Commands
            get_always_listening_status,
            set_always_listening_status,
            toggle_always_listening_mode,
            get_always_listening_sensitivity,
            set_always_listening_sensitivity,
            get_always_listening_wake_words,
            set_always_listening_wake_words,

            // Notification Commands
            get_notification_settings,
            set_notification_type,
            set_notification_sound_enabled,
            set_notification_duration,
            set_notification_position,
            set_notification_show_icons,
            set_notification_persist_important,
            check_notification_permission,
            request_notification_permission,
            send_notification,
            test_notification,
        ]
    };
}

/// Command categories for documentation and organization
pub mod categories {
    /// Core system functionality
    pub const CORE: &[&str] = &[
        "list_apps",
        "check_server_status",
        "test_system_context"
    ];

    /// AI agent commands
    pub const AGENT: &[&str] = &[
        "submit_query",
        "submit_orchestrated_query",
        "get_orchestrator_status",
        "configure_orchestrator",
        "create_orchestrator_task",
        "get_task_history",
        "get_active_tasks",
        "get_agent_capabilities",
        "cancel_task"
    ];

    /// MCP integration commands (handled by commands/mcp.rs)
    pub const MCP: &[&str] = &[
        "get_mcp_tools",
        "add_mcp_server",
        "remove_mcp_server",
        "start_mcp_server",
        "stop_mcp_server",
        "get_mcp_servers",
        "get_mcp_server_statuses",
        "update_mcp_server",
        "set_mcp_server_enabled",
        "toggle_mcp_server",
        "toggle_mcp_tool",
        "test_mcp_server_connection",
        "initialize_mcp_servers",
        "get_mcp_diagnostics",
        "restart_mcp_server_with_diagnostics",
        "troubleshoot_mcp_issues",
        "apply_mcp_quick_fixes",
    ];

    /// Workflow orchestration commands (handled by commands/orchestrator.rs)
    pub const WORKFLOW: &[&str] = &[
        "get_workflow_templates",
        "execute_workflow_template",
        "execute_mcp_task"
    ];

    /// Memory management commands (handled by commands/memory.rs)
    pub const MEMORY: &[&str] = &[
        "get_memory_status",
        "clear_conversation_memory",
        "clean_orphaned_tool_calls",
        "get_conversation_messages",
        "get_last_n_messages"
    ];

    /// Mouse interaction commands
    pub const MOUSE: &[&str] = &[
        "dev_right_click",
        "dev_middle_click",
        "dev_double_click",
        "dev_triple_click",
        "dev_mouse_move",
        "dev_left_mouse_down",
        "dev_left_mouse_up",
        "dev_left_click",
        "dev_left_click_drag",
        "dev_get_cursor_position",
        "dev_window_relative_click",
        "dev_focused_window_relative_click",
    ];

    /// QA testing commands
    pub const QA_TEST: &[&str] = &[
        "qa_test_click",
        "qa_test_click_series",
        "qa_test_coordinate_transformation",
        "qa_test_click_visualization",
        "qa_test_select_text",
        "qa_test_scroll"
    ];

    /// Production keyboard commands
    pub const KEYBOARD: &[&str] = &[
        "type_text",
        "press_key",
        "hold_key",
        "release_key",
        "global_type_text"
    ];

    /// Development keyboard commands
    pub const DEV_KEYBOARD: &[&str] = &[
        "dev_type_text",
        "dev_press_key",
        "dev_hold_key",
        "dev_release_key",
        "dev_global_type_text"
    ];

    /// Window management commands
    pub const WINDOW: &[&str] = &[
        "dev_get_window_list",
        "dev_get_window_info",
        "dev_focus_window",
        "dev_open_application",
        "dev_open_url",
        "dev_scroll_window"
    ];

    /// All command categories
    pub const ALL_CATEGORIES: &[(&str, &[&str])] = &[
        ("Core", CORE),
        ("Agent", AGENT),
        ("MCP", MCP),
        ("Workflow", WORKFLOW),
        ("Memory", MEMORY),
        ("Mouse", MOUSE),
        ("QA Test", QA_TEST),
        ("Keyboard", KEYBOARD),
        ("Dev Keyboard", DEV_KEYBOARD),
        ("Window", WINDOW),
    ];
}

/// Get the total number of registered commands
pub fn get_command_count() -> usize {
    categories::ALL_CATEGORIES
        .iter()
        .map(|(_, commands)| commands.len())
        .sum()
}

/// Check if a command exists in any category
pub fn command_exists(command_name: &str) -> bool {
    categories::ALL_CATEGORIES
        .iter()
        .any(|(_, commands)| commands.contains(&command_name))
}

/// Get the category for a specific command
pub fn get_command_category(command_name: &str) -> Option<&'static str> {
    categories::ALL_CATEGORIES
        .iter()
        .find(|(_, commands)| commands.contains(&command_name))
        .map(|(category, _)| *category)
}
