import { useSettingsContext } from "@/contexts/SettingsContext";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
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
  Terminal,
  Wrench,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
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
import {
  AdvancedSettingsProvider,
  useAdvancedSettings,
} from "./AdvancedSettingsContext";
import { SettingsCategory } from "./types";

/**
 * Sidebar sections. `advanced: true` hides a section until the
 * "Advanced settings" toggle is on. Sections without the flag can still
 * gate individual cards/fields with `<AdvancedOnly>`.
 */
export const settingsCategories: SettingsCategory[] = [
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
    id: "tools",
    name: "Tools",
    icon: <Wrench className="w-8 h-8" />,
    description: "Enable/disable agent tools and categories",
    advanced: true,
  },
  {
    id: "automations",
    name: "Automations",
    icon: <CalendarClock className="w-8 h-8" />,
    description: "Scheduled agent tasks that run automatically",
    advanced: true,
  },
  {
    id: "network",
    name: "Network",
    icon: <Network className="w-8 h-8" />,
    description: "MCP servers and network configuration",
    advanced: true,
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
    id: "advanced",
    name: "Advanced",
    icon: <Terminal className="w-8 h-8" />,
    description: "System settings and reset options",
    advanced: true,
  },
];

/** Categories visible for the given toggle state. Exported for tests. */
export function visibleCategories(showAdvanced: boolean): SettingsCategory[] {
  return settingsCategories.filter((c) => showAdvanced || !c.advanced);
}

export default function ModularSettingsWindow() {
  return (
    <AdvancedSettingsProvider>
      <SettingsWindowContent />
    </AdvancedSettingsProvider>
  );
}

function SettingsWindowContent() {
  const [selectedCategory, setSelectedCategory] = useState("general");
  const settings = useSettingsContext();
  const { advanced, loading: advancedLoading, setAdvanced } =
    useAdvancedSettings();
  const window = getCurrentWindow();

  const categories = useMemo(() => visibleCategories(advanced), [advanced]);

  // Turning the toggle off while on a hidden section: jump to the first
  // visible one so the content pane never shows an orphaned section.
  useEffect(() => {
    if (!categories.some((c) => c.id === selectedCategory)) {
      setSelectedCategory(categories[0]?.id ?? "general");
    }
  }, [categories, selectedCategory]);

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

  const current = categories.find((c) => c.id === selectedCategory);

  return (
    <div className="flex w-full min-w-0 h-screen bg-gray-50">
      {/* Sidebar with categories - macOS style */}
      <div className="w-64 shrink-0 bg-white border-r border-gray-200 flex flex-col">
        <div className="p-6 border-b border-gray-200">
          <h1 className="text-xl font-semibold text-gray-900">Settings</h1>
        </div>

        <div className="flex-1 overflow-y-auto p-4">
          <nav aria-label="Settings sections" className="space-y-1">
            {categories.map((category) => (
              <button
                key={category.id}
                onClick={() => setSelectedCategory(category.id)}
                aria-current={
                  selectedCategory === category.id ? "page" : undefined
                }
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
          </nav>
        </div>

        {/* Footer: advanced toggle (always visible) + close button */}
        <div className="p-4 border-t border-gray-200 space-y-3">
          <div className="flex items-center justify-between gap-3 px-1">
            <div className="min-w-0">
              <Label
                htmlFor="advanced-settings-toggle"
                className="text-sm font-medium text-gray-900"
              >
                Advanced settings
              </Label>
              <p className="text-xs text-gray-500 mt-0.5 leading-tight">
                Show every option
              </p>
            </div>
            <Switch
              id="advanced-settings-toggle"
              checked={advanced}
              onCheckedChange={(checked) => {
                void setAdvanced(checked);
              }}
              disabled={advancedLoading}
            />
          </div>
          <button
            onClick={handleCloseWindow}
            className="w-full px-4 py-2 bg-gray-100 hover:bg-gray-200 rounded-lg text-sm font-medium text-gray-700 transition-colors"
          >
            Close Settings
          </button>
        </div>
      </div>

      {/* Main content area */}
      <div className="flex-1 min-w-0 flex flex-col">
        {/* Title bar area */}
        <div className="h-12 flex items-center justify-between px-6 bg-transparent">
          <div className="flex items-center gap-3">
            <div className="text-gray-500">{current?.icon}</div>
            <h2 className="text-lg font-semibold text-gray-900">
              {current?.name}
            </h2>
          </div>
        </div>

        {/* Settings content */}
        <div className="flex-1 overflow-y-auto p-6">
          <div className="max-w-2xl">{renderCategoryContent()}</div>
        </div>
      </div>
    </div>
  );
}
