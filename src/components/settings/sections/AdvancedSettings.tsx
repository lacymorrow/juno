import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { invoke } from "@tauri-apps/api/core";
import { RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";
import { COMMANDS } from "@/lib/constants.generated";
import { cn } from "@/lib/utils";
import type { MouseControlMode } from "@/lib/inputControl";

interface AdvancedSettingsProps extends SettingsSectionProps {
  onNavigateToPermissions?: () => void;
}

/** The two-option picker the Mouse control row uses, sized like a macOS segment. */
function SegmentedChoice<T extends string>({
  id,
  value,
  options,
  disabled,
  onChange,
}: {
  id: string;
  value: T;
  options: ReadonlyArray<{ value: T; label: string }>;
  disabled?: boolean;
  onChange: (next: T) => void;
}) {
  return (
    <div
      id={id}
      role="radiogroup"
      aria-label="Mouse control"
      className="inline-flex items-center gap-0.5 rounded-[7px] border border-border bg-muted/40 p-0.5"
    >
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            disabled={disabled}
            onClick={() => onChange(option.value)}
            className={cn(
              "rounded-[5px] px-3 py-1 text-[12px] leading-tight transition-opacity duration-150",
              "disabled:opacity-50",
              selected
                ? "bg-background font-medium text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

const MOUSE_CONTROL_OPTIONS = [
  { value: "ask" as const, label: "Ask each time" },
  { value: "always" as const, label: "Always allow" },
];

export default function AdvancedSettings({
  settings,
  onNavigateToPermissions,
}: AdvancedSettingsProps) {
  const [debugMode, setDebugMode] = useState(false);

  // Background operation. Loaded together so the group has one loading state
  // and one error state instead of three.
  const [backgroundMode, setBackgroundMode] = useState(true);
  const [mouseControl, setMouseControl] = useState<MouseControlMode>("ask");
  const [dockIconVisible, setDockIconVisible] = useState(true);
  const [showTrayIcon, setShowTrayIcon] = useState(true);
  const [backgroundLoading, setBackgroundLoading] = useState(true);
  const [backgroundError, setBackgroundError] = useState(false);

  // Beta: one long-lived Claude CLI process per conversation.
  const [persistentSession, setPersistentSession] = useState(false);
  const [persistentSessionLoading, setPersistentSessionLoading] = useState(true);

  // Suppress unused parameter warning for onNavigateToPermissions
  void onNavigateToPermissions;

  // Load debug mode status on mount
  useEffect(() => {
    const loadDebugMode = async () => {
      try {
        const enabled = await invoke(COMMANDS.CORE_GET_DEBUG_MODE);
        setDebugMode(enabled as boolean);
      } catch (error) {
        console.error("Failed to get debug mode status:", error);
      }
    };
    loadDebugMode();
  }, []);

  // Load the background-operation settings on mount.
  useEffect(() => {
    let mounted = true;
    const load = async () => {
      try {
        const agent = await invoke<{
          background_mode?: boolean;
          mouse_control?: string;
          dock_icon_visible?: boolean;
          show_tray_icon?: boolean;
        }>(COMMANDS.SETTINGS_GET_AGENT_SETTINGS);
        if (!mounted) return;
        setBackgroundMode(agent?.background_mode ?? true);
        setMouseControl(agent?.mouse_control === "always" ? "always" : "ask");
        setDockIconVisible(agent?.dock_icon_visible ?? true);
        setShowTrayIcon(agent?.show_tray_icon ?? true);
        setBackgroundError(false);
      } catch (error) {
        console.error("Failed to load background settings:", error);
        if (mounted) setBackgroundError(true);
      } finally {
        if (mounted) setBackgroundLoading(false);
      }
    };
    load();
    return () => {
      mounted = false;
    };
  }, []);

  // Load the beta persistent-session flag on mount.
  useEffect(() => {
    let mounted = true;
    const load = async () => {
      try {
        const enabled = await invoke<boolean>(
          COMMANDS.SETTINGS_GET_CLI_PERSISTENT_SESSION_ENABLED,
        );
        if (mounted) setPersistentSession(enabled === true);
      } catch (error) {
        console.error("Failed to load the persistent session flag:", error);
      } finally {
        if (mounted) setPersistentSessionLoading(false);
      }
    };
    load();
    return () => {
      mounted = false;
    };
  }, []);

  const handlePersistentSessionChange = async (enabled: boolean) => {
    const previous = persistentSession;
    setPersistentSession(enabled);
    try {
      await invoke(COMMANDS.SETTINGS_SET_CLI_PERSISTENT_SESSION_ENABLED, {
        enabled,
      });
    } catch (error) {
      console.error("Failed to update the persistent session flag:", error);
      setPersistentSession(previous);
      toast.error("Could not change that setting");
    }
  };

  const handleBackgroundModeChange = async (enabled: boolean) => {
    const previous = backgroundMode;
    setBackgroundMode(enabled);
    try {
      await invoke(COMMANDS.INPUT_CONTROL_SET_BACKGROUND_MODE, { enabled });
    } catch (error) {
      console.error("Failed to update background mode:", error);
      setBackgroundMode(previous);
      toast.error("Could not change that setting");
    }
  };

  const handleMouseControlChange = async (mode: MouseControlMode) => {
    const previous = mouseControl;
    setMouseControl(mode);
    try {
      await invoke(COMMANDS.INPUT_CONTROL_SET_MOUSE_CONTROL, { mode });
    } catch (error) {
      console.error("Failed to update mouse control:", error);
      setMouseControl(previous);
      toast.error("Could not change that setting");
    }
  };

  const handleDockIconChange = async (visible: boolean) => {
    const previous = dockIconVisible;
    setDockIconVisible(visible);
    try {
      await invoke(COMMANDS.INPUT_CONTROL_SET_DOCK_ICON_VISIBLE, { visible });
      if (!visible) {
        // The one setting people regret. Say where Juno went, right now.
        toast("Juno is now in the menu bar only", {
          description:
            "Click the Juno icon in the menu bar at the top of your screen to open it. Turn this setting back on any time to get the Dock icon back.",
          duration: 8000,
        });
      }
    } catch (error) {
      console.error("Failed to update the Dock icon setting:", error);
      setDockIconVisible(previous);
      toast.error("Could not change that setting");
    }
  };

  const handleTrayIconChange = async (visible: boolean) => {
    const previous = showTrayIcon;
    setShowTrayIcon(visible);
    try {
      await invoke(COMMANDS.INPUT_CONTROL_SET_TRAY_ICON_VISIBLE, { visible });
    } catch (error) {
      console.error("Failed to update the tray icon setting:", error);
      setShowTrayIcon(previous);
      toast.error("Could not change that setting");
    }
  };

  return (
    <div className="space-y-6">
      <SettingsGroup
        title="Background"
        footer={
          backgroundError
            ? "These settings could not be loaded. Reopen Settings to try again."
            : "Juno works in other apps without interrupting you. When something can only be done with the real pointer, it asks first."
        }
      >
        <SettingsRow
          htmlFor="background-mode"
          label="Work in the background"
          description="Juno acts on other apps without taking your cursor or your active window, so you can keep working."
        >
          <Switch
            id="background-mode"
            checked={backgroundMode}
            onCheckedChange={handleBackgroundModeChange}
            disabled={backgroundLoading || backgroundError}
          />
        </SettingsRow>

        <SettingsRow
          id="mouse-control"
          label="Mouse control"
          description="Some steps need the real pointer. Ask each time, or let Juno take it whenever it needs to."
        >
          <SegmentedChoice
            id="mouse-control"
            value={mouseControl}
            options={MOUSE_CONTROL_OPTIONS}
            disabled={backgroundLoading || backgroundError}
            onChange={handleMouseControlChange}
          />
        </SettingsRow>

        <SettingsRow
          htmlFor="dock-icon-visible"
          label="Show in Dock"
          description="Turn this off and Juno leaves the Dock and the app switcher, and lives only in the menu bar. To bring the Dock icon back, click the Juno icon in the menu bar and turn this on again. Opening Juno from your Applications folder also brings its window back."
        >
          <Switch
            id="dock-icon-visible"
            checked={dockIconVisible}
            onCheckedChange={handleDockIconChange}
            disabled={backgroundLoading || backgroundError}
          />
        </SettingsRow>

        <SettingsRow
          htmlFor="show-tray-icon"
          label="Show system tray icon"
          description="Show Juno's icon in the menu bar at the top of your screen. Turn it off to hide the icon; Juno keeps running and stays reachable from the Dock and the floating bar."
        >
          <Switch
            id="show-tray-icon"
            checked={showTrayIcon}
            onCheckedChange={handleTrayIconChange}
            disabled={backgroundLoading || backgroundError}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Developer Options"
        footer="Advanced settings for developers and power users."
      >
        <SettingsRow
          htmlFor="debug-mode"
          label="Debug Mode"
          description="Enable verbose logging and debug features"
        >
          <Switch
            id="debug-mode"
            checked={debugMode}
            onCheckedChange={async (enabled) => {
              try {
                await invoke(COMMANDS.CORE_SET_DEBUG_MODE, { enabled });
                setDebugMode(enabled);
                toast.success(
                  `Debug mode ${enabled ? "enabled" : "disabled"}`
                );
              } catch (error) {
                toast.error("Failed to toggle debug mode");
              }
            }}
          />
        </SettingsRow>

        <SettingsRow
          htmlFor="performance-monitoring"
          label="Performance Monitoring"
          description="Monitor system resource usage"
        >
          <Switch
            id="performance-monitoring"
            checked={settings.performanceMonitoringEnabled}
            onCheckedChange={settings.handlePerformanceMonitoringChange}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Beta"
        footer="Beta features are still being proven out. They can be turned off here at any time."
      >
        <SettingsRow
          htmlFor="cli-persistent-session"
          label="Persistent Claude session"
          description="Keeps one Claude CLI process alive per conversation, so follow-up replies start 1.6–3.1s faster. If that process hangs, a reply can stall before Juno falls back to the standard path; nothing is lost either way. Applies to the Claude CLI provider, from your next message."
        >
          <Switch
            id="cli-persistent-session"
            checked={persistentSession}
            onCheckedChange={handlePersistentSessionChange}
            disabled={persistentSessionLoading}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title="Reset Settings">
        <SettingsRow
          id="reset-all-settings"
          label="Reset all settings"
          description="Reset all settings to their default values"
          below={
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button variant="destructive" className="w-full">
                  <RotateCcw className="w-4 h-4 mr-2" />
                  Reset All Settings
                </Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Reset all settings?</AlertDialogTitle>
                  <AlertDialogDescription>
                    Every setting goes back to its default, including your
                    API keys, your shortcuts and your AI provider choice. You
                    will have to set Juno up again. This cannot be undone.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                  <AlertDialogAction
                    onClick={async () => {
                      try {
                        await invoke(COMMANDS.SETTINGS_RESET_SETTINGS);
                        await settings.loadAllSettings();
                        toast.success("All settings have been reset to defaults");
                      } catch (error) {
                        console.error("Failed to reset settings:", error);
                        toast.error("Failed to reset settings");
                      }
                    }}
                  >
                    Reset
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          }
        />
      </SettingsGroup>
    </div>
  );
}
