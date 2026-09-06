import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { ChevronDown, Info, Keyboard, RefreshCw, RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";

import { SettingsSectionProps } from "../types";
import ShortcutInput from "../ShortcutInput";

export default function ShortcutsSettings({ settings }: SettingsSectionProps) {
  const [tipsOpen, setTipsOpen] = useState(false);

  const getShortcutDisplayName = (shortcutName: string): string => {
    const names: { [key: string]: string } = {
      agent_mode: "Agent Mode",
      dictation_input: "Start Dictation",
      stop_current_task: "Stop Current Task",
      open_settings: "Open Settings",
      voice_activation: "Voice Activation",
    };
    return names[shortcutName] || shortcutName;
  };

  const getShortcutDescription = (shortcutName: string): string => {
    const descriptions: { [key: string]: string } = {
      agent_mode: "Activate agent mode",
      dictation_input: "Activate voice input for dictation",
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
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Keyboard size={20} />
            Global Shortcuts
          </CardTitle>
          <CardDescription>
            System-wide shortcuts. Click a row's info icon for what it does.
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
              <div className="space-y-2">
                <h4 className="text-[11px] font-medium text-muted-foreground uppercase tracking-wide border-b pb-1.5">
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
              <div className="space-y-2">
                <h4 className="text-[11px] font-medium text-muted-foreground uppercase tracking-wide border-b pb-1.5">
                  System Shortcuts
                </h4>
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
          )}

          <div className="pt-3 border-t">
            <Button
              onClick={handleResetShortcuts}
              variant="outline"
              size="sm"
              disabled={settings.shortcutsLoading}
              className="w-full"
            >
              <RotateCcw className="size-3.5" />
              Reset to Defaults
            </Button>
          </div>

          {/* Usage tips — collapsed by default (progressive disclosure) */}
          <Collapsible open={tipsOpen} onOpenChange={setTipsOpen}>
            <CollapsibleTrigger asChild>
              <button
                type="button"
                className="flex w-full items-center gap-1.5 rounded-md px-1 py-1 text-xs text-muted-foreground hover:text-foreground"
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
              <ul className="mt-1 rounded-md bg-muted/50 p-3 text-xs text-muted-foreground space-y-1 list-disc list-inside">
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
        </CardContent>
      </Card>
    </div>
  );
}
