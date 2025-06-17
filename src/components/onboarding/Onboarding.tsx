"use client";

import { useState, useEffect } from "react";
import {
  ChevronRight,
  Sparkles,
  Shield,
  CheckCircle,
  Monitor,
  Keyboard,
  Eye,
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

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
    id: "input-monitoring",
    title: "Input Monitoring",
    description: "Allow Juno to monitor keyboard and mouse",
    icon: <Keyboard className="w-5 h-5" />,
    required: false,
  },
];

const getOnboardingSteps = (permissionsAlreadyGranted: boolean) => [
  {
    id: "welcome",
    title: "Welcome to Juno",
    subtitle: "Your intelligent Mac companion",
    description:
      "Juno helps you automate tasks, manage your workflow, and get more done with your Mac.",
    icon: <Sparkles className="w-12 h-12 text-blue-500" />,
    action: "Get Started",
  },
  {
    id: "shortcut",
    title: "Learn the Magic Key",
    subtitle: "Press ⌘J to summon Juno anywhere",
    description:
      "Try it now! Press the keyboard shortcut below to activate Juno from anywhere on your Mac.",
    icon: <Keyboard className="w-12 h-12 text-purple-500" />,
    action: "Continue",
  },
  ...(permissionsAlreadyGranted ? [] : [
    {
      id: "permissions",
      title: "Grant Permissions",
      subtitle: "Required for full functionality",
      description:
        "Juno needs these permissions to automate tasks and interact with your Mac securely.",
      icon: <Shield className="w-12 h-12 text-green-500" />,
      action: "Open System Preferences",
    }
  ]),
  {
    id: "complete",
    title: "Ready to Go",
    subtitle: "Juno is now active",
    description:
      permissionsAlreadyGranted 
        ? "Juno is ready to go! Press ⌘J anytime to get started!"
        : "You can always change these permissions later in System Preferences. Press ⌘J anytime to get started!",
    icon: <CheckCircle className="w-12 h-12 text-green-500" />,
    action: "Start Using Juno",
  },
];

// Keyboard shortcut component
function KeyboardShortcut({
  onShortcutPressed,
}: {
  onShortcutPressed: () => void;
}) {
  const [pressedKeys, setPressedKeys] = useState<Set<string>>(new Set());
  const [isComplete, setIsComplete] = useState(false);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const newPressedKeys = new Set(pressedKeys);

      if (e.metaKey) newPressedKeys.add("cmd");
      if (e.key.toLowerCase() === "j") newPressedKeys.add("j");

      setPressedKeys(newPressedKeys);

      // Check if correct combination is pressed
      if (e.metaKey && e.key.toLowerCase() === "j") {
        e.preventDefault();
        setIsComplete(true);
        setTimeout(() => {
          onShortcutPressed();
        }, 800);
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      const newPressedKeys = new Set(pressedKeys);

      if (!e.metaKey) newPressedKeys.delete("cmd");
      if (e.key.toLowerCase() === "j") newPressedKeys.delete("j");

      setPressedKeys(newPressedKeys);
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [pressedKeys, onShortcutPressed]);

  return (
    <div className="flex items-center justify-center gap-4 my-8">
      <motion.div
        className={`relative flex items-center justify-center w-20 h-16 rounded-xl border-2 transition-all duration-200 ${
          pressedKeys.has("cmd")
            ? "bg-blue-500 border-blue-600 text-white shadow-lg scale-95"
            : "bg-white border-gray-300 text-gray-700 shadow-sm"
        }`}
        animate={pressedKeys.has("cmd") ? { scale: 0.95 } : { scale: 1 }}
      >
        <span className="text-lg font-semibold">⌘</span>
      </motion.div>

      <div className="text-2xl font-light text-gray-400">+</div>

      <motion.div
        className={`relative flex items-center justify-center w-20 h-16 rounded-xl border-2 transition-all duration-200 ${
          pressedKeys.has("j")
            ? "bg-blue-500 border-blue-600 text-white shadow-lg scale-95"
            : "bg-white border-gray-300 text-gray-700 shadow-sm"
        }`}
        animate={pressedKeys.has("j") ? { scale: 0.95 } : { scale: 1 }}
      >
        <span className="text-lg font-semibold">J</span>
      </motion.div>

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

// Mock macOS permission dialog
function PermissionDialog({
  permission,
  onAllow,
  onDeny,
}: {
  permission: any;
  onAllow: () => void;
  onDeny: () => void;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.9 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.9 }}
      className="fixed inset-0 bg-black/40 flex items-center justify-center z-[60]"
    >
      <div className="bg-white rounded-xl shadow-2xl w-[420px] overflow-hidden">
        <div className="p-6">
          <div className="flex items-start gap-4">
            <div className="w-16 h-16 rounded-full bg-blue-100 flex items-center justify-center flex-shrink-0">
              <div className="w-8 h-8 rounded bg-blue-500 flex items-center justify-center text-white">
                J
              </div>
            </div>
            <div className="flex-1 pt-2">
              <h3 className="text-lg font-semibold text-gray-900 mb-2">
                "Juno" would like to{" "}
                {permission.title.toLowerCase() === "accessibility"
                  ? "control this computer using accessibility features"
                  : permission.description.toLowerCase()}
              </h3>
              <p className="text-sm text-gray-600 leading-relaxed">
                {permission.title === "Accessibility" &&
                  "This will allow Juno to automate tasks by controlling other applications and system functions."}
                {permission.title === "Screen Recording" &&
                  "This will allow Juno to see what's on your screen to provide contextual assistance."}
                {permission.title === "Input Monitoring" &&
                  "This will allow Juno to monitor keyboard and mouse input for advanced automation features."}
              </p>
            </div>
          </div>
        </div>
        <div className="bg-gray-50 px-6 py-4 flex justify-end gap-3">
          <button
            onClick={onDeny}
            className="px-4 py-2 text-sm font-medium text-gray-700 hover:text-gray-900 transition-colors"
          >
            Don't Allow
          </button>
          <button
            onClick={onAllow}
            className="px-6 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors"
          >
            Allow
          </button>
        </div>
      </div>
    </motion.div>
  );
}

interface OnboardingFlowProps {
  onComplete: () => void;
  onSkip?: () => void;
  permissionsAlreadyGranted?: boolean;
}

export default function OnboardingFlow({
  onComplete,
  onSkip,
  permissionsAlreadyGranted = false,
}: OnboardingFlowProps) {
  const [currentStep, setCurrentStep] = useState(0);
  const [isComplete, setIsComplete] = useState(false);
  const [showPermissionDialog, setShowPermissionDialog] = useState(false);
  const [currentPermission, setCurrentPermission] = useState(0);
  const [grantedPermissions, setGrantedPermissions] = useState<string[]>([]);
  const [shortcutPressed, setShortcutPressed] = useState(false);
  
  const onboardingSteps = getOnboardingSteps(permissionsAlreadyGranted);

  const handleNext = () => {
    if (currentStep === 1 && !shortcutPressed) {
      // Don't proceed if shortcut hasn't been pressed
      return;
    }

    const currentStepData = onboardingSteps[currentStep];
    if (currentStepData?.id === "permissions") {
      // Permissions step
      setShowPermissionDialog(true);
      setCurrentPermission(0);
    } else if (currentStep < onboardingSteps.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      setIsComplete(true);
      onComplete();
    }
  };

  const handleShortcutPressed = () => {
    setShortcutPressed(true);
    setTimeout(() => {
      setCurrentStep(currentStep + 1);
    }, 500);
  };

  const handlePermissionAllow = () => {
    const permission = permissions[currentPermission];
    setGrantedPermissions([...grantedPermissions, permission.id]);

    if (currentPermission < permissions.length - 1) {
      setCurrentPermission(currentPermission + 1);
    } else {
      setShowPermissionDialog(false);
      setCurrentStep(currentStep + 1);
    }
  };

  const handlePermissionDeny = () => {
    if (currentPermission < permissions.length - 1) {
      setCurrentPermission(currentPermission + 1);
    } else {
      setShowPermissionDialog(false);
      setCurrentStep(currentStep + 1);
    }
  };

  const handleSkip = () => {
    setIsComplete(true);
    if (onSkip) {
      onSkip();
    } else {
      onComplete();
    }
  };

  if (isComplete) {
    return null;
  }

  const step = onboardingSteps[currentStep];

  return (
    <>
      <div className="fixed inset-0 bg-black/10 backdrop-blur-md flex items-center justify-center z-50">
        <div className="bg-white/90 backdrop-blur-xl rounded-[20px] shadow-2xl w-[440px] border border-white/20">
          <div className="p-10">
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
                <div className="flex justify-center mb-8">
                  <div className="w-20 h-20 rounded-full bg-gray-50/80 flex items-center justify-center">
                    {step.icon}
                  </div>
                </div>

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
                      />
                      <p className="text-sm text-gray-500 mt-4">
                        {shortcutPressed
                          ? "Perfect! You've got it."
                          : "Press and hold both keys together"}
                      </p>
                    </div>
                  )}

                  {/* Permissions list */}
                  {step.id === "permissions" && (
                    <div className="space-y-3 mt-8">
                      {permissions.map((permission) => (
                        <div
                          key={permission.id}
                          className="flex items-center gap-4 p-4 bg-gray-50/60 rounded-xl"
                        >
                          <div className="w-8 h-8 rounded-full bg-blue-100 flex items-center justify-center text-blue-600">
                            {permission.icon}
                          </div>
                          <div className="flex-1 text-left">
                            <div className="font-medium text-gray-900">
                              {permission.title}
                            </div>
                            <div className="text-sm text-gray-600">
                              {permission.description}
                            </div>
                          </div>
                          {permission.required && (
                            <div className="text-xs text-orange-600 font-medium bg-orange-50 px-2 py-1 rounded-full">
                              Required
                            </div>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </div>

                {/* Actions */}
                <div className="flex gap-4">
                  {currentStep < onboardingSteps.length - 1 &&
                    step.id !== "shortcut" &&
                    step.id !== "permissions" && (
                      <button
                        onClick={handleSkip}
                        className="flex-1 py-3 text-gray-500 hover:text-gray-700 font-medium transition-colors"
                      >
                        Skip
                      </button>
                    )}
                  {/* Hide continue button for shortcut step until shortcut is pressed */}
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
      </div>

      {/* Permission Dialog */}
      <AnimatePresence>
        {showPermissionDialog && (
          <PermissionDialog
            permission={permissions[currentPermission]}
            onAllow={handlePermissionAllow}
            onDeny={handlePermissionDeny}
          />
        )}
      </AnimatePresence>
    </>
  );
}
