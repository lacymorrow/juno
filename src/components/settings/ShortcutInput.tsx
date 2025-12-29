import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { 
  AlertTriangle, 
  CheckCircle, 
  Edit3,
  Keyboard,
  Save 
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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

  if (isSystemManaged) {
    return (
      <div className="space-y-2">
        <Label>{label}</Label>
        <div className="flex items-center justify-between p-2 rounded border">
          <div className="flex items-center gap-3">
            <kbd className="px-2 py-1 bg-muted rounded text-sm min-w-[80px] text-center">
              {value}
            </kbd>
            <span className="text-sm text-muted-foreground">{description}</span>
          </div>
          <Badge variant="secondary" className="text-xs">System</Badge>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      <Label htmlFor={`shortcut-${shortcutName}`}>{label}</Label>
      <div className="space-y-2">
        {isEditing ? (
          <div className="space-y-3 p-3 border rounded-lg bg-muted/30">
            {/* Key capture area with enhanced feedback */}
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Label className="text-sm font-medium">Shortcut combination:</Label>
                {isCapturing && (
                  <div className="flex items-center gap-2">
                    <Badge variant="outline" className="text-xs animate-pulse bg-blue-50 border-blue-200">
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
                  "min-h-[50px] p-3 border-2 border-dashed rounded-lg flex items-center gap-2 cursor-pointer transition-all duration-200",
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
                        <kbd className="px-2 py-1 bg-background border rounded text-sm font-mono shadow-sm">
                          {key}
                        </kbd>
                        {index < pressedKeys.length - 1 && (
                          <span className="text-muted-foreground font-medium">+</span>
                        )}
                      </span>
                    ))}
                  </div>
                ) : currentValue ? (
                  <kbd className="px-2 py-1 bg-background border rounded text-sm font-mono shadow-sm">
                    {currentValue}
                  </kbd>
                ) : (
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <Keyboard className="h-4 w-4" />
                    <span className="text-sm">
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
            <div className="space-y-2">
              <Label className="text-sm">Or type manually:</Label>
              <Input
                value={currentValue}
                onChange={(e) => setCurrentValue(e.target.value)}
                placeholder={`e.g., ${getExampleShortcut()}, Ctrl+Shift+F1`}
                className="font-mono text-sm"
                disabled={isCapturing}
              />
              <div className="text-xs text-muted-foreground">
                Tip: Use modifiers like Alt, Ctrl, Cmd, Shift combined with letters
              </div>
            </div>

            {/* Enhanced validation feedback */}
            {(validationMessage || validationError) && (
              <div className={cn(
                "flex items-start gap-2 text-sm p-3 rounded-md border",
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
                size="sm"
                onClick={handleSave}
                disabled={isLoading || !!validationError || !currentValue.trim() || isCapturing}
                className="flex items-center gap-2"
              >
                <Save className="h-4 w-4" />
                Save
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={handleCancel}
                disabled={isLoading}
              >
                Cancel
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={isCapturing ? handleStopCapture : handleStartCapture}
                disabled={isLoading}
                className="ml-auto"
              >
                <Keyboard className="h-4 w-4 mr-1" />
                {isCapturing ? "Stop Capture" : "Capture Keys"}
              </Button>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-between p-2 rounded border hover:bg-muted/50 transition-colors">
            <div className="flex items-center gap-3">
              <kbd className="px-2 py-1 bg-muted rounded text-sm min-w-[80px] text-center font-mono">
                {value || "Not set"}
              </kbd>
              <span className="text-sm text-muted-foreground">{description}</span>
            </div>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setIsEditing(true)}
              disabled={isLoading}
              className="flex items-center gap-1"
            >
              <Edit3 className="h-4 w-4" />
              Edit
            </Button>
          </div>
        )}
      </div>
    </div>
  );
};

export default ShortcutInput;