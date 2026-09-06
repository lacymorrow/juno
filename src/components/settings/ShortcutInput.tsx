import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import {
  AlertTriangle,
  CheckCircle,
  Edit3,
  Info,
  Keyboard,
  Save,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import { KEYBOARD_SHORTCUTS } from "@/lib/constants.generated";

interface ShortcutInputProps {
  label: string;
  description: string;
  value: string;
  shortcutName: string;
  isSystemManaged?: boolean;
  onSave: (shortcutName: string, value: string) => Promise<void>;
  isLoading: boolean;
}

/**
 * Progressive disclosure for a shortcut's description: an info icon that opens
 * a popover. The description is not in the DOM until the icon is activated, so
 * long copy can never fight the shortcut chip for row width.
 */
export function ShortcutInfo({
  label,
  description,
}: {
  label: string;
  description: string;
}) {
  if (!description) return null;
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={`About ${label}`}
          className="inline-flex size-5 shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
        >
          <Info className="size-3.5" aria-hidden="true" />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-64 p-3 text-xs leading-relaxed">
        {description}
      </PopoverContent>
    </Popover>
  );
}

/** One-line shortcut chip that never wraps or shrinks below its own text. */
function ShortcutChip({ value }: { value: string }) {
  return (
    <kbd className="shrink-0 whitespace-nowrap rounded border bg-muted px-1.5 py-0.5 font-mono text-[11px] leading-4 text-foreground">
      {value || "Not set"}
    </kbd>
  );
}

const ShortcutInput: React.FC<ShortcutInputProps> = ({
  label,
  description,
  value,
  shortcutName,
  isSystemManaged = false,
  onSave,
  isLoading,
}) => {
  const [isEditing, setIsEditing] = useState(false);
  const [isCapturing, setIsCapturing] = useState(false);
  const [currentValue, setCurrentValue] = useState(value);
  const [validationMessage, setValidationMessage] = useState<string>("");
  const [validationError, setValidationError] = useState<string>("");
  const [pressedKeys, setPressedKeys] = useState<string[]>([]);
  const [captureTimeout, setCaptureTimeout] = useState<NodeJS.Timeout | null>(null);

  // Update current value when prop changes
  useEffect(() => {
    setCurrentValue(value);
    setValidationMessage("");
    setValidationError("");
  }, [value]);

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (captureTimeout) {
        clearTimeout(captureTimeout);
      }
    };
  }, [captureTimeout]);

  // Validation function with debouncing
  const validateShortcut = useCallback(async (shortcutValue: string) => {
    if (!shortcutValue.trim()) {
      setValidationMessage("Enter a shortcut combination");
      setValidationError("");
      return;
    }

    try {
      const result = await invoke<string>("validate_keyboard_shortcut", {
        shortcutValue: shortcutValue,
        shortcutName: shortcutName,
      });
      setValidationMessage(result);
      setValidationError("");
    } catch (error) {
      setValidationError(typeof error === 'string' ? error : 'Invalid shortcut');
      setValidationMessage("");
    }
  }, [shortcutName]);

  // Debounced validation
  useEffect(() => {
    if (isEditing) {
      const timeoutId = setTimeout(() => {
        validateShortcut(currentValue);
      }, 300);
      return () => clearTimeout(timeoutId);
    }
  }, [currentValue, isEditing, validateShortcut]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!isCapturing) return;

    e.preventDefault();
    e.stopPropagation();

    // Clear any existing timeout
    if (captureTimeout) {
      clearTimeout(captureTimeout);
    }

    const modifiers: string[] = [];

    // Detect modifiers with platform-aware naming
    if (e.ctrlKey || e.metaKey) {
      if (e.metaKey) {
        modifiers.push("Cmd");
      } else {
        modifiers.push("Ctrl");
      }
    }
    if (e.altKey) {
      // Use Option on macOS for better UX
      modifiers.push(navigator.platform.toLowerCase().includes('mac') ? "Option" : "Alt");
    }
    if (e.shiftKey) modifiers.push("Shift");

    let key = "";

    // Enhanced key detection with better special key handling
    switch (e.code) {
      case "Space":
        key = "Space";
        break;
      case "Escape":
        key = "Escape";
        break;
      case "Enter":
        key = "Enter";
        break;
      case "Tab":
        key = "Tab";
        break;
      case "Backspace":
        key = "Backspace";
        break;
      case "Delete":
        key = "Delete";
        break;
      case "Home":
        key = "Home";
        break;
      case "End":
        key = "End";
        break;
      case "PageUp":
        key = "PageUp";
        break;
      case "PageDown":
        key = "PageDown";
        break;
      case "Insert":
        key = "Insert";
        break;
      case "ArrowUp":
        key = "Up";
        break;
      case "ArrowDown":
        key = "Down";
        break;
      case "ArrowLeft":
        key = "Left";
        break;
      case "ArrowRight":
        key = "Right";
        break;
      case "PrintScreen":
        key = "PrintScreen";
        break;
      case "ScrollLock":
        key = "ScrollLock";
        break;
      case "Pause":
        key = "Pause";
        break;
      case "CapsLock":
        key = "CapsLock";
        break;
      case "NumLock":
        key = "NumLock";
        break;
      default:
        // Function keys (including extended range)
        if (e.code.startsWith("F") && e.code.length <= 3) {
          key = e.code;
        }
        // Number keys
        else if (e.code.startsWith("Digit")) {
          key = e.code.replace("Digit", "");
        }
        // Letter keys
        else if (e.code.startsWith("Key")) {
          key = e.code.replace("Key", "");
        }
        // Numpad keys
        else if (e.code.startsWith("Numpad")) {
          key = "Numpad" + e.code.replace("Numpad", "");
        }
        // Punctuation and special characters
        else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
          key = e.key;
        }
        // Fallback to the key name for other special keys
        else if (e.key && e.key !== "Control" && e.key !== "Alt" && e.key !== "Shift" && e.key !== "Meta") {
          key = e.key;
        }
        break;
    }

    if (key) {
      const allKeys = [...modifiers, key];
      setPressedKeys(allKeys);

      const shortcutString = allKeys.join("+");
      setCurrentValue(shortcutString);

      // Auto-finish capture with longer delay for complex combinations
      const delay = modifiers.length >= 2 ? 800 : 500;
      const newTimeout = setTimeout(() => {
        setIsCapturing(false);
        setPressedKeys([]);
        setCaptureTimeout(null);
      }, delay);
      setCaptureTimeout(newTimeout);
    }
  }, [isCapturing, captureTimeout]);

  const handleStartCapture = () => {
    setIsCapturing(true);
    setPressedKeys([]);
    setCurrentValue("");
    setValidationMessage("Press the key combination you want to use...");
    setValidationError("");

    // Auto-cancel capture after 10 seconds to prevent UI getting stuck
    const cancelTimeout = setTimeout(() => {
      setIsCapturing(false);
      setPressedKeys([]);
      setValidationMessage("");
      setCaptureTimeout(null);
    }, 10000);
    setCaptureTimeout(cancelTimeout);
  };

  const handleStopCapture = () => {
    if (captureTimeout) {
      clearTimeout(captureTimeout);
      setCaptureTimeout(null);
    }
    setIsCapturing(false);
    setPressedKeys([]);
  };

  const handleSave = async () => {
    if (validationError) {
      toast.error(validationError);
      return;
    }

    try {
      await onSave(shortcutName, currentValue);
      setIsEditing(false);
      setIsCapturing(false);
      setPressedKeys([]);
      if (captureTimeout) {
        clearTimeout(captureTimeout);
        setCaptureTimeout(null);
      }
      toast.success("Shortcut updated successfully");
    } catch (error) {
      console.error("Failed to save shortcut:", error);
      toast.error("Failed to save shortcut");
    }
  };

  const handleCancel = () => {
    setCurrentValue(value);
    setIsEditing(false);
    setIsCapturing(false);
    setPressedKeys([]);
    setValidationMessage("");
    setValidationError("");
    if (captureTimeout) {
      clearTimeout(captureTimeout);
      setCaptureTimeout(null);
    }
  };

  // Get platform-appropriate example
  const getExampleShortcut = () => {
    const shortcutKey = shortcutName.toUpperCase() as keyof typeof KEYBOARD_SHORTCUTS;
    if (shortcutKey in KEYBOARD_SHORTCUTS) {
      return KEYBOARD_SHORTCUTS[shortcutKey];
    }
    const isMac = navigator.platform.toLowerCase().includes('mac');
    return isMac ? "Cmd+K" : "Ctrl+K";
  };

  // Row layout (shared by system-managed and editable rows):
  //   [label (truncates)] [info icon]  ........  [chip] [Edit | System]
  // The label is the only flexible item (min-w-0 + truncate); the chip and the
  // trailing control are shrink-0, so nothing can overlap at the settings
  // window's 700px width (~350px of card content once the sidebar and card
  // padding are taken out).
  const rowLabel = (
    <div className="flex min-w-0 flex-1 items-center gap-1">
      <span
        id={`shortcut-${shortcutName}-label`}
        className="truncate text-xs font-medium"
      >
        {label}
      </span>
      <ShortcutInfo label={label} description={description} />
    </div>
  );

  if (isSystemManaged) {
    return (
      <div
        className="flex items-center gap-2 rounded-md border px-2 py-1.5"
        aria-labelledby={`shortcut-${shortcutName}-label`}
      >
        {rowLabel}
        <ShortcutChip value={value} />
        <Badge variant="secondary" className="text-[10px] px-1.5 py-0">
          System
        </Badge>
      </div>
    );
  }

  return (
    <div>
        {isEditing ? (
          <div className="space-y-2 p-2.5 border rounded-lg bg-muted/30">
            <div className="flex items-center gap-1">
              <span className="text-xs font-medium">{label}</span>
              <ShortcutInfo label={label} description={description} />
            </div>
            {/* Key capture area with enhanced feedback */}
            <div className="space-y-1.5">
              <div className="flex items-center gap-2">
                <Label className="text-xs font-medium">Shortcut combination:</Label>
                {isCapturing && (
                  <div className="flex items-center gap-2">
                    <Badge variant="outline" className="text-[10px] animate-pulse bg-blue-50 border-blue-200">
                      🎯 Listening...
                    </Badge>
                    <button
                      onClick={handleStopCapture}
                      className="text-xs text-muted-foreground hover:text-foreground"
                    >
                      Stop
                    </button>
                  </div>
                )}
              </div>
              <div
                className={cn(
                  "min-h-[40px] p-2 border-2 border-dashed rounded-lg flex items-center gap-2 cursor-pointer transition-all duration-200",
                  isCapturing
                    ? "border-blue-500 bg-blue-50 dark:bg-blue-950/20 shadow-sm"
                    : "border-muted-foreground/30 hover:border-muted-foreground/50 hover:bg-muted/50"
                )}
                onClick={!isCapturing ? handleStartCapture : undefined}
                onKeyDown={handleKeyDown}
                tabIndex={0}
                role="button"
                aria-label={isCapturing ? "Press keys to capture shortcut" : "Click to start capturing shortcut"}
              >
                {pressedKeys.length > 0 ? (
                  <div className="flex items-center gap-1 flex-wrap">
                    {pressedKeys.map((key, index) => (
                      <span key={index} className="flex items-center gap-1">
                        <kbd className="px-1.5 py-0.5 bg-background border rounded text-xs font-mono shadow-sm">
                          {key}
                        </kbd>
                        {index < pressedKeys.length - 1 && (
                          <span className="text-muted-foreground font-medium">+</span>
                        )}
                      </span>
                    ))}
                  </div>
                ) : currentValue ? (
                  <kbd className="px-1.5 py-0.5 bg-background border rounded text-xs font-mono shadow-sm">
                    {currentValue}
                  </kbd>
                ) : (
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <Keyboard className="h-4 w-4" />
                    <span className="text-xs">
                      {isCapturing
                        ? "Press the keys you want to use..."
                        : `Click here to capture shortcut (e.g., ${getExampleShortcut()})`
                      }
                    </span>
                  </div>
                )}
              </div>
            </div>

            {/* Manual input option with better guidance */}
            <div className="space-y-1.5">
              <Label className="text-xs">Or type manually:</Label>
              <Input
                value={currentValue}
                onChange={(e) => setCurrentValue(e.target.value)}
                placeholder={`e.g., ${getExampleShortcut()}, Ctrl+Shift+F1`}
                className="h-8 font-mono text-xs"
                disabled={isCapturing}
              />
              <div className="text-xs text-muted-foreground">
                Tip: Use modifiers like Alt, Ctrl, Cmd, Shift combined with letters
              </div>
            </div>

            {/* Enhanced validation feedback */}
            {(validationMessage || validationError) && (
              <div className={cn(
                "flex items-start gap-2 text-xs p-2 rounded-md border",
                validationError
                  ? "text-red-700 bg-red-50 border-red-200 dark:bg-red-950/20 dark:border-red-800 dark:text-red-400"
                  : "text-green-700 bg-green-50 border-green-200 dark:bg-green-950/20 dark:border-green-800 dark:text-green-400"
              )}>
                <div className="flex-shrink-0 mt-0.5">
                  {validationError ? (
                    <AlertTriangle className="h-4 w-4" />
                  ) : (
                    <CheckCircle className="h-4 w-4" />
                  )}
                </div>
                <div className="flex-1">
                  <span>{validationError || validationMessage}</span>
                  {validationError && validationError.includes("conflicts") && (
                    <div className="mt-1 text-xs opacity-75">
                      Consider using a different key combination to avoid conflicts.
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Action buttons */}
            <div className="flex items-center gap-2 pt-2 border-t">
              <Button
                size="xs"
                onClick={handleSave}
                disabled={isLoading || !!validationError || !currentValue.trim() || isCapturing}
                className="flex items-center gap-2"
              >
                <Save className="size-3.5" />
                Save
              </Button>
              <Button
                size="xs"
                variant="outline"
                onClick={handleCancel}
                disabled={isLoading}
              >
                Cancel
              </Button>
              <Button
                size="xs"
                variant="outline"
                onClick={isCapturing ? handleStopCapture : handleStartCapture}
                disabled={isLoading}
                className="ml-auto"
              >
                <Keyboard className="size-3.5" />
                {isCapturing ? "Stop Capture" : "Capture Keys"}
              </Button>
            </div>
          </div>
        ) : (
          <div
            className="flex items-center gap-2 rounded-md border px-2 py-1.5 hover:bg-muted/50 transition-colors"
            aria-labelledby={`shortcut-${shortcutName}-label`}
          >
            {rowLabel}
            <ShortcutChip value={value} />
            <Button
              size="xs"
              variant="outline"
              onClick={() => setIsEditing(true)}
              disabled={isLoading}
              aria-label={`Edit ${label} shortcut`}
            >
              <Edit3 className="size-3" />
              Edit
            </Button>
          </div>
        )}
    </div>
  );
};

export default ShortcutInput;