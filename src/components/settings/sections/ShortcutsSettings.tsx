import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { ChevronDown, Info, RefreshCw, RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";

import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";
import ShortcutInput from "../ShortcutInput";

export default function ShortcutsSettings({ settings }: SettingsSectionProps) {
  const [tipsOpen, setTipsOpen] = useState(false);

  const getShortcutDisplayName = (shortcutName: string): string => {
    const names: { [key: string]: string } = {
      stop_current_task: "Stop Current Task",
      open_settings: "Open Settings",
      voice_activation: "Voice Activation",
    };
    return names[shortcutName] || shortcutName;
  };

  const getShortcutDescription = (shortcutName: string): string => {
    const descriptions: { [key: string]: string } = {
      stop_current_task: "Stop the current AI task or operation",
      open_settings: "Open the settings window",
      voice_activation: "Toggle voice recording from anywhere — no Juno window required",
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
      {settings.shortcutsLoading ? (
        <SettingsGroup title="Global Shortcuts">
          <div className="flex items-center justify-center py-8">
            <RefreshCw className="h-6 w-6 animate-spin" />
            <span className="ml-2">Loading shortcuts...</span>
          </div>
        </SettingsGroup>
      ) : (
        <>
          <SettingsGroup
            title="Customizable Shortcuts"
            footer="System-wide shortcuts. Click a row's info icon for what it does."
          >
            <div className="space-y-2 p-3">
              {Object.entries(settings.keyboardShortcuts)
                // open_settings is system-managed; agent_mode + dictation_input
                // moved to the Triggers screen (single source of truth).
                .filter(
                  ([key]) =>
                    !["open_settings", "agent_mode", "dictation_input"].includes(
                      key,
                    ),
                )
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
          </SettingsGroup>

          <SettingsGroup title="System Shortcuts">
            <div className="space-y-2 p-3">
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
          </SettingsGroup>
        </>
      )}

      <SettingsGroup>
        <SettingsRow
          label="Reset all shortcuts"
          description="Restore every keyboard shortcut to its default"
        >
          <Button
            onClick={handleResetShortcuts}
            variant="outline"
            size="sm"
            disabled={settings.shortcutsLoading}
          >
            <RotateCcw className="size-3.5" />
            Reset to Defaults
          </Button>
        </SettingsRow>

        {/* Usage tips — collapsed by default (progressive disclosure) */}
        <div className="px-4 py-2.5">
          <Collapsible open={tipsOpen} onOpenChange={setTipsOpen}>
            <CollapsibleTrigger asChild>
              <button
                type="button"
                className="flex w-full items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground"
              >
                <Info className="size-3.5" aria-hidden="true" />
                <span>Keyboard shortcut tips</span>
                <ChevronDown
                  aria-hidden="true"
                  className={cn(
                    "ml-auto size-3.5 transition-transform",
                    tipsOpen && "rotate-180"
                  )}
                />
              </button>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <ul className="mt-2 rounded-md bg-muted/50 p-3 text-xs text-muted-foreground space-y-1 list-disc list-inside">
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
            </CollapsibleContent>
          </Collapsible>
        </div>
      </SettingsGroup>
    </div>
  );
}
