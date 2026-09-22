import React, { useState, useEffect } from "react";
import {
  Code,
  Search,
  FileText,
  Globe,
  Zap,
  Target,
  Palette,
  Monitor,
  Camera,
  Mouse,
  Gamepad2,
  Calculator,
  Keyboard,
  Play,
  TestTube,
  MousePointer2,
  RotateCcw,
  Grid3X3,
  Settings,
  ChevronRight,
  Loader2,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { isDevelopment } from "@/lib";
import { cn } from "@/lib/utils";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";

interface ExamplePrompt {
  icon: LucideIcon;
  title: string;
  prompt: string;
}

/** Whether the backend can take a query yet; the main window's `serverStatus`. */
export type BackendStatus = "connecting" | "connected" | "error";

interface ExamplePromptsProps {
  onPromptSelect: (prompt: string) => void;
  /**
   * Until the backend is connected the buttons wait, disabled, under one
   * quiet "Connecting…" line, and come alive the instant it is. Before this
   * they looked ready and did nothing, which read as broken.
   */
  backendStatus?: BackendStatus;
}

export const ExamplePrompts: React.FC<ExamplePromptsProps> = ({
  onPromptSelect,
  backendStatus = "connected",
}) => {
  const connecting = backendStatus === "connecting";
  const [isDevMode, setIsDevMode] = useState(false);
  // Closed on every mount so the pane always opens on the real empty state.
  // The dev commands are a workbench, not the first thing anyone should read.
  const [devCommandsOpen, setDevCommandsOpen] = useState(false);

  useEffect(() => {
    const checkDevMode = async () => {
      try {
        const devMode = await isDevelopment();
        setIsDevMode(devMode);
      } catch (error) {
        console.warn("Failed to check development mode:", error);
      }
    };

    checkDevMode();
  }, []);

  const productionPrompts: ExamplePrompt[] = [
    {
      icon: Globe,
      title: "Browse Web",
      prompt: "Open Google and search for the latest macOS updates",
    },
    {
      icon: Monitor,
      title: "Screenshot",
      prompt: "Take a screenshot and open System Preferences",
    },
    {
      icon: FileText,
      title: "Create Note",
      prompt: "Create a new note about my daily goals and open it",
    },
    {
      icon: Code,
      title: "Code Help",
      prompt: "Help me understand the current project structure",
    },
    {
      icon: Camera,
      title: "Describe Screen",
      prompt: "Take a screenshot and describe what's on my screen",
    },
    {
      icon: Search,
      title: "Research",
      prompt: "Research the benefits of AI-human collaboration",
    },
  ];

  const developmentPrompts: ExamplePrompt[] = [
    {
      icon: Mouse,
      title: "Mouse Square",
      prompt:
        "Move your mouse in a perfect square pattern on the screen, then return to center",
    },
    {
      icon: Target,
      title: "Click Test",
      prompt:
        "Take a screenshot, identify a safe clickable element, and perform a precise click on it",
    },
    {
      icon: MousePointer2,
      title: "Mouse Circle",
      prompt:
        "Move your mouse in a smooth circular pattern, making 3 complete circles",
    },
    {
      icon: RotateCcw,
      title: "Mouse Spiral",
      prompt:
        "Move your mouse in an expanding spiral pattern from center outward",
    },
    {
      icon: Keyboard,
      title: "Type Test",
      prompt:
        "Open TextEdit, type 'Hello from Juno AI! Testing keyboard input with special characters: @#$%&*', then save the file",
    },
    {
      icon: Code,
      title: "Code Typing",
      prompt:
        "Open a text editor and type a simple 'Hello World' function in Python with proper indentation",
    },
    {
      icon: Calculator,
      title: "Calculator Test",
      prompt:
        "Open the Calculator app and perform the calculation: 123 + 456 * 2, then tell me the result",
    },
    {
      icon: Settings,
      title: "System Prefs",
      prompt:
        "Open System Preferences, navigate to General settings, take a screenshot, then close it",
    },
    {
      icon: Gamepad2,
      title: "Chess Game",
      prompt:
        "Let's play chess! Open Chess.com or a chess app, start a new game, and make the first move as white",
    },
    {
      icon: Camera,
      title: "Desktop Scan",
      prompt:
        "Take a screenshot and provide a detailed description of every application, window, and UI element visible",
    },
    {
      icon: Monitor,
      title: "Window Count",
      prompt:
        "Take a screenshot and count how many windows are currently open, listing each application",
    },
    {
      icon: Grid3X3,
      title: "Screen Grid",
      prompt:
        "Take a screenshot and describe the screen layout using a 3x3 grid system (top-left, center, bottom-right, etc.)",
    },
    {
      icon: Zap,
      title: "Quick Tasks",
      prompt:
        "Perform this sequence: 1) Take screenshot 2) Open Finder 3) Navigate to Desktop 4) Take another screenshot 5) Compare what changed",
    },
    {
      icon: TestTube,
      title: "Web Navigation",
      prompt:
        "Open Safari, navigate to apple.com, click on a product category, take a screenshot, then go back to homepage",
    },
    {
      icon: Play,
      title: "Media Control",
      prompt:
        "Open Music app (or Spotify), search for 'relaxing music', play a song, adjust volume, then pause it",
    },
    {
      icon: Palette,
      title: "Creative Test",
      prompt:
        "Open a drawing or design app (like Preview or Photoshop), create a simple shape, change its color, and save the file",
    },
    {
      icon: FileText,
      title: "File Manager",
      prompt:
        "Create a new folder on Desktop called 'JunoTest', create a text file inside it, write some content, and take a screenshot",
    },
    {
      icon: Target,
      title: "Precision Test",
      prompt:
        "Open a drawing app and draw a perfect circle using only mouse movements, then draw a square inside it",
    },
  ];

  const renderPrompts = (prompts: ExamplePrompt[]) => (
    <div className="flex flex-wrap justify-center gap-2">
      {prompts.map((example, index) => (
        <button
          key={index}
          type="button"
          disabled={connecting}
          className={cn(
            "inline-flex items-center gap-1.5 rounded-full border border-border/40 px-3 py-1.5 text-xs text-foreground/80 transition-colors",
            connecting
              ? "cursor-default bg-secondary/30 opacity-50"
              : "cursor-pointer bg-secondary/50 hover:bg-secondary hover:text-foreground",
          )}
          onClick={() => onPromptSelect(example.prompt)}
        >
          <example.icon size={12} className="opacity-60 flex-shrink-0" />
          <span>{example.title}</span>
        </button>
      ))}
    </div>
  );

  return (
    <div className="w-full max-w-lg mx-auto space-y-3">
      {/* Development builds used to swap these out entirely, which meant nobody
          working on Juno ever saw the empty state a real user gets. */}
      {renderPrompts(productionPrompts)}

      {/* One line, no toast. The buttons above are the only thing on screen
          that could look ready, so this says why they are not yet. */}
      {connecting && (
        <div
          role="status"
          data-testid="example-prompts-connecting"
          className="flex items-center justify-center gap-1.5 text-xs text-muted-foreground"
        >
          <Loader2 size={12} className="animate-spin" aria-hidden="true" />
          <span>Connecting…</span>
        </div>
      )}
      {/* The empty state ignores system messages, so the backend's "not
          responding" notice would otherwise be invisible here. */}
      {backendStatus === "error" && (
        <p
          role="status"
          data-testid="example-prompts-error"
          className="text-center text-xs text-muted-foreground"
        >
          Backend is not responding. Check the logs.
        </p>
      )}

      {isDevMode && (
        <Collapsible open={devCommandsOpen} onOpenChange={setDevCommandsOpen}>
          <CollapsibleTrigger className="mx-auto flex w-fit items-center gap-1 rounded px-2 py-1 text-xs text-muted-foreground hover:text-foreground transition-colors cursor-pointer outline-none focus-visible:ring-ring/50 focus-visible:ring-[3px]">
            <ChevronRight
              size={12}
              className={cn(
                "opacity-60 transition-transform",
                devCommandsOpen && "rotate-90"
              )}
            />
            <span>Development test commands</span>
            <span className="opacity-50 tabular-nums">
              {developmentPrompts.length}
            </span>
          </CollapsibleTrigger>
          {/* Capped and scrolled in its own box: expanding a developer list must
              never push the conversation it introduces out of the pane. */}
          <CollapsibleContent>
            <div className="mt-2 max-h-40 overflow-y-auto overscroll-contain rounded-md border border-border/40 p-2">
              {renderPrompts(developmentPrompts)}
            </div>
          </CollapsibleContent>
        </Collapsible>
      )}
    </div>
  );
};
