import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area"; // To handle potentially large JSON
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"; // Use Alert for messages
import {
  Maximize2, // Example icon (replace as needed)
  MousePointerClick, // Example icon
  Keyboard, // Example icon
  ExternalLink, // Example icon
  AppWindow, // Example icon
  ArrowUpDown, // Example icon
  Clipboard, // Added
  ClipboardPaste, // Added
  Hand, // Added for Hold/Release
  Timer, // Added for Wait
} from "lucide-react"; // Import some icons

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
};

const DevToolsPanel: React.FC = () => {
  const [screenshotSrc, setScreenshotSrc] = useState<string | null>(null);
  const [focusedElementInfo, setFocusedElementInfo] = useState<string | null>(
    null
  );
  const [elementScreenshotSrc, setElementScreenshotSrc] = useState<
    string | null
  >(null);
  const [loadingStates, setLoadingStates] = useState<LoadingStates>({
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
  });
  const [error, setError] = useState<string | null>(null);

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

  const delayTimeoutRef = useRef<NodeJS.Timeout | null>(null); // Ref to store timeout ID

  // Cleanup timeout on component unmount
  useEffect(() => {
    return () => {
      if (delayTimeoutRef.current) {
        clearTimeout(delayTimeoutRef.current);
      }
    };
  }, []);

  // Generic handler to invoke a command and update loading/error/info
  const invokeCommand = async <T = any,>(
    command: string,
    args?: any,
    loadingKey?: keyof LoadingStates
  ): Promise<T | null> => {
    if (loadingKey && loadingStates[loadingKey]) return null; // Prevent concurrent calls
    setError(null);
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
      setError(`Failed command '${command}': ${err?.message || err}`);
    } finally {
      if (loadingKey) {
        setLoadingStates((prev) => ({ ...prev, [loadingKey]: false }));
      }
    }
    return result;
  };

  const handleCaptureScreenshot = async () => {
    setLoadingStates((prev) => ({ ...prev, screenshot: true }));
    setError(null);
    setScreenshotSrc(null); // Clear previous screenshot
    setElementScreenshotSrc(null);
    try {
      const base64String: string = await invoke("capture_screenshot_command");
      setScreenshotSrc(`data:image/png;base64,${base64String}`);
    } catch (err: any) {
      console.error("Failed to capture screenshot:", err);
      setError(`Failed to capture screenshot: ${err?.message || err}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, screenshot: false }));
    }
  };

  const handleCaptureElementScreenshot = async () => {
    setLoadingStates((prev) => ({ ...prev, elementScreenshot: true }));
    setError(null);
    setElementScreenshotSrc(null);
    try {
      const base64String: string = await invoke(
        "capture_element_screenshot_command"
      );
      setElementScreenshotSrc(`data:image/png;base64,${base64String}`);
    } catch (err: any) {
      console.error("Failed to capture element screenshot:", err);
      setError(`Failed to capture element screenshot: ${err?.message || err}`);
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
        setError("Received focus info, but failed to parse as JSON.");
      }
    } catch (err: any) {
      console.error("Failed to get focused element info:", err);
      setError(`Failed to get focused element info: ${err?.message || err}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, focusInfo: false }));
    }
  };

  const handleGetFocusInfo = async () => {
    setError(null);
    setFocusedElementInfo(null);
    await fetchAndSetFocusInfo();
  };

  const handleGetFocusInfoWithDelay = async () => {
    if (loadingStates.focusDelay) return; // Prevent multiple clicks

    setError(null);
    setFocusedElementInfo(null); // Clear previous info
    setLoadingStates((prev) => ({ ...prev, focusDelay: true }));

    setError("Waiting 5s... Switch focus now!"); // Using error state for now

    delayTimeoutRef.current = setTimeout(async () => {
      setError(null); // Clear the waiting message
      await fetchAndSetFocusInfo();
      setLoadingStates((prev) => ({ ...prev, focusDelay: false }));
      delayTimeoutRef.current = null;
    }, 5000); // 5-second delay
  };

  // Specific handlers using invokeCommand
  const handleClickFocused = () =>
    invokeCommand("dev_click_focused_element", {}, "clickFocus");
  const handleTypeText = () =>
    invokeCommand("dev_type_text", { text: textToType }, "typeText");
  const handlePressKey = () =>
    invokeCommand("dev_press_key", { key: keyToPress }, "pressKey");
  const handleOpenApp = () =>
    invokeCommand("dev_open_application", { appName: appToOpen }, "openApp");
  const handleOpenUrl = () =>
    invokeCommand("dev_open_url", { url: urlToOpen }, "openUrl");
  const handleScroll = (direction: "up" | "down") =>
    invokeCommand("dev_scroll_window", { direction }, "scroll");

  // New handlers
  const handleGlobalTypeText = () =>
    invokeCommand(
      "dev_global_type_text",
      { text: globalTextToType },
      "globalTypeText"
    );

  const handleGetClipboard = async () => {
    setClipboardResult(null); // Clear previous result
    const content = await invokeCommand<string>(
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
    invokeCommand("dev_hold_key", { key: modifierKey }, "holdKey");

  const handleReleaseKey = () =>
    invokeCommand("dev_release_key", { key: modifierKey }, "releaseKey");

  const handleWait = () => {
    const duration = parseInt(waitDuration, 10);
    if (isNaN(duration) || duration < 0) {
      setError("Invalid wait duration. Please enter a non-negative number.");
      return;
    }
    invokeCommand("dev_wait", { durationMs: duration }, "wait");
  };

  return (
    <div className="w-full space-y-3 text-sm">
      {" "}
      {/* Reduced spacing */}
      {/* Status/Error Messages - Using Alert */}
      {error && (
        <Alert variant="destructive" className="p-2 text-xs">
          {" "}
          {/* Smaller padding/text */}
          <AlertTitle className="text-xs font-semibold">Error</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      {/* Vision Context Section */}
      <h3 className="text-base font-semibold border-b pb-1">
        {" "}
        {/* Reduced size/padding */} Vision Context
      </h3>
      <div className="flex flex-wrap gap-2">
        {" "}
        {/* Use gap for spacing */}
        <Button
          size="sm" // Smaller button
          onClick={handleCaptureScreenshot}
          disabled={loadingStates.screenshot || loadingStates.focusDelay}
        >
          {loadingStates.screenshot ? "..." : "Screenshot"} {/* Shorter text */}
        </Button>
        <Button
          size="sm"
          onClick={handleGetFocusInfo}
          disabled={loadingStates.focusInfo || loadingStates.focusDelay}
        >
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
          <h4 className="text-xs font-semibold mb-1">Focused Element Info:</h4>
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
            <h4 className="text-xs font-semibold mb-1">Element Screenshot:</h4>
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
            {loadingStates.clickFocus ? "..." : "Click"}
          </Button>
          <span className="text-xs text-muted-foreground flex-1">
            Clicks the OS-focused element.
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
            {loadingStates.typeText ? "..." : "Type"}
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
            {loadingStates.pressKey ? "..." : "Press"}
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
            {/* Generic scroll icon */} Up
          </Button>
          <Button
            onClick={() => handleScroll("down")}
            disabled={loadingStates.scroll}
            variant="outline"
            size="sm"
            title="Scroll Down"
          >
            <ArrowUpDown size={14} className="mr-1" /> Down
          </Button>
          <span className="text-xs text-muted-foreground flex-1">
            Focused window.
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
      <h3 className="text-base font-semibold border-b pb-1">Global Actions</h3>
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
            {loadingStates.globalTypeText ? "..." : "Global Type"}
          </Button>
          <Input
            id="global-text-to-type"
            value={globalTextToType}
            onChange={(e) => setGlobalTextToType(e.target.value)}
            className="h-8 text-xs flex-1"
            placeholder="Text to type globally"
          />
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
            {loadingStates.holdKey ? "..." : "Hold"}
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
            {loadingStates.releaseKey ? "..." : "Release"}
          </Button>
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
    </div>
  );
};

export default DevToolsPanel;
