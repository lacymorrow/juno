import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { EVENTS, COMMANDS } from "@/lib/constants.generated";
import { Eye, EyeOff, AlertCircle, Loader2, ExternalLink } from "lucide-react";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactElement,
} from "react";
import { useEventListener } from "@/hooks/useEventListener";

// ── Visual language ──────────────────────────────────────────────────────────
// This flow is styled like a macOS setup assistant: the system SF stack, quiet
// neutrals from the theme tokens, hairline-bordered inset groups (matching the
// Settings window), and exactly two non-neutral colors — macOS system blue for
// the one primary action and system green for a granted state. No gradients,
// no glows, no decorative motion.
const SF_FONT =
  '-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "Helvetica Neue", Helvetica, Arial, sans-serif';

const FOCUS_RING =
  "focus:outline-none focus-visible:ring-2 focus-visible:ring-[#007AFF]/50 dark:focus-visible:ring-[#0A84FF]/50";

// The single primary button. Capsule shape — current macOS (Tahoe) push
// buttons and every control in System Settings are capsules, not rounded rects.
const BTN_PRIMARY = `inline-flex h-9 min-w-[200px] items-center justify-center rounded-full bg-[#007AFF] px-5 text-[13px] font-medium text-white transition-colors hover:bg-[#0071E8] active:bg-[#0068D6] disabled:bg-muted disabled:text-muted-foreground dark:bg-[#0A84FF] dark:hover:bg-[#2B90FF] dark:active:bg-[#0074E8] ${FOCUS_RING}`;

// Small in-row action (permission checklist rows).
const BTN_ROW = `inline-flex h-7 items-center justify-center rounded-full bg-[#007AFF] px-3.5 text-[12px] font-medium text-white transition-colors hover:bg-[#0071E8] disabled:opacity-50 dark:bg-[#0A84FF] dark:hover:bg-[#2B90FF] ${FOCUS_RING}`;
const BTN_ROW_QUIET = `inline-flex h-7 items-center justify-center rounded-full border border-border bg-transparent px-3.5 text-[12px] font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50 ${FOCUS_RING}`;

// Plain text link (skip affordances).
const LINK_QUIET = `rounded px-2 py-1 text-[12px] text-muted-foreground transition-colors hover:text-foreground ${FOCUS_RING}`;

// Apple systemGreen — used only as the fill of check glyphs, never as text
// (System Settings marks success with a green check beside gray text).
const GREEN = "text-[#34C759] dark:text-[#30D158]";

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

// ── The guided permission sequence ───────────────────────────────────────────
// Rendered as one quiet checklist; only the active row carries a button, and
// rows check off in order as grants land. Accessibility leads because it is
// the pivot: once Juno can control the Mac, the backend auto-grant run
// (auto_grant_permissions) can flip the remaining toggles itself. `stateKey` maps
// to the backend PermissionsState; the request command's first call shows the
// native prompt when one exists, later calls open the exact Settings pane.
// ── Hand-drawn glyphs ────────────────────────────────────────────────────────
// Drawn on a 24pt grid to echo the REAL System Settings privacy icons (filled
// forms, SF-weight strokes) rather than a generic icon pack — the tiles should
// read as the same rows the user is about to see in Settings. Verified against
// the current macOS Privacy & Security pane. All inherit currentColor.
type GlyphProps = { className?: string; style?: CSSProperties };
type GlyphComponent = (props: GlyphProps) => ReactElement;

/** Person-in-circle — the universal Accessibility mark. */
const AccessibilityGlyph: GlyphComponent = (props) => (
  <svg viewBox="0 0 24 24" aria-hidden {...props}>
    <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" strokeWidth="1.5" />
    <circle cx="12" cy="7" r="1.85" fill="currentColor" />
    <path
      d="M6.9 10.2c1.7.5 3.4.75 5.1.75s3.4-.25 5.1-.75M12 10.95v3M12 13.95l-1.95 4.4M12 13.95l1.95 4.4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

/** Filled display with stand — Screen & System Audio Recording. */
const ScreenGlyph: GlyphComponent = (props) => (
  <svg viewBox="0 0 24 24" aria-hidden {...props}>
    <rect x="3.2" y="4.6" width="17.6" height="11.5" rx="1.9" fill="currentColor" />
    <path d="M10.7 16.1h2.6v2.1h-2.6z" fill="currentColor" />
    <rect x="7.4" y="18.2" width="9.2" height="1.7" rx="0.85" fill="currentColor" />
  </svg>
);

/** Filled mic capsule with bracket and base. */
const MicGlyph: GlyphComponent = (props) => (
  <svg viewBox="0 0 24 24" aria-hidden {...props}>
    <rect x="9.5" y="2.6" width="5" height="10.2" rx="2.5" fill="currentColor" />
    <path
      d="M6.7 10.4v1.4a5.3 5.3 0 0 0 10.6 0v-1.4M12 17.2v2.6M8.9 20.9h6.2"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
    />
  </svg>
);

/** Keyboard with key rows and space bar — Input Monitoring. */
const KeyboardGlyph: GlyphComponent = (props) => (
  <svg viewBox="0 0 24 24" aria-hidden {...props}>
    <rect x="2.9" y="6.4" width="18.2" height="11" rx="1.9" fill="none" stroke="currentColor" strokeWidth="1.6" />
    {[6.2, 9.1, 12, 14.9, 17.8].map((x) => (
      <rect key={`a${x}`} x={x - 0.75} y="8.9" width="1.5" height="1.5" rx="0.35" fill="currentColor" />
    ))}
    {[7.65, 10.55, 13.45, 16.35].map((x) => (
      <rect key={`b${x}`} x={x - 0.75} y="11.6" width="1.5" height="1.5" rx="0.35" fill="currentColor" />
    ))}
    <rect x="7.6" y="14.3" width="8.8" height="1.5" rx="0.75" fill="currentColor" />
  </svg>
);

/** Vertical key with round bow and two teeth (SF key style). */
const KeyGlyph: GlyphComponent = (props) => (
  <svg viewBox="0 0 24 24" aria-hidden {...props}>
    <circle cx="12" cy="7" r="3.5" fill="none" stroke="currentColor" strokeWidth="2.1" />
    <path
      d="M12 10.5v8.4M12 15.1h2.7M12 18.7h2.2"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.1"
      strokeLinecap="round"
    />
  </svg>
);

/** Prompt chevron and command line — Terminal. */
const TerminalGlyph: GlyphComponent = (props) => (
  <svg viewBox="0 0 24 24" aria-hidden {...props}>
    <path
      d="m6 7.6 4.6 4.1L6 15.8M12.9 16.4h5.2"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

/** Filled green circle with a white check — how macOS lists mark "done". */
const CheckGlyph: GlyphComponent = (props) => (
  <svg viewBox="0 0 24 24" aria-hidden {...props}>
    <circle cx="12" cy="12" r="10" fill="currentColor" />
    <path
      d="m7.7 12.4 2.9 2.9 5.8-6"
      fill="none"
      stroke="#fff"
      strokeWidth="2.1"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

interface PermissionDef {
  stateKey: "accessibility" | "screen_recording" | "microphone" | "input_monitoring";
  title: string;
  why: string;
  icon: GlyphComponent;
  /** System Settings-style icon tile color (Apple system palette, light). */
  tint: string;
  /** Dark-mode tile color — Apple shifts every tile tint in dark mode. */
  tintDark: string;
  required: boolean;
  hasPrompt: boolean;
}

// Tile tints follow the real Privacy & Security list: Accessibility is system
// blue, Microphone is system orange, and the system-access cluster (Screen
// Recording, Input Monitoring, Full Disk Access…) shares graphite squircles —
// repeated graphite IS the native look; a rainbow of invented tints is not.
const PERMISSION_FLOW: PermissionDef[] = [
  {
    stateKey: "accessibility",
    title: "Accessibility",
    why: "Lets Juno click, type, and move the cursor for you.",
    icon: AccessibilityGlyph,
    tint: "#007AFF",
    tintDark: "#0A84FF",
    required: true,
    hasPrompt: false,
  },
  {
    stateKey: "screen_recording",
    title: "Screen Recording",
    why: "Lets Juno see what's on screen so it knows what to do.",
    icon: ScreenGlyph,
    tint: "#48484E",
    tintDark: "#63636B",
    required: true,
    hasPrompt: true,
  },
  {
    stateKey: "microphone",
    title: "Microphone",
    why: "Lets you talk to Juno instead of typing.",
    icon: MicGlyph,
    tint: "#FF9500",
    tintDark: "#FF9F0A",
    required: false,
    hasPrompt: true,
  },
  {
    stateKey: "input_monitoring",
    title: "Input Monitoring",
    why: "Lets your keyboard shortcut reach Juno from any app.",
    icon: KeyboardGlyph,
    tint: "#48484E",
    tintDark: "#63636B",
    required: false,
    hasPrompt: false,
  },
];

// Permissions the backend can flip itself once Accessibility is granted.
// Microphone is deliberately absent: macOS ignores synthetic input on TCC
// consent dialogs, so its native one-click prompt is the honest path.
const AUTOMATABLE: Array<"screen_recording" | "input_monitoring"> = [
  "screen_recording",
  "input_monitoring",
];

// Row copy for each backend auto-grant stage (permissions-auto-grant-progress).
const AUTO_STAGE_COPY: Record<string, string> = {
  opening_settings: "Opening System Settings…",
  toggling: "Switching Juno on…",
  confirming: "Confirming…",
};

/** macOS System Settings-style icon tile: a small rounded square in an Apple
 *  system color with a white glyph. Matches the Settings window's own list.
 *  The tint shifts between themes via CSS variables, the way Apple's do. */
function IconTile({
  icon: Icon,
  tint,
  tintDark,
  size = 28,
}: {
  icon: GlyphComponent;
  tint: string;
  tintDark?: string;
  size?: number;
}) {
  return (
    <span
      aria-hidden
      className="flex shrink-0 items-center justify-center rounded-[7px] bg-[var(--tile)] text-white shadow-[inset_0_0_0_0.5px_rgba(0,0,0,0.12)] dark:bg-[var(--tile-dark)]"
      style={
        {
          width: size,
          height: size,
          "--tile": tint,
          "--tile-dark": tintDark ?? tint,
        } as CSSProperties
      }
    >
      <Icon style={{ width: size * 0.64, height: size * 0.64 }} />
    </span>
  );
}

// Given the index we just finished, return the next index that still needs the
// user — skipping any permission that is already granted — or the length of the
// flow to signal "sub-flow complete".
function nextPendingIndex(from: number, state: PermissionsState | null): number {
  let i = from + 1;
  while (i < PERMISSION_FLOW.length && state?.[PERMISSION_FLOW[i].stateKey].granted) {
    i++;
  }
  return i;
}

// The first permission that still needs the user, used to seed the sub-flow so
// already-granted permissions are never shown as a step.
function firstPendingIndex(state: PermissionsState | null): number {
  const idx = PERMISSION_FLOW.findIndex((p) => !state?.[p.stateKey].granted);
  return idx === -1 ? PERMISSION_FLOW.length : idx;
}

const getOnboardingSteps = (
  permissionsAlreadyGranted: boolean,
  isDevelopmentMode: boolean = false,
  apiKeysAvailable: boolean = false
) => [
  {
    id: "welcome",
    title: "Welcome to Juno",
    description: isDevelopmentMode
      ? "Juno helps you automate tasks, manage your workflow, and get more done with your Mac. You're running in development mode, so onboarding will always show on startup."
      : "Talk to your Mac and it gets things done. Setup takes about a minute.",
    // No shadow on the app icon — Apple setup assistants never shadow them.
    icon: <img src="/juno.png" alt="Juno" className="h-24 w-24 object-contain" />,
    action: "Get Started",
  },
  // Onboarding stays as short as possible: only what the app needs to function
  // (a provider and permissions). Role capture and the keyboard drills were
  // deliberately cut — the shortcut is taught on the final screen without a
  // gate, and everything else is progressively disclosed in the app itself.
  ...(apiKeysAvailable
    ? []
    : [
        {
          id: "api-key",
          title: "Connect Your AI",
          description:
            "Use your Claude subscription, or paste an API key from Anthropic, OpenAI, or Google.",
          icon: <IconTile icon={KeyGlyph} tint="#8E8E93" tintDark="#98989D" size={48} />,
          action: "Continue",
        },
      ]),
  ...(permissionsAlreadyGranted
    ? []
    : [
        {
          id: "permissions",
          title: "Grant Access",
          description:
            "macOS asks before an app can control your Mac. Juno will open the right settings pane for each one.",
          icon: null, // The permission checklist is the content here.
          action: "Continue",
        },
      ]),
  {
    id: "complete",
    title: "You're all set",
    description:
      "Ask for anything in plain language. Juno can control your computer, browse the web, and manage files.",
    icon: null, // The live shortcut demo below is the hero of this screen.
    action: "Start Using Juno",
  },
];

/**
 * Visual keyboard shortcut display.
 * This component is PURELY visual — it renders key caps and reflects state.
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

    const displayKey = (() => {
      switch (key) {
        case "escape":
          return "esc";
        case "space":
          return "space";
        case "enter":
        case "return":
          return "return";
        case "backspace":
          return "delete";
        case "delete":
          return "fwd del";
        case "tab":
          return "tab";
        default:
          return key.toUpperCase();
      }
    })();

    return [...modifierKeys, displayKey];
  };

  const keys = displayKeys();

  return (
    <div className="my-8 flex items-center justify-center gap-3">
      {keys.map((keySymbol, index) => (
        <div key={index} className="flex items-center gap-3">
          {/* macOS-style key cap: flat fill, hairline border, subtle base edge.
              Proportions follow the Keyboard Viewer (shallower than square). */}
          <div
            className={`flex h-11 min-w-[44px] items-center justify-center rounded-[9px] border px-3 text-[14px] font-medium transition-colors duration-150 ${
              isActivated
                ? "border-[#007AFF] bg-[#007AFF] text-white dark:border-[#0A84FF] dark:bg-[#0A84FF]"
                : "border-border bg-muted/40 text-foreground shadow-[inset_0_-1px_0_rgba(0,0,0,0.06)] dark:shadow-[inset_0_-1px_0_rgba(0,0,0,0.4)]"
            }`}
          >
            {keySymbol}
          </div>
          {index < keys.length - 1 && (
            <span className="text-sm text-muted-foreground">+</span>
          )}
        </div>
      ))}
      {isActivated && <CheckGlyph className={`h-5 w-5 ${GREEN}`} />}
    </div>
  );
}

// ── Animated System Settings illustration ────────────────────────────────────
// A Codex-style instructional loop: a mock Settings row where the macOS
// pointer glides in and flips the "Juno" toggle. Shown while Accessibility is
// the active row — the one switch a human must flip — so the user sees the
// exact gesture before Settings even opens. Pure illustration (aria-hidden);
// with Reduce Motion on it renders the final frame, no cursor, switch on.
function SettingsToggleDemo() {
  const prefersReducedMotion = useReducedMotion() ?? false;
  // One shared timeline keeps the cursor and the switch in sync.
  const CYCLE = 4.4;
  const TIMES = [0, 0.1, 0.45, 0.52, 0.58, 0.86, 1];

  return (
    <div aria-hidden className="mb-4 flex justify-center">
      <div className="w-[300px] rounded-xl border border-border bg-card px-3 py-2.5 shadow-sm">
        <div className="relative flex items-center gap-2.5">
          <img src="/juno.png" alt="" className="h-5 w-5 object-contain" />
          <span className="flex-1 text-left text-[13px] text-foreground">Juno</span>

          {/* macOS-style switch — 36×20 with a 16pt knob, measured against the
              real System Settings toggles rather than the iOS 51×31 size. */}
          <div className="relative h-[20px] w-[36px]">
            {prefersReducedMotion ? (
              <div className="h-full w-full rounded-full bg-[#007AFF] dark:bg-[#0A84FF]">
                <div className="absolute top-[2px] h-[16px] w-[16px] translate-x-[18px] rounded-full bg-white shadow-sm" />
              </div>
            ) : (
              <>
                <motion.div
                  className="h-full w-full rounded-full"
                  animate={{
                    backgroundColor: [
                      "rgba(120,120,128,0.22)",
                      "rgba(120,120,128,0.22)",
                      "rgba(120,120,128,0.22)",
                      "rgba(120,120,128,0.22)",
                      "#007AFF",
                      "#007AFF",
                      "rgba(120,120,128,0.22)",
                    ],
                  }}
                  transition={{ duration: CYCLE, times: TIMES, repeat: Infinity, ease: "easeInOut" }}
                />
                <motion.div
                  className="absolute top-[2px] h-[16px] w-[16px] rounded-full bg-white shadow-sm"
                  animate={{ x: [2, 2, 2, 2, 18, 18, 2] }}
                  transition={{ duration: CYCLE, times: TIMES, repeat: Infinity, ease: "easeInOut" }}
                />
              </>
            )}

            {/* macOS pointer, gliding in from the lower left to click the switch */}
            {!prefersReducedMotion && (
              <motion.svg
                width="17"
                height="22"
                viewBox="0 0 13 20"
                className="absolute left-[9px] top-[9px] z-10"
                animate={{
                  x: [-130, -130, 0, 0, 0, 0, -130],
                  y: [64, 64, 0, 0, 0, 0, 64],
                  scale: [1, 1, 1, 0.82, 1, 1, 1],
                  opacity: [0, 1, 1, 1, 1, 1, 0],
                }}
                transition={{ duration: CYCLE, times: TIMES, repeat: Infinity, ease: "easeInOut" }}
              >
                <path
                  d="M1 1 L1 15.4 L4.6 12.2 L6.9 17.8 L9.4 16.7 L7.1 11.2 L11.9 11.2 Z"
                  className="fill-black stroke-white dark:fill-white dark:stroke-black"
                  strokeWidth="1.2"
                />
              </motion.svg>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// ── One row of the permission checklist ──────────────────────────────────────
// Exactly one row is "active" at a time and it carries the only button on the
// screen. Granted rows collapse to a green check; upcoming rows sit dimmed
// until the flow reaches them.
function PermissionRow({
  def,
  granted,
  active,
  waiting,
  isRequesting,
  autoStage,
  onRequest,
  onSkip,
}: {
  def: PermissionDef;
  granted: boolean;
  active: boolean;
  waiting: boolean;
  isRequesting: boolean;
  /** Set while the backend auto-grant run is working this row. */
  autoStage: string | null;
  onRequest: () => void;
  onSkip: () => void;
}) {
  const upcoming = !granted && !active && !autoStage;

  return (
    <div
      className={`flex items-start gap-3 px-4 py-3 text-left transition-opacity duration-200 ${
        upcoming ? "opacity-40" : ""
      }`}
    >
      <IconTile icon={def.icon} tint={def.tint} />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="text-[13px] font-medium text-foreground">{def.title}</span>
          {!def.required && (
            <span className="text-[11px] text-muted-foreground">Optional</span>
          )}
        </div>
        <p className="mt-px text-[12px] leading-[1.4] text-muted-foreground">{def.why}</p>
        {active && waiting && !granted && (
          <p className="mt-1.5 flex items-center gap-1.5 text-[12px] text-muted-foreground">
            <Loader2 className="h-3 w-3 animate-spin" aria-hidden />
            Waiting — switch Juno on in System Settings and this updates automatically.
          </p>
        )}
        {autoStage && !granted && (
          <p className="mt-1.5 flex items-center gap-1.5 text-[12px] text-muted-foreground">
            <Loader2 className="h-3 w-3 animate-spin" aria-hidden />
            {AUTO_STAGE_COPY[autoStage] ?? "Working…"}
          </p>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-2 pt-0.5">
        {granted ? (
          <span role="img" aria-label="Granted">
            <CheckGlyph className={`h-[18px] w-[18px] ${GREEN}`} />
          </span>
        ) : active ? (
          <>
            {!def.required && (
              <button onClick={onSkip} className={LINK_QUIET}>
                Skip
              </button>
            )}
            <button
              onClick={onRequest}
              disabled={isRequesting}
              className={waiting ? BTN_ROW_QUIET : BTN_ROW}
              aria-label={`${def.hasPrompt && !waiting ? "Allow" : "Open Settings for"} ${def.title}`}
            >
              {isRequesting ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden />
              ) : waiting || !def.hasPrompt ? (
                "Open Settings"
              ) : (
                "Allow"
              )}
            </button>
          </>
        ) : null}
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
  // `permissionsAlreadyGranted` is intentionally not read here: the component
  // always re-checks permissions on mount into `actualPermissionsGranted` (the
  // step list is derived from that), which keeps "Restart onboarding" honest.
  isDevelopmentMode = false,
}: OnboardingFlowProps) {
  const [currentStep, setCurrentStep] = useState(0);
  // Set when the backend reports the agent-mode shortcut / Escape was pressed.
  // Used only for the live two-stage demo on the final screen: trying the
  // agent shortcut fades that demo out and fades the Escape demo in. Nothing
  // is gated on either.
  const [shortcutPressed, setShortcutPressed] = useState(false);
  const [escapePressed, setEscapePressed] = useState(false);
  const [completeDemoStage, setCompleteDemoStage] = useState<"shortcut" | "escape">("shortcut");
  const [_backendShortcutsWorking, setBackendShortcutsWorking] =
    useState(false);
  const [keyboardShortcuts, setKeyboardShortcuts] = useState<any>(null);
  const [isComplete, setIsComplete] = useState(false);
  const [actualPermissionsGranted, setActualPermissionsGranted] = useState(
    // Always start with false to ensure we re-check permissions on mount
    // This is critical for the "Restart onboarding" functionality
    false
  );

  // Claude CLI state
  const [cliAvailable, setCliAvailable] = useState(false);
  const [cliAuthenticated, setCliAuthenticated] = useState(false);
  const [cliChecking, setCliChecking] = useState(true);
  const [cliSelected, setCliSelected] = useState(false);

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

  // New state for granular permissions
  const [permissionsState, setPermissionsState] =
    useState<PermissionsState | null>(null);
  const [isRequestingPermission, setIsRequestingPermission] = useState<
    string | null
  >(null);
  const [permissionsError, setPermissionsError] = useState<string | null>(null);

  // Guided permission sub-flow: which checklist row is active, and which
  // permissions the user has already attempted (so a row can switch to its
  // "waiting in System Settings" state). `permIndex === PERMISSION_FLOW.length`
  // means the sub-flow is complete.
  const [permIndex, setPermIndex] = useState(0);
  const [attemptedPerms, setAttemptedPerms] = useState<Set<string>>(new Set());
  const permInitRef = useRef(false);

  // Auto-grant: once Accessibility lands, Juno can drive the remaining System
  // Settings toggles itself. `null` → not offered yet; "offer" → the one-time
  // consent panel is up; "running" → the backend run is live; "manual" → the
  // user declined, cancelled, or the run finished (checklist behaves normally
  // from there); "finished" → transient handoff state after a successful run.
  const [autoGrantMode, setAutoGrantMode] = useState<
    null | "offer" | "running" | "manual" | "finished"
  >(null);
  const [autoGrantStage, setAutoGrantStage] = useState<{
    perm: string | null;
    stage: string;
  } | null>(null);
  // The microphone prompt is raised automatically once, right after a
  // successful auto-grant run — one Allow click finishes everything.
  const micAutoPromptedRef = useRef(false);

  const mountedRef = useRef(true);
  const onboardingTimers = useRef<ReturnType<typeof setTimeout>[]>([]);

  // ── Analytics (#14) ────────────────────────────────────────────────────────
  // All timestamps are relative — we never write user PII or query content.
  // Backend persists events to a 500-entry FIFO Tauri Store buffer.
  const analyticsStartMsRef = useRef<number>(Date.now());
  const phaseEnteredAtMsRef = useRef<number>(Date.now());
  const recordedStepIdsRef = useRef<Set<string>>(new Set());
  const completedRecordedRef = useRef<boolean>(false);
  // Tracks which optional permissions were never granted by the time the user
  // advanced past the permissions step — emitted as `onboarding_permission_skipped`.
  const previouslyGrantedRef = useRef<{ accessibility: boolean; screen_recording: boolean; microphone: boolean; input_monitoring: boolean }>(
    { accessibility: false, screen_recording: false, microphone: false, input_monitoring: false }
  );

  const recordEvent = useCallback(
    (eventName: string, payload?: Record<string, unknown>) => {
      // Fire-and-forget — analytics must never block UI progress.
      invoke("record_onboarding_event", { eventName, payload: payload ?? null }).catch((err) => {
        console.debug("[Onboarding] record_onboarding_event failed:", err);
      });
    },
    []
  );

  // Honor macOS "Reduce motion" — framer-motion's useReducedMotion hook reads
  // `prefers-reduced-motion: reduce`. When true we collapse the step fade to 0.
  const prefersReducedMotion = useReducedMotion() ?? false;
  // Motion here is a single quick cross-fade between steps. Nothing else moves.
  const motionDuration = prefersReducedMotion ? 0 : 0.18;

  useEffect(() => {
    mountedRef.current = true;
    // Fire onboarding_started once on mount.
    analyticsStartMsRef.current = Date.now();
    phaseEnteredAtMsRef.current = analyticsStartMsRef.current;
    recordEvent("onboarding_started");

    // Start 1Hz native permission polling so `permissions-changed` events
    // flow while onboarding is on screen. Idempotent — the backend cancels
    // any prior task before starting a new one. Best-effort; failure here
    // only degrades revocation responsiveness, not core onboarding.
    invoke("start_permissions_monitoring").catch((err) => {
      console.debug("[Onboarding] start_permissions_monitoring failed:", err);
    });

    return () => {
      mountedRef.current = false;
      for (const timer of onboardingTimers.current) {
        clearTimeout(timer);
      }
      onboardingTimers.current = [];
      // Stop monitoring when onboarding closes so we're not paying for a
      // 1s tick across the rest of the app's lifetime. PermissionsManager /
      // PermissionsFlow re-start it on demand when they mount.
      invoke("stop_permissions_monitoring").catch((err) => {
        console.debug("[Onboarding] stop_permissions_monitoring failed:", err);
      });
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Backend-driven shortcut detection ──
  // Global shortcuts (Option+D) are captured at the OS level by
  // tauri_plugin_global_shortcut — they NEVER reach the webview as keydown events.
  // The backend always emits these events even during onboarding (visual feedback
  // mode), so the final screen's keycaps can light up when the user tries the
  // shortcut. We use useEventListener (the project's canonical Tauri event hook).
  useEventListener<{ state: string; shortcut: string }>(
    EVENTS.SHORTCUTS_AGENT_MODE,
    (payload) => {
      if (payload.state === "pressed" && !shortcutPressed) {
        setShortcutPressed(true);
      }
    }
  );

  useEventListener<{ state: string; shortcut: string }>(
    EVENTS.SHORTCUTS_ESCAPE_KEY,
    (payload) => {
      if (payload.state === "pressed" && !escapePressed) {
        setEscapePressed(true);
      }
    }
  );

  // Progressive disclosure on the final screen: once the user tries the agent
  // shortcut, hold the lit keycaps for a beat, then hand the stage to the
  // Escape demo.
  useEffect(() => {
    if (!shortcutPressed || completeDemoStage !== "shortcut") return;
    const t = setTimeout(
      () => {
        if (mountedRef.current) setCompleteDemoStage("escape");
      },
      prefersReducedMotion ? 0 : 900
    );
    return () => clearTimeout(t);
  }, [shortcutPressed, completeDemoStage, prefersReducedMotion]);

  // ── Live permission state via backend monitoring (Phase D edge case #12) ──
  // The backend polls native APIs every 1s and emits `permissions-changed`
  // with the full state. Subscribing here lets the revocation-detection effect
  // (further down) react immediately if a previously-granted required
  // permission is revoked while the user is on a later onboarding step.
  //
  // Dedup: the backend emits a fresh object every tick, so we compare the
  // four grant booleans before committing the new state — otherwise a 1Hz
  // setState would re-render the whole onboarding tree (and the audio
  // visualizer) every second even when nothing has actually changed.
  useEventListener<PermissionsState>(
    EVENTS.PERMISSIONS_CHANGED,
    (payload) => {
      if (!mountedRef.current) return;
      setPermissionsState((prev) => {
        if (
          prev &&
          prev.all_granted === payload.all_granted &&
          prev.accessibility.granted === payload.accessibility.granted &&
          prev.screen_recording.granted === payload.screen_recording.granted &&
          prev.microphone.granted === payload.microphone.granted &&
          prev.input_monitoring.granted === payload.input_monitoring.granted
        ) {
          return prev;
        }
        return payload;
      });
      setActualPermissionsGranted((prev) =>
        prev === payload.all_granted ? prev : payload.all_granted
      );
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

  // Request a single permission. First call triggers the native prompt (when
  // one exists) or opens the exact Settings pane; subsequent calls open the
  // pane directly. The 1Hz monitor flips the row to granted automatically, so
  // we only need a light post-request re-check as a fallback.
  const requestPermission = async (permissionType: string) => {
    try {
      setIsRequestingPermission(permissionType);
      setPermissionsError(null);
      setAttemptedPerms((prev) => {
        const next = new Set(prev);
        next.add(permissionType);
        return next;
      });

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
        // CRITICAL: Always re-check permissions when component mounts
        await checkPermissionsStatus();
        if (!mounted) return;

        // Check if API keys are already available (from store or .env)
        try {
          const keysAvailable = await invoke<boolean>("check_api_keys_available");
          if (mounted) {
            setApiKeysAvailable(keysAvailable);
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

    // Add window focus listener to re-check permissions when window gains focus
    const handleWindowFocus = async () => {
      if (!mounted) return;
      await checkPermissionsStatus();
      try {
        const cliStatus = await invoke<{ available: boolean; authenticated: boolean }>("check_claude_cli_available");
        if (!mounted) return;
        setCliAvailable(cliStatus.available);
        setCliAuthenticated(cliStatus.authenticated);
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

  const stepId = onboardingSteps[currentStep]?.id;

  // ── Analytics: onboarding_phase_entered on step change ───────────────────────
  // Use the step ID (not the index) so phase names are stable when the step
  // list shrinks/grows based on `permissionsAlreadyGranted` and `apiKeysAvailable`.
  useEffect(() => {
    const currentId = onboardingSteps[currentStep]?.id;
    if (!currentId) return;
    if (recordedStepIdsRef.current.has(currentId)) return;
    recordedStepIdsRef.current.add(currentId);
    phaseEnteredAtMsRef.current = Date.now();
    recordEvent("onboarding_phase_entered", {
      phase: currentId,
      t_ms_since_start: Date.now() - analyticsStartMsRef.current,
    });
  }, [currentStep, onboardingSteps, recordEvent]);

  // ── Seed the permission sub-flow to the first pending permission ─────────────
  // Runs once when we first land on the permissions step with state in hand, so
  // already-granted permissions are never shown as an active row.
  useEffect(() => {
    if (stepId !== "permissions" || !permissionsState || permInitRef.current) return;
    permInitRef.current = true;
    setPermIndex(firstPendingIndex(permissionsState));
  }, [stepId, permissionsState]);

  // ── Auto-advance the checklist the instant the active permission flips ───────
  // The Wispr pattern: the row watches itself. When the active permission is
  // granted, pause for a beat so the green check registers, then move to the
  // next pending row (or complete the sub-flow).
  useEffect(() => {
    if (stepId !== "permissions" || !permissionsState) return;
    if (permIndex >= PERMISSION_FLOW.length) return;
    const active = PERMISSION_FLOW[permIndex];
    if (!permissionsState[active.stateKey].granted) return;
    const t = setTimeout(
      () => {
        if (mountedRef.current) {
          setPermIndex((i) => nextPendingIndex(i, permissionsState));
        }
      },
      prefersReducedMotion ? 0 : 600
    );
    // No need to register in onboardingTimers — this effect's own cleanup
    // clears the timer on unmount and on every dependency change.
    return () => clearTimeout(t);
  }, [stepId, permissionsState, permIndex, prefersReducedMotion]);

  // ── Auto-grant offer: the payoff for granting Accessibility ─────────────────
  // The moment Accessibility is on and automatable permissions remain, offer to
  // finish setup automatically. Offered at most once (`autoGrantMode` only
  // leaves `null` forward), with a quiet decline that returns to the manual
  // guided flow.
  useEffect(() => {
    if (stepId !== "permissions" || !permissionsState || autoGrantMode !== null) return;
    if (!permissionsState.accessibility.granted) return;
    if (!AUTOMATABLE.some((k) => !permissionsState[k].granted)) return;
    setAutoGrantMode("offer");
  }, [stepId, permissionsState, autoGrantMode]);

  // Progress stream from the backend run. Grants themselves land through the
  // 1Hz permissions poller (the single source of truth for row state); this
  // stream only narrates what Juno is doing and signals the end of the run.
  useEventListener<{ permission_type: string | null; stage: string; message: string | null }>(
    EVENTS.PERMISSIONS_AUTO_GRANT_PROGRESS,
    (payload) => {
      if (!mountedRef.current) return;
      switch (payload.stage) {
        case "done":
          recordEvent("onboarding_auto_grant_finished");
          setAutoGrantStage(null);
          setAutoGrantMode("finished");
          break;
        case "cancelled":
          recordEvent("onboarding_auto_grant_cancelled");
          setAutoGrantStage(null);
          setAutoGrantMode("manual");
          break;
        case "failed":
          // Not fatal — the run continues to the next permission, and the
          // failed row falls back to the manual guided flow afterwards.
          recordEvent("onboarding_auto_grant_failed", {
            permission: payload.permission_type,
          });
          setAutoGrantStage(null);
          break;
        case "granted":
          setAutoGrantStage(null);
          break;
        default:
          setAutoGrantStage({ perm: payload.permission_type, stage: payload.stage });
      }
    }
  );

  // Hand the checklist back after a successful run: re-seed to the first still
  // pending row and raise the microphone prompt once so the last grant is a
  // single Allow click. Anything that failed auto-grant simply becomes the
  // active manual row again.
  useEffect(() => {
    if (autoGrantMode !== "finished" || !permissionsState) return;
    setPermIndex(firstPendingIndex(permissionsState));
    if (
      !permissionsState.microphone.granted &&
      !micAutoPromptedRef.current &&
      !attemptedPerms.has("microphone")
    ) {
      micAutoPromptedRef.current = true;
      requestPermission("microphone");
    }
    setAutoGrantMode("manual");
    // attemptedPerms is intentionally omitted from the deps: this effect only
    // does work on the single render where autoGrantMode flips to "finished",
    // and that render's closure carries the current set. Adding it would
    // re-run the effect on unrelated attempts.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoGrantMode, permissionsState]);

  // Ref (not state) so a double-click on "Finish setup for me" can't race two
  // backend invocations before React re-renders.
  const autoGrantStartingRef = useRef(false);
  const startAutoGrant = useCallback(async () => {
    if (!permissionsState || autoGrantStartingRef.current) return;
    const pending = AUTOMATABLE.filter((k) => !permissionsState[k].granted);
    if (pending.length === 0) {
      setAutoGrantMode("manual");
      return;
    }
    autoGrantStartingRef.current = true;
    setAutoGrantMode("running");
    recordEvent("onboarding_auto_grant_started", { permissions: pending });
    try {
      await invoke(COMMANDS.PERMISSIONS_AUTO_GRANT_PERMISSIONS, { permissions: pending });
    } catch (error) {
      console.warn("[Onboarding] auto_grant_permissions failed to start:", error);
      // "already running" means a real run is live in the backend — dropping
      // to manual would put two actors on the same checklist. Only fall back
      // for genuine start failures.
      if (!String(error).toLowerCase().includes("already running")) {
        setAutoGrantMode("manual");
      }
    } finally {
      autoGrantStartingRef.current = false;
    }
  }, [permissionsState, recordEvent]);

  const stopAutoGrant = useCallback(async () => {
    try {
      await invoke(COMMANDS.PERMISSIONS_CANCEL_AUTO_GRANT);
    } catch (error) {
      console.warn("[Onboarding] cancel_auto_grant failed:", error);
    }
    // The backend also emits `cancelled`, but flip locally so the UI never
    // waits on an event to honor a Stop click.
    setAutoGrantStage(null);
    setAutoGrantMode("manual");
  }, []);

  // ── Resume on app restart (Phase D edge case #12) ───────────────────────────
  // The backend persists the last-entered phase to Tauri Store. On mount, if
  // we find a phase that maps to a step in the current `onboardingSteps` list,
  // jump there. The step list can differ between sessions (e.g. permissions
  // already granted now), so a missing match just means we start at step 0.
  const resumeAttemptedRef = useRef(false);
  useEffect(() => {
    if (resumeAttemptedRef.current) return;
    resumeAttemptedRef.current = true;
    (async () => {
      try {
        const lastPhase = await invoke<string | null>("get_last_onboarding_phase");
        if (!lastPhase || !mountedRef.current) return;
        const idx = onboardingSteps.findIndex((s) => s.id === lastPhase);
        // Don't resume to the welcome step (always start there if no progress)
        // or to the complete step (avoid skipping the user past final guidance).
        if (idx > 0 && idx < onboardingSteps.length - 1 && currentStep === 0) {
          setCurrentStep(idx);
        }
      } catch (err) {
        console.debug("[Onboarding] resume read failed:", err);
      }
    })();
    // Only run after steps are known and on initial mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [onboardingSteps]);

  // ── Analytics: onboarding_permission_granted on permission flip ─────────────
  // Also detects revocation of a previously-granted required permission and
  // returns the user to the permissions step (Phase D edge case #12).
  useEffect(() => {
    if (!permissionsState) return;

    const grantedNow = {
      accessibility: permissionsState.accessibility.granted,
      screen_recording: permissionsState.screen_recording.granted,
      microphone: permissionsState.microphone.granted,
      input_monitoring: permissionsState.input_monitoring.granted,
    };
    const prev = previouslyGrantedRef.current;

    (Object.keys(grantedNow) as Array<keyof typeof grantedNow>).forEach((perm) => {
      if (grantedNow[perm] && !prev[perm]) {
        // false → true
        recordEvent("onboarding_permission_granted", {
          permission: perm,
          t_ms_since_phase_entered: Date.now() - phaseEnteredAtMsRef.current,
        });
      } else if (!grantedNow[perm] && prev[perm]) {
        // true → false (revocation)
        const isRequired = perm === "accessibility" || perm === "screen_recording";
        if (isRequired) {
          const permIdx = onboardingSteps.findIndex((s) => s.id === "permissions");
          // Recover whether the user has already moved PAST the permissions
          // step or is still ON it. Both matter: the checklist tracks its own
          // position (permIndex), which may have advanced beyond the
          // now-revoked permission. Without rewinding it here, a later
          // optional-skip could complete the sub-flow with the footer Continue
          // disabled and no row actionable — trapping the user until the 1Hz
          // poller happened to see a re-grant.
          if (permIdx !== -1 && currentStep >= permIdx) {
            console.warn(`[Onboarding] Required permission '${perm}' revoked; rewinding checklist`);
            recordEvent("onboarding_error_recovery", {
              phase: onboardingSteps[currentStep]?.id ?? "unknown",
              error_kind: `${perm}_revoked`,
            });
            // If the auto-grant offer or run is up, it was predicated on the
            // now-revoked grant — stop it and return to the manual checklist
            // so the primary action reflects reality.
            if (autoGrantMode === "running") {
              stopAutoGrant();
            } else if (autoGrantMode === "offer") {
              setAutoGrantMode("manual");
            }
            recordedStepIdsRef.current.delete("permissions");
            // Rewind the checklist to the first still-needed permission...
            setPermIndex(firstPendingIndex(permissionsState));
            // ...and forget the stale "attempted" flag so its row shows the
            // primary action rather than the "waiting in System Settings" copy.
            setAttemptedPerms((prevSet) => {
              if (!prevSet.has(perm)) return prevSet;
              const next = new Set(prevSet);
              next.delete(perm);
              return next;
            });
            // Only navigate if we're on a later step; if we're already on the
            // permissions step, the rewind above is enough.
            if (currentStep > permIdx) setCurrentStep(permIdx);
          }
        }
      }
    });

    previouslyGrantedRef.current = grantedNow;
  }, [permissionsState, onboardingSteps, currentStep, recordEvent, autoGrantMode, stopAutoGrant]);

  const handleNext = useCallback(async () => {
    const currentStepData = onboardingSteps[currentStep];
    // Block navigation from permissions step if required permissions not granted
    if (currentStepData?.id === "permissions" && !areRequiredPermissionsGranted()) {
      return;
    }
    // While the permission checklist is still active, the footer Continue is
    // intentionally not rendered. The global Enter handler must not advance
    // either, or it would silently skip the optional rows the moment the
    // required ones are satisfied (Enter and the visible UI must never
    // disagree).
    if (currentStepData?.id === "permissions" && permIndex < PERMISSION_FLOW.length) {
      return;
    }

    // Save API key before advancing from api-key step (unless CLI was chosen)
    if (currentStepData?.id === "api-key" && !cliSelected && detectedProvider && !apiKeySaved) {
      await saveApiKey();
    }

    // Record permission_skipped for any optional permission that was never
    // granted by the time the user moves past the permissions step.
    if (currentStepData?.id === "permissions" && permissionsState) {
      if (!permissionsState.microphone.granted) {
        recordEvent("onboarding_permission_skipped", { permission: "microphone" });
      }
      if (!permissionsState.input_monitoring.granted) {
        recordEvent("onboarding_permission_skipped", { permission: "input_monitoring" });
      }
    }

    if (currentStep < onboardingSteps.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      if (!completedRecordedRef.current) {
        completedRecordedRef.current = true;
        recordEvent("onboarding_completed", {
          total_t_ms: Date.now() - analyticsStartMsRef.current,
          // Optional permissions ungranted at completion are the "skipped phases" for funnel purposes.
          skipped_phases: permissionsState
            ? [
                !permissionsState.microphone.granted ? "microphone" : null,
                !permissionsState.input_monitoring.granted ? "input_monitoring" : null,
              ].filter(Boolean)
            : [],
        });
      }
      setIsComplete(true);
      onComplete();
    }
  }, [currentStep, onboardingSteps, permissionsState, permIndex, detectedProvider, apiKeySaved, saveApiKey, onComplete, recordEvent]);

  const handleSkip = () => {
    // Skip the current step by jumping to the end
    if (!completedRecordedRef.current) {
      completedRecordedRef.current = true;
      recordEvent("onboarding_completed", {
        total_t_ms: Date.now() - analyticsStartMsRef.current,
        skipped_phases: ["__user_skipped_remaining__"],
      });
    }
    setIsComplete(true);
    if (onSkip) {
      onSkip();
    } else {
      onComplete();
    }
  };

  const handleSkipStep = () => {
    // Skip just the current step and move to the next one
    const currentStepData = onboardingSteps[currentStep];
    if (currentStepData) {
      recordEvent("onboarding_permission_skipped", {
        // Re-using the permission_skipped event for any explicit step skip
        // keeps the funnel schema small; phase carries the actual step id.
        permission: `step:${currentStepData.id}`,
      });
    }
    if (currentStep < onboardingSteps.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      // If this is the last step, complete onboarding
      if (!completedRecordedRef.current) {
        completedRecordedRef.current = true;
        recordEvent("onboarding_completed", {
          total_t_ms: Date.now() - analyticsStartMsRef.current,
          skipped_phases: [`step:${currentStepData?.id ?? "unknown"}`],
        });
      }
      setIsComplete(true);
      if (onSkip) {
        onSkip();
      } else {
        onComplete();
      }
    }
  };

  // Skip a single optional permission within the checklist.
  const skipCurrentPermission = () => {
    const active = PERMISSION_FLOW[permIndex];
    if (!active || active.required) return;
    recordEvent("onboarding_permission_skipped", { permission: active.stateKey });
    setPermIndex((i) => nextPendingIndex(i, permissionsState));
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

  // Checklist position.
  const permSubFlowComplete = permIndex >= PERMISSION_FLOW.length;
  // While the checklist is running, its active row owns the primary action, so
  // we hide the footer to keep exactly one primary action on screen.
  const inActivePermFlow = step.id === "permissions" && !permSubFlowComplete;

  // Determine if continue button should be disabled
  // Role step is intentionally optional — no guard here; handleNext saves only when a role is selected
  const isContinueDisabled =
    (step.id === "permissions" && !areRequiredPermissionsGranted()) ||
    (step.id === "api-key" && !detectedProvider && !cliSelected) ||
    (step.id === "api-key" && apiKeySaving);

  // Determine if skip should be hidden (permissions step with required perms not granted)
  const isSkipHidden =
    currentStep === onboardingSteps.length - 1 ||
    (step.id === "permissions" && !areRequiredPermissionsGranted());

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-background"
      style={{ fontFamily: SF_FONT, WebkitFontSmoothing: "antialiased" } as CSSProperties}
    >
      <div className="max-h-[90vh] w-full max-w-[520px] overflow-y-auto px-8 py-10">
        {/* Progress: a macOS-style page control. Only the current page is dark. */}
        <div
          className="mb-12 flex justify-center gap-2"
          role="progressbar"
          aria-valuenow={currentStep + 1}
          aria-valuemax={onboardingSteps.length}
          aria-label={`Step ${currentStep + 1} of ${onboardingSteps.length}`}
        >
          {onboardingSteps.map((_, index) => (
            <div
              key={index}
              className={`h-1.5 w-1.5 rounded-full transition-colors duration-300 ${
                index === currentStep ? "bg-foreground/70" : "bg-foreground/[0.15]"
              }`}
            />
          ))}
        </div>

        <AnimatePresence mode="wait">
          <motion.div
            key={step.id}
            initial={prefersReducedMotion ? false : { opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: motionDuration, ease: "easeOut" }}
            className="text-center"
          >
            {/* Icon (null for shortcut/cancel — floating bar is above) */}
            {step.icon && <div className="mb-7 flex justify-center">{step.icon}</div>}

            {/* Content */}
            <div className="mb-10 space-y-3">
              {/*
                aria-live="polite" + role="status" lets screen readers
                announce each onboarding step's title when the user advances.
                Polite (not assertive) so it never interrupts active narration.
              */}
              <div role="status" aria-live="polite" aria-atomic="true">
                <h2 className="text-[26px] font-semibold tracking-tight text-foreground">
                  {step.title}
                </h2>
              </div>

              <p className="mx-auto max-w-[42ch] text-[13px] leading-[1.45] text-muted-foreground">
                {step.description}
              </p>

              {/* Live two-stage demo, ungated. Stage 1: the agent shortcut,
                  detected via the backend event. Trying it fades this out and
                  fades in stage 2: Escape, also live. Continue works whether
                  or not the user plays along. */}
              {step.id === "complete" && (
                <div className="min-h-[140px]">
                  <AnimatePresence mode="wait">
                    {completeDemoStage === "shortcut" ? (
                      <motion.div
                        key="demo-shortcut"
                        initial={prefersReducedMotion ? false : { opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: motionDuration, ease: "easeOut" }}
                      >
                        <KeyboardShortcutDisplay
                          shortcutString={keyboardShortcuts?.agent_mode}
                          isActivated={shortcutPressed}
                        />
                        <p className="text-[13px] text-muted-foreground">
                          {shortcutPressed
                            ? "That summons Juno from anywhere."
                            : "Try it — this summons Juno from anywhere."}
                        </p>
                      </motion.div>
                    ) : (
                      <motion.div
                        key="demo-escape"
                        initial={prefersReducedMotion ? false : { opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: motionDuration, ease: "easeOut" }}
                      >
                        <KeyboardShortcutDisplay
                          shortcutString={keyboardShortcuts?.stop_current_task}
                          defaultShortcut="Escape"
                          isActivated={escapePressed}
                        />
                        <p className="text-[13px] text-muted-foreground">
                          {escapePressed
                            ? "Perfect. You're ready."
                            : "And Escape stops Juno anytime. Try it."}
                        </p>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              )}

              {/* Connect step: Claude CLI + API key dual path */}
              {step.id === "api-key" && (
                <div className="space-y-3 pt-4 text-left">
                  {/* Claude CLI card */}
                  <div
                    className={`rounded-xl border p-4 transition-colors ${
                      cliSelected
                        ? "border-[#007AFF] bg-[#007AFF]/[0.06] dark:border-[#0A84FF] dark:bg-[#0A84FF]/[0.10] cursor-pointer"
                        : cliAvailable && cliAuthenticated
                          ? "border-border bg-card hover:bg-muted/40 cursor-pointer"
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
                      <IconTile icon={TerminalGlyph} tint="#2C2C2E" tintDark="#3A3A3C" />
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-[13px] font-medium text-foreground">
                            Use your Claude subscription
                          </span>
                          {cliSelected && (
                            <span role="img" aria-label="Selected">
                              <CheckGlyph className="h-4 w-4 text-[#007AFF] dark:text-[#0A84FF]" />
                            </span>
                          )}
                        </div>
                        <p className="mt-0.5 text-[12px] text-muted-foreground">
                          No API key needed — uses Claude Code CLI authentication.
                        </p>
                        <div className="mt-1.5 text-[12px]">
                          {cliChecking ? (
                            <span className="inline-flex items-center gap-1.5 text-muted-foreground">
                              <Loader2 className="h-3 w-3 animate-spin" /> Checking
                            </span>
                          ) : cliAvailable && cliAuthenticated ? (
                            // Green check beside gray text — how System Settings
                            // marks status; green text itself is not native.
                            <span className="inline-flex items-center gap-1.5 text-muted-foreground">
                              <CheckGlyph className={`h-3 w-3 ${GREEN}`} /> Detected and signed in
                            </span>
                          ) : cliAvailable ? (
                            <span className="inline-flex items-center gap-1.5 text-muted-foreground">
                              <AlertCircle className="h-3 w-3" /> Installed but not signed in — run{" "}
                              <code className="rounded bg-muted px-1 py-0.5 font-mono text-[11px]">claude login</code>
                            </span>
                          ) : (
                            <a
                              href="https://claude.ai/code"
                              target="_blank"
                              rel="noopener noreferrer"
                              className="inline-flex items-center gap-1.5 text-muted-foreground transition-colors hover:text-foreground"
                            >
                              <ExternalLink className="h-3 w-3" /> Not installed — get Claude Code
                            </a>
                          )}
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Divider */}
                  <div className="flex items-center gap-3 px-2">
                    <div className="h-px flex-1 bg-border" />
                    <span className="text-[11px] text-muted-foreground">or</span>
                    <div className="h-px flex-1 bg-border" />
                  </div>

                  {/* API key input with show/hide toggle */}
                  <div className="relative">
                    <input
                      type={showApiKey ? "text" : "password"}
                      value={apiKey}
                      onChange={(e) => handleApiKeyChange(e.target.value)}
                      placeholder="Paste your API key"
                      className={`h-10 w-full rounded-[10px] border border-border bg-card px-3 pr-10 font-mono text-[13px] text-foreground placeholder:font-sans placeholder:text-muted-foreground transition-colors focus:border-[#007AFF] dark:focus:border-[#0A84FF] ${FOCUS_RING}`}
                      spellCheck={false}
                      autoComplete="off"
                      aria-label="API key"
                      disabled={cliSelected}
                    />
                    <button
                      type="button"
                      onClick={() => setShowApiKey(!showApiKey)}
                      className={`absolute right-2.5 top-1/2 -translate-y-1/2 rounded text-muted-foreground transition-colors hover:text-foreground ${FOCUS_RING}`}
                      aria-label={showApiKey ? "Hide API key" : "Show API key"}
                    >
                      {showApiKey ? (
                        <EyeOff className="h-4 w-4" />
                      ) : (
                        <Eye className="h-4 w-4" />
                      )}
                    </button>
                  </div>

                  {/* Status line: detection / warning / saved / error, one at a time. */}
                  <div className="min-h-[18px] text-center text-[12px]">
                    {apiKeyError ? (
                      <span className="text-red-600 dark:text-red-400">Couldn't save: {apiKeyError}</span>
                    ) : apiKeySaved ? (
                      <span className="inline-flex items-center gap-1.5 text-muted-foreground">
                        <CheckGlyph className={`h-3 w-3 ${GREEN}`} /> Saved — {detectedProvider?.name} is your active provider
                      </span>
                    ) : detectedProvider ? (
                      <span className="inline-flex items-center gap-1.5 text-muted-foreground">
                        <CheckGlyph className={`h-3 w-3 ${GREEN}`} /> {detectedProvider.name} key detected
                      </span>
                    ) : apiKey.trim().length > 0 ? (
                      <span className="inline-flex items-center gap-1.5 text-muted-foreground">
                        <AlertCircle className="h-3 w-3" /> Unrecognized key format
                      </span>
                    ) : (
                      <span className="text-muted-foreground">
                        Anthropic (sk-ant-), OpenAI (sk-proj-), or Google (AIza)
                      </span>
                    )}
                  </div>
                </div>
              )}

              {/* Guided permission checklist */}
              {step.id === "permissions" && (
                <div className="pt-4">
                  {permissionsError && (
                    <p className="mb-3 text-[12px] text-red-600 dark:text-red-400">
                      {permissionsError}
                    </p>
                  )}

                  {!permissionsState ? (
                    // Loading state — we haven't heard from the native check yet.
                    <div className="flex items-center justify-center gap-2 py-10 text-[13px] text-muted-foreground">
                      <Loader2 className="h-4 w-4 animate-spin" />
                      Checking permissions
                    </div>
                  ) : (
                    <>
                      {/* Instructional loop for the one toggle only a human can
                          flip. Disappears the moment Accessibility is granted. */}
                      {PERMISSION_FLOW[permIndex]?.stateKey === "accessibility" &&
                        !permissionsState.accessibility.granted && <SettingsToggleDemo />}

                      {/* The inset group, matching the Settings window. */}
                      <div className="divide-y divide-border overflow-hidden rounded-xl border border-border bg-card">
                        {PERMISSION_FLOW.map((def, i) => (
                          <PermissionRow
                            key={def.stateKey}
                            def={def}
                            granted={permissionsState[def.stateKey].granted}
                            // While the auto-grant offer or run owns the screen
                            // (or the one-render "finished" handoff is in
                            // flight), no row carries a button — one primary
                            // action, no flicker.
                            active={
                              i === permIndex &&
                              autoGrantMode !== "offer" &&
                              autoGrantMode !== "running" &&
                              autoGrantMode !== "finished"
                            }
                            waiting={attemptedPerms.has(def.stateKey)}
                            isRequesting={isRequestingPermission === def.stateKey}
                            autoStage={
                              autoGrantMode === "running" &&
                              autoGrantStage?.perm === def.stateKey
                                ? autoGrantStage.stage
                                : null
                            }
                            onRequest={() => requestPermission(def.stateKey)}
                            onSkip={skipCurrentPermission}
                          />
                        ))}
                      </div>
                      {autoGrantMode === "offer" && (
                        <div className="mt-5 flex flex-col items-center gap-3">
                          <p className="text-[12px] text-muted-foreground">
                            Juno can use Accessibility to switch the rest on for you.
                          </p>
                          <button onClick={startAutoGrant} className={BTN_PRIMARY}>
                            Finish setup for me
                          </button>
                          <button
                            onClick={() => setAutoGrantMode("manual")}
                            className={LINK_QUIET}
                          >
                            I'll do it myself
                          </button>
                        </div>
                      )}
                      {autoGrantMode === "running" && (
                        <div className="mt-5 flex flex-col items-center gap-2">
                          <p className="text-[12px] text-muted-foreground">
                            Juno is switching these on in System Settings.
                          </p>
                          <button onClick={stopAutoGrant} className={LINK_QUIET}>
                            Stop
                          </button>
                        </div>
                      )}
                      {permSubFlowComplete && (
                        <p className="mt-3 text-[12px] text-muted-foreground">
                          You can change these anytime in System Settings.
                        </p>
                      )}
                    </>
                  )}
                </div>
              )}
            </div>

            {/* Footer: one centered primary action, skip as a quiet text link.
                Hidden while the permission checklist owns the primary action. */}
            {!inActivePermFlow && (
              <div className="flex flex-col items-center gap-3">
                <button
                  onClick={handleNext}
                  disabled={isContinueDisabled}
                  className={`${BTN_PRIMARY} ${isContinueDisabled ? "cursor-not-allowed" : ""}`}
                  aria-label={step.action}
                >
                  {step.action}
                </button>

                {!isSkipHidden && (
                  <button
                    onClick={currentStep === 0 ? handleSkip : handleSkipStep}
                    className={LINK_QUIET}
                    aria-label={currentStep === 0 ? "Skip onboarding" : "Skip this step"}
                  >
                    {currentStep === 0 ? "Set up later" : "Skip"}
                  </button>
                )}
              </div>
            )}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
