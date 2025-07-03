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
            // === CORE SYSTEM COMMANDS ===
            list_apps,
            check_server_status,
            test_system_context,

            // === AI AGENT COMMANDS ===
            submit_query,
            submit_orchestrated_query,

            // === ANTHROPIC COMPUTER USE TOOLS (OFFICIAL API) ===
            // All mouse, keyboard, and screen interaction now handled by:
            // - computer tool (all mouse/keyboard/screen operations)
            // - bash tool (shell commands)
            // - str_replace_based_edit_tool (file operations)
            // Per official Anthropic Computer Use specification

            // === ORCHESTRATOR COMMANDS ===
            get_orchestrator_status,
            configure_orchestrator,
            create_orchestrator_task,
            get_task_history,
            get_active_tasks,
            get_agent_capabilities,
            cancel_task,

            // === MCP INTEGRATION COMMANDS ===
            get_mcp_tools,
            add_mcp_server,
            remove_mcp_server,
            start_mcp_server,
            stop_mcp_server,
            get_mcp_servers,
            get_mcp_server_statuses,
            update_mcp_server,
            set_mcp_server_enabled,
            toggle_mcp_server,
            toggle_mcp_tool,
            test_mcp_server_connection,
            initialize_mcp_servers,
            get_mcp_diagnostics,
            restart_mcp_server_with_diagnostics,
            troubleshoot_mcp_issues,
            apply_mcp_quick_fixes,

            // === WORKFLOW ORCHESTRATION ===
            get_workflow_templates,
            execute_workflow_template,
            execute_mcp_task,

            // === MEMORY MANAGEMENT ===
            get_memory_status,
            clear_conversation_memory,
            clean_orphaned_tool_calls,
            clean_orphaned_tool_results,
            get_conversation_messages,
            get_last_n_messages,
            get_visual_summaries,
            update_visual_config,
            get_visual_config,
            compress_all_screenshots,
            configure_screenshot_compression,
            get_memory_compression_stats,
            emergency_memory_recovery,
            get_conversation_summaries,
            optimize_memory,
            get_memory_config,
            update_memory_config,
            get_advanced_memory_metrics,
            force_memory_prune,
            get_tiered_memory_context,

            // === PRODUCTION SYSTEM COMMANDS ===
            // Core screenshot and system operations
            capture_screenshot_command,
            capture_window_screenshot_command,
            capture_focused_window_screenshot_command,

            // Production mouse operations (minimal set for system functions)
            get_cursor_position,

            // Production keyboard operations (minimal set for system functions)
            type_text,
            press_key,
            hold_key,
            release_key,
            global_type_text,

            // Production window operations
            scroll_window,
            get_window_list,
            get_window_info,
            focus_window,
            resize_window,
            move_window,
            close_window,

            // Production shell operations
            bash_command,

            // Element and system operations
            get_focused_element_info,
            click_focused_element,
            find_element_by_selector,
            click_element_by_selector,
            get_selected_text,

            // Application management
            open_application,
            open_url,

            // System utilities
            wait,
            get_clipboard,
            set_clipboard,



            // === SYSTEM MANAGEMENT ===
            // Permissions and security
            check_accessibility_permission,
            check_screen_recording_permission,
            request_accessibility_permission,
            request_screen_recording_permission,
            get_permission_status,

            // Error recovery and debugging
            get_error_recovery_status,
            clear_error_recovery_history,
            get_debug_info,
            test_debug_tools,

            // Voice and transcription
            transcribe_audio,
            test_voice_recognition,
            get_voice_transcription_status,
            set_voice_transcription_enabled,
            get_voice_transcription_settings,
            set_voice_transcription_settings,

            // System monitoring
            get_system_stats,
            get_hardware_info,

            // === CONFIGURATION COMMANDS ===
            // Settings management
            get_settings,
            set_setting,
            reset_settings,
            export_settings,
            import_settings,

            // Provider management
            get_providers,
            set_provider,
            get_provider_models,
            set_provider_model,
            test_provider_connection,

            // Tool Configuration Commands
            get_tool_configurations,
            get_tool_config,
            get_registered_tools,
            test_dynamic_tool_categorization,
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

            // Legacy floating bar commands removed - use new UI API instead
            // UI interactions handled through ui_handle_interaction

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
        "clean_orphaned_tool_results",
        "get_conversation_messages",
        "get_last_n_messages",
        "get_visual_summaries",
        "update_visual_config",
        "get_visual_config",
        "compress_all_screenshots",
        "configure_screenshot_compression",
        "get_memory_compression_stats",
        "emergency_memory_recovery",
        "get_conversation_summaries",
        "optimize_memory",
        "get_memory_config",
        "update_memory_config",
        "get_advanced_memory_metrics",
        "force_memory_prune",
        "get_tiered_memory_context"
    ];

    /// Mouse interaction commands (minimal set - most operations use computer tool)
    pub const MOUSE: &[&str] = &[
        "get_cursor_position",
    ];



    /// Production keyboard commands (minimal set - most operations use computer tool)
    pub const KEYBOARD: &[&str] = &[
        "type_text",
        "press_key",
        "hold_key",
        "release_key",
        "global_type_text"
    ];

    /// Window management commands (minimal set - scrolling uses computer tool)
    pub const WINDOW: &[&str] = &[
        "get_window_list",
        "get_window_info",
        "focus_window",
        "scroll_window",
        "resize_window",
        "move_window",
        "close_window"
    ];

    /// All command categories
    pub const ALL_CATEGORIES: &[(&str, &[&str])] = &[
        ("Core", CORE),
        ("Agent", AGENT),
        ("MCP", MCP),
        ("Workflow", WORKFLOW),
        ("Memory", MEMORY),
        ("Mouse", MOUSE),
        ("Keyboard", KEYBOARD),
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
