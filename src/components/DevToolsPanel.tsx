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
import SelfImprovementPanel from "./devtools/SelfImprovementPanel";
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
    <ScrollArea className="h-full w-full">
      <div className="p-6 space-y-8">
        {/* Header */}
        <div className="space-y-2">
          <h1 className="text-xl font-semibold bg-gradient-to-r from-purple-700 to-indigo-700 dark:from-purple-300 dark:to-indigo-300 bg-clip-text text-transparent">
            Development Tools
          </h1>
          <p className="text-sm text-muted-foreground">
            Test and debug Juno AI functionality
          </p>
        </div>

        {/* Cloud & WebSocket Testing */}
        <div className="rounded-xl bg-gradient-to-r from-blue-50/50 to-indigo-50/30 dark:from-blue-950/30 dark:to-indigo-950/20 border border-blue-200/50 dark:border-blue-800/50 backdrop-blur-sm p-4">
          <div className="flex items-center gap-2 mb-3">
            <div className="w-2 h-2 rounded-full bg-blue-500"></div>
            <h2 className="text-base font-semibold text-blue-900 dark:text-blue-100">Cloud & WebSocket Testing</h2>
          </div>
          <CloudTestPanel />
        </div>

        {/* Wake Word Testing */}
        <div className="rounded-xl bg-gradient-to-r from-green-50/50 to-emerald-50/30 dark:from-green-950/30 dark:to-emerald-950/20 border border-green-200/50 dark:border-green-800/50 backdrop-blur-sm p-4">
          <div className="flex items-center gap-2 mb-3">
            <div className="w-2 h-2 rounded-full bg-green-500"></div>
            <h2 className="text-base font-semibold text-green-900 dark:text-green-100">Wake Word Testing</h2>
          </div>
          <WakeWordTesting
            loadingStates={loadingStates}
            setLoadingStates={setLoadingStates}
          />
        </div>

        {/* Visualization Controls */}
        <div className="rounded-xl bg-gradient-to-r from-purple-50/50 to-violet-50/30 dark:from-purple-950/30 dark:to-violet-950/20 border border-purple-200/50 dark:border-purple-800/50 backdrop-blur-sm p-4">
          <div className="flex items-center gap-2 mb-3">
            <div className="w-2 h-2 rounded-full bg-purple-500"></div>
            <h2 className="text-base font-semibold text-purple-900 dark:text-purple-100">Visualization Controls</h2>
          </div>
          <VisualizationSettings />
        </div>

        {/* Screenshot & Testing */}
        <div className="rounded-xl bg-gradient-to-r from-orange-50/50 to-amber-50/30 dark:from-orange-950/30 dark:to-amber-950/20 border border-orange-200/50 dark:border-orange-800/50 backdrop-blur-sm p-4">
          <div className="flex items-center gap-2 mb-3">
            <div className="w-2 h-2 rounded-full bg-orange-500"></div>
            <h2 className="text-base font-semibold text-orange-900 dark:text-orange-100">Screenshot & Testing</h2>
          </div>
          <ScreenshotOperations />
        </div>

        {/* Keyboard Operations */}
        <div className="rounded-xl bg-gradient-to-r from-teal-50/50 to-cyan-50/30 dark:from-teal-950/30 dark:to-cyan-950/20 border border-teal-200/50 dark:border-teal-800/50 backdrop-blur-sm p-4">
          <div className="flex items-center gap-2 mb-3">
            <div className="w-2 h-2 rounded-full bg-teal-500"></div>
            <h2 className="text-base font-semibold text-teal-900 dark:text-teal-100">Keyboard Operations</h2>
          </div>
          <KeyboardOperations />
        </div>

        {/* Mouse Operations */}
        <div className="rounded-xl bg-gradient-to-r from-pink-50/50 to-rose-50/30 dark:from-pink-950/30 dark:to-rose-950/20 border border-pink-200/50 dark:border-pink-800/50 backdrop-blur-sm p-4">
          <div className="flex items-center gap-2 mb-3">
            <div className="w-2 h-2 rounded-full bg-pink-500"></div>
            <h2 className="text-base font-semibold text-pink-900 dark:text-pink-100">Mouse Operations</h2>
          </div>
          <MouseOperations />
        </div>

        {/* Window Operations */}
        <div className="rounded-xl bg-gradient-to-r from-indigo-50/50 to-blue-50/30 dark:from-indigo-950/30 dark:to-blue-950/20 border border-indigo-200/50 dark:border-indigo-800/50 backdrop-blur-sm p-4">
          <div className="flex items-center gap-2 mb-3">
            <div className="w-2 h-2 rounded-full bg-indigo-500"></div>
            <h2 className="text-base font-semibold text-indigo-900 dark:text-indigo-100">Window Operations</h2>
          </div>
          <WindowOperations />
        </div>

        {/* File Operations */}
        <div className="rounded-xl bg-gradient-to-r from-yellow-50/50 to-orange-50/30 dark:from-yellow-950/30 dark:to-orange-950/20 border border-yellow-200/50 dark:border-yellow-800/50 backdrop-blur-sm p-4">
          <div className="flex items-center gap-2 mb-3">
            <div className="w-2 h-2 rounded-full bg-yellow-500"></div>
            <h2 className="text-base font-semibold text-yellow-900 dark:text-yellow-100">File Operations</h2>
          </div>
          <FileOperations />
        </div>

        {/* Application Control */}
        <div className="rounded-xl bg-gradient-to-r from-slate-50/50 to-gray-50/30 dark:from-slate-950/30 dark:to-gray-950/20 border border-slate-200/50 dark:border-slate-800/50 backdrop-blur-sm p-4">
          <div className="flex items-center gap-2 mb-3">
            <div className="w-2 h-2 rounded-full bg-slate-500"></div>
            <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">Application Control</h2>
          </div>
          <div className="space-y-3">
            <div className="flex items-center gap-3 p-3 rounded-lg bg-background/50 backdrop-blur-sm border border-border/30">
              <ExternalLink className="h-4 w-4 text-muted-foreground flex-shrink-0" />
              <Input
                placeholder="App name (e.g., TextEdit)"
                value={appToOpen}
                onChange={(e) => setAppToOpen(e.target.value)}
                className="flex-1"
              />
              <Button onClick={handleOpenApp} size="sm" className="flex-shrink-0">
                Open App
              </Button>
            </div>

            <div className="flex items-center gap-3 p-3 rounded-lg bg-background/50 backdrop-blur-sm border border-border/30">
              <ExternalLink className="h-4 w-4 text-muted-foreground flex-shrink-0" />
              <Input
                placeholder="URL to open"
                value={urlToOpen}
                onChange={(e) => setUrlToOpen(e.target.value)}
                className="flex-1"
              />
              <Button onClick={handleOpenUrl} size="sm" className="flex-shrink-0">
                Open URL
              </Button>
            </div>

            <div className="flex items-center gap-3 p-3 rounded-lg bg-background/50 backdrop-blur-sm border border-border/30">
              <Timer className="h-4 w-4 text-muted-foreground flex-shrink-0" />
              <Input
                placeholder="Wait duration (ms)"
                value={waitDuration}
                onChange={(e) => setWaitDuration(e.target.value)}
                className="flex-1"
              />
              <Button onClick={handleWait} size="sm" className="flex-shrink-0">
                Wait
              </Button>
            </div>
          </div>
        </div>

        {/* Self Improvement */}
        <div className="rounded-xl bg-gradient-to-r from-emerald-50/50 to-teal-50/30 dark:from-emerald-950/30 dark:to-teal-950/20 border border-emerald-200/50 dark:border-emerald-800/50 backdrop-blur-sm p-4">
          <div className="flex items-center gap-2 mb-3">
            <div className="w-2 h-2 rounded-full bg-emerald-500"></div>
            <h2 className="text-base font-semibold text-emerald-900 dark:text-emerald-100">Self Improvement</h2>
          </div>
          <SelfImprovementPanel />
        </div>
      </div>
    </ScrollArea>
  );
};

export default DevToolsPanel;
