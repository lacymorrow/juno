import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { invokeCommand } from "@/lib/utils";
import type { LoadingStates } from "@/types/devtools";
import { ExternalLink, Timer } from "lucide-react";
import React, { useState, useCallback, useMemo } from "react";
import { toast } from "sonner";
import { CloudTestPanel } from "./devtools/CloudTestPanel";
import FileOperations from "./devtools/FileOperations";
import KeyboardOperations from "./devtools/KeyboardOperations";
import MouseOperations from "./devtools/MouseOperations";
import ScreenshotOperations from "./devtools/ScreenshotOperations";
import VisualizationSettings from "./devtools/VisualizationSettings";
import WakeWordTesting from "./devtools/WakeWordTesting";
import WindowOperations from "./devtools/WindowOperations";

// Custom hook for optimized loading state management
const useOptimizedLoadingStates = () => {
  const [loadingSet, setLoadingSet] = useState<Set<string>>(new Set());

  const setLoading = useCallback((key: string, isLoading: boolean) => {
    setLoadingSet((prev) => {
      const newSet = new Set(prev);
      if (isLoading) {
        newSet.add(key);
      } else {
        newSet.delete(key);
      }
      return newSet;
    });
  }, []);

  const isLoading = useCallback(
    (key: string) => {
      return loadingSet.has(key);
    },
    [loadingSet]
  );

  // Convert to LoadingStates format for compatibility with existing components
  const loadingStates = useMemo<LoadingStates>(
    () => ({
      screenshot: loadingSet.has("screenshot"),
      focusInfo: loadingSet.has("focusInfo"),
      focusDelay: loadingSet.has("focusDelay"),
      elementScreenshot: loadingSet.has("elementScreenshot"),
      clickFocus: loadingSet.has("clickFocus"),
      typeText: loadingSet.has("typeText"),
      pressKey: loadingSet.has("pressKey"),
      openApp: loadingSet.has("openApp"),
      openUrl: loadingSet.has("openUrl"),
      scroll: loadingSet.has("scroll"),
      globalTypeText: loadingSet.has("globalTypeText"),
      getClipboard: loadingSet.has("getClipboard"),
      setClipboard: loadingSet.has("setClipboard"),
      holdKey: loadingSet.has("holdKey"),
      releaseKey: loadingSet.has("releaseKey"),
      wait: loadingSet.has("wait"),
      findElement: loadingSet.has("findElement"),
      clickElement: loadingSet.has("clickElement"),
      getSelectedText: loadingSet.has("getSelectedText"),
      getWindowList: loadingSet.has("getWindowList"),
      getWindowInfo: loadingSet.has("getWindowInfo"),
      focusWindow: loadingSet.has("focusWindow"),
      resizeWindow: loadingSet.has("resizeWindow"),
      moveWindow: loadingSet.has("moveWindow"),
      closeWindow: loadingSet.has("closeWindow"),
      listFiles: loadingSet.has("listFiles"),
      getFileContent: loadingSet.has("getFileContent"),
      setFileContent: loadingSet.has("setFileContent"),
      mouseMove: loadingSet.has("mouseMove"),
      mouseDown: loadingSet.has("mouseDown"),
      mouseUp: loadingSet.has("mouseUp"),
      mouseClick: loadingSet.has("mouseClick"),
      mouseDoubleClick: loadingSet.has("mouseDoubleClick"),
      mouseDrag: loadingSet.has("mouseDrag"),
      testClickVisualization: loadingSet.has("testClickVisualization"),
      setDeveloperPlayback: loadingSet.has("setDeveloperPlayback"),
      playbackAudio: loadingSet.has("playbackAudio"),
      setTtsProvider: loadingSet.has("setTtsProvider"),
      testSystemContext: loadingSet.has("testSystemContext"),
      debugAlwaysListening: loadingSet.has("debugAlwaysListening"),
      startAlwaysListening: loadingSet.has("startAlwaysListening"),
      stopAlwaysListening: loadingSet.has("stopAlwaysListening"),
      toggleAlwaysListening: loadingSet.has("toggleAlwaysListening"),
      setAlwaysListeningSensitivity: loadingSet.has(
        "setAlwaysListeningSensitivity"
      ),
      setAlwaysListeningWakeWords: loadingSet.has(
        "setAlwaysListeningWakeWords"
      ),
    }),
    [loadingSet]
  );

  // Compatible setLoadingStates function for existing components
  const setLoadingStates = useCallback(
    (updateFn: React.SetStateAction<LoadingStates>) => {
      if (typeof updateFn === "function") {
        const currentStates = loadingStates;
        const newStates = updateFn(currentStates);

        // Update the set based on changes
        setLoadingSet(() => {
          const newSet = new Set<string>();
          Object.entries(newStates).forEach(([key, value]) => {
            if (value) {
              newSet.add(key);
            }
          });
          return newSet;
        });
      } else {
        // Direct state replacement
        const newSet = new Set<string>();
        Object.entries(updateFn).forEach(([key, value]) => {
          if (value) {
            newSet.add(key);
          }
        });
        setLoadingSet(newSet);
      }
    },
    [loadingStates]
  );

  return { loadingStates, setLoadingStates, setLoading, isLoading };
};

const DevToolsPanel: React.FC = () => {
  const { loadingStates, setLoadingStates } = useOptimizedLoadingStates();
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
          <h2 className="text-lg font-semibold">Visualization Controls</h2>
          <Separator className="my-2" />
          <VisualizationSettings />
        </div>

        <div>
          <h2 className="text-lg font-semibold">Screenshot & Testing</h2>
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
