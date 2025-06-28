//! # Error Message Constants
//!
//! Centralized error messages and patterns used throughout the application.
//! This helps maintain consistency and makes error handling more maintainable.

// Common error message patterns for string matching
pub mod patterns {
    // Permission-related error patterns
    pub const PERMISSION_DENIED: &str = "permission denied";
    pub const ACCESS_DENIED: &str = "access denied";
    pub const PERMISSION_REQUIRED: &str = "permission required";

    // Network-related error patterns
    pub const CONNECTION_REFUSED: &str = "connection refused";
    pub const CONNECTION_FAILED: &str = "connection failed";
    pub const NETWORK_UNREACHABLE: &str = "network unreachable";
    pub const TIMEOUT: &str = "timeout";
    pub const TIMED_OUT: &str = "timed out";

    // File system error patterns
    pub const NOT_FOUND: &str = "not found";
    pub const DOES_NOT_EXIST: &str = "does not exist";
    pub const FILE_NOT_FOUND: &str = "file not found";
    pub const DIRECTORY_NOT_FOUND: &str = "directory not found";

    // Application-specific error patterns
    pub const ELEMENT_NOT_FOUND: &str = "element not found";
    pub const PAGE_NOT_FOUND: &str = "page not found";
    pub const BROWSER_NOT_AVAILABLE: &str = "browser not available";
    pub const AGENT_EXECUTION_FAILED: &str = "agent execution failed";
}

// Standard error messages for user display
pub mod user_messages {
    // Permission error messages
    pub const ACCESSIBILITY_PERMISSION_REQUIRED: &str =
        "Accessibility permission is required for this operation";
    pub const PERMISSION_DENIED_GENERIC: &str =
        "Permission denied. Please check your system permissions.";

    // Network error messages
    pub const CONNECTION_TIMEOUT: &str =
        "The operation timed out. Please check your internet connection.";
    pub const SERVER_UNREACHABLE: &str = "Unable to connect to the server. Please try again later.";

    // File operation error messages
    pub const FILE_READ_ERROR: &str =
        "Unable to read the file. Please check if it exists and you have permission.";
    pub const FILE_WRITE_ERROR: &str =
        "Unable to write to the file. Please check permissions and disk space.";

    // Agent operation error messages
    pub const AGENT_TIMEOUT: &str = "The agent operation timed out. Please try again.";
    pub const AGENT_UNAVAILABLE: &str =
        "The AI agent is currently unavailable. Please try again later.";

    // Browser operation error messages
    pub const BROWSER_CONNECTION_FAILED: &str =
        "Failed to connect to the browser. Please ensure it's running.";
    pub const ELEMENT_INTERACTION_FAILED: &str = "Failed to interact with the specified element.";
}

// Technical error messages for logging/debugging
pub mod technical_messages {
    // System errors
    pub const FAILED_TO_ACQUIRE_LOCK: &str = "Failed to acquire mutex lock";
    pub const THREAD_SPAWN_FAILED: &str = "Failed to spawn background thread";
    pub const CHANNEL_SEND_FAILED: &str = "Failed to send message through channel";
    pub const CHANNEL_RECV_FAILED: &str = "Failed to receive message from channel";

    // Resource errors
    pub const MEMORY_ALLOCATION_FAILED: &str = "Failed to allocate memory";
    pub const DISK_SPACE_INSUFFICIENT: &str = "Insufficient disk space";
    pub const FILE_HANDLE_UNAVAILABLE: &str = "Unable to obtain file handle";

    // Network errors
    pub const TCP_CONNECTION_FAILED: &str = "TCP connection establishment failed";
    pub const HTTP_REQUEST_FAILED: &str = "HTTP request failed";
    pub const WEBSOCKET_CONNECTION_FAILED: &str = "WebSocket connection failed";

    // Agent-specific errors
    pub const TOOL_EXECUTION_FAILED: &str = "Tool execution failed";
    pub const CONTEXT_SWITCH_FAILED: &str = "Failed to switch agent context";
    pub const MEMORY_CORRUPTION_DETECTED: &str = "Agent memory corruption detected";
}

// Error codes for structured error handling
pub mod error_codes {
    // Permission-related error codes
    pub const ERR_PERMISSION_DENIED: &str = "ERR_PERMISSION_DENIED";
    pub const ERR_ACCESS_DENIED: &str = "ERR_ACCESS_DENIED";

    // Network-related error codes
    pub const ERR_CONNECTION_TIMEOUT: &str = "ERR_CONNECTION_TIMEOUT";
    pub const ERR_CONNECTION_REFUSED: &str = "ERR_CONNECTION_REFUSED";
    pub const ERR_NETWORK_UNREACHABLE: &str = "ERR_NETWORK_UNREACHABLE";

    // File system error codes
    pub const ERR_FILE_NOT_FOUND: &str = "ERR_FILE_NOT_FOUND";
    pub const ERR_DIRECTORY_NOT_FOUND: &str = "ERR_DIRECTORY_NOT_FOUND";
    pub const ERR_INSUFFICIENT_SPACE: &str = "ERR_INSUFFICIENT_SPACE";

    // Agent operation error codes
    pub const ERR_AGENT_TIMEOUT: &str = "ERR_AGENT_TIMEOUT";
    pub const ERR_TOOL_EXECUTION_FAILED: &str = "ERR_TOOL_EXECUTION_FAILED";
    pub const ERR_CONTEXT_SWITCH_FAILED: &str = "ERR_CONTEXT_SWITCH_FAILED";

    // Browser operation error codes
    pub const ERR_BROWSER_CONNECTION_FAILED: &str = "ERR_BROWSER_CONNECTION_FAILED";
    pub const ERR_ELEMENT_NOT_FOUND: &str = "ERR_ELEMENT_NOT_FOUND";
    pub const ERR_PAGE_LOAD_TIMEOUT: &str = "ERR_PAGE_LOAD_TIMEOUT";
}

// Recovery suggestions for different error types
pub mod recovery_suggestions {
    pub const PERMISSION_DENIED: &str =
        "Try running with elevated privileges or check system permissions";
    pub const CONNECTION_TIMEOUT: &str = "Check your internet connection and try again";
    pub const FILE_NOT_FOUND: &str = "Verify the file path and ensure the file exists";
    pub const BROWSER_CONNECTION_FAILED: &str =
        "Ensure the browser is running and restart if necessary";
    pub const AGENT_TIMEOUT: &str =
        "The operation may be complex - try breaking it into smaller steps";
}

// Tool-specific error messages
pub mod tool_errors {
    // Permission error messages
    pub const SCREEN_RECORDING_PERMISSION_REQUIRED: &str = "Screen recording permission is required for screenshots";
    pub const ACCESSIBILITY_PERMISSION_REQUIRED_MOUSE: &str = "Accessibility permission is required for mouse operations";
    pub const ACCESSIBILITY_PERMISSION_REQUIRED_KEYBOARD: &str = "Accessibility permission is required for keyboard operations";
    pub const ACCESSIBILITY_PERMISSION_REQUIRED_SCROLL: &str = "Accessibility permission is required for scroll operations";

    // Computer tool errors
    pub const MISSING_ACTION_PARAMETER: &str = "Missing 'action' parameter";
    pub const MISSING_COORDINATE_PARAMETER: &str = "Missing 'coordinate' parameter";
    pub const INVALID_X_COORDINATE: &str = "Invalid x coordinate";
    pub const INVALID_Y_COORDINATE: &str = "Invalid y coordinate";
    pub const INVALID_START_X_COORDINATE: &str = "Invalid start x coordinate";
    pub const INVALID_START_Y_COORDINATE: &str = "Invalid start y coordinate";
    pub const INVALID_END_X_COORDINATE: &str = "Invalid end x coordinate";
    pub const INVALID_END_Y_COORDINATE: &str = "Invalid end y coordinate";
    pub const MISSING_KEY_PARAMETER: &str = "Missing 'key' or 'text' parameter";
    pub const MISSING_TEXT_PARAMETER: &str = "Missing 'text' parameter";
    pub const MISSING_DURATION_PARAMETER: &str = "Missing 'duration_ms' or 'duration' parameter";
    pub const MISSING_SCROLL_DIRECTION_PARAMETER: &str = "Missing 'scroll_direction' parameter";
    pub const MISSING_SECONDS_PARAMETER: &str = "Missing 'seconds' or 'duration' parameter";
    pub const MISSING_COORDINATE_PARAMETER_FOR_DRAG: &str = "Missing 'coordinate' parameter for drag operation";

    // Bash tool errors
    pub const MISSING_COMMAND_PARAMETER: &str = "Missing 'command' parameter";

    // str_replace_tool errors
    pub const MISSING_PATH_PARAMETER: &str = "Missing 'path' parameter";
    pub const MISSING_OLD_STR_PARAMETER: &str = "Missing 'old_str' parameter";
    pub const MISSING_NEW_STR_PARAMETER: &str = "Missing 'new_str' parameter";
    pub const MISSING_FILE_TEXT_PARAMETER: &str = "Missing 'file_text' parameter";

    // Security validation errors
    pub const PATH_TRAVERSAL_NOT_ALLOWED: &str = "Path traversal not allowed";
    pub const HOME_DIRECTORY_ACCESS_NOT_ALLOWED: &str = "Home directory access not allowed";
    pub const LINE_NUMBERS_ARE_ONE_INDEXED_START: &str = "Line numbers are 1-indexed, start_line cannot be 0";
    pub const LINE_NUMBERS_ARE_ONE_INDEXED_END: &str = "Line numbers are 1-indexed, end_line cannot be 0";
    pub const START_LINE_MUST_BE_LESS_THAN_END: &str = "Start line must be less than end line";

    // File operation errors
    pub const FILE_ALREADY_EXISTS: &str = "File already exists";
    pub const STRING_NOT_FOUND_IN_FILE: &str = "String not found in file";

    // Unreachable error messages
    pub const MOUSE_ACTION_ALREADY_MATCHED: &str = "Mouse action already matched in outer pattern";
    pub const KEYBOARD_ACTION_ALREADY_MATCHED: &str = "Keyboard action already matched in outer pattern";
}

// Tool-specific success messages
pub mod tool_success {
    pub const SCREENSHOT_SUCCESS: &str = "✅ Screenshot captured successfully";
    pub const CLICK_SUCCESS: &str = "✅ Click operation completed successfully";
    pub const TYPE_SUCCESS: &str = "✅ Text typing completed successfully";
    pub const KEY_PRESS_SUCCESS: &str = "✅ Key press completed successfully";
    pub const SCROLL_SUCCESS: &str = "✅ Scroll operation completed successfully";
    pub const MOUSE_MOVE_SUCCESS: &str = "✅ Mouse movement completed successfully";
    pub const DRAG_SUCCESS: &str = "✅ Drag operation completed successfully";
    pub const WAIT_SUCCESS: &str = "✅ Wait operation completed successfully";
    pub const BASH_COMMAND_SUCCESS: &str = "✅ Bash command executed successfully";
    pub const FILE_CREATED_SUCCESSFULLY: &str = "Successfully created file";
    pub const TEXT_REPLACED_SUCCESSFULLY: &str = "Successfully replaced text in";
}

// Format string constants for dynamic error messages
pub mod format_strings {
    // File operation format strings
    pub const FILE_EXTENSION_NOT_ALLOWED: &str = "File extension '{}' not allowed";
    pub const FILE_SIZE_EXCEEDS_LIMIT: &str = "File size {} bytes exceeds limit of {} bytes";
    pub const START_LINE_EXCEEDS_FILE_LENGTH: &str = "Start line {} exceeds file length of {} lines";
    pub const END_LINE_EXCEEDS_FILE_LENGTH: &str = "End line {} exceeds file length of {} lines";
    pub const FAILED_TO_READ_FILE: &str = "Failed to read file '{}': {}";
    pub const FAILED_TO_WRITE_FILE: &str = "Failed to write file '{}': {}";
    pub const FAILED_TO_CREATE_FILE: &str = "Failed to create file '{}': {}";
    pub const FAILED_TO_CREATE_DIRECTORIES: &str = "Failed to create directories for '{}': {}";
    pub const FILE_ALREADY_EXISTS: &str = "File '{}' already exists";
    pub const STRING_NOT_FOUND_IN_FILE: &str = "String '{}' not found in file '{}'";

    // Tool operation format strings
    pub const COMPUTER_ACTION_EXECUTION: &str = "🖥️ Computer Use: {} → {}";
    pub const PERMISSION_VALIDATION_FAILED: &str = "Permission validation failed: {}";
    pub const SCREENSHOT_FAILED: &str = "Screenshot failed: {}";
    pub const LEFT_CLICK_FAILED: &str = "Left click failed: {}";
    pub const RIGHT_CLICK_FAILED: &str = "Right click failed: {}";
    pub const MIDDLE_CLICK_FAILED: &str = "Middle click failed: {}";
    pub const DOUBLE_CLICK_FAILED: &str = "Double click failed: {}";
    pub const TRIPLE_CLICK_FAILED: &str = "Triple click failed: {}";
    pub const LEFT_CLICK_DRAG_FAILED: &str = "Left click drag failed: {}";
    pub const MOUSE_MOVE_FAILED: &str = "Mouse move failed: {}";
    pub const TYPE_TEXT_FAILED: &str = "Type text failed: {}";
    pub const KEY_PRESS_FAILED: &str = "Key press failed: {}";
    pub const SCROLL_FAILED: &str = "Scroll failed: {}";
    pub const HOLD_KEY_FAILED: &str = "Hold key failed: {}";
    pub const RELEASE_KEY_FAILED: &str = "Release key failed: {}";
    pub const LEFT_MOUSE_DOWN_FAILED: &str = "Left mouse down failed: {}";
    pub const LEFT_MOUSE_UP_FAILED: &str = "Left mouse up failed: {}";
    pub const WAIT_FAILED: &str = "Wait failed: {}";
    pub const BASH_COMMAND_FAILED: &str = "Bash command failed: {}";
    pub const UNKNOWN_STR_REPLACE_COMMAND: &str = "Unknown str_replace_based_edit_tool command: {}";
    pub const PARSE_BASH_RESULT_FAILED: &str = "Failed to parse bash command result as JSON: '{}'. Raw result was: '{}'";
    pub const UNKNOWN_ACTION: &str = "Unknown action: {}";

    // Logging format strings
    pub const EXECUTING_COMPUTER_ACTION: &str = "Executing computer action: {}";
    pub const RAW_BASH_COMMAND_RESULT: &str = "Raw bash_command result: {}";
    pub const PARSED_BASH_RESULT: &str = "Parsed bash result - stdout: '{}', stderr: '{}', exit_code: {}, success: {}";
    pub const MISSING_INVALID_STDOUT: &str = "Missing or invalid 'stdout' field in bash command result: {}";
    pub const MISSING_INVALID_EXIT_CODE: &str = "Missing or invalid 'exit_code' field in bash command result: {}";

    // Success message format strings
    pub const SUCCESSFULLY_CREATED_FILE: &str = "Successfully created file '{}'";
    pub const SUCCESSFULLY_REPLACED_TEXT_IN: &str = "Successfully replaced text in '{}'";
    pub const TOOL_COMPLETED_SUCCESSFULLY: &str = "✅ {} completed successfully in {}ms";
    pub const TOOL_FAILED: &str = "❌ {} failed";
    pub const SUCCESSFULLY_REGISTERED_TOOLS: &str = "Successfully registered {} official Anthropic Computer Use tools";

    // Additional format strings for JSON parsing and bash results
    pub const FAILED_TO_PARSE_JSON_RESULT: &str = "Failed to parse bash command result as JSON. Error: {}, Raw result: '{}'";
    pub const MISSING_INVALID_STDERR: &str = "Missing or invalid 'stderr' field in bash command result: {}";
}
