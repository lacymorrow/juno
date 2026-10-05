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
import { AgentModeGroup } from "../AgentModeGroup";
import { BuildGroup } from "../BuildGroup";
import { OnboardingGroup } from "../OnboardingGroup";

interface AdvancedSettingsProps extends SettingsSectionProps {
  onNavigateToPermissions?: () => void;
}

/** A short picker sized like a macOS segment. */
function SegmentedChoice<T extends string>({
  id,
  label,
  value,
  options,
  disabled,
  onChange,
}: {
  id: string;
  label: string;
  value: T;
  options: ReadonlyArray<{ value: T; label: string }>;
  disabled?: boolean;
  onChange: (next: T) => void;
}) {
  return (
    <div
      id={id}
      role="radiogroup"
      aria-label={label}
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

/**
 * Where Juno shows up. Two switches used to allow "nowhere", which is the one
 * state people regret; three named places cannot express it.
 */
type Presence = "menu_bar" | "dock" | "both";
const PRESENCE_OPTIONS = [
  { value: "menu_bar" as const, label: "Menu bar" },
  { value: "dock" as const, label: "Dock" },
  { value: "both" as const, label: "Both" },
];
const presenceOf = (dock: boolean, tray: boolean): Presence =>
  dock && tray ? "both" : dock ? "dock" : "menu_bar";

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

  // One long-lived Claude CLI process per conversation. On by default.
  const [persistentSession, setPersistentSession] = useState(true);
  const [persistentSessionLoading, setPersistentSessionLoading] = useState(true);

  // Beta: smart routing. Default off.
  const [smartRouting, setSmartRouting] = useState(false);
  const [smartRoutingLoading, setSmartRoutingLoading] = useState(true);

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

  // Load the persistent-session flag on mount.
  useEffect(() => {
    let mounted = true;
    const load = async () => {
      try {
        const enabled = await invoke<boolean>(
          COMMANDS.SETTINGS_GET_CLI_PERSISTENT_SESSION_ENABLED,
        );
        if (mounted) setPersistentSession(enabled !== false);
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

  // Load the beta smart-routing flag on mount.
  useEffect(() => {
    let mounted = true;
    invoke<boolean>(COMMANDS.SETTINGS_GET_SMART_ROUTING_ENABLED)
      .then((enabled) => {
        if (mounted) setSmartRouting(enabled === true);
      })
      .catch((error) => {
        console.error("Failed to load the smart routing flag:", error);
      })
      .finally(() => {
        if (mounted) setSmartRoutingLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, []);

  const handleSmartRoutingChange = async (enabled: boolean) => {
    const previous = smartRouting;
    setSmartRouting(enabled);
    try {
      await invoke(COMMANDS.SETTINGS_SET_SMART_ROUTING_ENABLED, { enabled });
    } catch (error) {
      console.error("Failed to update the smart routing flag:", error);
      setSmartRouting(previous);
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

  const handlePresenceChange = async (next: Presence) => {
    const previous = { dock: dockIconVisible, tray: showTrayIcon };
    const dock = next !== "menu_bar";
    const tray = next !== "dock";
    setDockIconVisible(dock);
    setShowTrayIcon(tray);
    try {
      // Turn the new place on before the old one off, so Juno is never
      // nowhere, even for the moment between the two calls.
      const calls: Array<Promise<unknown>> = [];
      if (dock && !previous.dock) calls.push(invoke(COMMANDS.INPUT_CONTROL_SET_DOCK_ICON_VISIBLE, { visible: true }));
      if (tray && !previous.tray) calls.push(invoke(COMMANDS.INPUT_CONTROL_SET_TRAY_ICON_VISIBLE, { visible: true }));
      await Promise.all(calls);
      if (!dock && previous.dock) await invoke(COMMANDS.INPUT_CONTROL_SET_DOCK_ICON_VISIBLE, { visible: false });
      if (!tray && previous.tray) await invoke(COMMANDS.INPUT_CONTROL_SET_TRAY_ICON_VISIBLE, { visible: false });
      if (!dock && previous.dock) {
        // The one change people regret. Say where Juno went, right now.
        toast("Juno is now in the menu bar only", {
          description:
            "Click the Juno icon in the menu bar at the top of your screen to open it. Choose Dock or Both here any time to get the Dock icon back.",
          duration: 8000,
        });
      }
    } catch (error) {
      console.error("Failed to change where Juno shows up:", error);
      setDockIconVisible(previous.dock);
      setShowTrayIcon(previous.tray);
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
            label="Mouse control"
            value={mouseControl}
            options={MOUSE_CONTROL_OPTIONS}
            disabled={backgroundLoading || backgroundError}
            onChange={handleMouseControlChange}
          />
        </SettingsRow>

        <SettingsRow
          id="show-juno-in"
          label="Show Juno in"
          description="Menu bar only keeps Juno out of the Dock and the app switcher. To get the Dock icon back, click the Juno icon in the menu bar and choose Dock or Both here. Opening Juno from your Applications folder also brings its window back."
        >
          <SegmentedChoice
            id="show-juno-in"
            label="Show Juno in"
            value={presenceOf(dockIconVisible, showTrayIcon)}
            options={PRESENCE_OPTIONS}
            disabled={backgroundLoading || backgroundError}
            onChange={handlePresenceChange}
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

      <SettingsGroup title="Claude CLI">
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

      <SettingsGroup
        title="Beta"
        footer="Beta features are still being proven out. They can be turned off here at any time."
      >
        <SettingsRow
          htmlFor="smart-routing"
          label="Smart routing"
          description="Picks a model for each request and decides whether Juno needs to use your computer. Applies to the Anthropic API provider, from your next message."
        >
          <Switch
            id="smart-routing"
            checked={smartRouting}
            onCheckedChange={handleSmartRoutingChange}
            disabled={smartRoutingLoading}
          />
        </SettingsRow>
      </SettingsGroup>

      <AgentModeGroup settings={settings} />

      <OnboardingGroup />

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
                        // Rust picks the provider, because only Rust knows
                        // what is installed and signed in on this machine,
                        // and names it back so the confirmation can say which
                        // one Juno is on rather than claiming success and
                        // leaving the person to find out.
                        const provider = await invoke<string>(
                          COMMANDS.SETTINGS_RESET_SETTINGS,
                        );
                        await settings.loadAllSettings();
                        toast.success(
                          `Settings reset. Juno is using ${provider}.`,
                        );
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

      <BuildGroup />
    </div>
  );
}
