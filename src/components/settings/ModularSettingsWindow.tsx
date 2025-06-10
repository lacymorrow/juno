import { useSettings } from "@/hooks/useSettings";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import {
  Brain,
  Keyboard,
  Mic,
  MonitorSpeaker,
  Network,
  Settings,
  Shield,
  Terminal,
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
  ToolsSettings
} from './index';
import { SettingsCategory } from './types';

const settingsCategories: SettingsCategory[] = [
  {
    id: "general",
    name: "General",
    icon: <Settings className="w-8 h-8" />,
    description: "Basic app settings and preferences",
  },
  {
    id: "voice",
    name: "Voice & Audio",
    icon: <Mic className="w-8 h-8" />,
    description: "Voice transcription and audio settings",
  },
  {
    id: "ai",
    name: "AI Provider",
    icon: <Brain className="w-8 h-8" />,
    description: "Configure AI models and providers",
  },
  {
    id: "network",
    name: "Network",
    icon: <Network className="w-8 h-8" />,
    description: "MCP servers and network configuration",
  },
  {
    id: "security",
    name: "Security & Privacy",
    icon: <Shield className="w-8 h-8" />,
    description: "Permissions and security settings",
  },
  {
    id: "shortcuts",
    name: "Keyboard Shortcuts",
    icon: <Keyboard className="w-8 h-8" />,
    description: "Customize keyboard shortcuts",
  },
  {
    id: "tools",
    name: "Tools",
    icon: <MonitorSpeaker className="w-8 h-8" />,
    description: "Configure available tools and features",
  },
  {
    id: "advanced",
    name: "Advanced",
    icon: <Terminal className="w-8 h-8" />,
    description: "Advanced settings and developer options",
  },
];

export default function ModularSettingsWindow() {
  const [selectedCategory, setSelectedCategory] = useState("general");
  const settings = useSettings();
  const window = getCurrentWindow();

  useEffect(() => {
    // Set up the window properly for macOS
    const setupWindow = async () => {
      try {
        await window.setTitle("Juno Settings");
        if (window.label === "settings") {
          console.log("Modular settings window initialized");
        }
      } catch (error) {
        console.error("Failed to setup modular settings window:", error);
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
      case "network":
        return <NetworkSettings settings={settings} />;
      case "security":
        return <SecuritySettings settings={settings} />;
      case "shortcuts":
        return <ShortcutsSettings settings={settings} />;
      case "tools":
        return <ToolsSettings settings={settings} />;
      case "advanced":
        return <AdvancedSettings settings={settings} />;
      default:
        return <GeneralSettings settings={settings} />;
    }
  };

  return (
    <div className="flex h-screen bg-gray-50">
      {/* Sidebar with categories - macOS style */}
      <div className="w-64 bg-white border-r border-gray-200 flex flex-col">
        <div className="p-6 border-b border-gray-200">
          <h1 className="text-xl font-semibold text-gray-900">Settings</h1>
        </div>

        <div className="flex-1 overflow-y-auto p-4">
          <div className="space-y-1">
            {settingsCategories.map((category) => (
              <button
                key={category.id}
                onClick={() => setSelectedCategory(category.id)}
                className={`w-full flex items-center gap-3 px-3 py-3 rounded-lg text-left transition-colors ${
                  selectedCategory === category.id
                    ? "bg-blue-100 text-blue-700"
                    : "text-gray-700 hover:bg-gray-100"
                }`}
              >
                <div
                  className={`${
                    selectedCategory === category.id
                      ? "text-blue-600"
                      : "text-gray-500"
                  }`}
                >
                  {category.icon}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="font-medium text-sm">{category.name}</div>
                  <div className="text-xs text-gray-500 mt-0.5 leading-tight">
                    {category.description}
                  </div>
                </div>
              </button>
            ))}
          </div>
        </div>

        {/* Footer with close button */}
        <div className="p-4 border-t border-gray-200">
          <button
            onClick={handleCloseWindow}
            className="w-full px-4 py-2 bg-gray-100 hover:bg-gray-200 rounded-lg text-sm font-medium text-gray-700 transition-colors"
          >
            Close Settings
          </button>
        </div>
      </div>

      {/* Main content area */}
      <div className="flex-1 flex flex-col">
        {/* Title bar area */}
        <div className="h-12 flex items-center justify-between px-6 bg-transparent">
          <div className="flex items-center gap-3">
            <div className="text-gray-500">
              {settingsCategories.find((c) => c.id === selectedCategory)?.icon}
            </div>
            <h2 className="text-lg font-semibold text-gray-900">
              {settingsCategories.find((c) => c.id === selectedCategory)?.name}
            </h2>
          </div>
        </div>

        {/* Settings content */}
        <div className="flex-1 overflow-y-auto p-6">
          <div className="max-w-2xl">
            {renderCategoryContent()}
          </div>
        </div>
      </div>
    </div>
  );
}