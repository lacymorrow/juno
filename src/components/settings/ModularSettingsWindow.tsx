import { useSettingsContext } from "@/contexts/SettingsContext";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import {
  Brain,
  CalendarClock,
  Keyboard,
  Mic,
  Network,
  Settings,
  Shield,
  SlidersHorizontal,
  Terminal,
  Wrench,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import {
  GeneralSettings,
  VoiceSettings,
  AIProviderSettings,
  SecuritySettings,
  AdvancedSettings,
  AutomationsSettings,
  NetworkSettings,
  ShortcutsSettings,
  ToolsSettings,
} from "./index";
import { SettingsCategory } from "./types";
import { AdvancedSettingsProvider } from "./SettingsPrimitives";

const settingsCategories: SettingsCategory[] = [
  {
    id: "general",
    name: "General",
    icon: <Settings className="h-4 w-4" />,
    description: "Basic app settings and preferences",
    tier: "essential",
  },
  {
    id: "voice",
    name: "Voice & Audio",
    icon: <Mic className="h-4 w-4" />,
    description: "Voice transcription and audio settings",
    tier: "essential",
  },
  {
    id: "ai",
    name: "AI Provider",
    icon: <Brain className="h-4 w-4" />,
    description: "Configure AI models and providers",
    tier: "essential",
  },
  {
    id: "tools",
    name: "Tools",
    icon: <Wrench className="h-4 w-4" />,
    description: "Enable/disable agent tools and categories",
    tier: "advanced",
  },
  {
    id: "automations",
    name: "Automations",
    icon: <CalendarClock className="h-4 w-4" />,
    description: "Scheduled agent tasks that run automatically",
    tier: "advanced",
  },
  {
    id: "network",
    name: "Network",
    icon: <Network className="h-4 w-4" />,
    description: "MCP servers and network configuration",
    tier: "advanced",
  },
  {
    id: "security",
    name: "Security & Privacy",
    icon: <Shield className="h-4 w-4" />,
    description: "Permissions and security settings",
    tier: "advanced",
  },
  {
    id: "shortcuts",
    name: "Keyboard Shortcuts",
    icon: <Keyboard className="h-4 w-4" />,
    description: "Customize keyboard shortcuts",
    tier: "advanced",
  },
  {
    id: "advanced",
    name: "Advanced",
    icon: <Terminal className="h-4 w-4" />,
    description: "System settings and reset options",
    tier: "advanced",
  },
];

export default function ModularSettingsWindow() {
  const [selectedCategory, setSelectedCategory] = useState("general");
  // Progressive disclosure: default to the simple view every time settings
  // opens (Claude / Wispr Flow behavior). Advanced categories and rows stay
  // hidden until the user opts in — no persistence needed, keeps it basic.
  const [showAdvanced, setShowAdvanced] = useState(false);
  const settings = useSettingsContext();
  const window = getCurrentWindow();

  useEffect(() => {
    // Set up the window properly for macOS
    const setupWindow = async () => {
      try {
        await window.setTitle("Juno Settings");
      } catch (error) {
        console.error("Failed to setup modular settings window:", error);
      }
    };

    setupWindow();
  }, [window]);

  // If advanced is turned off while viewing an advanced category, fall back to
  // General so the content pane is never left blank.
  useEffect(() => {
    if (showAdvanced) return;
    const current = settingsCategories.find((c) => c.id === selectedCategory);
    if (current?.tier === "advanced") {
      setSelectedCategory("general");
    }
  }, [showAdvanced, selectedCategory]);

  const handleCloseWindow = async () => {
    try {
      await invoke("close_settings_window");
    } catch (error) {
      console.error("Failed to close settings window:", error);
    }
  };

  const renderCategoryContent = () => {
    switch (selectedCategory) {
      case "general":
        return <GeneralSettings settings={settings} />;
      case "voice":
        return <VoiceSettings settings={settings} />;
      case "ai":
        return <AIProviderSettings settings={settings} />;
      case "tools":
        return <ToolsSettings settings={settings} />;
      case "automations":
        return <AutomationsSettings />;
      case "network":
        return <NetworkSettings settings={settings} />;
      case "security":
        return <SecuritySettings />;
      case "shortcuts":
        return <ShortcutsSettings settings={settings} />;
      case "advanced":
        return <AdvancedSettings settings={settings} />;
      default:
        return <GeneralSettings settings={settings} />;
    }
  };

  const essentials = settingsCategories.filter((c) => c.tier === "essential");
  const advanced = settingsCategories.filter((c) => c.tier === "advanced");
  const activeCategory = settingsCategories.find(
    (c) => c.id === selectedCategory
  );

  const renderNavButton = (category: SettingsCategory) => {
    const active = selectedCategory === category.id;
    return (
      <button
        key={category.id}
        onClick={() => setSelectedCategory(category.id)}
        className={cn(
          "flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-left text-sm transition-colors",
          active
            ? "bg-accent text-accent-foreground font-medium"
            : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
        )}
      >
        <span className={active ? "text-foreground" : "text-muted-foreground"}>
          {category.icon}
        </span>
        <span className="truncate">{category.name}</span>
      </button>
    );
  };

  return (
    <div className="flex h-screen w-full min-w-0 bg-background text-foreground">
      {/* Sidebar */}
      <div className="flex w-60 shrink-0 flex-col border-r border-border bg-muted/30">
        <div className="px-5 pb-3 pt-6">
          <h1 className="text-base font-semibold">Settings</h1>
        </div>

        <nav className="flex-1 space-y-4 overflow-y-auto px-3 py-2">
          <div className="space-y-0.5">{essentials.map(renderNavButton)}</div>

          {showAdvanced && (
            <div className="space-y-0.5">
              <div className="px-2.5 pb-1 pt-2 text-[11px] font-medium uppercase tracking-wide text-muted-foreground/70">
                Advanced
              </div>
              {advanced.map(renderNavButton)}
            </div>
          )}
        </nav>

        {/* Show-advanced toggle + close */}
        <div className="space-y-3 border-t border-border p-3">
          <label
            htmlFor="show-advanced"
            className="flex cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-1.5 hover:bg-accent/50"
          >
            <span className="flex items-center gap-2 text-sm text-muted-foreground">
              <SlidersHorizontal className="h-4 w-4" />
              Advanced settings
            </span>
            <Switch
              id="show-advanced"
              checked={showAdvanced}
              onCheckedChange={setShowAdvanced}
            />
          </label>
          <button
            onClick={handleCloseWindow}
            className="w-full rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-foreground transition-colors hover:bg-accent/80"
          >
            Close
          </button>
        </div>
      </div>

      {/* Main content */}
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 items-center gap-2.5 border-b border-border px-6">
          <span className="text-muted-foreground">{activeCategory?.icon}</span>
          <h2 className="text-base font-semibold">{activeCategory?.name}</h2>
        </header>

        <div className="flex-1 overflow-y-auto px-6 py-6">
          <div className="mx-auto max-w-2xl space-y-6">
            <AdvancedSettingsProvider showAdvanced={showAdvanced}>
              {renderCategoryContent()}
            </AdvancedSettingsProvider>
          </div>
        </div>
      </div>
    </div>
  );
}
