import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { invokeCommand } from "@/lib/utils";
import type { LoadingStates } from "@/types/devtools";
import { ExternalLink, Timer } from "lucide-react";
import React, { useState } from "react";
import { toast } from "sonner";
import { CloudTestPanel } from "./devtools/CloudTestPanel";
import FileOperations from "./devtools/FileOperations";
import KeyboardOperations from "./devtools/KeyboardOperations";
import MouseOperations from "./devtools/MouseOperations";
import ScreenshotOperations from "./devtools/ScreenshotOperations";
import WakeWordTesting from "./devtools/WakeWordTesting";
import WindowOperations from "./devtools/WindowOperations";

const DevToolsPanel: React.FC = () => {
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
    setDeveloperPlayback: false,
    playbackAudio: false,
    setTtsProvider: false,
    testSystemContext: false,
    // Always Listening Testing
    debugAlwaysListening: false,
    startAlwaysListening: false,
    stopAlwaysListening: false,
    toggleAlwaysListening: false,
    setAlwaysListeningSensitivity: false,
    setAlwaysListeningWakeWords: false,
  });
  const [appToOpen, setAppToOpen] = useState<string>("TextEdit");
  const [urlToOpen, setUrlToOpen] = useState<string>("https://www.google.com");
  const [waitDuration, setWaitDuration] = useState<string>("1000");

  const handleOpenApp = async () => {
    if (!appToOpen.trim()) {
      toast.error("Please enter an app name.");
      return;
    }
    await invokeCommand(
      "dev_open_app",
      { appName: appToOpen.trim() },
      "openApp"
    );
  };

  const handleOpenUrl = async () => {
    if (!urlToOpen.trim()) {
      toast.error("Please enter a URL.");
      return;
    }
    await invokeCommand("dev_open_url", { url: urlToOpen.trim() }, "openUrl");
  };

  const handleWait = async () => {
    const duration = parseInt(waitDuration, 10);
    if (isNaN(duration) || duration <= 0) {
      toast.error("Please enter a valid duration in milliseconds.");
      return;
    }
    await invokeCommand("dev_wait", { duration }, "wait");
  };

  return (
    <ScrollArea className="h-full w-full rounded-md border p-4">
      <div className="space-y-6">
        <div>
          <h2 className="text-lg font-semibold">Cloud & WebSocket Testing</h2>
          <Separator className="my-2" />
          <CloudTestPanel />
        </div>

        <div>
          <h2 className="text-lg font-semibold">Wake Word Testing</h2>
          <Separator className="my-2" />
          <WakeWordTesting
            loadingStates={loadingStates}
            setLoadingStates={setLoadingStates}
          />
        </div>

        <div>
          <h2 className="text-lg font-semibold">Screenshot & Visualization</h2>
          <Separator className="my-2" />
          <ScreenshotOperations />
        </div>

        <div>
          <h2 className="text-lg font-semibold">Keyboard Operations</h2>
          <Separator className="my-2" />
          <KeyboardOperations />
        </div>

        <div>
          <h2 className="text-lg font-semibold">Mouse Operations</h2>
          <Separator className="my-2" />
          <MouseOperations />
        </div>

        <div>
          <h2 className="text-lg font-semibold">Window Operations</h2>
          <Separator className="my-2" />
          <WindowOperations />
        </div>

        <div>
          <h2 className="text-lg font-semibold">File Operations</h2>
          <Separator className="my-2" />
          <FileOperations />
        </div>

        <div>
          <h2 className="text-lg font-semibold">Application Control</h2>
          <Separator className="my-2" />
          <div className="space-y-4">
            <div className="flex items-center space-x-2">
              <ExternalLink className="h-4 w-4" />
              <Input
                placeholder="App name (e.g., TextEdit)"
                value={appToOpen}
                onChange={(e) => setAppToOpen(e.target.value)}
              />
              <Button onClick={handleOpenApp}>Open App</Button>
            </div>

            <div className="flex items-center space-x-2">
              <ExternalLink className="h-4 w-4" />
              <Input
                placeholder="URL to open"
                value={urlToOpen}
                onChange={(e) => setUrlToOpen(e.target.value)}
              />
              <Button onClick={handleOpenUrl}>Open URL</Button>
            </div>

            <div className="flex items-center space-x-2">
              <Timer className="h-4 w-4" />
              <Input
                placeholder="Wait duration (ms)"
                value={waitDuration}
                onChange={(e) => setWaitDuration(e.target.value)}
              />
              <Button onClick={handleWait}>Wait</Button>
            </div>
          </div>
        </div>
      </div>
    </ScrollArea>
  );
};

export default DevToolsPanel;
