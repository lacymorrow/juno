import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Keyboard, RefreshCw, RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

import { SettingsSectionProps } from "../types";
import ShortcutInput from "../ShortcutInput";

export default function ShortcutsSettings({ settings }: SettingsSectionProps) {
  const getShortcutDisplayName = (shortcutName: string): string => {
    const names: { [key: string]: string } = {
      agent_mode: "Agent Mode",
      dictation_input: "Start Dictation",
      stop_current_task: "Stop Current Task",
      open_settings: "Open Settings",
    };
    return names[shortcutName] || shortcutName;
  };

  const getShortcutDescription = (shortcutName: string): string => {
    const descriptions: { [key: string]: string } = {
      agent_mode: "Activate agent mode",
      dictation_input: "Activate voice input for dictation",
      stop_current_task: "Stop the current AI task or operation",
      open_settings: "Open the settings window",
    };
    return descriptions[shortcutName] || "";
  };

  const handleShortcutChange = async (shortcutName: string, value: string) => {
    try {
      await invoke("set_keyboard_shortcut", { shortcutName, shortcut: value });
      await settings.loadKeyboardShortcuts();
      toast.success("Keyboard shortcut updated");
    } catch (error) {
      console.error("Failed to set keyboard shortcut:", error);
      toast.error("Failed to update keyboard shortcut");
    }
  };

  const handleResetShortcuts = async () => {
    try {
      await invoke("reset_keyboard_shortcuts");
      await settings.loadKeyboardShortcuts();
      toast.success("Keyboard shortcuts reset to defaults");
    } catch (error) {
      console.error("Failed to reset keyboard shortcuts:", error);
      toast.error("Failed to reset keyboard shortcuts");
    }
  };

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Keyboard size={20} />
            Global Shortcuts
          </CardTitle>
          <CardDescription>
            Configure keyboard shortcuts that work system-wide with interactive
            key capture
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {settings.shortcutsLoading ? (
            <div className="flex items-center justify-center py-8">
              <RefreshCw className="h-6 w-6 animate-spin" />
              <span className="ml-2">Loading shortcuts...</span>
            </div>
          ) : (
            <div className="space-y-4">
              {/* Customizable Shortcuts */}
              <div className="space-y-4">
                <h4 className="text-sm font-medium text-muted-foreground uppercase tracking-wide border-b pb-2">
                  Customizable Shortcuts
                </h4>
                {Object.entries(settings.keyboardShortcuts)
                  .filter(([key]) => key !== "open_settings") // Don't allow changing settings shortcut
                  .map(([shortcutName, shortcutValue]) => (
                    <ShortcutInput
                      key={shortcutName}
                      label={getShortcutDisplayName(shortcutName)}
                      description={getShortcutDescription(shortcutName)}
                      value={shortcutValue}
                      shortcutName={shortcutName}
                      isSystemManaged={false}
                      onSave={handleShortcutChange}
                      isLoading={settings.shortcutsLoading}
                    />
                  ))}
              </div>

              {/* Fixed System Shortcuts */}
              <div className="space-y-4">
                <h4 className="text-sm font-medium text-muted-foreground uppercase tracking-wide border-b pb-2">
                  System Shortcuts
                </h4>
                <div className="space-y-3">
                  <ShortcutInput
                    label="Cancel Current Operation"
                    description="Stop any running AI task or operation"
                    value="Escape"
                    shortcutName="stop_current_task"
                    isSystemManaged={true}
                    onSave={handleShortcutChange}
                    isLoading={settings.shortcutsLoading}
                  />
                  <ShortcutInput
                    label="Open Settings"
                    description="Open the settings menu"
                    value={settings.keyboardShortcuts.open_settings || "⌘+,"}
                    shortcutName="open_settings"
                    isSystemManaged={true}
                    onSave={handleShortcutChange}
                    isLoading={settings.shortcutsLoading}
                  />
                </div>
              </div>
            </div>
          )}

          <div className="pt-4 border-t">
            <Button
              onClick={handleResetShortcuts}
              variant="outline"
              disabled={settings.shortcutsLoading}
              className="w-full"
            >
              <RotateCcw className="w-4 h-4 mr-2" />
              Reset to Defaults
            </Button>
          </div>

          {/* Usage Tips */}
          <div className="bg-muted/50 p-4 rounded-lg">
            <h5 className="text-sm font-medium mb-2">
              💡 Keyboard Shortcut Tips
            </h5>
            <ul className="text-xs text-muted-foreground space-y-1 list-disc list-inside">
              <li>
                Click on the capture area and press your desired key combination
              </li>
              <li>
                Use modifier keys like Alt, Cmd, Ctrl, Shift combined with
                letters
              </li>
              <li>
                Function keys (F1-F12) and special keys are also supported
              </li>
              <li>
                Real-time validation prevents conflicts with system shortcuts
              </li>
              <li>Changes are applied immediately and saved automatically</li>
            </ul>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
