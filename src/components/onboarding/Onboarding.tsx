import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import { EVENTS, COMMANDS } from "@/lib/constants.generated";
import { ChevronRight } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useEventListener } from "@/hooks/useEventListener";
import AudioVisualizer from "../bar/audio-visualizer";

// ─── Types ────────────────────────────────────────────────────────────────────

interface PermissionStatus {
  permission_type: string;
  granted: boolean;
  required: boolean;
  description: string;
  instructions: string;
}

interface PermissionsState {
  accessibility: PermissionStatus;
  screen_recording: PermissionStatus;
  microphone: PermissionStatus;
  input_monitoring: PermissionStatus;
  all_granted: boolean;
  app_name: string;
}

interface OnboardingFlowProps {
  onComplete: () => void;
  onSkip?: () => void;
  permissionsAlreadyGranted?: boolean;
  isDevelopmentMode?: boolean;
}

// Ordered permission sequence — simpler grants first (per spec)
const PERMISSION_SEQUENCE = [
  {
    key: "screen_recording" as const,
    label: "Screen Recording",
    purpose: "See what's on your screen",
    command: COMMANDS.PERMISSIONS_REQUEST_SCREEN_RECORDING_PERMISSION,
    required: true,
  },
  {
    key: "accessibility" as const,
    label: "Accessibility",
    purpose: "Click, type, and navigate for you",
    command: COMMANDS.PERMISSIONS_REQUEST_ACCESSIBILITY_PERMISSION,
    required: true,
  },
  {
    key: "input_monitoring" as const,
    label: "Input Monitoring",
    purpose: "Detect keyboard shortcuts globally",
    command: COMMANDS.PERMISSIONS_REQUEST_INPUT_MONITORING_PERMISSION,
    required: false,
  },
  {
    key: "microphone" as const,
    label: "Microphone",
    purpose: "Listen when you want to talk",
    command: COMMANDS.PERMISSIONS_REQUEST_MICROPHONE_PERMISSION,
    required: false,
  },
];

// ─── Reduced motion hook ───────────────────────────────────────────────────────

function usePrefersReducedMotion() {
  const [reduced, setReduced] = useState(
    () => window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const handler = (e: MediaQueryListEvent) => setReduced(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);
  return reduced;
}

// ─── KeyboardShortcutDisplay ──────────────────────────────────────────────────

function KeyboardShortcutDisplay({
  shortcutString,
  defaultShortcut = "Option+D",
  isActivated,
}: {
  shortcutString?: string;
  defaultShortcut?: string;
  isActivated: boolean;
}) {
  const reduced = usePrefersReducedMotion();

  const parseShortcut = (s: string) => {
    const parts = s.split("+").map((p) => p.trim().toLowerCase());
    return { modifiers: parts.slice(0, -1), key: parts[parts.length - 1] };
  };

  const { modifiers, key } = parseShortcut(shortcutString || defaultShortcut);

  const renderModifier = (mod: string) => {
    switch (mod) {
      case "option":
      case "alt":
        return "⌥";
      case "cmd":
      case "command":
      case "commandorcontrol":
        return "⌘";
      case "ctrl":
      case "control":
        return "⌃";
      case "shift":
        return "⇧";
      default:
        return mod.toUpperCase();
    }
  };

  const renderKey = (k: string) => {
    switch (k) {
      case "escape":
        return "Esc";
      case "space":
        return "Space";
      case "enter":
      case "return":
        return "Return";
      case "backspace":
        return "Delete";
      case "tab":
        return "Tab";
      default:
        return k.toUpperCase();
    }
  };

  const allKeys = [...modifiers.map(renderModifier), renderKey(key)];

  const allLabels = [
    ...modifiers.map((mod) => {
      const map: Record<string, string> = {
        option: "Option",
        alt: "Option",
        cmd: "Command",
        command: "Command",
        commandorcontrol: "Command",
        ctrl: "Control",
        control: "Control",
        shift: "Shift",
      };
      return map[mod] || mod;
    }),
    renderKey(key),
  ];

  return (
    <div
      className="flex items-center justify-center gap-2 my-8"
      aria-label={`Keyboard shortcut: ${allLabels.join(" plus ")}`}
    >
      {allKeys.map((symbol, i) => (
        <motion.div
          key={i}
          className={`flex items-center justify-center w-14 h-14 rounded-xl border font-mono text-sm font-medium tracking-wide transition-colors duration-150 ${
            isActivated
              ? "bg-primary/20 border-primary text-primary"
              : "bg-card border-border text-card-foreground"
          }`}
          animate={reduced ? {} : isActivated ? { scale: 0.95 } : { scale: 1 }}
          transition={{ duration: 0.15 }}
        >
          {symbol}
        </motion.div>
      ))}
    </div>
  );
}

// ─── PermissionItem ────────────────────────────────────────────────────────────

function PermissionItem({
  label,
  purpose,
  required,
  granted,
  waiting,
  onGrant,
  revealed,
  reduced,
}: {
  label: string;
  purpose: string;
  required: boolean;
  granted: boolean;
  waiting: boolean;
  onGrant: () => void;
  revealed: boolean;
  reduced: boolean;
}) {
  return (
    <motion.div
      initial={reduced ? { opacity: 0 } : { opacity: 0, y: -8 }}
      animate={revealed ? { opacity: 1, y: 0 } : { opacity: 0, y: -8 }}
      transition={{ duration: 0.3, ease: "easeOut" }}
      aria-live="polite"
      role="status"
    >
      <div
        className={`flex items-center justify-between py-4 border-b border-border/40 last:border-0 transition-opacity duration-300 ${
          granted ? "opacity-50" : "opacity-100"
        }`}
      >
        <div>
          <div className="flex items-center gap-2">
            <span className="font-medium text-foreground text-sm">{label}</span>
            <span className="text-xs text-muted-foreground">
              {required ? "Required" : "Optional"}
            </span>
          </div>
          <p className="text-sm text-muted-foreground mt-0.5">{purpose}</p>
        </div>

        <div className="flex-shrink-0 ml-4">
          {granted ? (
            <span className="text-sm text-primary font-medium">✓</span>
          ) : (
            <button
              onClick={onGrant}
              disabled={waiting}
              className="px-4 py-1.5 text-sm font-medium rounded-lg transition-colors duration-150 bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {waiting ? "Waiting..." : "Grant"}
            </button>
          )}
        </div>
      </div>
    </motion.div>
  );
}

// ─── Main Component ────────────────────────────────────────────────────────────

export default function OnboardingFlow({
  onComplete,
  onSkip,
  permissionsAlreadyGranted = false,
  isDevelopmentMode: _isDevelopmentMode = false,
}: OnboardingFlowProps) {
  const reduced = usePrefersReducedMotion();

  // ── Step state ──
  const [currentStep, setCurrentStep] = useState(0);

  // ── Welcome step ──
  const [showSkip, setShowSkip] = useState(false);

  // ── Keyboard step ──
  const [keyPhase, setKeyPhase] = useState<"agent" | "escape">("agent");
  const [agentShortcutPressed, setAgentShortcutPressed] = useState(false);
  const [escapePressed, setEscapePressed] = useState(false);
  const [keyboardShortcuts, setKeyboardShortcuts] = useState<any>(null);
  const bothKeysConfirmed = agentShortcutPressed && escapePressed;

  // ── Connect AI step ──
  const [claudeCliAvailable, setClaudeCliAvailable] = useState<boolean | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [showApiKey, setShowApiKey] = useState(false);
  const [detectedProvider, setDetectedProvider] = useState<{ id: string; name: string } | null>(null);
  const [apiKeySaving, setApiKeySaving] = useState(false);
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [apiKeyError, setApiKeyError] = useState<string | null>(null);
  const [apiKeysAvailable, setApiKeysAvailable] = useState(false);

  // ── Permissions step ──
  const [permissionsState, setPermissionsState] = useState<PermissionsState | null>(null);
  const [currentPermIdx, setCurrentPermIdx] = useState(0);
  const [waitingPermission, setWaitingPermission] = useState<string | null>(null);
  const [permPollInterval, setPermPollInterval] = useState<ReturnType<typeof setInterval> | null>(null);
  const [allRequiredGranted, setAllRequiredGranted] = useState(permissionsAlreadyGranted);

  // ── Misc ──
  const mountedRef = useRef(true);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      for (const t of timers.current) clearTimeout(t);
    };
  }, []);

  // Show skip after 1.5s on welcome
  useEffect(() => {
    if (currentStep === 0) {
      const t = setTimeout(() => {
        if (mountedRef.current) setShowSkip(true);
      }, 1500);
      timers.current.push(t);
    }
  }, [currentStep]);

  // Load initial data
  useEffect(() => {
    const load = async () => {
      try {
        // Keyboard shortcuts
        try {
          const info = await invoke<any>("get_onboarding_info");
          if (mountedRef.current && info?.shortcuts) {
            setKeyboardShortcuts(info.shortcuts);
          }
        } catch {}

        // Claude CLI availability
        try {
          const available = await invoke<boolean>("check_claude_cli_available");
          if (mountedRef.current) setClaudeCliAvailable(available);
        } catch {
          if (mountedRef.current) setClaudeCliAvailable(false);
        }

        // API keys already present?
        try {
          const keysAvail = await invoke<boolean>("check_api_keys_available");
          if (mountedRef.current) setApiKeysAvailable(keysAvail);
        } catch {}

        // Permissions
        try {
          const perms = await invoke<PermissionsState>(
            COMMANDS.PERMISSIONS_CHECK_PERMISSIONS_STATUS
          );
          if (mountedRef.current) {
            setPermissionsState(perms);
            setAllRequiredGranted(
              perms.accessibility.granted && perms.screen_recording.granted
            );
          }
        } catch {}
      } catch {}
    };
    load();
  }, []);

  // ── Backend-driven shortcut detection ──
  useEventListener<{ state: string }>(EVENTS.SHORTCUTS_AGENT_MODE, (payload) => {
    if (payload.state === "pressed" && !agentShortcutPressed) {
      setAgentShortcutPressed(true);
      // Auto-advance to escape phase after brief delay
      const t = setTimeout(() => {
        if (mountedRef.current) setKeyPhase("escape");
      }, 600);
      timers.current.push(t);
    }
  });

  useEventListener<{ state: string }>(EVENTS.SHORTCUTS_ESCAPE_KEY, (payload) => {
    if (payload.state === "pressed" && keyPhase === "escape" && !escapePressed) {
      setEscapePressed(true);
      // Auto-advance to next step 800ms after escape confirmed
      const t = setTimeout(() => {
        if (mountedRef.current) advanceStep();
      }, 800);
      timers.current.push(t);
    }
  });

  // ── Permissions polling ──
  const checkPermissions = useCallback(async () => {
    try {
      const result = await invoke<PermissionsState>(
        COMMANDS.PERMISSIONS_CHECK_PERMISSIONS_STATUS
      );
      if (!mountedRef.current) return result;
      setPermissionsState(result);
      const reqGranted =
        result.accessibility.granted && result.screen_recording.granted;
      setAllRequiredGranted(reqGranted);
      return result;
    } catch {
      return null;
    }
  }, []);

  const startPolling = useCallback(() => {
    const interval = setInterval(async () => {
      if (!mountedRef.current) return;
      const result = await checkPermissions();
      if (!result) return;

      // Advance permission index if current one is now granted
      const currentPerm = PERMISSION_SEQUENCE[currentPermIdx];
      if (currentPerm) {
        const permState = result[currentPerm.key] as PermissionStatus;
        if (permState?.granted) {
          setWaitingPermission(null);
          setCurrentPermIdx((i) => Math.min(i + 1, PERMISSION_SEQUENCE.length));
        }
      }
    }, 1000);
    setPermPollInterval(interval);
  }, [checkPermissions, currentPermIdx]);

  const stopPolling = useCallback(() => {
    if (permPollInterval) {
      clearInterval(permPollInterval);
      setPermPollInterval(null);
    }
  }, [permPollInterval]);

  // Start polling when entering permissions step
  useEffect(() => {
    if (currentStep === 3 && !allRequiredGranted) {
      checkPermissions();
      startPolling();
      return () => stopPolling();
    }
  }, [currentStep]); // eslint-disable-line react-hooks/exhaustive-deps

  // ── Provider detection ──
  const detectProvider = useCallback((key: string) => {
    const k = key.trim();
    if (k.startsWith("sk-ant-")) return { id: "anthropic", name: "Anthropic" };
    if (k.startsWith("sk-proj-") || (k.startsWith("sk-") && !k.startsWith("sk-ant-")))
      return { id: "openai", name: "OpenAI" };
    if (k.startsWith("AIza")) return { id: "gemini", name: "Google Gemini" };
    return null;
  }, []);

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
    } catch (err) {
      setApiKeyError(err as string);
    } finally {
      setApiKeySaving(false);
    }
  }, [detectedProvider, apiKey]);

  // ── Request a permission ──
  const requestPermission = useCallback(
    async (permKey: string, command: string) => {
      setWaitingPermission(permKey);
      try {
        await invoke<boolean>(command);
        // Polling handles the state update
      } catch {
        setWaitingPermission(null);
      }
    },
    []
  );

  // ── Step management ──
  const steps = [
    "welcome",
    "keys",
    ...(apiKeysAvailable ? [] : ["connect"]),
    ...(allRequiredGranted ? [] : ["permissions"]),
    "ready",
  ];

  const advanceStep = useCallback(() => {
    setCurrentStep((s) => Math.min(s + 1, steps.length - 1));
  }, [steps.length]);

  const handleNext = useCallback(async () => {
    const step = steps[currentStep];

    if (step === "keys" && !bothKeysConfirmed) return;
    if (step === "permissions" && !allRequiredGranted) return;

    if (step === "connect" && detectedProvider && !apiKeySaved) {
      await saveApiKey();
    }

    if (currentStep >= steps.length - 1) {
      onComplete();
    } else {
      advanceStep();
    }
  }, [currentStep, steps, bothKeysConfirmed, allRequiredGranted, detectedProvider, apiKeySaved, saveApiKey, onComplete, advanceStep]);

  const handleSkip = () => {
    onSkip ? onSkip() : onComplete();
  };

  // Enter key → advance
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleNext();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleNext]);

  // ── Step renders ──
  const step = steps[currentStep];

  const isContinueDisabled =
    (step === "keys" && !bothKeysConfirmed) ||
    (step === "permissions" && !allRequiredGranted) ||
    (step === "connect" && !apiKeysAvailable && !claudeCliAvailable && !detectedProvider) ||
    (step === "connect" && apiKeySaving);

  const transition = {
    duration: reduced ? 0 : 0.4,
    ease: [0.16, 1, 0.3, 1] as [number, number, number, number],
  };

  const stepVariants = {
    initial: { opacity: 0, y: reduced ? 0 : 15 },
    animate: { opacity: 1, y: 0 },
    exit: { opacity: 0, y: reduced ? 0 : -15 },
  };

  return (
    <div className="fixed inset-0 flex items-center justify-center bg-background">
      {/* Skip link — welcome step only, appears after 1.5s */}
      <AnimatePresence>
        {currentStep === 0 && showSkip && (
          <motion.button
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={handleSkip}
            className="absolute top-6 right-6 text-sm text-muted-foreground hover:text-foreground transition-colors"
          >
            skip
          </motion.button>
        )}
      </AnimatePresence>

      <div className="w-full max-w-[440px] px-10 pt-12 pb-8">
        {/* Progress bar */}
        <div
          className="flex gap-1.5 mb-12"
          role="progressbar"
          aria-valuenow={currentStep + 1}
          aria-valuemax={steps.length}
          aria-label={`Step ${currentStep + 1} of ${steps.length}`}
        >
          {steps.map((_, i) => (
            <div
              key={i}
              className={`h-0.5 flex-1 rounded-full transition-all duration-500 ${
                i <= currentStep ? "bg-primary" : "bg-border"
              }`}
            />
          ))}
        </div>

        {/* Floating bar — visible on keys step */}
        <AnimatePresence>
          {step === "keys" && (
            <motion.div
              key="bar-preview"
              initial={{ opacity: 0, y: -8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={transition}
              className="flex justify-center mb-8"
            >
              <AudioVisualizer
                appState={
                  escapePressed
                    ? "error"
                    : agentShortcutPressed
                    ? "listening"
                    : "idle"
                }
                width={320}
                height={52}
                enableMicrophone={false}
                intensity={escapePressed ? 1.8 : agentShortcutPressed ? 1.4 : 1.0}
                animationStyle="organic"
              />
            </motion.div>
          )}
        </AnimatePresence>

        {/* Step content */}
        <AnimatePresence mode="wait">
          <motion.div
            key={step}
            variants={stepVariants}
            initial="initial"
            animate="animate"
            exit="exit"
            transition={transition}
          >
            {step === "welcome" && <WelcomeStep reduced={reduced} />}
            {step === "keys" && (
              <KeysStep
                reduced={reduced}
                keyPhase={keyPhase}
                agentShortcutPressed={agentShortcutPressed}
                escapePressed={escapePressed}
                keyboardShortcuts={keyboardShortcuts}
              />
            )}
            {step === "connect" && (
              <ConnectStep
                claudeCliAvailable={claudeCliAvailable}
                apiKey={apiKey}
                showApiKey={showApiKey}
                onToggleShowApiKey={() => setShowApiKey((v) => !v)}
                onApiKeyChange={handleApiKeyChange}
                detectedProvider={detectedProvider}
                apiKeySaved={apiKeySaved}
                apiKeyError={apiKeyError}
              />
            )}
            {step === "permissions" && (
              <PermissionsStep
                permissionsState={permissionsState}
                currentPermIdx={currentPermIdx}
                waitingPermission={waitingPermission}
                allRequiredGranted={allRequiredGranted}
                onGrant={requestPermission}
                reduced={reduced}
              />
            )}
            {step === "ready" && <ReadyStep keyboardShortcuts={keyboardShortcuts} />}
          </motion.div>
        </AnimatePresence>

        {/* Actions */}
        <div className="flex gap-4 mt-10">
          {currentStep > 0 &&
            currentStep < steps.length - 1 &&
            step !== "permissions" && (
              <button
                onClick={handleSkip}
                className="flex-1 py-3 text-sm text-muted-foreground hover:text-foreground font-medium transition-colors"
              >
                Skip
              </button>
            )}

          <motion.button
            onClick={handleNext}
            disabled={isContinueDisabled}
            animate={
              step === "permissions" && allRequiredGranted && !reduced
                ? { scale: [1, 1.02, 1] }
                : {}
            }
            transition={{ duration: 0.2 }}
            className={`flex-1 py-3 px-6 text-sm font-medium rounded-xl transition-all duration-200 flex items-center justify-center gap-1.5 ${
              isContinueDisabled
                ? "bg-muted text-muted-foreground cursor-not-allowed"
                : "bg-primary text-primary-foreground hover:bg-primary/90"
            }`}
          >
            {step === "welcome"
              ? "Let's go"
              : step === "ready"
              ? "Start using Juno"
              : "Continue"}
            <ChevronRight className="w-4 h-4" />
          </motion.button>
        </div>
      </div>
    </div>
  );
}

// ─── Step sub-components ───────────────────────────────────────────────────────

function WelcomeStep({ reduced }: { reduced: boolean }) {
  return (
    <div className="text-center space-y-6">
      <motion.div
        className="flex justify-center"
        initial={reduced ? {} : { scale: 0.6, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{
          duration: reduced ? 0 : 0.8,
          ease: [0.16, 1, 0.3, 1],
        }}
      >
        <img src="/juno.png" alt="Juno" className="w-20 h-20 object-contain" />
      </motion.div>

      <div>
        <h1 className="text-3xl font-light tracking-tight text-foreground">
          Juno
        </h1>
        <motion.p
          className="mt-3 text-base text-muted-foreground"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: reduced ? 0 : 0.4, duration: 0.4 }}
        >
          Desktop automation that works the way you think.
        </motion.p>
      </div>
    </div>
  );
}

function KeysStep({
  reduced,
  keyPhase,
  agentShortcutPressed,
  escapePressed,
  keyboardShortcuts,
}: {
  reduced: boolean;
  keyPhase: "agent" | "escape";
  agentShortcutPressed: boolean;
  escapePressed: boolean;
  keyboardShortcuts: any;
}) {
  return (
    <div className="text-center space-y-4">
      <div>
        <h2 className="text-3xl font-light tracking-tight text-foreground">
          Two keys to know
        </h2>
        <p className="mt-3 text-sm text-primary font-medium">
          These work from anywhere on your Mac.
        </p>
      </div>

      <div className="relative min-h-[140px]">
        <AnimatePresence mode="wait">
          {keyPhase === "agent" ? (
            <motion.div
              key="agent-phase"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: reduced ? 0 : 0.3 }}
            >
              <KeyboardShortcutDisplay
                shortcutString={keyboardShortcuts?.agent_mode}
                defaultShortcut="Option+D"
                isActivated={agentShortcutPressed}
              />
              <p className="text-sm text-muted-foreground">
                {agentShortcutPressed
                  ? "Got it."
                  : keyboardShortcuts?.agent_mode
                  ? `Hold these together to activate Juno.`
                  : "Hold these together to activate Juno."}
              </p>
            </motion.div>
          ) : (
            <motion.div
              key="escape-phase"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: reduced ? 0 : 0.3 }}
            >
              <KeyboardShortcutDisplay
                shortcutString={keyboardShortcuts?.stop_current_task}
                defaultShortcut="Escape"
                isActivated={escapePressed}
              />
              <p className="text-sm text-muted-foreground">
                {escapePressed
                  ? "You're in control."
                  : "Press Escape to stop anything Juno is doing."}
              </p>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

function ConnectStep({
  claudeCliAvailable,
  apiKey,
  showApiKey,
  onToggleShowApiKey,
  onApiKeyChange,
  detectedProvider,
  apiKeySaved,
  apiKeyError,
}: {
  claudeCliAvailable: boolean | null;
  apiKey: string;
  showApiKey: boolean;
  onToggleShowApiKey: () => void;
  onApiKeyChange: (v: string) => void;
  detectedProvider: { id: string; name: string } | null;
  apiKeySaved: boolean;
  apiKeyError: string | null;
}) {
  return (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-3xl font-light tracking-tight text-foreground">
          Connect to AI
        </h2>
      </div>

      {/* Claude CLI card */}
      <div className="p-4 rounded-xl border border-border space-y-2">
        <p className="text-sm font-medium text-foreground">
          Use your Claude subscription
        </p>
        <p className="text-sm text-muted-foreground">
          If you have Claude Code installed, Juno can use it directly. No API key needed.
        </p>
        <div className="mt-2">
          {claudeCliAvailable === null ? (
            <span className="text-xs text-muted-foreground">Checking...</span>
          ) : claudeCliAvailable ? (
            <span className="text-xs text-primary font-medium">✓ Claude CLI detected</span>
          ) : (
            <a
              href="https://claude.ai/code"
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs text-muted-foreground hover:text-foreground transition-colors"
            >
              Not installed — Get Claude Code →
            </a>
          )}
        </div>
      </div>

      {/* API key card */}
      <div className="space-y-3">
        <p className="text-sm font-medium text-foreground">Or paste an API key</p>

        <div className="relative">
          <input
            type={showApiKey ? "text" : "password"}
            value={apiKey}
            onChange={(e) => onApiKeyChange(e.target.value)}
            placeholder="sk-ant-... or sk-proj-..."
            className="w-full px-4 py-3 pr-16 rounded-xl border border-border bg-card text-foreground placeholder:text-muted-foreground focus:outline-none focus:border-primary transition-colors font-mono text-sm"
            spellCheck={false}
            autoComplete="off"
          />
          <button
            type="button"
            onClick={onToggleShowApiKey}
            className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            {showApiKey ? "hide" : "show"}
          </button>
        </div>

        <AnimatePresence mode="wait">
          {detectedProvider && (
            <motion.p
              key="detected"
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className="text-xs text-primary"
            >
              ✓ {detectedProvider.name} detected{apiKeySaved ? " — saved" : ""}
            </motion.p>
          )}
          {apiKeyError && (
            <motion.p
              key="error"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="text-xs text-destructive"
            >
              {apiKeyError}
            </motion.p>
          )}
        </AnimatePresence>

        <p className="text-xs text-muted-foreground">
          Works with Anthropic, OpenAI, and Google Gemini. You can add this later in Settings.
        </p>
      </div>
    </div>
  );
}

function PermissionsStep({
  permissionsState,
  currentPermIdx,
  waitingPermission,
  allRequiredGranted,
  onGrant,
  reduced,
}: {
  permissionsState: PermissionsState | null;
  currentPermIdx: number;
  waitingPermission: string | null;
  allRequiredGranted: boolean;
  onGrant: (key: string, command: string) => void;
  reduced: boolean;
}) {
  return (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-3xl font-light tracking-tight text-foreground">
          {allRequiredGranted ? "All set" : "A few things Juno needs"}
        </h2>
        <p className="mt-3 text-sm text-muted-foreground">
          These let Juno see and interact with your screen.
        </p>
      </div>

      <div>
        {PERMISSION_SEQUENCE.map((perm, i) => {
          const permState = permissionsState?.[perm.key] as PermissionStatus | undefined;
          const granted = permState?.granted ?? false;
          const revealed = i <= currentPermIdx;

          return (
            <PermissionItem
              key={perm.key}
              label={perm.label}
              purpose={perm.purpose}
              required={perm.required}
              granted={granted}
              waiting={waitingPermission === perm.key}
              onGrant={() => onGrant(perm.key, perm.command)}
              revealed={revealed}
              reduced={reduced}
            />
          );
        })}
      </div>
    </div>
  );
}

function ReadyStep({ keyboardShortcuts }: { keyboardShortcuts: any }) {
  const agentShortcut = keyboardShortcuts?.agent_mode
    ? keyboardShortcuts.agent_mode.replace("Option", "⌥").replace("+", "")
    : "⌥D";

  return (
    <div className="space-y-8">
      <div className="text-center">
        <h2 className="text-3xl font-light tracking-tight text-foreground">
          You're ready
        </h2>
        <p className="mt-3 text-sm text-muted-foreground">
          Here's what you need to remember.
        </p>
      </div>

      <div className="rounded-xl border border-border overflow-hidden">
        {[
          { keys: agentShortcut, desc: "Activate Juno" },
          { keys: "Esc", desc: "Stop current task" },
          { keys: "⌘B", desc: "Toggle floating bar" },
        ].map(({ keys, desc }) => (
          <div
            key={keys}
            className="flex items-center justify-between px-4 py-3 border-b border-border/40 last:border-0"
          >
            <code className="text-sm font-mono font-medium text-primary bg-primary/8 px-2 py-0.5 rounded">
              {keys}
            </code>
            <span className="text-sm text-muted-foreground">{desc}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
