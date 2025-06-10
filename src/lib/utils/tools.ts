// Tool categorization functions
export const isScreenshotTool = (toolName: string): boolean => {
  const screenshotTools = [
    "screenshot",
    "screen",
    "capture",
    "element_screenshot",
    "full_screen_screenshot",
  ];
  return screenshotTools.some((tool) =>
    toolName.toLowerCase().includes(tool.toLowerCase())
  );
};

export const isFileOperationTool = (toolName: string): boolean => {
  const fileTools = [
    "read_file",
    "write_file",
    "create_file",
    "delete_file",
    "save_agent_response",
    "save_file",
    "file",
  ];
  return fileTools.some((tool) =>
    toolName.toLowerCase().includes(tool.toLowerCase())
  );
};

export const isBrowserTool = (toolName: string): boolean => {
  const browserTools = ["open_url", "navigate", "browser", "web", "url"];
  return browserTools.some((tool) =>
    toolName.toLowerCase().includes(tool.toLowerCase())
  );
};

export const isSystemTool = (toolName: string): boolean => {
  const systemTools = ["restart", "shutdown", "system", "permission"];
  return systemTools.some((tool) =>
    toolName.toLowerCase().includes(tool.toLowerCase())
  );
};

export const isImportantTool = (toolName: string): boolean => {
  return (
    isSystemTool(toolName) ||
    isFileOperationTool(toolName) ||
    isScreenshotTool(toolName)
  );
};

export const getFriendlyToolName = (toolName: string): string => {
  const friendlyNames: Record<string, string> = {
    // Screenshot tools
    screenshot: "📸 Screenshot",
    screen: "📸 Screen Capture",
    capture: "📸 Capture",
    element_screenshot: "📸 Element Screenshot",
    full_screen_screenshot: "📸 Full Screen Screenshot",

    // Click and interaction tools
    click: "👆 Click",
    double_click: "👆 Double Click",
    right_click: "👆 Right Click",
    left_click_drag: "👆 Click & Drag",
    scroll: "📜 Scroll",
    type: "⌨️ Type Text",
    key: "⌨️ Key Press",

    // File operations
    read_file: "📖 Read File",
    write_file: "📝 Write File",
    create_file: "📄 Create File",
    delete_file: "🗑️ Delete File",
    save_agent_response: "💾 Save Response",
    save_file: "💾 Save File",

    // Browser tools
    open_url: "🌐 Open URL",
    navigate: "🧭 Navigate",
    browser: "🌐 Browser",

    // Window management
    list_windows: "🪟 List Windows",
    get_window_info: "🪟 Window Info",
    focus_window: "🪟 Focus Window",

    // System tools
    restart: "🔄 Restart",
    shutdown: "⚡ Shutdown",
    permission: "🔐 Permission",

    // Cursor and positioning
    cursor_position: "🎯 Cursor Position",
    move_cursor: "🎯 Move Cursor",

    // Text and clipboard
    get_selected_text: "📋 Get Selected Text",
    clipboard: "📋 Clipboard",

    // Application control
    open_application: "🚀 Open App",
    close_application: "❌ Close App",

    // Search and find
    find_element: "🔍 Find Element",
    search: "🔍 Search",

    // Waiting and timing
    wait: "⏳ Wait",
    sleep: "💤 Sleep",
    timer: "⏲️ Timer",

    // Voice and audio
    dictation: "🎤 Dictation",
    voice: "🎤 Voice",
    tts: "🔊 Text-to-Speech",

    // Development tools
    run_command: "⚡ Run Command",
    execute: "⚡ Execute",
    compile: "🔨 Compile",
    build: "🔨 Build",

    // Network and connectivity
    ping: "📡 Ping",
    download: "⬇️ Download",
    upload: "⬆️ Upload",

    // Security and authentication
    login: "🔐 Login",
    logout: "🔓 Logout",
    authenticate: "🔐 Authenticate",

    // Database operations
    query: "🗃️ Query",
    insert: "📥 Insert",
    update: "📝 Update",
    delete: "🗑️ Delete",

    // Email and messaging
    send_email: "📧 Send Email",
    message: "💬 Message",

    // Calendar and scheduling
    schedule: "📅 Schedule",
    calendar: "📅 Calendar",
    reminder: "⏰ Reminder",

    // Media and graphics
    resize_image: "🖼️ Resize Image",
    convert: "🔄 Convert",
    edit: "✏️ Edit",

    // Analytics and monitoring
    monitor: "📊 Monitor",
    track: "📈 Track",
    analyze: "🧪 Analyze",

    // Configuration and settings
    configure: "⚙️ Configure",
    settings: "⚙️ Settings",
    preference: "⚙️ Preference",
  };

  // Try to find exact match first
  if (friendlyNames[toolName]) {
    return friendlyNames[toolName];
  }

  // Try to find partial match
  for (const [key, value] of Object.entries(friendlyNames)) {
    if (toolName.toLowerCase().includes(key.toLowerCase())) {
      return value;
    }
  }

  // If no match found, capitalize the tool name and add a generic icon
  return `🔧 ${toolName
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ")}`;
};

// Notification utilities
export const getNotificationDuration = (
  notificationLevel: string,
  estimatedDuration?: string
): number => {
  // Base durations in milliseconds
  const baseDurations = {
    low: 3000, // 3 seconds
    medium: 5000, // 5 seconds
    high: 8000, // 8 seconds
    critical: 12000, // 12 seconds
  };

  let duration = baseDurations.medium; // Default

  // Set duration based on notification level
  if (notificationLevel === "low") duration = baseDurations.low;
  else if (notificationLevel === "medium") duration = baseDurations.medium;
  else if (notificationLevel === "high") duration = baseDurations.high;
  else if (notificationLevel === "critical") duration = baseDurations.critical;

  // Adjust based on estimated duration if provided
  if (estimatedDuration) {
    if (estimatedDuration.includes("quick") || estimatedDuration.includes("fast")) {
      duration = Math.max(duration - 2000, 2000); // Reduce but keep minimum
    } else if (estimatedDuration.includes("slow") || estimatedDuration.includes("long")) {
      duration += 3000; // Increase for longer operations
    }
  }

  return duration;
};

export const getNotificationClassName = (
  toolCategory?: string,
  success?: boolean
): string => {
  let className = "border-l-4 ";

  // Color based on success/failure
  if (success === false) {
    className += "border-red-500 bg-red-50 text-red-800 dark:bg-red-900/20 dark:text-red-200";
  } else if (success === true) {
    className += "border-green-500 bg-green-50 text-green-800 dark:bg-green-900/20 dark:text-green-200";
  } else {
    // Color based on tool category
    if (toolCategory === "screenshot" || isScreenshotTool(toolCategory || "")) {
      className += "border-blue-500 bg-blue-50 text-blue-800 dark:bg-blue-900/20 dark:text-blue-200";
    } else if (toolCategory === "browser" || isBrowserTool(toolCategory || "")) {
      className += "border-purple-500 bg-purple-50 text-purple-800 dark:bg-purple-900/20 dark:text-purple-200";
    } else if (toolCategory === "system" || isSystemTool(toolCategory || "")) {
      className += "border-orange-500 bg-orange-50 text-orange-800 dark:bg-orange-900/20 dark:text-orange-200";
    } else if (toolCategory === "file" || isFileOperationTool(toolCategory || "")) {
      className += "border-yellow-500 bg-yellow-50 text-yellow-800 dark:bg-yellow-900/20 dark:text-yellow-200";
    } else {
      // Default styling
      className += "border-gray-500 bg-gray-50 text-gray-800 dark:bg-gray-900/20 dark:text-gray-200";
    }
  }

  return className;
};