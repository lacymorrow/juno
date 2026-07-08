import { useSettingsContext } from "@/contexts/SettingsContext";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import {
  Cpu,
  Keyboard,
  Mic,
  Network,
  Settings,
  Shield,
  Terminal,
  Wrench,
} from "lucide-react";
import { useEffect, useState } from "react";
import {
  GeneralSettings,
  VoiceSettings,
  AIProviderSettings,
  SecuritySettings,
  AdvancedSettings,
  NetworkSettings,
  ShortcutsSettings,
  ToolsSettings,
} from "./index";
import { SettingsCategory } from "./types";

const settingsCategories: SettingsCategory[] = [
  { id: "general", name: "General", icon: <Settings className="w-4 h-4" /> },
  { id: "ai", name: "AI Provider", icon: <Cpu className="w-4 h-4" /> },
  { id: "voice", name: "Voice & Audio", icon: <Mic className="w-4 h-4" /> },
  { id: "tools", name: "Tools", icon: <Wrench className="w-4 h-4" /> },
  { id: "network", name: "Network", icon: <Network className="w-4 h-4" /> },
  { id: "security", name: "Permissions", icon: <Shield className="w-4 h-4" /> },
  { id: "shortcuts", name: "Shortcuts", icon: <Keyboard className="w-4 h-4" /> },
  { id: "advanced", name: "Advanced", icon: <Terminal className="w-4 h-4" /> },
];

export default function ModularSettingsWindow() {
  const [selectedCategory, setSelectedCategory] = useState("general");
  const settings = useSettingsContext();
  const window = getCurrentWindow();

  useEffect(() => {
    const setupWindow = async () => {
      try {
        await window.setTitle("Juno Settings");
      } catch (error) {
        console.error("Failed to setup settings window:", error);
      }
    };
    setupWindow();
  }, [window]);

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

  const selectedName =
    settingsCategories.find((c) => c.id === selectedCategory)?.name ?? "";

  return (
    <div className="flex w-full min-w-0 h-screen bg-background">
      {/* Sidebar */}
      <div className="w-48 shrink-0 border-r border-border flex flex-col bg-muted/30">
        <div className="p-4 pb-2">
          <h1 className="text-sm font-semibold text-foreground tracking-tight">
            Settings
          </h1>
        </div>

        <nav className="flex-1 overflow-y-auto px-2 pb-2">
          <div className="space-y-0.5">
            {settingsCategories.map((category) => (
              <button
                key={category.id}
                onClick={() => setSelectedCategory(category.id)}
                className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-md text-left transition-colors text-[13px] ${
                  selectedCategory === category.id
                    ? "bg-accent text-accent-foreground font-medium"
                    : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                }`}
              >
                <span className="shrink-0">{category.icon}</span>
                <span className="truncate">{category.name}</span>
              </button>
            ))}
          </div>
        </nav>

        <div className="p-2 border-t border-border">
          <button
            onClick={handleCloseWindow}
            className="w-full px-2.5 py-1.5 rounded-md text-[13px] text-muted-foreground hover:bg-accent/50 hover:text-foreground transition-colors"
          >
            Close
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0 flex flex-col">
        <div className="px-6 pt-5 pb-2">
          <h2 className="text-lg font-semibold text-foreground">{selectedName}</h2>
        </div>

        <div className="flex-1 overflow-y-auto px-6 pb-6">
          <div className="max-w-2xl">{renderCategoryContent()}</div>
        </div>
      </div>
    </div>
  );
}
