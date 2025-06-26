import React, { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { AppWindow, Maximize2, Move, X, Info, Focus } from "lucide-react";
import { invokeCommand } from "@/lib/utils";

const WindowOperations: React.FC = () => {
  const [windowListResult, setWindowListResult] = useState<string | null>(null);
  const [windowIdInfo, setWindowIdInfo] = useState<string>("");
  const [windowInfoResult, setWindowInfoResult] = useState<string | null>(null);
  const [windowIdFocus, setWindowIdFocus] = useState<string>("");
  const [windowIdResize, setWindowIdResize] = useState<string>("");
  const [windowWidth, setWindowWidth] = useState<string>("");
  const [windowHeight, setWindowHeight] = useState<string>("");
  const [windowIdMove, setWindowIdMove] = useState<string>("");
  const [windowX, setWindowX] = useState<string>("");
  const [windowY, setWindowY] = useState<string>("");
  const [windowIdClose, setWindowIdClose] = useState<string>("");

  const handleGetWindowList = async () => {
    setWindowListResult(null);
    const result = await invokeCommand<string | null>(
      "get_window_list",
      {},
      "getWindowList"
    );
    if (result !== null && typeof result === "string") {
      try {
        const parsedList = JSON.parse(result);
        setWindowListResult(JSON.stringify(parsedList, null, 2));
      } catch (parseError) {
        console.error("Failed to parse window list JSON:", parseError);
        setWindowListResult(result);
        toast.error("Received window list, but failed to parse as JSON.");
      }
    } else if (result !== null) {
      setWindowListResult(String(result));
      toast.error("Received unexpected non-string result for window list.");
    }
  };

  const handleGetWindowInfo = async () => {
    if (!windowIdInfo.trim()) {
      toast.error("Please enter a Window ID.");
      return;
    }
    setWindowInfoResult(null);
    // Use production function instead of dev_get_window_info
    const result = await invokeCommand<string | null>(
      "get_window_info",
      { window_id: windowIdInfo.trim() },
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
    }
  };

  const handleFocusWindow = async () => {
    if (!windowIdFocus.trim()) {
      toast.error("Please enter a Window ID to focus.");
      return;
    }
    // Use production function instead of dev_focus_window
    await invokeCommand(
      "focus_window",
      { window_id: windowIdFocus.trim() },
      "focusWindow"
    );
  };

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

    // Use production function instead of dev_resize_window
    await invokeCommand(
      "resize_window",
      { window_id: windowIdResize.trim(), width, height },
      "resizeWindow"
    );
  };

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

    // Use production function instead of dev_move_window
    await invokeCommand(
      "move_window",
      { window_id: windowIdMove.trim(), x, y },
      "moveWindow"
    );
  };

  const handleCloseWindow = async () => {
    if (!windowIdClose.trim()) {
      toast.error("Please enter a Window ID to close.");
      return;
    }
    // Use production function instead of dev_close_window
    await invokeCommand(
      "close_window",
      { window_id: windowIdClose.trim() },
      "closeWindow"
    );
  };

  return (
    <div className="space-y-4">
      {/* Consolidation Notice */}
      <div className="bg-blue-50 border border-blue-200 rounded-lg p-3 mb-4">
        <div className="flex items-start space-x-2">
          <Info className="h-4 w-4 text-blue-600 mt-0.5 flex-shrink-0" />
          <div className="text-sm text-blue-800">
            <strong>Tool Consolidation:</strong> This component now uses
            production window management functions with built-in debug
            capabilities instead of dev_* functions.
          </div>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <AppWindow className="h-4 w-4" />
          <Button onClick={handleGetWindowList}>Get Window List</Button>
        </div>
        {windowListResult && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm">
            {windowListResult}
          </pre>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Info className="h-4 w-4" />
          <Input
            placeholder="Window ID for info"
            value={windowIdInfo}
            onChange={(e) => setWindowIdInfo(e.target.value)}
          />
          <Button onClick={handleGetWindowInfo}>Get Info</Button>
        </div>
        {windowInfoResult && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm">
            {windowInfoResult}
          </pre>
        )}
      </div>

      <div className="flex items-center space-x-2">
        <Focus className="h-4 w-4" />
        <Input
          placeholder="Window ID to focus"
          value={windowIdFocus}
          onChange={(e) => setWindowIdFocus(e.target.value)}
        />
        <Button onClick={handleFocusWindow}>Focus Window</Button>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Maximize2 className="h-4 w-4" />
          <Input
            placeholder="Window ID to resize"
            value={windowIdResize}
            onChange={(e) => setWindowIdResize(e.target.value)}
          />
          <Input
            placeholder="Width"
            value={windowWidth}
            onChange={(e) => setWindowWidth(e.target.value)}
          />
          <Input
            placeholder="Height"
            value={windowHeight}
            onChange={(e) => setWindowHeight(e.target.value)}
          />
          <Button onClick={handleResizeWindow}>Resize Window</Button>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Move className="h-4 w-4" />
          <Input
            placeholder="Window ID to move"
            value={windowIdMove}
            onChange={(e) => setWindowIdMove(e.target.value)}
          />
          <Input
            placeholder="X coordinate"
            value={windowX}
            onChange={(e) => setWindowX(e.target.value)}
          />
          <Input
            placeholder="Y coordinate"
            value={windowY}
            onChange={(e) => setWindowY(e.target.value)}
          />
          <Button onClick={handleMoveWindow}>Move Window</Button>
        </div>
      </div>

      <div className="flex items-center space-x-2">
        <X className="h-4 w-4" />
        <Input
          placeholder="Window ID to close"
          value={windowIdClose}
          onChange={(e) => setWindowIdClose(e.target.value)}
        />
        <Button onClick={handleCloseWindow}>Close Window</Button>
      </div>
    </div>
  );
};

export default WindowOperations;
