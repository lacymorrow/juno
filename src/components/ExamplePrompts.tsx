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
} from "lucide-react";
import { isDevelopment } from "@/lib";

interface ExamplePromptsProps {
  onPromptSelect: (prompt: string) => void;
}

export const ExamplePrompts: React.FC<ExamplePromptsProps> = ({
  onPromptSelect,
}) => {
  const [isDevMode, setIsDevMode] = useState(false);

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

  const productionPrompts = [
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

  const developmentPrompts = [
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

  const prompts = isDevMode ? developmentPrompts : productionPrompts;

  return (
    <div className="w-full max-w-lg mx-auto">
      {isDevMode && (
        <div className="text-xs text-amber-600 dark:text-amber-400 mb-3 text-center font-medium">
          Development Test Commands
        </div>
      )}
      <div className="flex flex-wrap justify-center gap-2">
        {prompts.map((example, index) => (
          <button
            key={index}
            className="inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs bg-secondary/50 hover:bg-secondary border border-border/40 text-foreground/80 hover:text-foreground transition-colors cursor-pointer"
            onClick={() => onPromptSelect(example.prompt)}
          >
            <example.icon size={12} className="opacity-60 flex-shrink-0" />
            <span>{example.title}</span>
          </button>
        ))}
      </div>
      {isDevMode && (
        <div className="text-xs text-muted-foreground text-center mt-3">
          Click any command to test agent capabilities
        </div>
      )}
    </div>
  );
};
