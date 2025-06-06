import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Edit3,
  Keyboard,
  Mic,
  MousePointer,
  RefreshCw,
  Save,
  Settings as SettingsIcon,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

interface ShortcutConfig {
  id: string;
  name: string;
  description: string;
  key: string;
  modifiers: string[];
  enabled: boolean;
  category: "voice" | "general" | "agent";
  system_managed: boolean; // Can't be changed by user
}

interface ShortcutStatus {
  shortcut_id: string;
  is_active: boolean;
  last_triggered?: number;
  error?: string;
}

export function ShortcutManager() {
  const [shortcuts, setShortcuts] = useState<ShortcutConfig[]>([]);
  const [shortcutStatus, setShortcutStatus] = useState<
    Record<string, ShortcutStatus>
  >({});
  const [editingShortcut, setEditingShortcut] = useState<string | null>(null);
  const [tempKey, setTempKey] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [keyCapture, setKeyCapture] = useState(false);

  // Load shortcuts configuration
  const loadShortcuts = async () => {
    try {
      setIsLoading(true);
      const config = await invoke<ShortcutConfig[]>(
        "get_shortcut_configuration"
      );
      setShortcuts(config);
    } catch (error) {
      console.error("Failed to load shortcuts:", error);
      toast.error("Failed to load shortcut configuration");
    } finally {
      setIsLoading(false);
    }
  };

  // Load shortcut status
  const loadShortcutStatus = async () => {
    try {
      const status = await invoke<Record<string, ShortcutStatus>>(
        "get_shortcut_status"
      );
      setShortcutStatus(status);
    } catch (error) {
      console.error("Failed to load shortcut status:", error);
    }
  };

  // Listen for shortcut events
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await listen<{ shortcut_id: string; triggered_at: number }>(
        "shortcut-triggered",
        (event) => {
          setShortcutStatus((prev) => ({
            ...prev,
            [event.payload.shortcut_id]: {
              ...prev[event.payload.shortcut_id],
              last_triggered: event.payload.triggered_at,
            },
          }));
        }
      );
    };

    setupListener();
    return () => unlisten?.();
  }, []);

  // Load data on component mount
  useEffect(() => {
    loadShortcuts();
    loadShortcutStatus();
  }, []);

  // Handle shortcut toggle
  const handleToggleShortcut = async (shortcutId: string, enabled: boolean) => {
    try {
      await invoke("toggle_shortcut", { shortcutId, enabled });
      setShortcuts((prev) =>
        prev.map((s) => (s.id === shortcutId ? { ...s, enabled } : s))
      );

      // Reload status after change
      await loadShortcutStatus();

      toast.success(
        `${shortcuts.find((s) => s.id === shortcutId)?.name} ${
          enabled ? "enabled" : "disabled"
        }`
      );
    } catch (error) {
      console.error("Failed to toggle shortcut:", error);
      toast.error("Failed to update shortcut");
    }
  };

  // Handle key capture for editing
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!keyCapture) return;

    e.preventDefault();
    e.stopPropagation();

    const modifiers: string[] = [];
    if (e.ctrlKey) modifiers.push("Ctrl");
    if (e.metaKey) modifiers.push("Cmd");
    if (e.altKey) modifiers.push("Alt");
    if (e.shiftKey) modifiers.push("Shift");

    let key = e.code;
    // Convert common keys to readable format
    if (key === "Space") key = "Space";
    else if (key.startsWith("Key")) key = key.replace("Key", "");
    else if (key.startsWith("Digit")) key = key.replace("Digit", "");

    const shortcutString = [...modifiers, key].join("+");
    setTempKey(shortcutString);
  };

  // Start editing a shortcut
  const startEditingShortcut = (shortcutId: string) => {
    const shortcut = shortcuts.find((s) => s.id === shortcutId);
    if (!shortcut || shortcut.system_managed) return;

    setEditingShortcut(shortcutId);
    setTempKey([...shortcut.modifiers, shortcut.key].join("+"));
    setKeyCapture(false);
  };

  // Save shortcut changes
  const saveShortcutEdit = async () => {
    if (!editingShortcut || !tempKey) return;

    try {
      const parts = tempKey.split("+");
      const key = parts.pop() || "";
      const modifiers = parts;

      await invoke("update_shortcut", {
        shortcutId: editingShortcut,
        key,
        modifiers,
      });

      setShortcuts((prev) =>
        prev.map((s) =>
          s.id === editingShortcut ? { ...s, key, modifiers } : s
        )
      );

      setEditingShortcut(null);
      setTempKey("");
      setKeyCapture(false);

      // Reload status after change
      await loadShortcutStatus();

      toast.success("Shortcut key combination saved successfully");
    } catch (error) {
      console.error("Failed to save shortcut:", error);
      toast.error(
        "Failed to save shortcut. Make sure the combination is not already in use."
      );
    }
  };

  // Cancel editing
  const cancelEdit = () => {
    setEditingShortcut(null);
    setTempKey("");
    setKeyCapture(false);
  };

  // Reset to defaults
  const resetToDefaults = async () => {
    try {
      await invoke("reset_shortcuts_to_default");
      await loadShortcuts();
      await loadShortcutStatus();

      toast.success("All shortcuts have been reset to default values");
    } catch (error) {
      console.error("Failed to reset shortcuts:", error);
      toast.error("Failed to reset shortcuts");
    }
  };

  // Group shortcuts by category
  const groupedShortcuts = shortcuts.reduce((acc, shortcut) => {
    if (!acc[shortcut.category]) {
      acc[shortcut.category] = [];
    }
    acc[shortcut.category].push(shortcut);
    return acc;
  }, {} as Record<string, ShortcutConfig[]>);

  // Get category info
  const getCategoryInfo = (category: string) => {
    switch (category) {
      case "voice":
        return {
          title: "Voice Input",
          description: "Voice dictation and AI agent shortcuts",
          icon: <Mic className="h-4 w-4" />,
        };
      case "agent":
        return {
          title: "AI Agent",
          description: "AI agent control and interaction",
          icon: <MousePointer className="h-4 w-4" />,
        };
      case "general":
        return {
          title: "General",
          description: "Application navigation and control",
          icon: <SettingsIcon className="h-4 w-4" />,
        };
      default:
        return {
          title: category,
          description: "",
          icon: <Keyboard className="h-4 w-4" />,
        };
    }
  };

  // Format last triggered time
  const formatLastTriggered = (timestamp?: number) => {
    if (!timestamp) return "Never used";

    const now = Date.now();
    const diff = now - timestamp;

    if (diff < 60000) return "Just now";
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
    return `${Math.floor(diff / 86400000)}d ago`;
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">Keyboard Shortcuts</h2>
          <p className="text-muted-foreground">
            Customize and manage your keyboard shortcuts for better productivity
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={loadShortcuts}
            disabled={isLoading}
          >
            <RefreshCw
              className={cn("h-4 w-4 mr-2", isLoading && "animate-spin")}
            />
            Refresh
          </Button>
          <Button variant="outline" size="sm" onClick={resetToDefaults}>
            Reset to Defaults
          </Button>
        </div>
      </div>

      {Object.entries(groupedShortcuts).map(([category, categoryShortcuts]) => {
        const categoryInfo = getCategoryInfo(category);

        return (
          <Card key={category}>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                {categoryInfo.icon}
                {categoryInfo.title}
              </CardTitle>
              <CardDescription>{categoryInfo.description}</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-4">
                {categoryShortcuts.map((shortcut) => {
                  const status = shortcutStatus[shortcut.id];
                  const isEditing = editingShortcut === shortcut.id;

                  return (
                    <div
                      key={shortcut.id}
                      className="flex items-center justify-between p-3 border rounded-lg"
                    >
                      <div className="flex-1">
                        <div className="flex items-center gap-3">
                          <div>
                            <h4 className="font-medium">{shortcut.name}</h4>
                            <p className="text-sm text-muted-foreground">
                              {shortcut.description}
                            </p>
                            {status?.last_triggered && (
                              <p className="text-xs text-muted-foreground mt-1">
                                Last used:{" "}
                                {formatLastTriggered(status.last_triggered)}
                              </p>
                            )}
                          </div>
                        </div>
                      </div>

                      <div className="flex items-center gap-3">
                        {/* Status indicator */}
                        <div className="flex items-center gap-2">
                          <div
                            className={cn(
                              "w-2 h-2 rounded-full",
                              status?.is_active && shortcut.enabled
                                ? "bg-green-500"
                                : "bg-gray-400"
                            )}
                          />
                          <Badge
                            variant={
                              status?.is_active && shortcut.enabled
                                ? "default"
                                : "secondary"
                            }
                          >
                            {status?.is_active && shortcut.enabled
                              ? "Active"
                              : "Inactive"}
                          </Badge>
                        </div>

                        {/* Key combination display/editor */}
                        <div className="min-w-[120px]">
                          {isEditing ? (
                            <div className="flex items-center gap-2">
                              <Input
                                value={tempKey}
                                onChange={(e) => setTempKey(e.target.value)}
                                onKeyDown={handleKeyDown}
                                onFocus={() => setKeyCapture(true)}
                                onBlur={() => setKeyCapture(false)}
                                placeholder="Press keys..."
                                className="w-32 h-8 text-sm"
                              />
                              <Button
                                size="sm"
                                variant="outline"
                                onClick={saveShortcutEdit}
                              >
                                <Save className="h-3 w-3" />
                              </Button>
                              <Button
                                size="sm"
                                variant="outline"
                                onClick={cancelEdit}
                              >
                                <X className="h-3 w-3" />
                              </Button>
                            </div>
                          ) : (
                            <div className="flex items-center gap-2">
                              <kbd className="px-2 py-1 bg-muted rounded text-sm font-mono">
                                {[...shortcut.modifiers, shortcut.key].join(
                                  "+"
                                )}
                              </kbd>
                              {!shortcut.system_managed && (
                                <Button
                                  size="sm"
                                  variant="ghost"
                                  onClick={() =>
                                    startEditingShortcut(shortcut.id)
                                  }
                                  className="h-6 w-6 p-0"
                                >
                                  <Edit3 className="h-3 w-3" />
                                </Button>
                              )}
                            </div>
                          )}
                        </div>

                        {/* Enable/disable toggle */}
                        <div className="flex items-center gap-2">
                          <Label
                            htmlFor={`toggle-${shortcut.id}`}
                            className="sr-only"
                          >
                            Toggle {shortcut.name}
                          </Label>
                          <Switch
                            id={`toggle-${shortcut.id}`}
                            checked={shortcut.enabled}
                            onCheckedChange={(enabled) =>
                              handleToggleShortcut(shortcut.id, enabled)
                            }
                            disabled={shortcut.system_managed}
                          />
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </CardContent>
          </Card>
        );
      })}

      {/* Help text */}
      <Card>
        <CardContent className="pt-6">
          <div className="text-sm text-muted-foreground space-y-2">
            <p>
              <strong>Tips for customizing shortcuts:</strong>
            </p>
            <ul className="list-disc list-inside space-y-1 ml-4">
              <li>
                Click the edit button next to any shortcut to change its key
                combination
              </li>
              <li>
                Use modifier keys (Cmd, Alt, Ctrl, Shift) to avoid conflicts
                with other apps
              </li>
              <li>
                System-managed shortcuts cannot be changed for stability reasons
              </li>
              <li>
                Disabled shortcuts won't trigger any actions but remain
                configured
              </li>
              <li>
                Green status indicates the shortcut is actively registered with
                the system
              </li>
            </ul>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
