import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area"; // To handle potentially large JSON
import { Separator } from "@/components/ui/separator";
import { Textarea } from "@/components/ui/textarea"; // Added for Set File Content
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event"; // Import listen for tool usage events
import {
  AppWindow, // Example icon
  ArrowUpDown, // Example icon
  Clipboard, // Added
  ClipboardPaste, // Example icon
  ExternalLink,
  FileEdit,
  FileText, // Added for Wait
  Focus, // Added for Close Window
  Folder, // Added
  Hand, // Added for Wait
  Info, // Example icon
  Keyboard,
  Layers, // Added for Window List
  Maximize2,
  Mouse, // Example icon (replace as needed)
  MousePointerClick, // Added for Wait
  Move, // Added for Hold/Release
  TextSelect, // Added for Get Selected Text
  Timer,
  X,
} from "lucide-react"; // Import some icons
import React, { useEffect, useRef, useState } from "react";
import { toast } from "sonner";

// Helper type for tracking loading states
type LoadingStates = {
  screenshot: boolean;
  focusInfo: boolean;
  focusDelay: boolean;
  elementScreenshot: boolean;
  clickFocus: boolean;
  typeText: boolean;
  pressKey: boolean;
  openApp: boolean;
  openUrl: boolean;
  scroll: boolean;
  // New loading states
  globalTypeText: boolean;
  getClipboard: boolean;
  setClipboard: boolean;
  holdKey: boolean;
  releaseKey: boolean;
  wait: boolean;
  findElement: boolean;
  clickElement: boolean;
  getSelectedText: boolean; // Added
  getWindowList: boolean; // Added
  getWindowInfo: boolean; // Added
  focusWindow: boolean; // Added
  resizeWindow: boolean; // Added
  moveWindow: boolean; // Added
  closeWindow: boolean; // Added
  listFiles: boolean; // Added
  getFileContent: boolean;
  setFileContent: boolean; // Added
  mouseMove: boolean; // Added
  mouseDown: boolean; // Added
  mouseUp: boolean; // Added
  mouseClick: boolean; // Added
  mouseDoubleClick: boolean; // Added
  mouseDrag: boolean; // Added
  testClickVisualization: boolean; // Added for click visualization testing
};

// Helper type for file listing result (assuming backend sends this structure)
// Adjust based on actual backend implementation
type FileEntry = {
  name: string;
  is_dir: boolean;
  // Add other relevant fields like size, modified date if needed
};

// Type for tool usage events
type ToolUsageEntry = {
  timestamp: number;
  tool: string;
  inputs: Record<string, any>;
  result?: any;
  success: boolean;
  screenshot_base64?: string; // Optional screenshot data
};

// Default loading states
const initialLoadingStates: LoadingStates = {
  screenshot: false,
  focusInfo: false,
  focusDelay: false,
  elementScreenshot: false,
  clickFocus: false,
  typeText: false,
  pressKey: false,
  openApp: false,
  openUrl: false,
  scroll: false,
  globalTypeText: false,
  getClipboard: false,
  setClipboard: false,
  holdKey: false,
  releaseKey: false,
  wait: false,
  findElement: false,
  clickElement: false,
  getSelectedText: false,
  getWindowList: false,
  getWindowInfo: false,
  focusWindow: false,
  resizeWindow: false,
  moveWindow: false,
  closeWindow: false,
  listFiles: false,
  getFileContent: false,
  setFileContent: false,
  mouseMove: false,
  mouseDown: false,
  mouseUp: false,
  mouseClick: false,
  mouseDoubleClick: false,
  mouseDrag: false,
  testClickVisualization: false,
};

// Type for result data from QA tests
type ClickQAResult = {
  success: boolean;
  operation: string;
  coordinates: [number, number];
  original_coordinates?: [number, number];
  error?: string;
  visualization_success: boolean;
  cursor_position_after?: [number, number];
  latency_ms: number;
};

// Type for coordinate transformation test results
type CoordinateTestResult = {
  original: { x: number; y: number };
  transformed_to_screen: { x: number; y: number };
  transformed_back: { x: number; y: number };
  error: { x: number; y: number };
  scaling_info?: any;
  is_accurate: boolean;
};

// Type for visualization test results
type VisualizationTestResult = {
  test: string;
  results: Array<{
    position: { x: number; y: number };
    color: string;
    success: boolean;
    error?: string;
  }>;
  success_rate: number;
};

const DevToolsPanel: React.FC = () => {
  const [screenshotSrc, setScreenshotSrc] = useState<string | null>(null);
  const [focusedElementInfo, setFocusedElementInfo] = useState<string | null>(
    null
  );
  const [elementScreenshotSrc, setElementScreenshotSrc] = useState<
    string | null
  >(null);
  const [loadingStates, setLoadingStates] =
    useState<LoadingStates>(initialLoadingStates);
  // Input states
  const [textToType, setTextToType] = useState<string>("Hello from DevTools!");
  const [keyToPress, setKeyToPress] = useState<string>("Return"); // e.g., Return, Tab, cmd+s
  const [appToOpen, setAppToOpen] = useState<string>("TextEdit");
  const [urlToOpen, setUrlToOpen] = useState<string>("https://www.google.com");
  // New input states
  const [globalTextToType, setGlobalTextToType] =
    useState<string>("Global text");
  const [clipboardContent, setClipboardContent] = useState<string>(""); // For setting
  const [clipboardResult, setClipboardResult] = useState<string | null>(null); // For displaying get result
  const [modifierKey, setModifierKey] = useState<string>("shift"); // For hold/release
  const [waitDuration, setWaitDuration] = useState<string>("1000"); // Wait duration in ms

  // Added state for selector-based actions
  const [selectorString, setSelectorString] = useState<string>("button:OK");
  const [findElementResult, setFindElementResult] = useState<string | null>(
    null
  );
  const [selectedTextResult, setSelectedTextResult] = useState<string | null>(
    null
  ); // Added
  const [windowListResult, setWindowListResult] = useState<string | null>(null); // Added
  const [windowIdInput, setWindowIdInput] = useState<string>(""); // Added
  const [windowInfoResult, setWindowInfoResult] = useState<string | null>(null); // Added
  const [windowIdFocus, setWindowIdFocus] = useState<string>(""); // Added
  const [windowIdResize, setWindowIdResize] = useState<string>(""); // Added
  const [windowWidth, setWindowWidth] = useState<string>("800"); // Added
  const [windowHeight, setWindowHeight] = useState<string>("600"); // Added
  const [windowIdMove, setWindowIdMove] = useState<string>(""); // Added
  const [windowX, setWindowX] = useState<string>("100"); // Added
  const [windowY, setWindowY] = useState<string>("100"); // Added
  const [windowIdClose, setWindowIdClose] = useState<string>(""); // Added
  const [pathToList, setPathToList] = useState<string>("~"); // Added, default to home
  const [fileListResult, setFileListResult] = useState<string | null>(null); // Added
  const [pathGetContent, setPathGetContent] = useState<string>(""); // Added
  const [fileContentResult, setFileContentResult] = useState<string | null>(
    null
  ); // Added
  const [pathSetContent, setPathSetContent] = useState<string>(""); // Added
  const [fileContentToSet, setFileContentToSet] = useState<string>(""); // Added
  const [mouseX, setMouseX] = useState<string>("100"); // Added
  const [mouseY, setMouseY] = useState<string>("100"); // Added
  const [mouseButton, setMouseButton] = useState<"left" | "right" | "middle">(
    "left"
  ); // Added
  const [mouseStartX, setMouseStartX] = useState<string>("100"); // Added for drag
  const [mouseStartY, setMouseStartY] = useState<string>("100"); // Added for drag
  const [mouseEndX, setMouseEndX] = useState<string>("200"); // Added for drag
  const [mouseEndY, setMouseEndY] = useState<string>("200"); // Added for drag

  // Results states for QA tests
  const [qaClickResult, setQaClickResult] = useState<ClickQAResult | null>(
    null
  );
  const [qaClickSeriesResults, setQaClickSeriesResults] = useState<
    ClickQAResult[] | null
  >(null);
  const [coordinateTestResult, setCoordinateTestResult] =
    useState<CoordinateTestResult | null>(null);
  const [visualizationTestResult, setVisualizationTestResult] =
    useState<VisualizationTestResult | null>(null);

  // Input fields for QA tests
  const [qaClickType, setQaClickType] = useState<string>("left");
  const [qaClickX, setQaClickX] = useState<number>(400);
  const [qaClickY, setQaClickY] = useState<number>(300);

  // State for coordinate testing
  const [coordTestX, setCoordTestX] = useState("");
  const [coordTestY, setCoordTestY] = useState("");
  const [coordTestLoading, setCoordTestLoading] = useState(false);

  const delayTimeoutRef = useRef<NodeJS.Timeout | null>(null); // Ref to store timeout ID

  const [toolHistory, setToolHistory] = useState<ToolUsageEntry[]>([]);

  // Cleanup timeout on component unmount
  useEffect(() => {
    return () => {
      if (delayTimeoutRef.current) {
        clearTimeout(delayTimeoutRef.current);
      }
    };
  }, []);

  // Listen for tool usage events
  useEffect(() => {
    const unlisten = listen<ToolUsageEntry>("tool-usage", (event) => {
      console.log("Tool usage event received:", event.payload);
      setToolHistory((prev) => [event.payload, ...prev]);
    });

    return () => {
      unlisten.then((unlistenFn) => unlistenFn());
    };
  }, []);

  // Generic handler to invoke a command and update loading/error/info
  const invokeCommand = async <T = any,>(
    command: string,
    args?: any,
    loadingKey?: keyof LoadingStates
  ): Promise<T | null> => {
    if (loadingKey && loadingStates[loadingKey]) return null; // Prevent concurrent calls
    if (loadingKey) {
      setLoadingStates((prev) => ({ ...prev, [loadingKey]: true }));
    }

    let result: T | null = null;
    try {
      result = await invoke<T>(command, args);
      // Call notification command on success instead of setting info state
      // Assuming a command 'show_notification_command' exists in Rust
      // Adjust title/body as needed
      invoke("show_notification_command", {
        title: "Success",
        body: `Command '${command}' executed.`,
      }).catch(console.error); // Log error if notification fails
    } catch (err: any) {
      console.error(`Failed to invoke ${command}:`, err);
      toast.error(`Failed command '${command}': ${err?.message || err}`);
    } finally {
      if (loadingKey) {
        setLoadingStates((prev) => ({ ...prev, [loadingKey]: false }));
      }
    }
    return result;
  };

  // Helper function to invoke commands with a delay for focus switching
  const invokeCommandWithDelay = async <T = any,>(
    command: string,
    args: any,
    loadingKey: keyof LoadingStates,
    delayMs: number = 5000
  ): Promise<T | null> => {
    if (loadingStates[loadingKey]) return null; // Prevent multiple clicks
    setLoadingStates((prev) => ({ ...prev, [loadingKey]: true }));
    toast.info(`Waiting ${delayMs / 1000}s... Switch focus now!`); // Indicate delay

    // Clear any existing timeout
    if (delayTimeoutRef.current) {
      clearTimeout(delayTimeoutRef.current);
    }

    return new Promise((resolve) => {
      delayTimeoutRef.current = setTimeout(async () => {
        toast.dismiss(); // Clear waiting message
        const result = await invokeCommand<T>(command, args); // Capture the result with type T
        setLoadingStates((prev) => ({ ...prev, [loadingKey]: false })); // Clear loading state after execution
        delayTimeoutRef.current = null;
        resolve(result); // Resolve the promise with the result
      }, delayMs);
    });
  };

  const handleCaptureScreenshot = async () => {
    setLoadingStates((prev) => ({ ...prev, screenshot: true }));
    toast.dismiss();
    setScreenshotSrc(null); // Clear previous screenshot
    setElementScreenshotSrc(null);
    try {
      const base64String: string = await invoke("capture_screenshot_command");
      setScreenshotSrc(`data:image/png;base64,${base64String}`);
    } catch (err: any) {
      console.error("Failed to capture screenshot:", err);
      toast.error(`Failed to capture screenshot: ${err?.message || err}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, screenshot: false }));
    }
  };

  const handleCaptureElementScreenshot = async () => {
    setLoadingStates((prev) => ({ ...prev, elementScreenshot: true }));
    toast.dismiss();
    setElementScreenshotSrc(null);
    try {
      const base64String: string = await invoke(
        "capture_element_screenshot_command"
      );
      setElementScreenshotSrc(`data:image/png;base64,${base64String}`);
    } catch (err: any) {
      console.error("Failed to capture element screenshot:", err);
      toast.error(
        `Failed to capture element screenshot: ${err?.message || err}`
      );
    } finally {
      setLoadingStates((prev) => ({ ...prev, elementScreenshot: false }));
    }
  };

  const fetchAndSetFocusInfo = async () => {
    setLoadingStates((prev) => ({ ...prev, focusInfo: true }));
    setElementScreenshotSrc(null);
    try {
      const infoJsonString: string = await invoke(
        "dev_get_focused_element_info"
      );
      try {
        const parsedInfo = JSON.parse(infoJsonString);
        setFocusedElementInfo(JSON.stringify(parsedInfo, null, 2));
      } catch (parseError) {
        console.error("Failed to parse focus info JSON:", parseError);
        setFocusedElementInfo(infoJsonString);
        toast.error("Received focus info, but failed to parse as JSON.");
      }
    } catch (err: any) {
      console.error("Failed to get focused element info:", err);
      toast.error(`Failed to get focused element info: ${err?.message || err}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, focusInfo: false }));
    }
  };

  const handleGetFocusInfo = async () => {
    toast.dismiss();
    setFocusedElementInfo(null);
    await fetchAndSetFocusInfo();
  };

  const handleGetFocusInfoWithDelay = async () => {
    if (loadingStates.focusDelay) return; // Prevent multiple clicks

    toast.dismiss();
    setFocusedElementInfo(null); // Clear previous info
    setLoadingStates((prev) => ({ ...prev, focusDelay: true }));

    // Use delayMessage instead of error for the waiting indication
    toast.info("Waiting 5s... Switch focus now!");
    // setError("Waiting 5s... Switch focus now!"); // Using error state for now

    delayTimeoutRef.current = setTimeout(async () => {
      // setError(null); // Clear the waiting message
      toast.dismiss(); // Clear the waiting message
      await fetchAndSetFocusInfo();
      setLoadingStates((prev) => ({ ...prev, focusDelay: false }));
      delayTimeoutRef.current = null;
    }, 5000); // 5-second delay
  };

  // Specific handlers using invokeCommand or invokeCommandWithDelay
  const handleClickFocused = () =>
    invokeCommandWithDelay("dev_click_focused_element", {}, "clickFocus");

  // handleTypeText already has a manual delay implementation, kept for reference or potential refactor
  const handleTypeText = () => {
    if (loadingStates.typeText) return; // Prevent multiple clicks

    toast.dismiss();
    setLoadingStates((prev) => ({ ...prev, typeText: true }));
    // setError("Waiting 5s... Switch focus to target element now!"); // Indicate delay
    toast.info("Waiting 5s... Switch focus to target element now!"); // Indicate delay

    // Clear any existing timeout
    if (delayTimeoutRef.current) {
      clearTimeout(delayTimeoutRef.current);
    }

    delayTimeoutRef.current = setTimeout(async () => {
      // setError(null); // Clear waiting message
      toast.dismiss(); // Clear waiting message
      await invokeCommand("dev_type_text", { text: textToType }); // Pass loadingKey manually handled
      setLoadingStates((prev) => ({ ...prev, typeText: false })); // Clear loading state after execution
      delayTimeoutRef.current = null;
    }, 5000); // 5-second delay
  };

  const handlePressKey = () =>
    invokeCommandWithDelay("dev_press_key", { key: keyToPress }, "pressKey");

  const handleOpenApp = () =>
    invokeCommand("dev_open_application", { appName: appToOpen }, "openApp");
  const handleOpenUrl = () =>
    invokeCommand("dev_open_url", { url: urlToOpen }, "openUrl");
  const handleScroll = (direction: "up" | "down") =>
    invokeCommandWithDelay("dev_scroll_window", { direction }, "scroll");

  // New handlers
  const handleGlobalTypeText = () =>
    invokeCommandWithDelay(
      // Add delay here too
      "dev_global_type_text",
      { text: globalTextToType },
      "globalTypeText"
    );

  const handleGetClipboard = async () => {
    setClipboardResult(null); // Clear previous result
    const content = await invokeCommand<string>( // No delay needed
      "dev_get_clipboard",
      {},
      "getClipboard"
    );
    if (content !== null) {
      // Check if invoke succeeded
      setClipboardResult(content);
    }
  };

  const handleSetClipboard = () =>
    invokeCommand(
      "dev_set_clipboard",
      { content: clipboardContent },
      "setClipboard"
    );

  const handleHoldKey = () =>
    invokeCommandWithDelay("dev_hold_key", { key: modifierKey }, "holdKey");

  const handleReleaseKey = () =>
    invokeCommandWithDelay(
      "dev_release_key",
      { key: modifierKey },
      "releaseKey"
    );

  const handleWait = () => {
    const duration = parseInt(waitDuration, 10);
    if (isNaN(duration) || duration < 0) {
      toast.error("Invalid wait duration. Please enter a non-negative number.");
      return;
    }
    invokeCommand("dev_wait", { durationMs: duration }, "wait"); // No delay needed
  };

  // Handler for finding element by selector
  const handleFindElement = async () => {
    setFindElementResult(null); // Clear previous result
    const result = await invokeCommandWithDelay<string | null>(
      "dev_find_element_by_selector",
      { selectorStr: selectorString },
      "findElement"
    );
    if (result !== null && typeof result === "string") {
      try {
        const parsedInfo = JSON.parse(result);
        setFindElementResult(JSON.stringify(parsedInfo, null, 2));
      } catch (parseError) {
        console.error("Failed to parse find element result JSON:", parseError);
        setFindElementResult(result); // Show raw string if JSON parsing fails
        toast.error(
          "Received find element result, but failed to parse as JSON."
        );
      }
    } else if (result !== null) {
      // Handle cases where result is not null but also not a string (if possible)
      setFindElementResult(String(result)); // Convert to string if not null/string
      toast.error("Received unexpected non-string result from find element.");
    }
  };

  // Handler for clicking element by selector
  const handleClickElement = () =>
    invokeCommandWithDelay(
      "dev_click_element_by_selector",
      { selectorStr: selectorString },
      "clickElement"
    );

  // Handler for getting selected text
  const handleGetSelectedText = async () => {
    setSelectedTextResult(null); // Clear previous result
    const result = await invokeCommandWithDelay<string | null>( // Use delay as focus might be needed
      "dev_get_selected_text",
      {}, // No arguments needed for this command
      "getSelectedText"
    );
    if (result !== null) {
      setSelectedTextResult(result);
    } else {
      // Handle case where command returns null or error (already logged by invokeCommand)
      // Optionally set a specific message like "No text selected or failed to retrieve."
      setSelectedTextResult("(No text selected or failed to retrieve)");
    }
  };

  // Handler for getting window list
  const handleGetWindowList = async () => {
    setWindowListResult(null); // Clear previous result
    const result = await invokeCommand<string | null>( // No delay needed usually
      "dev_get_window_list",
      {}, // No arguments
      "getWindowList"
    );
    if (result !== null && typeof result === "string") {
      try {
        // Attempt to parse and re-stringify for pretty printing
        const parsedList = JSON.parse(result);
        setWindowListResult(JSON.stringify(parsedList, null, 2));
      } catch (parseError) {
        console.error("Failed to parse window list JSON:", parseError);
        setWindowListResult(result); // Show raw string if JSON parsing fails
        toast.error("Received window list, but failed to parse as JSON.");
      }
    } else if (result !== null) {
      setWindowListResult(String(result)); // Handle non-string, non-null results
      toast.error("Received unexpected non-string result for window list.");
    } else {
      // Handle null result (error handled by invokeCommand)
      setWindowListResult("(Failed to retrieve window list)");
    }
  };

  // Handler for getting specific window info
  const handleGetWindowInfo = async () => {
    if (!windowIdInput.trim()) {
      toast.error("Please enter a Window ID.");
      return;
    }
    setWindowInfoResult(null); // Clear previous result
    const result = await invokeCommand<string | null>( // No delay usually needed
      "dev_get_window_info",
      { windowId: windowIdInput.trim() }, // Pass window ID
      "getWindowInfo"
    );
    if (result !== null && typeof result === "string") {
      try {
        const parsedInfo = JSON.parse(result);
        setWindowInfoResult(JSON.stringify(parsedInfo, null, 2));
      } catch (parseError) {
        console.error("Failed to parse window info JSON:", parseError);
        setWindowInfoResult(result);
        toast.error("Received window info, but failed to parse as JSON.");
      }
    } else if (result !== null) {
      setWindowInfoResult(String(result));
      toast.error("Received unexpected non-string result for window info.");
    } else {
      setWindowInfoResult("(Failed to retrieve window info)");
    }
  };

  // Handler for focusing a specific window
  const handleFocusWindow = async () => {
    if (!windowIdFocus.trim()) {
      toast.error("Please enter a Window ID to focus.");
      return;
    }
    await invokeCommand(
      "dev_focus_window",
      { windowId: windowIdFocus.trim() }, // Pass window ID
      "focusWindow"
    );
    // No result to display, success/error handled by invokeCommand
  };

  // Handler for resizing a specific window
  const handleResizeWindow = async () => {
    const width = parseInt(windowWidth, 10);
    const height = parseInt(windowHeight, 10);

    if (!windowIdResize.trim()) {
      toast.error("Please enter a Window ID to resize.");
      return;
    }
    if (isNaN(width) || width <= 0) {
      toast.error("Invalid width. Please enter a positive number.");
      return;
    }
    if (isNaN(height) || height <= 0) {
      toast.error("Invalid height. Please enter a positive number.");
      return;
    }

    await invokeCommand(
      "dev_resize_window",
      { windowId: windowIdResize.trim(), width, height }, // Pass ID and dimensions
      "resizeWindow"
    );
  };

  // Handler for moving a specific window
  const handleMoveWindow = async () => {
    const x = parseInt(windowX, 10);
    const y = parseInt(windowY, 10);

    if (!windowIdMove.trim()) {
      toast.error("Please enter a Window ID to move.");
      return;
    }
    if (isNaN(x)) {
      toast.error("Invalid X coordinate. Please enter a number.");
      return;
    }
    if (isNaN(y)) {
      toast.error("Invalid Y coordinate. Please enter a number.");
      return;
    }

    await invokeCommand(
      "dev_move_window",
      { windowId: windowIdMove.trim(), x, y }, // Pass ID and coordinates
      "moveWindow"
    );
  };

  // Handler for closing a specific window
  const handleCloseWindow = async () => {
    if (!windowIdClose.trim()) {
      toast.error("Please enter a Window ID to close.");
      return;
    }
    await invokeCommand(
      "dev_close_window",
      { windowId: windowIdClose.trim() }, // Pass window ID
      "closeWindow"
    );
  };

  // Handler for listing files in a directory
  const handleListFiles = async () => {
    if (!pathToList.trim()) {
      toast.error("Please enter a path to list.");
      return;
    }
    setFileListResult(null); // Clear previous result
    const result = await invokeCommand<string | null>( // Using string for now, might refine
      "dev_list_files",
      { pathStr: pathToList.trim() }, // Changed `path` to `pathStr`
      "listFiles"
    );

    if (result !== null && typeof result === "string") {
      try {
        // Attempt to parse and re-stringify for pretty printing
        const parsedList: FileEntry[] = JSON.parse(result);
        // Format for display (example: simple list)
        const formattedList = parsedList
          .map((entry) => `${entry.is_dir ? "[D]" : "[F]"} ${entry.name}`)
          .join("\n");
        setFileListResult(formattedList);
        // Alternatively, keep JSON string: setFileListResult(JSON.stringify(parsedList, null, 2));
      } catch (parseError) {
        console.error("Failed to parse file list JSON:", parseError);
        setFileListResult(result); // Show raw string if parsing fails
        toast.error("Received file list, but failed to parse or format.");
      }
    } else if (result !== null) {
      setFileListResult(String(result));
      toast.error("Received unexpected non-string result for file list.");
    } else {
      setFileListResult("(Failed to list files)");
    }
  };

  // Handler for getting file content
  const handleGetFileContent = async () => {
    if (!pathGetContent.trim()) {
      toast.error("Please enter a file path to read.");
      return;
    }
    setFileContentResult(null); // Clear previous result
    const result = await invokeCommand<string | null>(
      "dev_get_file_content",
      { pathStr: pathGetContent.trim() }, // Changed path to pathStr
      "getFileContent"
    );
    if (result !== null) {
      setFileContentResult(result); // Display raw content
    } else {
      setFileContentResult("(Failed to get file content)");
    }
  };

  // Handler for setting file content
  const handleSetFileContent = async () => {
    if (!pathSetContent.trim()) {
      toast.error("Please enter a file path to write to.");
      return;
    }
    // Note: No check for empty content, allow writing empty files
    await invokeCommand(
      "dev_set_file_content",
      { pathStr: pathSetContent.trim(), content: fileContentToSet }, // Changed path to pathStr
      "setFileContent"
    );
    // No result to display, success/error handled by invokeCommand
  };

  // Handler for moving the mouse
  const handleMouseMove = async () => {
    const x = parseInt(mouseX, 10);
    const y = parseInt(mouseY, 10);

    if (isNaN(x)) {
      toast.error("Invalid X coordinate for mouse move.");
      return;
    }
    if (isNaN(y)) {
      toast.error("Invalid Y coordinate for mouse move.");
      return;
    }

    await invokeCommand(
      "dev_mouse_move",
      { x, y }, // Pass coordinates
      "mouseMove"
    );
  };

  // Handler for mouse down
  const handleMouseDown = async () => {
    await invokeCommandWithDelay(
      "dev_mouse_down",
      { button: mouseButton }, // Pass the selected button
      "mouseDown"
    );
  };

  // Handler for mouse up
  const handleMouseUp = async () => {
    await invokeCommandWithDelay(
      "dev_mouse_up",
      { button: mouseButton }, // Pass the selected button
      "mouseUp"
    );
  };

  // Handler for mouse click
  const handleMouseClick = async () => {
    await invokeCommandWithDelay(
      "dev_mouse_click",
      { button: mouseButton }, // Pass the selected button
      "mouseClick"
    );
  };

  // Handler for mouse double click
  const handleMouseDoubleClick = async () => {
    await invokeCommandWithDelay(
      "dev_mouse_double_click",
      { button: mouseButton }, // Pass the selected button
      "mouseDoubleClick"
    );
  };

  // Handler for mouse drag
  const handleMouseDrag = async () => {
    const startX = parseInt(mouseStartX, 10);
    const startY = parseInt(mouseStartY, 10);
    const endX = parseInt(mouseEndX, 10);
    const endY = parseInt(mouseEndY, 10);

    if (isNaN(startX) || isNaN(startY) || isNaN(endX) || isNaN(endY)) {
      toast.error("Invalid coordinates for mouse drag. Please enter numbers.");
      return;
    }

    await invokeCommand(
      "dev_mouse_drag",
      { startX, startY, endX, endY, button: mouseButton }, // Pass coordinates and button
      "mouseDrag"
    );
  };

  // Handler for testing click visualization
  const handleTestClickVisualization = async () => {
    const x = parseInt(mouseX, 10);
    const y = parseInt(mouseY, 10);

    if (isNaN(x) || isNaN(y)) {
      toast.error(
        "Invalid coordinates for visualization. Please enter numbers."
      );
      return;
    }

    let color = "#ff0000"; // Default red
    switch (mouseButton) {
      case "left":
        color = "#ff0000"; // Red for left click
        break;
      case "right":
        color = "#0000ff"; // Blue for right click
        break;
      case "middle":
        color = "#00ff00"; // Green for middle click
        break;
      default:
        color = "#ff0000"; // Default red
    }

    await invokeCommand(
      "dev_test_click_visualization",
      { x, y, color },
      "testClickVisualization"
    );
  };

  // QA test handlers
  const handleQaTestClick = async () => {
    // Add a delay before invoking the command
    await new Promise((resolve) => setTimeout(resolve, 1000));

    const result = await invokeCommand<ClickQAResult>(
      "qa_test_click",
      { x: qaClickX, y: qaClickY, clickType: qaClickType }, // Use clickType here
      "mouseClick"
    );
    if (result) {
      setQaClickResult(result);
    }
  };

  const handleQaTestClickSeries = async () => {
    // Add a delay before invoking the command
    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Create a series of clicks at different positions
    const positions = [
      [200, 200, "left"] as [number, number, string],
      [300, 200, "right"] as [number, number, string],
      [400, 200, "middle"] as [number, number, string],
      [500, 200, "double"] as [number, number, string],
      [600, 200, "triple"] as [number, number, string],
    ];

    const results = await invokeCommand<ClickQAResult[]>(
      "qa_test_click_series",
      { positions },
      "mouseClick"
    );
    if (results) {
      setQaClickSeriesResults(results);
    }
  };

  const handleQaTestCoordinateTransformation = async () => {
    const x = parseFloat(coordTestX);
    const y = parseFloat(coordTestY);

    if (isNaN(x) || isNaN(y)) {
      console.error("Invalid coordinates entered");
      // Use toast for user feedback instead of state
      toast.error("Invalid coordinates entered for QA test.");
      return;
    }

    setCoordTestLoading(true);
    setCoordinateTestResult(null);

    try {
      const result = await invoke<CoordinateTestResult>(
        "qa_test_coordinate_transformation",
        { x, y }
      );
      console.log("Coordinate Test Result:", result);
      setCoordinateTestResult(result);
    } catch (error) {
      console.error("Coordinate Transformation Test Error:", error);
      // Use toast for error feedback
      toast.error(`Coordinate Test Failed: ${error}`);
      // Optionally set state to show error in UI if needed
      // setCoordinateTestResult({ error: String(error) });
    } finally {
      setCoordTestLoading(false);
    }
  };

  const handleQaTestClickVisualization = async () => {
    // Add a delay before invoking the command
    await new Promise((resolve) => setTimeout(resolve, 1000));

    const result = await invokeCommand<VisualizationTestResult>(
      "qa_test_click_visualization",
      {},
      "testClickVisualization"
    );
    if (result) {
      setVisualizationTestResult(result);
    }
  };

  return (
    <div className="space-y-6">
      {/* Tool History Section */}
      <div className="space-y-2">
        <h3 className="text-base font-semibold border-b pb-1">
          AI Tool Usage History
        </h3>
        {toolHistory.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No tool usage recorded yet.
          </p>
        ) : (
          <div className="max-h-[300px] overflow-y-auto border rounded-md p-2">
            {toolHistory.map((entry, index) => (
              <div
                key={index}
                className="mb-2 border-b pb-2 last:border-b-0 last:pb-0"
              >
                <div className="flex justify-between items-start">
                  <span className="font-medium text-sm">{entry.tool}</span>
                  <span className="text-xs text-muted-foreground">
                    {new Date(entry.timestamp).toLocaleTimeString()}
                  </span>
                </div>
                <div className="text-xs mt-1">
                  <div className="text-muted-foreground">Inputs:</div>
                  <pre className="bg-muted p-1 rounded text-[10px] mt-1 overflow-x-auto">
                    {JSON.stringify(entry.inputs, null, 2)}
                  </pre>
                </div>
                {entry.result && (
                  <div className="text-xs mt-1">
                    <div className="text-muted-foreground">Result:</div>
                    <pre className="bg-muted p-1 rounded text-[10px] mt-1 overflow-x-auto">
                      {typeof entry.result === "string"
                        ? entry.result.length > 150
                          ? entry.result.substring(0, 150) + "..."
                          : entry.result
                        : JSON.stringify(entry.result, null, 2)}
                    </pre>
                  </div>
                )}
                {entry.screenshot_base64 && (
                  <div className="mt-1">
                    <div className="text-xs text-muted-foreground">
                      Screenshot:
                    </div>
                    <img
                      src={`data:image/png;base64,${entry.screenshot_base64}`}
                      alt="Tool Screenshot"
                      className="mt-1 border rounded w-full object-contain max-h-[200px]"
                    />
                  </div>
                )}
                <div
                  className={`text-xs mt-1 ${
                    entry.success ? "text-green-500" : "text-red-500"
                  }`}
                >
                  {entry.success ? "Success" : "Failed"}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Existing UI elements */}
      <Separator className="my-3" />
      <h3 className="text-base font-semibold border-b pb-1">Vision</h3>
      <div className="w-full space-y-3 text-sm">
        {" "}
        {/* Reduced spacing */}
        {/* Status/Error Messages - Using Alert */}
        {/* Vision Context Section */}
        <h3 className="text-base font-semibold border-b pb-1">
          {" "}
          {/* Reduced size/padding */} Vision Context
        </h3>
        <div className="flex flex-wrap gap-2">
          {" "}
          {/* Use gap for spacing */}
          <Button
            size="sm"
            onClick={handleCaptureScreenshot}
            disabled={loadingStates.screenshot || loadingStates.focusDelay}
            title="Capture Screenshot"
          >
            <Maximize2 size={14} className="mr-1" />
            {loadingStates.screenshot ? "..." : "Screenshot"}{" "}
            {/* Shorter text */}
          </Button>
          <Button
            size="sm"
            onClick={handleGetFocusInfo}
            disabled={loadingStates.focusInfo || loadingStates.focusDelay}
            title="Get Focused Element Info"
          >
            <Maximize2 size={14} className="mr-1" />
            {loadingStates.focusInfo ? "..." : "Focus Info"}
          </Button>
          <Button
            size="sm"
            onClick={handleGetFocusInfoWithDelay}
            disabled={
              loadingStates.focusDelay ||
              loadingStates.focusInfo ||
              loadingStates.screenshot
            }
            title="Get Focused Element Info (After 5s Delay)" // Use title for long text
          >
            <Maximize2 size={14} className="mr-1" />
            {loadingStates.focusDelay ? "Waiting..." : "Focus Info (5s)"}
          </Button>
          <Button
            size="sm"
            onClick={handleCaptureElementScreenshot}
            disabled={
              loadingStates.elementScreenshot ||
              loadingStates.focusDelay ||
              loadingStates.focusInfo ||
              loadingStates.screenshot
            }
          >
            {loadingStates.elementScreenshot ? "..." : "Element Screenshot"}
          </Button>
        </div>
        {focusedElementInfo && (
          <div className="mt-2 border rounded-md p-2">
            {" "}
            {/* Reduced margin/padding */}
            <h4 className="text-xs font-semibold mb-1">
              Focused Element Info:
            </h4>
            <ScrollArea className="h-28 w-full rounded-md border p-2">
              {" "}
              {/* Reduced height/padding */}
              <pre className="text-xs whitespace-pre-wrap break-words">
                <code>{focusedElementInfo}</code>
              </pre>
            </ScrollArea>
          </div>
        )}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
          {" "}
          {/* Reduced gap */}
          {screenshotSrc && (
            <div className="mt-2 border rounded-md p-2">
              {" "}
              {/* Reduced margin/padding */}
              <h4 className="text-xs font-semibold mb-1">Screenshot:</h4>
              <img
                src={screenshotSrc}
                alt="Screenshot"
                className="max-w-full h-auto border rounded"
              />
            </div>
          )}
          {elementScreenshotSrc && (
            <div className="mt-2 border rounded-md p-2">
              {" "}
              {/* Reduced margin/padding */}
              <h4 className="text-xs font-semibold mb-1">
                Element Screenshot:
              </h4>
              <img
                src={elementScreenshotSrc}
                alt="Element Screenshot"
                className="max-w-full h-auto border rounded"
              />
            </div>
          )}
        </div>
        <Separator className="my-3" /> {/* Reduced margin */}
        {/* Interaction Section */}
        <h3 className="text-base font-semibold border-b pb-1">Interactions</h3>
        <div className="space-y-2">
          {" "}
          {/* Reduced spacing */}
          {/* Click Focused */}
          <div className="flex items-center gap-2">
            {" "}
            {/* Use gap */}
            <Button
              size="sm"
              onClick={handleClickFocused}
              disabled={loadingStates.clickFocus}
              variant="outline"
              title="Click Focused Element" // Tooltip
            >
              <MousePointerClick size={14} className="mr-1" /> {/* Icon */}
              {loadingStates.clickFocus ? "Waiting..." : "Click"}
            </Button>
            <span className="text-xs text-muted-foreground flex-1">
              Clicks the OS-focused element (after 5s delay).
            </span>
          </div>
          {/* Type Text */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleTypeText}
              disabled={loadingStates.typeText}
              variant="outline"
              title="Type Text"
            >
              <Keyboard size={14} className="mr-1" />
              {loadingStates.typeText ? "Waiting..." : "Type"}
            </Button>
            <Input
              id="text-to-type"
              value={textToType}
              onChange={(e) => setTextToType(e.target.value)}
              className="h-8 text-xs flex-1" // Smaller height/text
              placeholder="Text to type"
            />
          </div>
          {/* Press Key */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handlePressKey}
              disabled={loadingStates.pressKey}
              variant="outline"
              title="Press Key Combination"
            >
              <Keyboard size={14} className="mr-1" />
              {loadingStates.pressKey ? "Waiting..." : "Press"}
            </Button>
            <Input
              id="key-to-press"
              value={keyToPress}
              onChange={(e) => setKeyToPress(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="e.g., Return, Tab, a, cmd+s"
            />
          </div>
          {/* Scroll Window */}
          <div className="flex items-center gap-2">
            <Label className="text-xs">Scroll:</Label>
            <Button
              onClick={() => handleScroll("up")}
              disabled={loadingStates.scroll}
              variant="outline"
              size="sm"
              title="Scroll Up"
            >
              <ArrowUpDown size={14} className="mr-1" />{" "}
              {/* Generic scroll icon */}
              {loadingStates.scroll ? "Waiting..." : "Up"}
            </Button>
            <Button
              onClick={() => handleScroll("down")}
              disabled={loadingStates.scroll}
              variant="outline"
              size="sm"
              title="Scroll Down"
            >
              <ArrowUpDown size={14} className="mr-1" />
              {loadingStates.scroll ? "Waiting..." : "Down"}
            </Button>
            <span className="text-xs text-muted-foreground flex-1">
              Focused window (after 5s delay).
            </span>
          </div>
        </div>
        <Separator className="my-3" />
        {/* Application/URL Section */}
        <h3 className="text-base font-semibold border-b pb-1">
          App / URL Control
        </h3>
        <div className="space-y-2">
          {/* Open Application */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleOpenApp}
              disabled={loadingStates.openApp}
              variant="outline"
              title="Open Application"
            >
              <AppWindow size={14} className="mr-1" />
              {loadingStates.openApp ? "..." : "Open App"}
            </Button>
            <Input
              id="app-to-open"
              value={appToOpen}
              onChange={(e) => setAppToOpen(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="e.g., TextEdit, Calculator"
            />
          </div>

          {/* Open URL */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleOpenUrl}
              disabled={loadingStates.openUrl}
              variant="outline"
              title="Open URL in Browser"
            >
              <ExternalLink size={14} className="mr-1" />
              {loadingStates.openUrl ? "..." : "Open URL"}
            </Button>
            <Input
              id="url-to-open"
              type="url"
              value={urlToOpen}
              onChange={(e) => setUrlToOpen(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="https://..."
            />
          </div>
        </div>
        <Separator className="my-3" />
        {/* Global Actions Section */}
        <h3 className="text-base font-semibold border-b pb-1">
          Global Actions
        </h3>
        <div className="space-y-2">
          {/* Global Type Text */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleGlobalTypeText}
              disabled={loadingStates.globalTypeText}
              variant="outline"
              title="Type Text Globally"
            >
              <Keyboard size={14} className="mr-1" />
              {loadingStates.globalTypeText ? "Waiting..." : "Global Type"}
            </Button>
            <Input
              id="global-text-to-type"
              value={globalTextToType}
              onChange={(e) => setGlobalTextToType(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="Text to type globally"
            ></Input>
          </div>
          {/* Hold/Release Key */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleHoldKey}
              disabled={loadingStates.holdKey}
              variant="outline"
              title="Hold Key"
            >
              <Hand size={14} className="mr-1" />
              {loadingStates.holdKey ? "Waiting..." : "Hold"}
            </Button>
            <Input
              id="modifier-key"
              value={modifierKey}
              onChange={(e) => setModifierKey(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="Modifier key (shift, cmd)"
            />
          </div>
          {/* Release Key */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleReleaseKey}
              disabled={loadingStates.releaseKey}
              variant="outline"
              title="Release Key"
            >
              <Hand size={14} className="mr-1" />
              {loadingStates.releaseKey ? "Waiting..." : "Release"}
            </Button>
            <span className="text-xs text-muted-foreground flex-1">
              Releases held key (after 5s delay). Requires a preceding 'Hold'.
            </span>
          </div>
          {/* Wait */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleWait}
              disabled={loadingStates.wait}
              variant="outline"
              title="Wait"
            >
              <Timer size={14} className="mr-1" />
              {loadingStates.wait ? "..." : "Wait"}
            </Button>
            <Input
              id="wait-duration"
              value={waitDuration}
              onChange={(e) => setWaitDuration(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="Wait duration (ms)"
            />
          </div>
        </div>
        <Separator className="my-3" />
        {/* Element Selector Section */}
        <h3 className="text-base font-semibold border-b pb-1">
          Element Selector
        </h3>
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Input
              id="selector-string"
              value={selectorString}
              onChange={(e) => setSelectorString(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="Selector (e.g., button:Submit, Name:Username, #id)"
            />
          </div>
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleFindElement}
              disabled={loadingStates.findElement || loadingStates.clickElement}
              variant="outline"
              title="Find Element by Selector"
            >
              <Maximize2 size={14} className="mr-1" /> {/* Reuse icon? */}
              {loadingStates.findElement ? "Finding..." : "Find Element"}
            </Button>
            <Button
              size="sm"
              onClick={handleClickElement}
              disabled={loadingStates.clickElement || loadingStates.findElement}
              variant="outline"
              title="Click Element by Selector"
            >
              <MousePointerClick size={14} className="mr-1" />
              {loadingStates.clickElement ? "Waiting..." : "Click Element"}
            </Button>
          </div>
          {findElementResult && (
            <div className="mt-2 border rounded-md p-2">
              <h4 className="text-xs font-semibold mb-1">
                Find Element Result:
              </h4>
              <ScrollArea className="h-28 w-full rounded-md border p-2">
                <pre className="text-xs whitespace-pre-wrap break-words">
                  <code>{findElementResult}</code>
                </pre>
              </ScrollArea>
            </div>
          )}
        </div>
        <Separator className="my-3" />
        {/* Selected Text Section */}
        <h3 className="text-base font-semibold border-b pb-1">Selected Text</h3>
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleGetSelectedText}
              disabled={loadingStates.getSelectedText}
              variant="outline"
              title="Get Selected Text (after 5s delay)"
            >
              <TextSelect size={14} className="mr-1" />
              {loadingStates.getSelectedText
                ? "Waiting..."
                : "Get Selected Text"}
            </Button>
            <span className="text-xs text-muted-foreground flex-1">
              Retrieves selected text from focused app after delay.
            </span>
          </div>
          {selectedTextResult !== null && (
            <div className="mt-1 border rounded-md p-2 bg-muted text-muted-foreground text-xs">
              <p className="font-mono break-all">
                {selectedTextResult || "(Empty)"}
              </p>
            </div>
          )}
        </div>
        <Separator className="my-3" />
        {/* Window Management Section */}
        <h3 className="text-base font-semibold border-b pb-1">
          Window Management
        </h3>
        <div className="space-y-2">
          {/* Get Window List */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleGetWindowList}
              disabled={loadingStates.getWindowList}
              variant="outline"
              title="Get List of Open Windows"
            >
              <Layers size={14} className="mr-1" />
              {loadingStates.getWindowList ? "Getting..." : "Get Window List"}
            </Button>
          </div>
          {windowListResult !== null && (
            <div className="mt-2 border rounded-md p-2">
              <h4 className="text-xs font-semibold mb-1">Window List:</h4>
              <ScrollArea className="h-32 w-full rounded-md border p-2">
                {" "}
                {/* Increased height */}
                <pre className="text-xs whitespace-pre-wrap break-words">
                  <code>{windowListResult}</code>
                </pre>
              </ScrollArea>
            </div>
          )}
          {/* Get Window Info */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleGetWindowInfo}
              disabled={loadingStates.getWindowInfo}
              variant="outline"
              title="Get Info for Specific Window"
            >
              <Info size={14} className="mr-1" />
              {loadingStates.getWindowInfo ? "Getting..." : "Get Window Info"}
            </Button>
            <Input
              id="window-id-info"
              value={windowIdInput}
              onChange={(e) => setWindowIdInput(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="Window ID"
            />
          </div>
          {windowInfoResult !== null && (
            <div className="mt-2 border rounded-md p-2">
              <h4 className="text-xs font-semibold mb-1">Window Info:</h4>
              <ScrollArea className="h-32 w-full rounded-md border p-2">
                <pre className="text-xs whitespace-pre-wrap break-words">
                  <code>{windowInfoResult}</code>
                </pre>
              </ScrollArea>
            </div>
          )}
          {/* Focus Window */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleFocusWindow}
              disabled={loadingStates.focusWindow}
              variant="outline"
              title="Focus Specific Window"
            >
              <Focus size={14} className="mr-1" />
              {loadingStates.focusWindow ? "Focusing..." : "Focus Window"}
            </Button>
            <Input
              id="window-id-focus"
              value={windowIdFocus}
              onChange={(e) => setWindowIdFocus(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="Window ID to focus"
            />
          </div>
          {/* Resize Window */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleResizeWindow}
              disabled={loadingStates.resizeWindow}
              variant="outline"
              title="Resize Specific Window"
            >
              <Move size={14} className="mr-1" /> {/* Reusing Move icon */}
              {loadingStates.resizeWindow ? "Resizing..." : "Resize Window"}
            </Button>
            <Input
              id="window-id-resize"
              value={windowIdResize}
              onChange={(e) => setWindowIdResize(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="Window ID"
            />
            <Input
              id="window-width"
              value={windowWidth}
              onChange={(e) => setWindowWidth(e.target.value)}
              type="number"
              className="h-8 text-xs w-16" // Fixed width
              placeholder="Width"
            />
            <Input
              id="window-height"
              value={windowHeight}
              onChange={(e) => setWindowHeight(e.target.value)}
              type="number"
              className="h-8 text-xs w-16" // Fixed width
              placeholder="Height"
            />
          </div>
          {/* Move Window */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleMoveWindow}
              disabled={loadingStates.moveWindow}
              variant="outline"
              title="Move Specific Window"
            >
              <Move size={14} className="mr-1" />
              {loadingStates.moveWindow ? "Moving..." : "Move Window"}
            </Button>
            <Input
              id="window-id-move"
              value={windowIdMove}
              onChange={(e) => setWindowIdMove(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="Window ID"
            />
            <Input
              id="window-x"
              value={windowX}
              onChange={(e) => setWindowX(e.target.value)}
              type="number"
              className="h-8 text-xs w-16"
              placeholder="X"
            />
            <Input
              id="window-y"
              value={windowY}
              onChange={(e) => setWindowY(e.target.value)}
              type="number"
              className="h-8 text-xs w-16"
              placeholder="Y"
            />
          </div>
          {/* Close Window */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleCloseWindow}
              disabled={loadingStates.closeWindow}
              variant="outline"
              title="Close Specific Window"
            >
              <X size={14} className="mr-1" />
              {loadingStates.closeWindow ? "Closing..." : "Close Window"}
            </Button>
            <Input
              id="window-id-close"
              value={windowIdClose}
              onChange={(e) => setWindowIdClose(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="Window ID to close"
            />
          </div>
        </div>
        <Separator className="my-3" />
        {/* File System Section */}
        <h3 className="text-base font-semibold border-b pb-1">File System</h3>
        <div className="space-y-2">
          {/* List Files */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleListFiles}
              disabled={loadingStates.listFiles}
              variant="outline"
              title="List Directory Contents"
            >
              <Folder size={14} className="mr-1" />
              {loadingStates.listFiles ? "Listing..." : "List Files"}
            </Button>
            <Input
              id="path-to-list"
              value={pathToList}
              onChange={(e) => setPathToList(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="Directory path (e.g., ~/, /tmp)"
            />
          </div>
          {fileListResult !== null && (
            <div className="mt-2 border rounded-md p-2">
              <h4 className="text-xs font-semibold mb-1">
                Directory Contents:
              </h4>
              <ScrollArea className="h-32 w-full rounded-md border p-2">
                <pre className="text-xs whitespace-pre-wrap break-words">
                  <code>{fileListResult}</code>
                </pre>
              </ScrollArea>
            </div>
          )}
          {/* Get File Content */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleGetFileContent}
              disabled={loadingStates.getFileContent}
              variant="outline"
              title="Get File Content"
            >
              <FileText size={14} className="mr-1" />
              {loadingStates.getFileContent ? "Reading..." : "Get Content"}
            </Button>
            <Input
              id="path-get-content"
              value={pathGetContent}
              onChange={(e) => setPathGetContent(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="File path (e.g., ~/file.txt)"
            />
          </div>
          {fileContentResult !== null && (
            <div className="mt-2 border rounded-md p-2">
              <h4 className="text-xs font-semibold mb-1">File Content:</h4>
              <ScrollArea className="h-32 w-full rounded-md border p-2">
                <pre className="text-xs whitespace-pre-wrap break-words">
                  <code>{fileContentResult}</code>
                </pre>
              </ScrollArea>
            </div>
          )}
          {/* Set File Content */}
          <div className="flex items-start gap-2">
            {" "}
            {/* Use items-start for alignment with textarea */}
            <div className="flex flex-col gap-2 flex-1">
              <div className="flex items-center gap-2">
                <Button
                  size="sm"
                  onClick={handleSetFileContent}
                  disabled={loadingStates.setFileContent}
                  variant="outline"
                  title="Set File Content"
                >
                  <FileEdit size={14} className="mr-1" />
                  {loadingStates.setFileContent ? "Writing..." : "Set Content"}
                </Button>
                <Input
                  id="path-set-content"
                  value={pathSetContent}
                  onChange={(e) => setPathSetContent(e.target.value)}
                  className="h-8 text-xs flex-1"
                  placeholder="File path (e.g., ~/new_file.txt)"
                />
              </div>
              <Textarea
                id="file-content-to-set"
                value={fileContentToSet}
                onChange={(e) => setFileContentToSet(e.target.value)}
                placeholder="Content to write to the file..."
                className="text-xs h-24" // Adjusted height
              />
            </div>
          </div>
        </div>
        <Separator className="my-3" />
        {/* Mouse Control Section */}
        <h3 className="text-base font-semibold border-b pb-1">Mouse Control</h3>
        <div className="space-y-2">
          {/* Mouse Move */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleMouseMove}
              disabled={loadingStates.mouseMove}
              variant="outline"
              title="Move Mouse Cursor"
            >
              <Mouse size={14} className="mr-1" />
              {loadingStates.mouseMove ? "Moving..." : "Move Mouse"}
            </Button>
            <Input
              id="mouse-x"
              value={mouseX}
              onChange={(e) => setMouseX(e.target.value)}
              type="number"
              className="h-8 text-xs w-16"
              placeholder="X"
            />
            <Input
              id="mouse-y"
              value={mouseY}
              onChange={(e) => setMouseY(e.target.value)}
              type="number"
              className="h-8 text-xs w-16"
              placeholder="Y"
            />
          </div>
          {/* Mouse Down */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleMouseDown}
              disabled={loadingStates.mouseDown}
              variant="outline"
              title="Press Mouse Button Down (after 5s delay)"
            >
              <Mouse size={14} className="mr-1" />
              {loadingStates.mouseDown ? "Waiting..." : "Mouse Down"}
            </Button>
            {/* TODO: Replace with RadioGroup or Select if ui components are available */}
            <div className="flex items-center gap-1 text-xs">
              <input
                type="radio"
                id="mouse-left"
                name="mouseButton"
                value="left"
                checked={mouseButton === "left"}
                onChange={() => setMouseButton("left")}
                className="mr-1"
              />
              <label htmlFor="mouse-left">Left</label>
              <input
                type="radio"
                id="mouse-right"
                name="mouseButton"
                value="right"
                checked={mouseButton === "right"}
                onChange={() => setMouseButton("right")}
                className="ml-2 mr-1"
              />
              <label htmlFor="mouse-right">Right</label>
              <input
                type="radio"
                id="mouse-middle"
                name="mouseButton"
                value="middle"
                checked={mouseButton === "middle"}
                onChange={() => setMouseButton("middle")}
                className="ml-2 mr-1"
              />
              <label htmlFor="mouse-middle">Middle</label>
            </div>
            <span className="text-xs text-muted-foreground flex-1">
              Presses button down (after 5s delay).
            </span>
          </div>
          {/* Mouse Up */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleMouseUp}
              disabled={loadingStates.mouseUp}
              variant="outline"
              title="Release Mouse Button (after 5s delay)"
            >
              <Mouse size={14} className="mr-1" />
              {loadingStates.mouseUp ? "Waiting..." : "Mouse Up"}
            </Button>
            <span className="text-xs text-muted-foreground flex-1">
              Releases the selected button (after 5s delay). Usually follows
              Mouse Down.
            </span>
          </div>
          {/* Mouse Click */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleMouseClick}
              disabled={loadingStates.mouseClick}
              variant="outline"
              title="Simulate Mouse Click (after 5s delay)"
            >
              <MousePointerClick size={14} className="mr-1" />
              {loadingStates.mouseClick ? "Waiting..." : "Mouse Click"}
            </Button>
            <span className="text-xs text-muted-foreground flex-1">
              Clicks the selected button (after 5s delay). Uses the button
              selected above.
            </span>
          </div>
          {/* Mouse Double Click */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleMouseDoubleClick}
              disabled={loadingStates.mouseDoubleClick}
              variant="outline"
              title="Simulate Mouse Double Click (after 5s delay)"
            >
              <MousePointerClick size={14} className="mr-1" />
              {loadingStates.mouseDoubleClick ? "Waiting..." : "Double Click"}
            </Button>
            <span className="text-xs text-muted-foreground flex-1">
              Double-clicks the selected button (after 5s delay). Uses the
              button selected above.
            </span>
          </div>
          {/* Mouse Drag */}
          <div className="flex items-center gap-2 flex-wrap">
            {" "}
            {/* Allow wrapping */}
            <Button
              size="sm"
              onClick={handleMouseDrag}
              disabled={loadingStates.mouseDrag}
              variant="outline"
              title="Simulate Mouse Drag"
            >
              <Move size={14} className="mr-1" /> {/* Use Move icon */}
              {loadingStates.mouseDrag ? "Dragging..." : "Mouse Drag"}
            </Button>
            <Input
              id="mouse-start-x"
              value={mouseStartX}
              onChange={(e) => setMouseStartX(e.target.value)}
              type="number"
              className="h-8 text-xs w-16"
              placeholder="Start X"
            />
            <Input
              id="mouse-start-y"
              value={mouseStartY}
              onChange={(e) => setMouseStartY(e.target.value)}
              type="number"
              className="h-8 text-xs w-16"
              placeholder="Start Y"
            />
            <span className="text-xs">&rarr;</span> {/* Right arrow */}
            <Input
              id="mouse-end-x"
              value={mouseEndX}
              onChange={(e) => setMouseEndX(e.target.value)}
              type="number"
              className="h-8 text-xs w-16"
              placeholder="End X"
            />
            <Input
              id="mouse-end-y"
              value={mouseEndY}
              onChange={(e) => setMouseEndY(e.target.value)}
              type="number"
              className="h-8 text-xs w-16"
              placeholder="End Y"
            />
            <span className="text-xs text-muted-foreground flex-1 min-w-full md:min-w-0 md:flex-none">
              {/* Ensure description doesn't break layout */}
              Drags using the selected button (no delay). Uses button selected
              above.
            </span>
          </div>
        </div>
        <Separator className="my-3" />
        {/* Clipboard Section */}
        <h3 className="text-base font-semibold border-b pb-1">Clipboard</h3>
        <div className="space-y-2">
          {/* Get Clipboard */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleGetClipboard}
              disabled={loadingStates.getClipboard}
              variant="outline"
              title="Get Clipboard"
            >
              <Clipboard size={14} className="mr-1" />
              {loadingStates.getClipboard ? "Getting..." : "Get Clipboard"}
            </Button>
          </div>
          {clipboardResult !== null && (
            <div className="mt-1 border rounded-md p-2 bg-muted text-muted-foreground text-xs">
              <p className="font-mono break-all">
                {clipboardResult || "(Empty)"}
              </p>
            </div>
          )}
          {/* Set Clipboard */}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={handleSetClipboard}
              disabled={loadingStates.setClipboard}
              variant="outline"
              title="Set Clipboard"
            >
              <ClipboardPaste size={14} className="mr-1" />
              {loadingStates.setClipboard ? "..." : "Set Clipboard"}
            </Button>
            <Input
              id="clipboard-content"
              value={clipboardContent}
              onChange={(e) => setClipboardContent(e.target.value)}
              className="h-8 text-xs flex-1"
              placeholder="Text to set clipboard"
            />
          </div>
        </div>
        {/* Test Click Visualization */}
        <div className="flex items-center gap-2">
          <Button
            size="sm"
            onClick={handleTestClickVisualization}
            disabled={loadingStates.testClickVisualization}
            variant="outline"
            title="Test Click Visualization"
          >
            <MousePointerClick size={14} className="mr-1" />
            {loadingStates.testClickVisualization
              ? "Testing..."
              : "Test Click Visualization"}
          </Button>
        </div>
        {/* QA Testing Tools Section */}
        <div className="border rounded-md p-3 shadow-sm">
          <h3 className="text-md font-medium mb-3">QA Testing Tools</h3>

          <div className="space-y-4">
            {/* Single Click Test */}
            <div className="space-y-2">
              <h4 className="text-sm font-medium">Test Individual Click</h4>
              <div className="flex flex-wrap gap-2">
                <select
                  className="px-2 py-1 border rounded-md text-sm"
                  value={qaClickType}
                  onChange={(e) => setQaClickType(e.target.value)}
                >
                  <option value="left">Left Click</option>
                  <option value="right">Right Click</option>
                  <option value="middle">Middle Click</option>
                  <option value="double">Double Click</option>
                  <option value="triple">Triple Click</option>
                </select>

                <input
                  type="number"
                  placeholder="X position"
                  className="px-2 py-1 border rounded-md text-sm w-24"
                  value={qaClickX}
                  onChange={(e) => setQaClickX(parseInt(e.target.value))}
                />

                <input
                  type="number"
                  placeholder="Y position"
                  className="px-2 py-1 border rounded-md text-sm w-24"
                  value={qaClickY}
                  onChange={(e) => setQaClickY(parseInt(e.target.value))}
                />

                <Button
                  size="sm"
                  onClick={handleQaTestClick}
                  disabled={loadingStates.mouseClick}
                >
                  {loadingStates.mouseClick ? "Testing..." : "Test Click"}
                </Button>
              </div>

              {qaClickResult && (
                <div className="mt-2 p-2 bg-muted rounded-md text-xs">
                  <div className="font-semibold">
                    Result:{" "}
                    {qaClickResult.success ? (
                      <span className="text-green-500">Success</span>
                    ) : (
                      <span className="text-red-500">Failed</span>
                    )}
                  </div>
                  <div>Operation: {qaClickResult.operation}</div>
                  <div>
                    Coordinates: ({qaClickResult.coordinates[0]},{" "}
                    {qaClickResult.coordinates[1]})
                  </div>
                  {qaClickResult.original_coordinates && (
                    <div>
                      Original: ({qaClickResult.original_coordinates[0]},{" "}
                      {qaClickResult.original_coordinates[1]})
                    </div>
                  )}
                  <div>Latency: {qaClickResult.latency_ms.toFixed(2)}ms</div>
                  {qaClickResult.error && (
                    <div className="text-red-500">
                      Error: {qaClickResult.error}
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Click Series Test */}
            <div className="space-y-2">
              <h4 className="text-sm font-medium">Test Click Series</h4>
              <Button
                size="sm"
                onClick={handleQaTestClickSeries}
                disabled={loadingStates.mouseClick}
              >
                {loadingStates.mouseClick
                  ? "Running Series..."
                  : "Run Click Series"}
              </Button>

              {qaClickSeriesResults && (
                <div className="mt-2 p-2 bg-muted rounded-md text-xs">
                  <div className="font-semibold">
                    {qaClickSeriesResults.filter((r) => r.success).length} /{" "}
                    {qaClickSeriesResults.length} successful
                  </div>
                  <div className="mt-1 space-y-1">
                    {qaClickSeriesResults.map((result, index) => (
                      <div
                        key={index}
                        className={`flex items-center gap-1 ${
                          result.success ? "text-green-500" : "text-red-500"
                        }`}
                      >
                        <span>
                          {index + 1}. {result.operation}
                        </span>
                        <span>
                          ({result.coordinates[0]}, {result.coordinates[1]})
                        </span>
                        <span>{result.latency_ms.toFixed(1)}ms</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {/* Coordinate Transformation Test */}
            <div className="space-y-2">
              <h4 className="text-sm font-medium">
                Test Coordinate Transformation
              </h4>
              <Button
                size="sm"
                onClick={handleQaTestCoordinateTransformation}
                disabled={loadingStates.mouseMove}
              >
                {loadingStates.mouseMove ? "Testing..." : "Test Coordinates"}
              </Button>

              {coordinateTestResult && (
                <div className="mt-2 p-2 bg-muted rounded-md text-xs">
                  <div className="font-semibold">
                    Accuracy:{" "}
                    {coordinateTestResult.is_accurate ? (
                      <span className="text-green-500">Good</span>
                    ) : (
                      <span className="text-yellow-500">Poor</span>
                    )}
                  </div>
                  <div>
                    Original: ({coordinateTestResult.original.x},{" "}
                    {coordinateTestResult.original.y})
                  </div>
                  <div>
                    Screen: (
                    {coordinateTestResult.transformed_to_screen.x.toFixed(1)},{" "}
                    {coordinateTestResult.transformed_to_screen.y.toFixed(1)})
                  </div>
                  <div>
                    Back to Scaled: (
                    {coordinateTestResult.transformed_back.x.toFixed(1)},{" "}
                    {coordinateTestResult.transformed_back.y.toFixed(1)})
                  </div>
                  <div>
                    Error: x={coordinateTestResult.error.x.toFixed(2)}, y=
                    {coordinateTestResult.error.y.toFixed(2)}
                  </div>
                </div>
              )}
            </div>

            {/* Click Visualization Test */}
            <div className="space-y-2">
              <h4 className="text-sm font-medium">Test Click Visualization</h4>
              <Button
                size="sm"
                onClick={handleQaTestClickVisualization}
                disabled={loadingStates.testClickVisualization}
              >
                {loadingStates.testClickVisualization
                  ? "Testing..."
                  : "Test Visualization"}
              </Button>

              {visualizationTestResult && (
                <div className="mt-2 p-2 bg-muted rounded-md text-xs">
                  <div className="font-semibold">
                    Success Rate:{" "}
                    {(visualizationTestResult.success_rate * 100).toFixed(0)}%
                  </div>
                  <div className="mt-1">
                    {visualizationTestResult.results.map((point, index) => (
                      <div key={index} className="flex items-center gap-2">
                        <div
                          className="w-3 h-3 rounded-full"
                          style={{ backgroundColor: point.color }}
                        />
                        <span>
                          Point {index + 1}: {point.success ? "✓" : "✗"}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
      {/* Test Click Visualization */}
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          onClick={handleTestClickVisualization}
          disabled={loadingStates.testClickVisualization}
          variant="outline"
          title="Test Click Visualization"
        >
          <MousePointerClick size={14} className="mr-1" />
          {loadingStates.testClickVisualization
            ? "Testing..."
            : "Test Click Visualization"}
        </Button>
      </div>
    </div>
  );
};

export default DevToolsPanel;
