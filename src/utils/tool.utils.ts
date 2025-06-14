// Tool classification functions
export const isScreenshotTool = (toolName: string): boolean => {
  const screenshotTools = [
    "screenshot",
    "take_screenshot",
    "capture_screen",
    "screen_capture",
  ];
  return screenshotTools.includes(toolName);
};

export const isFileOperationTool = (toolName: string): boolean => {
  const fileTools = [
    "read_file",
    "write_file",
    "delete_file",
    "list_directory",
    "create_directory",
    "move_file",
    "copy_file",
  ];
  return fileTools.includes(toolName);
};

export const isBrowserTool = (toolName: string): boolean => {
  const browserTools = [
    "open_url",
    "browser_action",
    "click_element",
    "type_text",
    "scroll_page",
  ];
  return browserTools.includes(toolName);
};

export const isSystemTool = (toolName: string): boolean => {
  const systemTools = [
    "run_command",
    "execute_shell",
    "system_info",
    "process_list",
    "kill_process",
  ];
  return systemTools.includes(toolName);
};

export const isImportantTool = (toolName: string): boolean => {
  return (
    isScreenshotTool(toolName) ||
    isFileOperationTool(toolName) ||
    isBrowserTool(toolName) ||
    isSystemTool(toolName)
  );
};

export const getFriendlyToolName = (toolName: string): string => {
  const friendlyNames: Record<string, string> = {
    // Basic tools
    echo: "Echo",
    say: "Text-to-Speech",
    notification: "Notification",
    delay: "Delay",
    random: "Random Number",

    // File operations
    read_file: "Read File",
    write_file: "Write File",
    delete_file: "Delete File",
    list_directory: "List Directory",
    create_directory: "Create Directory",
    move_file: "Move File",
    copy_file: "Copy File",
    search_files: "Search Files",
    get_file_info: "Get File Info",

    // Desktop control
    screenshot: "Take Screenshot",
    take_screenshot: "Take Screenshot",
    capture_screen: "Capture Screen",
    screen_capture: "Screen Capture",
    click: "Click",
    double_click: "Double Click",
    right_click: "Right Click",
    drag: "Drag",
    type: "Type Text",
    key: "Press Key",
    scroll: "Scroll",
    move_mouse: "Move Mouse",

    // Browser automation
    open_url: "Open URL",
    browser_action: "Browser Action",
    click_element: "Click Element",
    type_text: "Type Text",
    scroll_page: "Scroll Page",
    get_page_content: "Get Page Content",
    take_page_screenshot: "Take Page Screenshot",
    navigate_back: "Navigate Back",
    navigate_forward: "Navigate Forward",
    refresh_page: "Refresh Page",

    // System operations
    run_command: "Run Command",
    execute_shell: "Execute Shell",
    system_info: "System Info",
    process_list: "Process List",
    kill_process: "Kill Process",
    get_environment: "Get Environment",
    set_environment: "Set Environment",

    // Network operations
    http_request: "HTTP Request",
    download_file: "Download File",
    upload_file: "Upload File",
    ping: "Ping",
    resolve_dns: "Resolve DNS",

    // Memory and state
    remember: "Remember",
    recall: "Recall",
    forget: "Forget",
    get_context: "Get Context",
    set_context: "Set Context",

    // Time and scheduling
    get_time: "Get Time",
    set_timer: "Set Timer",
    cancel_timer: "Cancel Timer",
    schedule_task: "Schedule Task",

    // Development tools
    code_analysis: "Code Analysis",
    run_tests: "Run Tests",
    format_code: "Format Code",
    lint_code: "Lint Code",
    build_project: "Build Project",

    // AI and ML
    text_analysis: "Text Analysis",
    image_recognition: "Image Recognition",
    speech_recognition: "Speech Recognition",
    language_detection: "Language Detection",
    sentiment_analysis: "Sentiment Analysis",

    // Multimedia
    play_audio: "Play Audio",
    record_audio: "Record Audio",
    convert_audio: "Convert Audio",
    play_video: "Play Video",
    record_video: "Record Video",
    convert_video: "Convert Video",

    // Communication
    send_email: "Send Email",
    send_sms: "Send SMS",
    make_call: "Make Call",
    send_notification: "Send Notification",

    // Database operations
    query_database: "Query Database",
    insert_record: "Insert Record",
    update_record: "Update Record",
    delete_record: "Delete Record",

    // Cloud services
    upload_to_cloud: "Upload to Cloud",
    download_from_cloud: "Download from Cloud",
    sync_with_cloud: "Sync with Cloud",

    // Security
    encrypt_data: "Encrypt Data",
    decrypt_data: "Decrypt Data",
    generate_password: "Generate Password",
    hash_data: "Hash Data",

    // Monitoring
    monitor_system: "Monitor System",
    check_health: "Check Health",
    log_event: "Log Event",
    alert: "Alert",

    // Integration
    webhook: "Webhook",
    api_call: "API Call",
    oauth_login: "OAuth Login",
    sync_data: "Sync Data",
  };

  return friendlyNames[toolName] || toolName.replace(/_/g, " ").replace(/\b\w/g, (l) => l.toUpperCase());
};