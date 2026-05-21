import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import { EVENTS, COMMANDS } from "@/lib/constants.generated";
import {
  CheckCircle,
  ChevronRight,
  Eye,
  EyeOff,
  ExternalLink,
  Key,
  Keyboard,
  Loader2,
  Monitor,
  Shield,
  Sparkles,
  RefreshCw,
  Settings,
  AlertCircle,
  Info,
  Mic,
  Terminal,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useEventListener } from "@/hooks/useEventListener";
import AudioVisualizer from "../bar/audio-visualizer";

// Permission status interface matching backend (snake_case)
interface PermissionStatus {
  permission_type: string;
  granted: boolean;
  required: boolean;
  description: string;
  instructions: string;
}

// Complete permissions state interface (snake_case)
interface PermissionsState {
  accessibility: PermissionStatus;
  screen_recording: PermissionStatus;
  microphone: PermissionStatus;
  input_monitoring: PermissionStatus;
  all_granted: boolean;
  app_name: string;
}

const permissions = [
  {
    id: "accessibility",
    title: "Accessibility",
    description: "Allow Juno to control your computer",
    icon: <Eye className="w-5 h-5" />,
    required: true,
  },
  {
    id: "screen-recording",
    title: "Screen Recording",
    description: "Allow Juno to capture screen content",
    icon: <Monitor className="w-5 h-5" />,
    required: true,
  },
  {
    id: "microphone",
    title: "Microphone",
    description: "Allow Juno to use voice features",
    icon: <Sparkles className="w-5 h-5" />,
    required: false,
  },
  {
    id: "input-monitoring",
    title: "Input Monitoring",
    description: "Allow Juno to monitor keyboard and mouse",
    icon: <Keyboard className="w-5 h-5" />,
    required: false,
  },
];

const getOnboardingSteps = (
  permissionsAlreadyGranted: boolean,
  isDevelopmentMode: boolean = false,
  apiKeysAvailable: boolean = false
) => [
  {
    id: "welcome",
    title: "Welcome to Juno",
    subtitle: isDevelopmentMode
      ? "Your intelligent Mac companion (Development Mode)"
      : "Your intelligent Mac companion",
    description: isDevelopmentMode
      ? "Juno helps you automate tasks, manage your workflow, and get more done with your Mac. You're running in development mode, so onboarding will always show on startup."
      : "Juno helps you automate tasks, manage your workflow, and get more done with your Mac.",
    icon: (
      <img src="/juno.png" alt="Juno" className="w-50 h-50 object-contain" />
    ),
    action: "Get Started",
  },
  {
    id: "shortcut",
    title: "Learn the Magic Keys",
    subtitle: "Quick shortcuts to control Juno",
    description:
      "Try the agent mode shortcut below! This activates Juno from anywhere on your Mac. Watch the floating bar react — toggle it anytime with \u2318B.",
    icon: null, // Floating bar rendered persistently outside AnimatePresence
    action: "Continue",
  },
  {
    id: "cancel",
    title: "Escape to Cancel",
    subtitle: "Stop any operation with a single key",
    description:
      "Sometimes you need to stop what Juno is doing. Press Escape and the floating bar will confirm it stopped.",
    icon: null, // Floating bar rendered persistently outside AnimatePresence
    action: "Continue",
  },
  ...(apiKeysAvailable
    ? []
    : [
        {
          id: "api-key",
          title: "Connect Your AI",
          subtitle: "Choose how to power Juno",
          description:
            "Use your existing Claude subscription or paste an API key from any supported provider.",
          icon: <Key className="w-12 h-12 text-primary" />,
          action: "Continue",
        },
      ]),
  ...(permissionsAlreadyGranted
    ? []
    : [
        {
          id: "permissions",
          title: "Grant Permissions",
          subtitle: "Required for full functionality",
          description:
            "Juno needs these permissions to automate tasks and interact with your Mac securely.",
          icon: <Shield className="w-12 h-12 text-green-500" />,
          action: "Continue",
        },
      ]),
  {
    id: "complete",
    title: "Ready to Go",
    subtitle: "AI-powered desktop automation",
    description: "Juno can control your computer, browse the web, manage files, and execute tasks through natural language." + (permissionsAlreadyGranted
      ? " Just describe what you need — use the shortcut you learned to activate Juno anytime."
      : " You can always change permissions later in System Preferences."),
    icon: <CheckCircle className="w-12 h-12 text-green-500" />,
    action: "Start Using Juno",
  },
];

/**
 * Visual keyboard shortcut display.
 * This component is PURELY visual — it renders key boxes and animates them.
 * Shortcut detection is handled by the parent via backend Tauri events,
 * since global shortcuts (Option+D) are captured at the OS level and never
 * reach the webview as keydown events.
 */
function KeyboardShortcutDisplay({
  shortcutString,
  defaultShortcut = "Option+D",
  isActivated,
}: {
  shortcutString?: string;
  defaultShortcut?: string;
  isActivated: boolean;
}) {
  const parseShortcut = (shortcut: string) => {
    const parts = shortcut.split("+").map((part) => part.trim().toLowerCase());
    const modifiers = parts.slice(0, -1);
    const key = parts[parts.length - 1];
    return { modifiers, key };
  };

  const { modifiers, key } = parseShortcut(shortcutString || defaultShortcut);

  const displayKeys = () => {
    const modifierKeys = modifiers.map((mod) => {
      switch (mod) {
        case "option":
        case "alt":
          return "\u2325";
        case "cmd":
        case "command":
          return "\u2318";
        case "ctrl":
        case "control":
          return "\u2303";
        case "commandorcontrol":
          return "\u2318";
        case "shift":
          return "\u21E7";
        default:
          return mod.toUpperCase();
      }
    });

    const displayKey = (() => {
      switch (key) {
        case "escape":
          return "Esc";
        case "space":
          return "Space";
        case "enter":
        case "return":
          return "Return";
        case "backspace":
          return "Delete";
        case "delete":
          return "Fwd Del";
        case "tab":
          return "Tab";
        default:
          return key.toUpperCase();
      }
    })();

    return [...modifierKeys, displayKey];
  };

  const keys = displayKeys();

  return (
    <div className="flex items-center justify-center gap-4 my-8 relative">
      {keys.map((keySymbol, index) => (
        <div key={index} className="flex items-center">
          <motion.div
            className={`relative flex items-center justify-center w-16 h-16 rounded-xl border-2 transition-all duration-200 ${
              isActivated
                ? "bg-primary border-primary text-primary-foreground shadow-lg scale-95"
                : "bg-card border-border text-card-foreground shadow-sm"
            }`}
            animate={isActivated ? { scale: 0.95 } : { scale: 1 }}
          >
            <span className="text-lg font-semibold">{keySymbol}</span>
          </motion.div>
          {index < keys.length - 1 && (
            <div className="text-2xl font-light text-muted-foreground mx-2">+</div>
          )}
        </div>
      ))}

      {isActivated && (
        <motion.div
          initial={{ opacity: 0, scale: 0.8 }}
          animate={{ opacity: 1, scale: 1 }}
          className="absolute -right-12 top-1/2 transform -translate-y-1/2"
        >
          <div className="w-8 h-8 rounded-full bg-green-500 flex items-center justify-center">
            <CheckCircle className="w-5 h-5 text-white" />
          </div>
        </motion.div>
      )}
    </div>
  );
}

// Component for individual permission card
function PermissionCard({
  permission,
  permissionStatus,
  onRequest,
  isRequesting,
}: {
  permission: any;
  permissionStatus: PermissionStatus | null;
  onRequest: () => void;
  isRequesting: boolean;
}) {
  const granted = permissionStatus?.granted ?? false;
  const isRequired = permission.required;

  // Map permission IDs to icons
  const getPermissionIcon = () => {
    switch (permission.id) {
      case "accessibility":
        return <Eye className="w-5 h-5" />;
      case "screen-recording":
        return <Monitor className="w-5 h-5" />;
      case "microphone":
        return <Mic className="w-5 h-5" />;
      case "input-monitoring":
        return <Keyboard className="w-5 h-5" />;
      default:
        return <Shield className="w-5 h-5" />;
    }
  };

  return (
    <div
      className={`p-4 rounded-xl border-2 transition-all duration-200 ${
        granted
          ? "border-green-200 bg-green-50/30 dark:border-green-800 dark:bg-green-950/30"
          : isRequired
          ? "border-red-200 bg-red-50/30 dark:border-red-800 dark:bg-red-950/30"
          : "border-yellow-200 bg-yellow-50/30 dark:border-yellow-800 dark:bg-yellow-950/30"
      }`}
    >
      <div className="flex items-start gap-4">
        {/* Icon and Status */}
        <div className="flex items-center gap-2">
          <div
            className={`w-10 h-10 rounded-full flex items-center justify-center ${
              granted
                ? "bg-green-100 text-green-600 dark:bg-green-900 dark:text-green-400"
                : isRequired
                ? "bg-red-100 text-red-600 dark:bg-red-900 dark:text-red-400"
                : "bg-yellow-100 text-yellow-600 dark:bg-yellow-900 dark:text-yellow-400"
            }`}
          >
            {granted ? (
              <CheckCircle className="w-5 h-5" />
            ) : (
              getPermissionIcon()
            )}
          </div>
          {granted && (
            <div className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
          )}
        </div>

        {/* Content */}
        <div className="flex-1">
          <div className="flex items-center gap-2 mb-1">
            <h4 className="font-semibold text-foreground">{permission.title}</h4>
            {isRequired && (
              <span className="text-xs bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300 px-2 py-1 rounded-full font-medium">
                Required
              </span>
            )}
            {granted && (
              <span className="text-xs bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300 px-2 py-1 rounded-full font-medium">
                Granted
              </span>
            )}
          </div>
          <p className="text-sm text-muted-foreground mb-3">{permission.description}</p>

          {!granted && permissionStatus && (
            <div
              className={`text-xs p-2 rounded-md mb-3 ${
                isRequired
                  ? "bg-red-50 border border-red-200 text-red-700 dark:bg-red-950 dark:border-red-800 dark:text-red-300"
                  : "bg-yellow-50 border border-yellow-200 text-yellow-700 dark:bg-yellow-950 dark:border-yellow-800 dark:text-yellow-300"
              }`}
            >
              <div className="flex items-start gap-1">
                {isRequired ? (
                  <AlertCircle className="w-3 h-3 mt-0.5 flex-shrink-0" />
                ) : (
                  <Info className="w-3 h-3 mt-0.5 flex-shrink-0" />
                )}
                <span>{permissionStatus.instructions}</span>
              </div>
            </div>
          )}

          {/* Action Buttons */}
          {!granted && (
            <div className="flex gap-2">
              <button
                onClick={onRequest}
                disabled={isRequesting}
                className={`px-3 py-2 rounded-lg text-sm font-medium transition-all flex items-center gap-2 ${
                  isRequired
                    ? "bg-primary hover:bg-primary/90 text-primary-foreground"
                    : "bg-muted-foreground hover:bg-muted-foreground/90 text-background"
                } disabled:opacity-50 disabled:cursor-not-allowed`}
              >
                {isRequesting ? (
                  <>
                    <RefreshCw className="w-4 h-4 animate-spin" />
                    Opening...
                  </>
                ) : (
                  <>
                    <Settings className="w-4 h-4" />
                    Grant Permission
                  </>
                )}
              </button>
            </div>
          )}

          {granted && (
            <div className="flex items-center gap-2 text-sm text-green-700 dark:text-green-300">
              <CheckCircle className="w-4 h-4" />
              <span className="font-medium">Ready to use</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

interface OnboardingFlowProps {
  onComplete: () => void;
  onSkip?: () => void;
  permissionsAlreadyGranted?: boolean;
  isDevelopmentMode?: boolean;
}

export default function OnboardingFlow({
  onComplete,
  onSkip,
  permissionsAlreadyGranted = false,
  isDevelopmentMode = false,
}: OnboardingFlowProps) {
  console.log("OnboardingFlow: Component rendering with props:", {
    permissionsAlreadyGranted,
    isDevelopmentMode,
  });

  const [currentStep, setCurrentStep] = useState(0);
  const [shortcutPressed, setShortcutPressed] = useState(false);
  const [escapePressed, setEscapePressed] = useState(false);
  const [_backendShortcutsWorking, setBackendShortcutsWorking] =
    useState(false);
  const [keyboardShortcuts, setKeyboardShortcuts] = useState<any>(null);
  const [isComplete, setIsComplete] = useState(false);
  const [actualPermissionsGranted, setActualPermissionsGranted] = useState(
    // Always start with false to ensure we re-check permissions on mount
    // This is critical for the "Restart onboarding" functionality
    false
  );

  // API key state
  const [apiKeysAvailable, setApiKeysAvailable] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [detectedProvider, setDetectedProvider] = useState<{
    id: string;
    name: string;
  } | null>(null);
  const [showApiKey, setShowApiKey] = useState(false);
  const [apiKeySaving, setApiKeySaving] = useState(false);
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [apiKeyError, setApiKeyError] = useState<string | null>(null);

  // Claude CLI state
  const [cliAvailable, setCliAvailable] = useState(false);
  const [cliAuthenticated, setCliAuthenticated] = useState(false);
  const [cliChecking, setCliChecking] = useState(true);
  const [cliSelected, setCliSelected] = useState(false);

  // New state for granular permissions
  const [permissionsState, setPermissionsState] =
    useState<PermissionsState | null>(null);
  const [isRequestingPermission, setIsRequestingPermission] = useState<
    string | null
  >(null);
  const [permissionsError, setPermissionsError] = useState<string | null>(null);

  const mountedRef = useRef(true);
  const onboardingTimers = useRef<ReturnType<typeof setTimeout>[]>([]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      for (const timer of onboardingTimers.current) {
        clearTimeout(timer);
      }
      onboardingTimers.current = [];
    };
  }, []);

  // ── Backend-driven shortcut detection ──
  // Global shortcuts (Option+D, Escape) are captured at the OS level by
  // tauri_plugin_global_shortcut — they NEVER reach the webview as keydown events.
  // The backend always emits these events even during onboarding (visual feedback mode).
  // We use useEventListener (the project's canonical Tauri event hook) to detect them.
  useEventListener<{ state: string; shortcut: string }>(
    EVENTS.SHORTCUTS_AGENT_MODE,
    (payload) => {
      if (payload.state === "pressed" && !shortcutPressed) {
        console.log("[Onboarding] Agent mode shortcut detected via backend event");
        setShortcutPressed(true);
      }
    }
  );

  useEventListener<{ state: string; shortcut: string }>(
    EVENTS.SHORTCUTS_ESCAPE_KEY,
    (payload) => {
      if (payload.state === "pressed" && !escapePressed) {
        console.log("[Onboarding] Escape key detected via backend event");
        setEscapePressed(true);
      }
    }
  );

  // Function to check current permissions status
  const checkPermissionsStatus = async () => {
    try {
      setPermissionsError(null);
      const result = await invoke<PermissionsState>(
        COMMANDS.PERMISSIONS_CHECK_PERMISSIONS_STATUS
      );
      setPermissionsState(result);
      setActualPermissionsGranted(result.all_granted);
      console.log("OnboardingFlow: Updated permissions state:", result);
      return result.all_granted;
    } catch (error) {
      console.warn("Failed to check permissions status:", error);
      setPermissionsError(error as string);
      return false;
    }
  };

  // Check if required permissions (accessibility + screen recording) are granted
  const areRequiredPermissionsGranted = (): boolean => {
    if (!permissionsState) return false;
    return (
      permissionsState.accessibility.granted &&
      permissionsState.screen_recording.granted
    );
  };

  // Individual permission request functions
  const requestPermission = async (permissionType: string) => {
    try {
      setIsRequestingPermission(permissionType);
      setPermissionsError(null);

      let commandName = "";
      switch (permissionType) {
        case "accessibility":
          commandName = COMMANDS.PERMISSIONS_REQUEST_ACCESSIBILITY_PERMISSION;
          break;
        case "screen_recording":
          commandName = COMMANDS.PERMISSIONS_REQUEST_SCREEN_RECORDING_PERMISSION;
          break;
        case "microphone":
          commandName = COMMANDS.PERMISSIONS_REQUEST_MICROPHONE_PERMISSION;
          break;
        case "input_monitoring":
          commandName = COMMANDS.PERMISSIONS_REQUEST_INPUT_MONITORING_PERMISSION;
          break;
        default:
          throw new Error(`Unknown permission type: ${permissionType}`);
      }

      const granted = await invoke<boolean>(commandName);

      if (granted) {
        // Permission was already granted
        await checkPermissionsStatus();
      } else {
        // System Settings should be open for user to grant permission
        // Wait a moment and then refresh to check if user granted it
        onboardingTimers.current.push(setTimeout(async () => {
          if (mountedRef.current) await checkPermissionsStatus();
        }, 2000));
      }
    } catch (error) {
      console.error(`Error requesting ${permissionType} permission:`, error);
      setPermissionsError(error as string);
    } finally {
      setIsRequestingPermission(null);
    }
  };

  // Auto-detect provider from API key prefix
  const detectProvider = useCallback(
    (key: string): { id: string; name: string } | null => {
      const trimmed = key.trim();
      // Anthropic keys: sk-ant-api03-...
      if (trimmed.startsWith("sk-ant-")) {
        return { id: "anthropic", name: "Anthropic" };
      }
      // OpenAI keys: sk-proj-... (current) or sk-... (legacy, but not sk-ant-)
      if (trimmed.startsWith("sk-proj-") || (trimmed.startsWith("sk-") && !trimmed.startsWith("sk-ant-"))) {
        return { id: "openai", name: "OpenAI" };
      }
      // Google Gemini keys: AIza...
      if (trimmed.startsWith("AIza")) {
        return { id: "gemini", name: "Google Gemini" };
      }
      return null;
    },
    []
  );

  const handleApiKeyChange = useCallback(
    (value: string) => {
      setApiKey(value);
      setApiKeySaved(false);
      setApiKeyError(null);
      setDetectedProvider(detectProvider(value));
    },
    [detectProvider]
  );

  const saveApiKey = useCallback(async () => {
    if (!detectedProvider || !apiKey.trim()) return;
    try {
      setApiKeySaving(true);
      setApiKeyError(null);
      await invoke(COMMANDS.PROVIDERS_UPDATE_PROVIDER_API_KEY, {
        providerId: detectedProvider.id,
        apiKey: apiKey.trim(),
      });
      await invoke(COMMANDS.PROVIDERS_SET_ACTIVE_PROVIDER, {
        providerId: detectedProvider.id,
      });
      setApiKeySaved(true);
    } catch (error) {
      console.error("Failed to save API key:", error);
      setApiKeyError(error as string);
    } finally {
      setApiKeySaving(false);
    }
  }, [detectedProvider, apiKey]);

  const selectClaudeCli = useCallback(async () => {
    try {
      setApiKeySaving(true);
      setApiKeyError(null);
      await invoke(COMMANDS.PROVIDERS_SET_ACTIVE_PROVIDER, {
        providerId: "claude_cli",
      });
      setCliSelected(true);
    } catch (error) {
      console.error("Failed to set Claude CLI as active provider:", error);
      setApiKeyError(error as string);
    } finally {
      setApiKeySaving(false);
    }
  }, []);

  const deselectClaudeCli = useCallback(() => {
    setCliSelected(false);
  }, []);

  useEffect(() => {
    let mounted = true;

    const loadInitialData = async () => {
      try {
        console.log("OnboardingFlow: Loading initial data...");

        // CRITICAL: Always re-check permissions when component mounts
        console.log("OnboardingFlow: Re-checking permissions status...");
        await checkPermissionsStatus();
        if (!mounted) return;

        // Check if API keys are already available (from store or .env)
        try {
          const keysAvailable = await invoke<boolean>("check_api_keys_available");
          if (mounted) {
            setApiKeysAvailable(keysAvailable);
            if (keysAvailable) {
              console.log("OnboardingFlow: API keys already available, skipping API key step");
            }
          }
        } catch (error) {
          console.warn("Failed to check API keys availability:", error);
        }
        if (!mounted) return;

        // Check Claude CLI availability and auth status
        try {
          setCliChecking(true);
          const cliStatus = await invoke<{ available: boolean; authenticated: boolean }>("check_claude_cli_available");
          if (mounted) {
            setCliAvailable(cliStatus.available);
            setCliAuthenticated(cliStatus.authenticated);
            console.log("OnboardingFlow: Claude CLI status:", cliStatus);
          }
        } catch (error) {
          console.warn("Failed to check Claude CLI availability:", error);
        } finally {
          if (mounted) setCliChecking(false);
        }
        if (!mounted) return;

        // Load onboarding info and shortcuts
        const onboardingInfo = await invoke("get_onboarding_info");
        if (!mounted) return;
        if (
          onboardingInfo &&
          typeof onboardingInfo === "object" &&
          "shortcuts" in onboardingInfo
        ) {
          setKeyboardShortcuts((onboardingInfo as any).shortcuts);
        }

        // Test if backend shortcuts are working
        const shortcutsWorking = await invoke<boolean>(
          "test_global_shortcuts_working"
        );
        if (!mounted) return;
        setBackendShortcutsWorking(shortcutsWorking);

        // Load keyboard shortcuts as fallback
        try {
          const shortcuts = await invoke("get_keyboard_shortcuts");
          if (mounted) {
            setKeyboardShortcuts((prev: any) => prev ?? shortcuts);
          }
        } catch (error) {
          console.warn("Failed to load keyboard shortcuts:", error);
        }
      } catch (error) {
        console.error("Failed to load onboarding data:", error);
      }
    };

    // Add window focus listener to re-check permissions and CLI status when window gains focus
    const handleWindowFocus = async () => {
      if (!mounted) return;
      console.log(
        "OnboardingFlow: Window gained focus, re-checking permissions and CLI status"
      );
      await checkPermissionsStatus();
      try {
        const cliStatus = await invoke<{ available: boolean; authenticated: boolean }>("check_claude_cli_available");
        if (!mounted) return;
        if (cliStatus.available !== cliAvailable || cliStatus.authenticated !== cliAuthenticated) {
          setCliAvailable(cliStatus.available);
          setCliAuthenticated(cliStatus.authenticated);
        }
      } catch (error) {
        console.warn("Failed to refresh Claude CLI status on focus:", error);
      }
    };

    window.addEventListener("focus", handleWindowFocus);
    loadInitialData();

    return () => {
      mounted = false;
      window.removeEventListener("focus", handleWindowFocus);
    };
  }, []);

  const onboardingSteps = getOnboardingSteps(
    actualPermissionsGranted,
    isDevelopmentMode,
    apiKeysAvailable
  );

  console.log(
    "OnboardingFlow: Generated onboarding steps:",
    onboardingSteps.map((step) => ({ id: step.id, title: step.title }))
  );

  const handleNext = useCallback(async () => {
    // Block navigation from keyboard shortcut steps if shortcut hasn't been pressed
    const currentStepData = onboardingSteps[currentStep];
    if (currentStepData?.id === "shortcut" && !shortcutPressed) {
      return;
    }
    if (currentStepData?.id === "cancel" && !escapePressed) {
      return;
    }
    // Block navigation from permissions step if required permissions not granted
    if (currentStepData?.id === "permissions" && !areRequiredPermissionsGranted()) {
      return;
    }
    // Save API key before advancing from api-key step (unless CLI was chosen)
    if (currentStepData?.id === "api-key" && !cliSelected && detectedProvider && !apiKeySaved) {
      await saveApiKey();
    }

    if (currentStep < onboardingSteps.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      setIsComplete(true);
      onComplete();
    }
  }, [currentStep, onboardingSteps, shortcutPressed, escapePressed, permissionsState, detectedProvider, apiKeySaved, saveApiKey, onComplete]);

  const handleSkip = () => {
    // Skip the current step by jumping to the end
    setIsComplete(true);
    if (onSkip) {
      onSkip();
    } else {
      onComplete();
    }
  };

  const handleSkipStep = () => {
    // Skip just the current step and move to the next one
    if (currentStep < onboardingSteps.length - 1) {
      // Reset step-specific states when skipping
      const currentStepData = onboardingSteps[currentStep];
      if (currentStepData?.id === "shortcut") {
        setShortcutPressed(true); // Allow progression if they come back
      }
      setCurrentStep(currentStep + 1);
    } else {
      // If this is the last step, complete onboarding
      setIsComplete(true);
      if (onSkip) {
        onSkip();
      } else {
        onComplete();
      }
    }
  };

  // Keyboard navigation: Enter key advances steps
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleNext();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleNext]);

  if (isComplete) {
    return null;
  }

  const step = onboardingSteps[currentStep];

  // Determine if continue button should be disabled
  const isContinueDisabled =
    (step.id === "shortcut" && !shortcutPressed) ||
    (step.id === "cancel" && !escapePressed) ||
    (step.id === "permissions" && !areRequiredPermissionsGranted()) ||
    (step.id === "api-key" && !detectedProvider && !cliSelected) ||
    (step.id === "api-key" && apiKeySaving);

  // Determine if skip should be hidden (permissions step with required perms not granted)
  const isSkipHidden =
    currentStep === onboardingSteps.length - 1 ||
    (step.id === "permissions" && !areRequiredPermissionsGranted());

  return (
    <>
      <div className="fixed inset-0 flex items-center justify-center z-50">
        <div className="bg-background/90 max-w-[600px] w-full max-h-[90vh] overflow-y-auto p-10">
          {/* Progress indicator */}
          <div
            className="flex gap-2 mb-10"
            role="progressbar"
            aria-valuenow={currentStep + 1}
            aria-valuemax={onboardingSteps.length}
            aria-label={`Step ${currentStep + 1} of ${onboardingSteps.length}`}
          >
            {onboardingSteps.map((_, index) => (
              <div
                key={index}
                className={`h-1 flex-1 rounded-full transition-all duration-500 ${
                  index <= currentStep ? "bg-primary" : "bg-muted"
                }`}
              />
            ))}
          </div>

          {/* Persistent floating bar preview across shortcut & cancel steps */}
          <AnimatePresence>
            {(step.id === "shortcut" || step.id === "cancel") && (
              <motion.div
                key="floating-bar-preview"
                initial={{ opacity: 0, y: -10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -10 }}
                transition={{ duration: 0.4, ease: "easeOut" }}
                className="flex justify-center mb-8"
              >
                <AudioVisualizer
                  appState={
                    (() => {
                      if (step.id === "cancel") {
                        return escapePressed ? "error" : "listening";
                      }
                      if (step.id === "shortcut") {
                        return shortcutPressed ? "listening" : "idle";
                      }
                      return "idle";
                    })()
                  }
                  width={350}
                  height={60}
                  enableMicrophone={false}
                  intensity={step.id === "cancel" ? 1.8 : 1.2}
                  animationStyle="organic"
                />
              </motion.div>
            )}
          </AnimatePresence>

          <AnimatePresence mode="wait">
            <motion.div
              key={step.id}
              initial={{ opacity: 0, y: 15 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -15 }}
              transition={{ duration: 0.4, ease: "easeOut" }}
              className="text-center"
            >
              {/* Icon (null for shortcut/cancel — floating bar is above) */}
              {step.icon && <div className="flex justify-center mb-8">{step.icon}</div>}

              {/* Content */}
              <div className="space-y-4 mb-10">
                <div>
                  <h2 className="text-3xl font-light text-foreground mb-3">
                    {step.title}
                  </h2>
                  <p className="text-sm text-primary font-medium mb-4">
                    {step.subtitle}
                  </p>
                </div>

                <p className="text-muted-foreground leading-relaxed text-base">
                  {step.description}
                </p>

                {/* Keyboard shortcut for shortcut step — visual only, detection via backend */}
                {step.id === "shortcut" && (
                  <div className="relative">
                    <KeyboardShortcutDisplay
                      shortcutString={keyboardShortcuts?.agent_mode}
                      isActivated={shortcutPressed}
                    />
                    <p className="text-sm text-muted-foreground mt-4 text-center">
                      {shortcutPressed
                        ? "Perfect! You've got it."
                        : keyboardShortcuts?.agent_mode
                        ? `Press ${keyboardShortcuts.agent_mode.replace(
                            /\+/g,
                            " + "
                          )} to activate agent mode`
                        : "Press the keys above together to activate agent mode"}
                    </p>
                  </div>
                )}

                {/* Cancel shortcut for cancel step — visual only, detection via backend */}
                {step.id === "cancel" && (
                  <div className="relative">
                    <KeyboardShortcutDisplay
                      shortcutString={keyboardShortcuts?.stop_current_task}
                      defaultShortcut="Escape"
                      isActivated={escapePressed}
                    />
                    <p className="text-sm text-muted-foreground mt-4 text-center">
                      {escapePressed
                        ? "Perfect! You've got it."
                        : keyboardShortcuts?.stop_current_task
                        ? `Press ${keyboardShortcuts.stop_current_task.replace(
                            /\+/g,
                            " + "
                          )} to stop Juno`
                        : "Press the key above to stop Juno"}
                    </p>
                  </div>
                )}

                {/* Connect step: Claude CLI + API key dual path */}
                {step.id === "api-key" && (
                  <div className="space-y-4 mt-6 text-left">
                    {/* Claude CLI card */}
                    <div
                      className={`rounded-xl border-2 p-4 transition-colors ${
                        cliSelected
                          ? "border-green-500 bg-green-50/50 dark:bg-green-950/30 hover:border-green-400 cursor-pointer"
                          : cliAvailable && cliAuthenticated
                            ? "border-primary/50 bg-card hover:border-primary cursor-pointer"
                            : "border-border bg-card"
                      }`}
                      onClick={cliAvailable && cliAuthenticated ? (cliSelected ? deselectClaudeCli : selectClaudeCli) : undefined}
                      role={cliAvailable && cliAuthenticated ? "button" : undefined}
                      tabIndex={cliAvailable && cliAuthenticated ? 0 : undefined}
                      onKeyDown={cliAvailable && cliAuthenticated ? (e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          cliSelected ? deselectClaudeCli() : selectClaudeCli();
                        }
                      } : undefined}
                    >
                      <div className="flex items-start gap-3">
                        <div className="mt-0.5">
                          <Terminal className="w-5 h-5 text-primary" />
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2">
                            <span className="font-medium text-foreground">
                              Use your Claude subscription
                            </span>
                            {cliSelected && (
                              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-green-100 dark:bg-green-900 text-xs font-medium text-green-700 dark:text-green-300">
                                <CheckCircle className="w-3 h-3" /> Connected
                              </span>
                            )}
                          </div>
                          <p className="text-sm text-muted-foreground mt-1">
                            No API key needed — uses Claude Code CLI authentication.
                          </p>
                          <div className="mt-2">
                            {cliChecking ? (
                              <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground">
                                <Loader2 className="w-3 h-3 animate-spin" /> Checking...
                              </span>
                            ) : cliAvailable && cliAuthenticated ? (
                              <span className="inline-flex items-center gap-1.5 text-xs text-green-600 dark:text-green-400">
                                <CheckCircle className="w-3 h-3" /> Claude CLI detected and authenticated
                              </span>
                            ) : cliAvailable ? (
                              <span className="inline-flex items-center gap-1.5 text-xs text-yellow-600 dark:text-yellow-400">
                                <AlertCircle className="w-3 h-3" /> Installed but not logged in — run{" "}
                                <code className="px-1 py-0.5 rounded bg-muted font-mono">claude login</code>
                              </span>
                            ) : (
                              <a
                                href="https://claude.ai/code"
                                target="_blank"
                                rel="noopener noreferrer"
                                className="inline-flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
                              >
                                <ExternalLink className="w-3 h-3" /> Not installed — get Claude Code
                              </a>
                            )}
                          </div>
                        </div>
                      </div>
                    </div>

                    {/* Divider */}
                    <div className="flex items-center gap-3 px-2">
                      <div className="flex-1 h-px bg-border" />
                      <span className="text-xs text-muted-foreground uppercase tracking-wider">or</span>
                      <div className="flex-1 h-px bg-border" />
                    </div>

                    {/* API key input */}
                    <div className={`rounded-xl border-2 p-4 transition-colors ${
                      apiKeySaved
                        ? "border-green-500 bg-green-50/50 dark:bg-green-950/30"
                        : "border-border bg-card"
                    }`}>
                      <div className="flex items-center gap-2 mb-3">
                        <Key className="w-4 h-4 text-muted-foreground" />
                        <span className="text-sm font-medium text-foreground">Paste an API key</span>
                      </div>
                      <div className="relative">
                        <input
                          type={showApiKey ? "text" : "password"}
                          value={apiKey}
                          onChange={(e) => handleApiKeyChange(e.target.value)}
                          placeholder="sk-ant-... or sk-proj-... or AIza..."
                          className="w-full px-3 py-2 pr-10 rounded-lg border border-border bg-background text-foreground placeholder:text-muted-foreground focus:outline-none focus:border-primary transition-colors font-mono text-sm"
                          spellCheck={false}
                          autoComplete="off"
                          disabled={cliSelected}
                        />
                        <button
                          type="button"
                          onClick={() => setShowApiKey(!showApiKey)}
                          className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                          aria-label={showApiKey ? "Hide API key" : "Show API key"}
                        >
                          {showApiKey ? (
                            <EyeOff className="w-4 h-4" />
                          ) : (
                            <Eye className="w-4 h-4" />
                          )}
                        </button>
                      </div>

                      {/* Provider detection badge */}
                      {detectedProvider && (
                        <motion.div
                          initial={{ opacity: 0, y: 5 }}
                          animate={{ opacity: 1, y: 0 }}
                          className="flex items-center gap-2 mt-2"
                        >
                          <CheckCircle className="w-3.5 h-3.5 text-green-600 dark:text-green-400" />
                          <span className="text-xs font-medium text-green-700 dark:text-green-300">
                            {detectedProvider.name} detected
                          </span>
                        </motion.div>
                      )}

                      {/* Unknown key warning */}
                      {apiKey.trim().length > 0 && !detectedProvider && (
                        <motion.div
                          initial={{ opacity: 0, y: 5 }}
                          animate={{ opacity: 1, y: 0 }}
                          className="flex items-center gap-2 mt-2"
                        >
                          <AlertCircle className="w-3.5 h-3.5 text-yellow-600 dark:text-yellow-400" />
                          <span className="text-xs font-medium text-yellow-700 dark:text-yellow-300">
                            Unrecognized key format
                          </span>
                        </motion.div>
                      )}

                      {/* Save confirmation */}
                      {apiKeySaved && (
                        <motion.div
                          initial={{ opacity: 0, y: 5 }}
                          animate={{ opacity: 1, y: 0 }}
                          className="flex items-center gap-2 mt-2"
                        >
                          <CheckCircle className="w-3.5 h-3.5 text-green-600 dark:text-green-400" />
                          <span className="text-xs font-medium text-green-700 dark:text-green-300">
                            API key saved! {detectedProvider?.name} set as active.
                          </span>
                        </motion.div>
                      )}
                    </div>

                    {/* Error message */}
                    {apiKeyError && (
                      <div className="p-2 bg-red-50 border border-red-200 rounded-lg dark:bg-red-950 dark:border-red-800">
                        <p className="text-sm text-red-700 dark:text-red-300">
                          Failed to save: {apiKeyError}
                        </p>
                      </div>
                    )}

                    {/* Footer hint */}
                    <p className="text-xs text-muted-foreground text-center">
                      You can skip this step and configure later in Settings.
                    </p>
                  </div>
                )}

                {/* Granular permissions interface */}
                {step.id === "permissions" && (
                  <div className="space-y-4 mt-8 text-left">
                    {/* Header */}
                    <div className="text-center mb-6">
                      <h3 className="text-lg font-semibold text-foreground mb-2">
                        Grant Permissions
                      </h3>
                      <p className="text-sm text-muted-foreground">
                        Click "Grant Permission" for each item below to enable
                        full functionality
                      </p>
                      {permissionsError && (
                        <div className="mt-2 p-2 bg-red-50 border border-red-200 rounded-md dark:bg-red-950 dark:border-red-800">
                          <p className="text-sm text-red-700 dark:text-red-300">
                            Error: {permissionsError}
                          </p>
                        </div>
                      )}
                    </div>

                    {/* Permission Cards */}
                    <div className="space-y-3">
                      {permissions.map((permission) => {
                        const permissionKey = permission.id.replace(
                          "-",
                          "_"
                        ) as keyof PermissionsState;
                        const permissionStatus =
                          (permissionsState?.[
                            permissionKey
                          ] as PermissionStatus) || null;

                        return (
                          <PermissionCard
                            key={permission.id}
                            permission={permission}
                            permissionStatus={permissionStatus}
                            onRequest={() =>
                              requestPermission(permission.id.replace("-", "_"))
                            }
                            isRequesting={
                              isRequestingPermission ===
                              permission.id.replace("-", "_")
                            }
                          />
                        );
                      })}
                    </div>

                    {/* Summary */}
                    {permissionsState && (
                      <div className="mt-6 p-4 bg-muted rounded-lg">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            {areRequiredPermissionsGranted() ? (
                              <>
                                <CheckCircle className="w-5 h-5 text-green-600 dark:text-green-400" />
                                <span className="font-medium text-green-800 dark:text-green-300">
                                  All required permissions granted!
                                </span>
                              </>
                            ) : (
                              <>
                                <AlertCircle className="w-5 h-5 text-orange-600 dark:text-orange-400" />
                                <span className="font-medium text-orange-800 dark:text-orange-300">
                                  Accessibility and Screen Recording are required to continue.
                                </span>
                              </>
                            )}
                          </div>
                          <button
                            onClick={checkPermissionsStatus}
                            className="text-sm text-primary hover:text-primary/80 flex items-center gap-1"
                            aria-label="Refresh permission status"
                          >
                            <RefreshCw className="w-4 h-4" />
                            Refresh
                          </button>
                        </div>
                      </div>
                    )}
                  </div>
                )}
              </div>

              {/* Actions */}
              <div className="flex gap-4">
                {/* Skip button: hidden on final step and on permissions step when required perms not granted */}
                {isSkipHidden ? null : currentStep === 0 ? (
                  <button
                    onClick={handleSkip}
                    className="flex-1 py-3 text-muted-foreground hover:text-foreground font-medium transition-colors"
                    aria-label="Skip onboarding"
                  >
                    Skip
                  </button>
                ) : (
                  <button
                    onClick={handleSkipStep}
                    className="flex-1 py-3 text-muted-foreground hover:text-foreground font-medium transition-colors"
                    aria-label="Skip this step"
                  >
                    Skip
                  </button>
                )}

                {/* Continue button - disable for shortcut/cancel/permissions steps until conditions met */}
                <button
                  onClick={handleNext}
                  disabled={isContinueDisabled}
                  className={`flex-1 py-3 px-6 font-medium rounded-xl transition-all duration-200 flex items-center justify-center gap-2 ${
                    isContinueDisabled
                      ? "bg-muted text-muted-foreground cursor-not-allowed"
                      : "bg-primary hover:bg-primary/90 text-primary-foreground shadow-lg hover:shadow-xl"
                  }`}
                  aria-label={step.action}
                >
                  {step.action}
                  <ChevronRight className="w-4 h-4" />
                </button>
              </div>
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </>
  );
}
