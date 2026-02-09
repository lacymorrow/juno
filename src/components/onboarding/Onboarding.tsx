import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "framer-motion";
import { EVENTS, COMMANDS } from "@/lib/constants.generated";
import {
  CheckCircle,
  ChevronRight,
  Eye,
  Keyboard,
  Monitor,
  Shield,
  Sparkles,
  RefreshCw,
  Settings,
  AlertCircle,
  Info,
  Mic,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";
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
  isDevelopmentMode: boolean = false
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
      "Try the agent mode shortcut below! This will activate Juno's AI assistant from anywhere on your Mac.",
    icon: (
      <AudioVisualizer
        appState="listening"
        width={350}
        height={60}
        enableMicrophone={false}
        intensity={1.2}
        animationStyle="organic"
      />
    ),
    action: "Continue",
  },
  {
    id: "cancel",
    title: "Escape to Cancel",
    subtitle: "Stop any operation with a single key",
    description:
      "Sometimes you need to stop what Juno is doing. Press Escape to stop Juno.",
    icon: (
      <AudioVisualizer
        appState="error"
        width={350}
        height={60}
        enableMicrophone={false}
        intensity={1.8}
        animationStyle="organic"
      />
    ),
    action: "Continue",
  },
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
          action: "Open System Preferences",
        },
      ]),
  {
    id: "complete",
    title: "Ready to Go",
    subtitle: "Juno is now active",
    description: permissionsAlreadyGranted
      ? "Juno is ready to go! Press ⌘J anytime to get started!"
      : "You can always change these permissions later in System Preferences. Press ⌘J anytime to get started!",
    icon: <CheckCircle className="w-12 h-12 text-green-500" />,
    action: "Start Using Juno",
  },
];

// Keyboard shortcut component
function KeyboardShortcut({
  onShortcutPressed,
  shortcutString,
}: {
  onShortcutPressed: () => void;
  shortcutString?: string;
}) {
  const [isComplete, setIsComplete] = useState(false);
  const [pressedKeys, setPressedKeys] = useState<Set<string>>(new Set());

  const parseShortcut = (shortcut: string) => {
    const parts = shortcut.split("+").map((part) => part.trim().toLowerCase());
    const modifiers = parts.slice(0, -1);
    const key = parts[parts.length - 1];
    return { modifiers, key };
  };

  const shortcut = shortcutString
    ? parseShortcut(shortcutString)
    : { modifiers: ["option"], key: "d" };
  const { modifiers, key } = shortcut;

  // Use refs to avoid re-creating listeners on every keystroke
  const pressedKeysRef = useRef(pressedKeys);
  pressedKeysRef.current = pressedKeys;
  const onShortcutPressedRef = useRef(onShortcutPressed);
  onShortcutPressedRef.current = onShortcutPressed;
  const pendingTimers = useRef<ReturnType<typeof setTimeout>[]>([]);

  // Listen for both frontend key events and backend shortcut detection
  useEffect(() => {
    let mounted = true;
    let unlisten: (() => void) | undefined;

    const handleKeyDown = (e: KeyboardEvent) => {
      const newPressedKeys = new Set(pressedKeysRef.current);

      // Check for modifiers
      if (
        e.altKey &&
        (modifiers.includes("option") || modifiers.includes("alt"))
      ) {
        newPressedKeys.add("option");
      }
      if (
        e.metaKey &&
        (modifiers.includes("cmd") ||
          modifiers.includes("command") ||
          modifiers.includes("commandorcontrol"))
      ) {
        newPressedKeys.add("cmd");
      }
      if (
        e.ctrlKey &&
        (modifiers.includes("ctrl") ||
          modifiers.includes("control") ||
          modifiers.includes("commandorcontrol"))
      ) {
        newPressedKeys.add("ctrl");
      }
      if (e.shiftKey && modifiers.includes("shift")) {
        newPressedKeys.add("shift");
      }

      // Check for key
      if (e.key.toLowerCase() === key) {
        newPressedKeys.add(key);
      }

      setPressedKeys(newPressedKeys);

      // Check if correct combination is pressed (frontend detection)
      const modifierPressed = modifiers.every((mod) => {
        switch (mod) {
          case "option":
          case "alt":
            return e.altKey;
          case "cmd":
          case "command":
            return e.metaKey;
          case "ctrl":
          case "control":
            return e.ctrlKey;
          case "commandorcontrol":
            return e.metaKey || e.ctrlKey;
          case "shift":
            return e.shiftKey;
          default:
            return false;
        }
      });

      if (modifierPressed && e.key.toLowerCase() === key) {
        e.preventDefault();
        setIsComplete(true);
        pendingTimers.current.push(setTimeout(() => {
          if (mounted) onShortcutPressedRef.current();
        }, 800));
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      const newPressedKeys = new Set(pressedKeysRef.current);

      if (!e.altKey) newPressedKeys.delete("option");
      if (!e.metaKey) newPressedKeys.delete("cmd");
      if (!e.ctrlKey) newPressedKeys.delete("ctrl");
      if (!e.shiftKey) newPressedKeys.delete("shift");
      if (e.key.toLowerCase() === key) newPressedKeys.delete(key);

      setPressedKeys(newPressedKeys);
    };

    // Listen for backend shortcut detection events
    const setupBackendListener = async () => {
      try {
        const fn = await listen(EVENTS.SHORTCUTS_AGENT_MODE, (event: any) => {
          if (!mounted) return;
          if (event.payload?.state === "pressed") {
            setIsComplete(true);
            const allShortcutKeys = new Set([...modifiers, key]);
            setPressedKeys(allShortcutKeys);

            pendingTimers.current.push(setTimeout(() => {
              if (mounted) onShortcutPressedRef.current();
            }, 800));

            pendingTimers.current.push(setTimeout(() => {
              if (mounted) setPressedKeys(new Set());
            }, 1200));
          }
        });

        if (mounted) {
          unlisten = fn;
        } else {
          safeCleanupEventListener(fn);
        }
      } catch (error) {
        console.warn("Failed to setup backend shortcut listener:", error);
      }
    };

    setupBackendListener();

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);

    return () => {
      mounted = false;
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      safeCleanupEventListener(unlisten);
      for (const timer of pendingTimers.current) {
        clearTimeout(timer);
      }
      pendingTimers.current = [];
    };
  }, [modifiers, key]);

  // Display the shortcut keys
  const displayKeys = () => {
    const modifierKeys = modifiers.map((mod) => {
      switch (mod) {
        case "option":
        case "alt":
          return "⌥";
        case "cmd":
        case "command":
          return "⌘";
        case "ctrl":
        case "control":
          return "⌃";
        case "commandorcontrol":
          return "⌘";
        case "shift":
          return "⇧";
        default:
          return mod.toUpperCase();
      }
    });

    return [...modifierKeys, key.toUpperCase()];
  };

  const keys = displayKeys();

  return (
    <div className="flex items-center justify-center gap-4 my-8 relative">
      {keys.map((keySymbol, index) => {
        const isPressed =
          (keySymbol === "⌥" && pressedKeys.has("option")) ||
          (keySymbol === "⌘" && pressedKeys.has("cmd")) ||
          (keySymbol === "⌃" && pressedKeys.has("ctrl")) ||
          (keySymbol === "⇧" && pressedKeys.has("shift")) ||
          (keySymbol === key.toUpperCase() && pressedKeys.has(key));

        return (
          <div key={index} className="flex items-center">
            <motion.div
              className={`relative flex items-center justify-center w-16 h-16 rounded-xl border-2 transition-all duration-200 ${
                isPressed
                  ? "bg-blue-500 border-blue-600 text-white shadow-lg scale-95"
                  : "bg-white border-gray-300 text-gray-700 shadow-sm"
              }`}
              animate={isPressed ? { scale: 0.95 } : { scale: 1 }}
            >
              <span className="text-lg font-semibold">{keySymbol}</span>
            </motion.div>
            {index < keys.length - 1 && (
              <div className="text-2xl font-light text-gray-400 mx-2">+</div>
            )}
          </div>
        );
      })}

      {isComplete && (
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
          ? "border-green-200 bg-green-50/30"
          : isRequired
          ? "border-red-200 bg-red-50/30"
          : "border-yellow-200 bg-yellow-50/30"
      }`}
    >
      <div className="flex items-start gap-4">
        {/* Icon and Status */}
        <div className="flex items-center gap-2">
          <div
            className={`w-10 h-10 rounded-full flex items-center justify-center ${
              granted
                ? "bg-green-100 text-green-600"
                : isRequired
                ? "bg-red-100 text-red-600"
                : "bg-yellow-100 text-yellow-600"
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
            <h4 className="font-semibold text-gray-900">{permission.title}</h4>
            {isRequired && (
              <span className="text-xs bg-orange-100 text-orange-700 px-2 py-1 rounded-full font-medium">
                Required
              </span>
            )}
            {granted && (
              <span className="text-xs bg-green-100 text-green-700 px-2 py-1 rounded-full font-medium">
                Granted
              </span>
            )}
          </div>
          <p className="text-sm text-gray-600 mb-3">{permission.description}</p>

          {!granted && permissionStatus && (
            <div
              className={`text-xs p-2 rounded-md mb-3 ${
                isRequired
                  ? "bg-red-50 border border-red-200 text-red-700"
                  : "bg-yellow-50 border border-yellow-200 text-yellow-700"
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
                    ? "bg-blue-600 hover:bg-blue-700 text-white"
                    : "bg-gray-600 hover:bg-gray-700 text-white"
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
            <div className="flex items-center gap-2 text-sm text-green-700">
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
  const [_backendShortcutsWorking, setBackendShortcutsWorking] =
    useState(false);
  const [keyboardShortcuts, setKeyboardShortcuts] = useState<any>(null);
  const [isComplete, setIsComplete] = useState(false);
  const [actualPermissionsGranted, setActualPermissionsGranted] = useState(
    // Always start with false to ensure we re-check permissions on mount
    // This is critical for the "Restart onboarding" functionality
    false
  );

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

  console.log(
    "OnboardingFlow: State - currentStep:",
    currentStep,
    "isComplete:",
    isComplete,
    "shortcutPressed:",
    shortcutPressed,
    "actualPermissionsGranted:",
    actualPermissionsGranted,
    "permissionsAlreadyGranted prop:",
    permissionsAlreadyGranted
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

  useEffect(() => {
    let mounted = true;

    const loadInitialData = async () => {
      try {
        console.log("OnboardingFlow: Loading initial data...");

        // CRITICAL: Always re-check permissions when component mounts
        console.log("OnboardingFlow: Re-checking permissions status...");
        await checkPermissionsStatus();
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

    // Add window focus listener to re-check permissions when window gains focus
    const handleWindowFocus = async () => {
      if (!mounted) return;
      console.log(
        "OnboardingFlow: Window gained focus, re-checking permissions"
      );
      await checkPermissionsStatus();
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
    isDevelopmentMode
  );

  console.log(
    "OnboardingFlow: Generated onboarding steps:",
    onboardingSteps.map((step) => ({ id: step.id, title: step.title }))
  );

  const handleNext = () => {
    // Block navigation from shortcut step if shortcut hasn't been pressed
    const currentStepData = onboardingSteps[currentStep];
    if (currentStepData?.id === "shortcut" && !shortcutPressed) {
      return;
    }

    if (currentStep < onboardingSteps.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      setIsComplete(true);
      onComplete();
    }
  };

  const handleShortcutPressed = () => {
    setShortcutPressed(true);
  };

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

  if (isComplete) {
    return null;
  }

  const step = onboardingSteps[currentStep];

  return (
    <>
      <div className="fixed inset-0 flex items-center justify-center z-50">
        <div className="bg-white/90 max-w-[600px] w-full max-h-[90vh] overflow-y-auto p-10">
          {/* Progress indicator */}
          <div className="flex gap-2 mb-10">
            {onboardingSteps.map((_, index) => (
              <div
                key={index}
                className={`h-1 flex-1 rounded-full transition-all duration-500 ${
                  index <= currentStep ? "bg-blue-500" : "bg-gray-200"
                }`}
              />
            ))}
          </div>

          <AnimatePresence mode="wait">
            <motion.div
              key={step.id}
              initial={{ opacity: 0, y: 15 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -15 }}
              transition={{ duration: 0.4, ease: "easeOut" }}
              className="text-center"
            >
              {/* Icon */}
              <div className="flex justify-center mb-8">{step.icon}</div>

              {/* Content */}
              <div className="space-y-4 mb-10">
                <div>
                  <h2 className="text-3xl font-light text-gray-900 mb-3">
                    {step.title}
                  </h2>
                  <p className="text-sm text-blue-600 font-medium mb-4">
                    {step.subtitle}
                  </p>
                </div>

                <p className="text-gray-600 leading-relaxed text-base">
                  {step.description}
                </p>

                {/* Keyboard shortcut for shortcut step */}
                {step.id === "shortcut" && (
                  <div className="relative">
                    <KeyboardShortcut
                      onShortcutPressed={handleShortcutPressed}
                      shortcutString={keyboardShortcuts?.agent_mode_toggle}
                    />
                    <p className="text-sm text-gray-500 mt-4">
                      {shortcutPressed
                        ? "Perfect! You've got it."
                        : keyboardShortcuts?.agent_mode_toggle
                        ? `Press ${keyboardShortcuts.agent_mode_toggle.replace(
                            /\+/g,
                            " + "
                          )} to activate agent mode`
                        : "Press the keys above together to activate agent mode"}
                    </p>
                  </div>
                )}

                {/* Cancel shortcut for cancel step */}
                {step.id === "cancel" && (
                  <div className="relative">
                    {/* Simple visual representation of Escape key */}
                    <div className="flex justify-center my-6">
                      <div className="flex items-center justify-center w-20 h-20 rounded-xl border-2 bg-white border-gray-300 text-gray-700 shadow-sm">
                        <span className="text-lg font-semibold">Esc</span>
                      </div>
                    </div>

                    <p className="text-sm text-gray-500 mt-4 text-center">
                      The Escape key is your universal "stop" button in Juno
                    </p>
                  </div>
                )}

                {/* Granular permissions interface */}
                {step.id === "permissions" && (
                  <div className="space-y-4 mt-8 text-left">
                    {/* Header */}
                    <div className="text-center mb-6">
                      <h3 className="text-lg font-semibold text-gray-900 mb-2">
                        Grant Permissions
                      </h3>
                      <p className="text-sm text-gray-600">
                        Click "Grant Permission" for each item below to enable
                        full functionality
                      </p>
                      {permissionsError && (
                        <div className="mt-2 p-2 bg-red-50 border border-red-200 rounded-md">
                          <p className="text-sm text-red-700">
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
                      <div className="mt-6 p-4 bg-gray-50 rounded-lg">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            {permissionsState.all_granted ? (
                              <>
                                <CheckCircle className="w-5 h-5 text-green-600" />
                                <span className="font-medium text-green-800">
                                  All required permissions granted!
                                </span>
                              </>
                            ) : (
                              <>
                                <AlertCircle className="w-5 h-5 text-orange-600" />
                                <span className="font-medium text-orange-800">
                                  Some permissions still needed
                                </span>
                              </>
                            )}
                          </div>
                          <button
                            onClick={checkPermissionsStatus}
                            className="text-sm text-blue-600 hover:text-blue-700 flex items-center gap-1"
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
                {/* Show skip all button on final step */}
                {[0, onboardingSteps.length - 1].includes(currentStep) ? (
                  <button
                    onClick={handleSkip}
                    className="flex-1 py-3 text-gray-500 hover:text-gray-700 font-medium transition-colors"
                  >
                    Skip
                  </button>
                ) : (
                  <button
                    onClick={handleSkipStep}
                    className="flex-1 py-3 text-gray-500 hover:text-gray-700 font-medium transition-colors"
                  >
                    Skip
                  </button>
                )}

                {/* Continue button - hide for shortcut step until completed */}
                {(step.id !== "shortcut" || shortcutPressed) && (
                  <button
                    onClick={handleNext}
                    className="flex-1 py-3 px-6 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded-xl transition-all duration-200 flex items-center justify-center gap-2 shadow-lg hover:shadow-xl"
                  >
                    {step.action}
                    <ChevronRight className="w-4 h-4" />
                  </button>
                )}
              </div>
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </>
  );
}
