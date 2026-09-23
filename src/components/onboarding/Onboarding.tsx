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
import type { SttDownloadProgress, SttModelsStatus } from "@/hooks/useSttModels";

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

// Complete permissions state interface (snake_case).
// A second hand-maintained copy of this drifted once already: it was missing
// `everything_granted`, which is exactly the field that decides whether this
// step still has anything to show.
/** Only the parts of a Trigger this screen round-trips; the rest is preserved. */
interface TriggerShape {
  method: string;
  target: string;
  binding: unknown;
  [key: string]: unknown;
}

interface PermissionsState {
  accessibility: PermissionStatus;
  screen_recording: PermissionStatus;
  microphone: PermissionStatus;
  input_monitoring: PermissionStatus;
  /** Accessibility and Screen Recording only. */
  all_granted: boolean;
  /** Every permission, the optional two included. */
  everything_granted?: boolean;
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

/**
 * After an auto-grant run, raise the microphone prompt only when nothing is
 * left for the user in the rows above it: every automatable permission is
 * granted and the microphone itself is not. A failed row is the active manual
 * row at that point; stacking the mic prompt on it would show two asks at once.
 * Exported for tests.
 */
export function shouldAutoPromptMicrophone(state: PermissionsState): boolean {
  if (state.microphone.granted) return false;
  return AUTOMATABLE.every((k) => state[k].granted);
}

/** Display names for the run-level narration. */
const PERMISSION_TITLES: Record<string, string> = {
  accessibility: "Accessibility",
  screen_recording: "Screen Recording",
  microphone: "Microphone",
  input_monitoring: "Input Monitoring",
};

/** Verb per stage, for the line that stays up for the whole run. */
const AUTO_STAGE_NARRATION: Record<string, string> = {
  opening_settings: "Opening System Settings for",
  toggling: "Switching on",
  confirming: "Checking",
};

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

/** What the local Claude CLI can do right now, from the backend. */
interface ClaudeCliStatus {
  available: boolean;
  authenticated: boolean;
  email: string | null;
}

/** The dictation model onboarding offers, when the backend says to. */
interface DictationOffer {
  modelId: string;
  /** Short family name for the button ("Parakeet"). */
  family: string;
  sizeMb: number;
}

const getOnboardingSteps = (
  permissionsAlreadyGranted: boolean,
  isDevelopmentMode: boolean = false,
  apiKeysAvailable: boolean = false,
  dictationOffer: DictationOffer | null = null
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
          title: "What Juno can do",
          description:
            "Juno already works. Turn these on whenever you want it to reach outside this window, now or later, and you can turn them back off any time.",
          icon: null, // The permission checklist is the content here.
          action: "Continue",
        },
      ]),
  // Offered, never assumed: a 600 MB download is the person's call. The
  // backend decides whether to ask at all (Apple Silicon, not downloaded, not
  // declined before), so an Intel Mac or a repeat run never sees this screen.
  ...(dictationOffer
    ? [
        {
          id: "dictation-model",
          title: "Better dictation",
          description: `${dictationOffer.family} is a speech model that runs on your Mac. Faster and more accurate than the one built in, and your voice never leaves the machine.`,
          icon: null, // The model card below is the content here.
          action: `Download ${dictationOffer.family} (${dictationOffer.sizeMb} MB)`,
        },
      ]
    : []),
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
            Waiting. Switch Juno on in System Settings and this updates automatically.
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
            {/* Every row can be passed over, including the two Juno leans on
                most. Onboarding is not the moment to make someone trade
                Accessibility for a promise; the ask returns when Juno actually
                reaches for it, with the reason attached. */}
            <button onClick={onSkip} className={LINK_QUIET}>
              Later
            </button>
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
  const [cliEmail, setCliEmail] = useState<string | null>(null);
  // Which provider Juno is actually on. Read rather than assumed, because
  // when the Claude CLI is signed in the backend selects it on launch and the
  // "Connect Your AI" step never appears — so this screen is the only place
  // that says so before Settings does.
  const [activeProvider, setActiveProvider] = useState<string | null>(null);

  // Dictation model offer (see getOnboardingSteps). `sttDownload` tracks the
  // download this screen started; the download itself runs in the backend and
  // keeps going after onboarding closes.
  const [dictationOffer, setDictationOffer] = useState<DictationOffer | null>(null);
  const [sttDownload, setSttDownload] = useState<"idle" | "started" | "done" | "failed">("idle");
  const [sttProgress, setSttProgress] = useState<SttDownloadProgress | null>(null);

  // API key state
  const [apiKeysAvailable, setApiKeysAvailable] = useState(false);
  // The step list changes shape once these land, so resuming before then would
  // jump to a position that means something different a moment later.
  const [stepsSettled, setStepsSettled] = useState(false);
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
  // What the run is doing, kept for the whole run rather than cleared between
  // permissions. The per-row stage above blanks on every "granted" and only
  // ever renders on one row, so for long stretches (up to four seconds waiting
  // for the Settings window, plus retries) the screen said nothing at all
  // while Juno worked behind System Settings.
  const [autoGrantNarration, setAutoGrantNarration] = useState<string | null>(null);
  // Permissions granted this launch that Juno cannot act on until it restarts.
  // macOS says so itself with a "quit and reopen" sheet, which the auto-grant
  // run dismisses with Later so setup can continue. Nobody ever mentioned it
  // again, so Screen Recording looked switched on and behaved switched off.
  const [awaitingRelaunch, setAwaitingRelaunch] = useState<string[]>([]);
  // Whether this keyboard actually has the globe key, learned by someone
  // pressing it rather than by interrogating the hardware. "Does this machine
  // have an Fn key" has no single answer once a second keyboard is plugged in,
  // and a press proves the key reaches Juno, which no capability check can.
  const [fnOffered, setFnOffered] = useState(false);
  const [fnSaveError, setFnSaveError] = useState<string | null>(null);
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
      // Whether setup still has something to offer, which is not the same
      // question as whether Juno can act. Keyed off every permission, so the
      // optional two do not become unreachable the moment the required two land.
      const nothingLeftToOffer = payload.everything_granted ?? payload.all_granted;
      setActualPermissionsGranted((prev) =>
        prev === nothingLeftToOffer ? prev : nothingLeftToOffer
      );
    }
  );

  const adoptFnAsTalkKey = useCallback(async () => {
    setFnSaveError(null);
    try {
      const triggers = await invoke<TriggerShape[]>(COMMANDS.TRIGGERS_GET_TRIGGERS);
      // The globe key is a keyboard key, so it is a keyboard binding whose
      // shortcut string is "Fn". Which watcher can see it is the backend's
      // problem, not a second kind of binding.
      const fnBinding = { kind: "keyboard" as const, shortcut: "Fn" as const };
      const isTalkTrigger = (trigger: TriggerShape) =>
        trigger.method === "push_to_talk" && trigger.target === "dictation";
      // Also switched on, because a trigger that is off is never registered:
      // pressing the key to adopt it and then finding it does nothing is worse
      // than not offering it at all.
      let next: TriggerShape[] = triggers.map((trigger) =>
        isTalkTrigger(trigger)
          ? { ...trigger, binding: fnBinding, enabled: true }
          : trigger
      );
      // Nothing to hold means nothing to hold a key for. A store that has no
      // hold-to-talk trigger (it was deleted, or the dictation trigger is a
      // tap) used to accept this silently and the screen said the globe key
      // was in use while nothing was bound to it.
      if (!next.some(isTalkTrigger)) {
        next = [
          ...next,
          {
            method: "push_to_talk",
            target: "dictation",
            binding: fnBinding,
            phrase: null,
            require_hey_prefix: false,
            enabled: true,
          },
        ];
      }
      const saved = await invoke<TriggerShape[]>(COMMANDS.TRIGGERS_SET_TRIGGERS, {
        triggers: next,
      });
      // Believe the list that came back, not the one we sent: the backend
      // normalizes and can reject.
      const adopted = saved.some((trigger) => {
        if (!isTalkTrigger(trigger)) return false;
        const binding = trigger.binding as
          | { kind?: string; shortcut?: string }
          | null;
        return (
          binding?.kind === "keyboard" &&
          binding.shortcut?.trim().toLowerCase() === "fn"
        );
      });
      if (!mountedRef.current) return;
      if (adopted) {
        setFnOffered(true);
      } else {
        setFnSaveError("Could not switch to the globe key. You can set it in Settings.");
      }
    } catch (error) {
      console.error("[Onboarding] could not switch to the globe key:", error);
      if (mountedRef.current) {
        setFnSaveError("Could not switch to the globe key. You can set it in Settings.");
      }
    }
  }, []);

  useEventListener<{ key: string }>(EVENTS.TRIGGERS_KEY_CAPTURED, (payload) => {
    if (payload?.key === "fn") void adoptFnAsTalkKey();
  });

  const refreshRelaunchPending = useCallback(async () => {
    try {
      const pending = await invoke<string[]>(COMMANDS.PERMISSIONS_AWAITING_RELAUNCH);
      if (mountedRef.current) setAwaitingRelaunch(pending);
    } catch (error) {
      console.debug("[Onboarding] relaunch check failed:", error);
    }
  }, []);

  // Function to check current permissions status
  const checkPermissionsStatus = async () => {
    try {
      setPermissionsError(null);
      const result = await invoke<PermissionsState>(
        COMMANDS.PERMISSIONS_CHECK_PERMISSIONS_STATUS
      );
      setPermissionsState(result);
      const nothingLeftToOffer = result.everything_granted ?? result.all_granted;
      setActualPermissionsGranted(nothingLeftToOffer);
      void refreshRelaunchPending();
      return result.all_granted;
    } catch (error) {
      console.warn("Failed to check permissions status:", error);
      setPermissionsError(error as string);
      return false;
    }
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

        // Should setup offer the recommended dictation model? The backend
        // answers no on Intel, when it is already on disk, or when the person
        // declined before.
        try {
          const stt = await invoke<SttModelsStatus>(COMMANDS.STT_MODELS_GET_STATUS);
          const recommended = stt.models.find((m) => m.recommended);
          if (mounted && stt.offer_recommended && recommended && !recommended.downloaded) {
            setDictationOffer({
              modelId: recommended.id,
              family: recommended.engine === "parakeet" ? "Parakeet" : "Whisper",
              sizeMb: recommended.size_mb,
            });
          }
        } catch (error) {
          console.warn("Failed to check dictation models:", error);
        }
        if (!mounted) return;

        // Check if API keys are already available (from store or .env)
        try {
          const keysAvailable = await invoke<boolean>("check_api_keys_available");
          if (mounted) {
            setApiKeysAvailable(keysAvailable);
          }
        } catch (error) {
          console.warn("Failed to check API keys availability:", error);
        } finally {
          // Settled either way: a failed check still stops the list moving.
          if (mounted) setStepsSettled(true);
        }
        if (!mounted) return;

        // Check Claude CLI availability and auth status
        try {
          setCliChecking(true);
          const cliStatus = await invoke<ClaudeCliStatus>("check_claude_cli_available");
          if (mounted) {
            setCliAvailable(cliStatus.available);
            setCliAuthenticated(cliStatus.authenticated);
            setCliEmail(cliStatus.email ?? null);
          }
        } catch (error) {
          console.warn("Failed to check Claude CLI availability:", error);
        } finally {
          if (mounted) setCliChecking(false);
        }
        if (!mounted) return;

        try {
          const active = await invoke<string>(COMMANDS.PROVIDERS_GET_ACTIVE_PROVIDER);
          if (mounted) setActiveProvider(active);
        } catch (error) {
          console.warn("Failed to read the active provider:", error);
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
        const cliStatus = await invoke<ClaudeCliStatus>("check_claude_cli_available");
        if (!mounted) return;
        setCliAvailable(cliStatus.available);
        setCliAuthenticated(cliStatus.authenticated);
        setCliEmail(cliStatus.email ?? null);
        setActiveProvider(await invoke<string>(COMMANDS.PROVIDERS_GET_ACTIVE_PROVIDER));
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
    apiKeysAvailable,
    dictationOffer
  );

  // Progress for the download this screen started. Same events the Models
  // pane draws; the offer is just an earlier place to press Download.
  useEventListener<SttDownloadProgress>(EVENTS.STT_MODELS_DOWNLOAD_PROGRESS, (payload) => {
    if (payload.model_id === dictationOffer?.modelId) setSttProgress(payload);
  });
  useEventListener<{ model_id: string }>(EVENTS.STT_MODELS_DOWNLOAD_COMPLETE, (payload) => {
    if (payload.model_id === dictationOffer?.modelId) {
      setSttProgress(null);
      setSttDownload("done");
    }
  });
  useEventListener<{ model_id: string; cancelled: boolean }>(
    EVENTS.STT_MODELS_DOWNLOAD_ERROR,
    (payload) => {
      if (payload.model_id === dictationOffer?.modelId) {
        setSttProgress(null);
        setSttDownload("failed");
      }
    }
  );

  const startDictationDownload = useCallback(async () => {
    if (!dictationOffer) return;
    try {
      await invoke(COMMANDS.STT_MODELS_DOWNLOAD, {
        modelId: dictationOffer.modelId,
        activate: true,
      });
      setSttDownload("started");
    } catch (error) {
      console.warn("[Onboarding] dictation model download did not start:", error);
      setSttDownload("failed");
    }
  }, [dictationOffer]);

  const declineDictationOffer = useCallback(async () => {
    try {
      await invoke(COMMANDS.STT_MODELS_DECLINE_OFFER);
    } catch (error) {
      console.warn("[Onboarding] could not record the declined offer:", error);
    }
  }, []);

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
      const named = PERMISSION_TITLES[payload.permission_type ?? ""] ?? null;
      switch (payload.stage) {
        case "done":
          recordEvent("onboarding_auto_grant_finished");
          setAutoGrantStage(null);
          setAutoGrantNarration(null);
          setAutoGrantMode("finished");
          break;
        case "cancelled":
          recordEvent("onboarding_auto_grant_cancelled");
          setAutoGrantStage(null);
          setAutoGrantNarration(null);
          setAutoGrantMode("manual");
          break;
        case "failed":
          // Not fatal: the run continues to the next permission, and the
          // failed row falls back to the manual guided flow afterwards. Say
          // which one and why, rather than filing it to analytics in silence.
          recordEvent("onboarding_auto_grant_failed", {
            permission: payload.permission_type,
          });
          setAutoGrantStage(null);
          setAutoGrantNarration(
            payload.message ??
              (named
                ? `Could not switch ${named} on. You can do it yourself below.`
                : "Could not finish. You can switch these on yourself below.")
          );
          // A failure with no permission named is the whole run giving up. It
          // used to leave the checklist in its "running" state for good, which
          // greys out every row, hides every button, and spins forever. Hand
          // the list back to the person.
          if (!payload.permission_type) {
            setAutoGrantMode("manual");
            setPermIndex(firstPendingIndex(permissionsState));
          }
          break;
        case "granted":
          setAutoGrantStage(null);
          setAutoGrantNarration(named ? `${named} is on.` : "Done.");
          break;
        default:
          setAutoGrantStage({ perm: payload.permission_type, stage: payload.stage });
          setAutoGrantNarration(
            named
              ? `${AUTO_STAGE_NARRATION[payload.stage] ?? "Working on"} ${named}…`
              : "Working…"
          );
      }
    }
  );

  // Hand the checklist back after the run: re-seed to the first still pending
  // row. Only when every automatable permission landed do we also raise the
  // microphone prompt once, so the last grant is a single Allow click. If
  // anything failed, that row is now the active manual row (it comes before
  // the microphone in the checklist) and raising the mic prompt on top of it
  // would put two asks on screen at once — so we don't.
  useEffect(() => {
    if (autoGrantMode !== "finished" || !permissionsState) return;
    setPermIndex(firstPendingIndex(permissionsState));
    if (
      shouldAutoPromptMicrophone(permissionsState) &&
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
  //
  // This has to wait for `stepsSettled`. It used to run on the first render,
  // resolve "permissions" to index 2 of the full list, and set that index. The
  // list then shrank as the checks came back, and index 2 became the final
  // step, so a force quit during setup resumed straight to "You're all set"
  // with permissions still ungranted and no way back to them.
  const resumeAttemptedRef = useRef(false);
  useEffect(() => {
    if (!stepsSettled || resumeAttemptedRef.current) return;
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
  }, [onboardingSteps, stepsSettled]);

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
    // Nothing here blocks. Onboarding used to refuse to continue until
    // Accessibility and Screen Recording were granted, which asked for the two
    // most alarming switches macOS has before Juno had done anything worth
    // trusting it with. The ask now happens at the moment Juno needs one.
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

    // The offer screen's primary action is the download itself; once it is
    // running (or done) the same button reads Continue and advances.
    if (currentStepData?.id === "dictation-model" && (sttDownload === "idle" || sttDownload === "failed")) {
      await startDictationDownload();
      return;
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
  }, [currentStep, onboardingSteps, permissionsState, permIndex, detectedProvider, apiKeySaved, saveApiKey, onComplete, recordEvent, sttDownload, startDictationDownload]);

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

  // Leave the whole permissions checklist and carry on with setup. The footer
  // is hidden while a row is active, so this is the only way out of that state,
  // and "later" has to lead somewhere.
  const skipPermissionsStep = () => {
    recordEvent("onboarding_permission_skipped", { permission: "step:permissions" });
    setPermIndex(PERMISSION_FLOW.length);
    if (currentStep < onboardingSteps.length - 1) {
      setCurrentStep(currentStep + 1);
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

  // Pass over a single permission in the checklist. Any of them, including the
  // two Juno leans on: refusing to let someone move past Accessibility here is
  // what turned setup into a toll gate.
  const skipCurrentPermission = () => {
    const active = PERMISSION_FLOW[permIndex];
    if (!active) return;
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

  // Listen for the globe key only while the last screen is up, and stop as
  // soon as it is not. Outside this moment a press means "talk to Juno", not
  // "choose a key", and the monitor must not swallow that.
  //
  // This sits above the `isComplete` early return on purpose. Below it, the
  // hook disappeared from the render on the one transition that matters,
  // every exit from the last screen sets `isComplete` first, so React threw on
  // the changed hook count instead of running this cleanup. Capture stayed on
  // in the backend and the globe key did nothing, anywhere, until Juno was
  // quit. `isComplete` is a dependency rather than an early bail so finishing
  // withdraws the request the same way leaving the step does.
  const onFinalStep = currentStep === onboardingSteps.length - 1;
  const wantsCapture = onFinalStep && !isComplete;
  useEffect(() => {
    if (!wantsCapture) return;
    void invoke(COMMANDS.TRIGGERS_SET_TRIGGER_CAPTURE, { active: true }).catch((error) =>
      console.debug("[Onboarding] could not listen for the globe key:", error)
    );
    return () => {
      void invoke(COMMANDS.TRIGGERS_SET_TRIGGER_CAPTURE, { active: false }).catch(() => {});
    };
  }, [wantsCapture]);

  if (isComplete) {
    return null;
  }

  // Only on the last screen: interrupting setup halfway to restart would lose
  // the thread, and the remaining steps work fine without these.
  const needsRelaunch =
    awaitingRelaunch.length > 0 && currentStep === onboardingSteps.length - 1;
  const relaunchNames = awaitingRelaunch
    .map((key) => PERMISSION_TITLES[key] ?? key)
    .join(" and ");

  const handleRelaunch = async () => {
    // Record completion first: a restart must not reopen setup.
    try {
      await onComplete();
    } catch (error) {
      console.warn("[Onboarding] could not record completion before restart:", error);
    }
    try {
      await invoke(COMMANDS.PERMISSIONS_RESTART_AFTER_PERMISSIONS);
    } catch (error) {
      console.error("[Onboarding] restart failed:", error);
      setPermissionsError(
        "Juno could not restart itself. Quit and open it again to finish."
      );
    }
  };

  // The list can shrink underneath a resumed index. Rendering `step.id` on
  // undefined throws and takes the whole setup window with it, so clamp.
  const step =
    onboardingSteps[currentStep] ?? onboardingSteps[onboardingSteps.length - 1];

  // Checklist position.
  const permSubFlowComplete = permIndex >= PERMISSION_FLOW.length;
  // While the checklist is running, its active row owns the primary action, so
  // we hide the footer to keep exactly one primary action on screen.
  const inActivePermFlow = step.id === "permissions" && !permSubFlowComplete;

  // Determine if continue button should be disabled
  // Role step is intentionally optional — no guard here; handleNext saves only when a role is selected
  const isContinueDisabled =
    (step.id === "api-key" && !detectedProvider && !cliSelected) ||
    (step.id === "api-key" && apiKeySaving);

  // Skip stays available on the permissions step however little is granted:
  // "later" has to be a real answer, not a dead link.
  const isSkipHidden = currentStep === onboardingSteps.length - 1;

  // The offer screen: its primary action starts the download, then reads
  // Continue; "Not now" is remembered so setup never asks again. Once the
  // download is running, "Not now" no longer applies and the link goes away.
  const isDictationOffer = step.id === "dictation-model" && sttDownload === "idle";
  const primaryLabel =
    step.id === "dictation-model"
      ? sttDownload === "failed"
        ? "Try again"
        : sttDownload === "idle"
          ? step.action
          : "Continue"
      : step.action;
  const declineAndSkipDictation = () => {
    void declineDictationOffer();
    handleSkipStep();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-background"
      style={{ fontFamily: SF_FONT, WebkitFontSmoothing: "antialiased" } as CSSProperties}
    >
      {/* The onboarding window is a fixed 440×700 and cannot be resized, so the
          column may use its full height; anything that overflows scrolls, but
          every step is laid out to fit without it (the auto-grant offer's
          "I'll do it myself" link was below the fold at 90vh). */}
      <div className="max-h-full w-full max-w-[520px] overflow-y-auto px-8 py-8">
        {/* Progress: a macOS-style page control. Only the current page is dark. */}
        <div
          className={`${inActivePermFlow ? "mb-8" : "mb-12"} flex justify-center gap-2`}
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

            {/* Content. The bottom margin separates it from the footer; while
                the permission checklist owns the primary action the footer is
                hidden, so the margin goes too. */}
            <div className={`${inActivePermFlow ? "" : "mb-10"} space-y-3`}>
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
                            : "Try it. This summons Juno from anywhere."}
                        </p>
                        {/* Offered by invitation rather than by detection: if
                            this keyboard has a globe key, pressing it proves
                            it, and if it does not, nothing happens and the
                            shortcut above keeps working. Either way nobody has
                            to answer a question about their hardware. */}
                        <p className="mt-3 text-[12px] leading-snug text-muted-foreground">
                          {fnOffered
                            ? "Using the globe key to talk. You can change this in Settings."
                            : "Prefer to hold one key? Press the globe key now to use that instead."}
                        </p>
                        {/* Said once it is actually in use, not before: macOS
                            gives the globe key its own job by default, so
                            without this the emoji picker opens every time you
                            talk to Juno and the key looks broken. */}
                        {fnOffered && (
                          <p className="mt-1 text-[12px] leading-snug text-muted-foreground">
                            If the emoji picker opens too, set System Settings,
                            Keyboard, "Press globe key to" to "Do Nothing".
                          </p>
                        )}
                        {fnSaveError && (
                          <p className="mt-1 text-[12px] text-destructive" role="alert">
                            {fnSaveError}
                          </p>
                        )}
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

                  {/* The person on a Claude subscription is never asked to
                      connect anything, so this is where they find out what
                      they are connected to. One line, and where to change it.
                      Shown only once the backend confirms it, so it can never
                      claim a subscription that is not being used. */}
                  {activeProvider === "claude_cli" && cliAuthenticated && (
                    <p className="mt-4 text-[12px] leading-snug text-muted-foreground">
                      {cliEmail
                        ? `Using your Claude subscription (${cliEmail}). No API key needed. You can change this in Settings.`
                        : "Using your Claude subscription. No API key needed. You can change this in Settings."}
                    </p>
                  )}
                </div>
              )}

              {/* Dictation model offer: one card, one number, one decision. */}
              {step.id === "dictation-model" && dictationOffer && (
                <div className="pt-4 text-left">
                  <div className="rounded-xl border border-border bg-card p-4">
                    <div className="flex items-baseline justify-between gap-3">
                      <span className="text-[14px] font-semibold text-foreground">
                        {dictationOffer.family}
                      </span>
                      <span className="text-[12px] tabular-nums text-muted-foreground">
                        {dictationOffer.sizeMb} MB
                      </span>
                    </div>
                    <p className="mt-1 text-[12px] leading-snug text-muted-foreground">
                      Runs on your Mac. Until it lands, dictation keeps using the built-in
                      model; Juno switches over on its own.
                    </p>
                    {sttDownload === "started" && (
                      <div className="mt-3 space-y-1" role="status">
                        <div className="h-1.5 w-full overflow-hidden rounded-full bg-foreground/[0.08]">
                          <div
                            className="h-full rounded-full bg-[#007AFF] transition-[width] duration-300"
                            style={{ width: `${Math.min(100, Math.max(0, sttProgress?.percent ?? 0))}%` }}
                          />
                        </div>
                        <p className="text-[11px] tabular-nums text-muted-foreground">
                          {sttProgress
                            ? `${Math.round(sttProgress.percent)}% downloaded. You can keep going.`
                            : "Starting download. You can keep going."}
                        </p>
                      </div>
                    )}
                    {sttDownload === "done" && (
                      <p className="mt-3 text-[12px] text-muted-foreground" role="status">
                        Downloaded. {dictationOffer.family} is now your dictation model.
                      </p>
                    )}
                    {sttDownload === "failed" && (
                      <p className="mt-3 text-[12px] text-destructive" role="alert">
                        The download did not finish. Try again now, or later in Settings, Models.
                      </p>
                    )}
                  </div>
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
                          No API key needed. Uses Claude Code CLI authentication.
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
                              <AlertCircle className="h-3 w-3" /> Installed but not signed in. Run{" "}
                              <code className="rounded bg-muted px-1 py-0.5 font-mono text-[11px]">claude login</code>
                            </span>
                          ) : (
                            <a
                              href="https://claude.ai/code"
                              target="_blank"
                              rel="noopener noreferrer"
                              className="inline-flex items-center gap-1.5 text-muted-foreground transition-colors hover:text-foreground"
                            >
                              <ExternalLink className="h-3 w-3" /> Not installed. Get Claude Code
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
                        <CheckGlyph className={`h-3 w-3 ${GREEN}`} /> Saved. {detectedProvider?.name} is your active provider
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
                <div className="pt-2">
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
                        <div className="mt-4 flex flex-col items-center gap-2">
                          <p className="text-[12px] leading-[1.4] text-muted-foreground">
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
                        <div className="mt-4 flex flex-col items-center gap-2">
                          <p className="flex items-center gap-1.5 text-[12px] text-muted-foreground">
                            <Loader2 className="h-3 w-3 animate-spin" aria-hidden />
                            {autoGrantNarration ?? "Juno is switching these on in System Settings."}
                          </p>
                          <p className="text-[11px] text-muted-foreground/70">
                            System Settings will come to the front while this happens.
                          </p>
                          <button onClick={stopAutoGrant} className={LINK_QUIET}>
                            Stop
                          </button>
                        </div>
                      )}
                      {autoGrantMode === "manual" && autoGrantNarration && (
                        <p className="mt-3 text-[12px] text-muted-foreground" role="status">
                          {autoGrantNarration}
                        </p>
                      )}
                      {permSubFlowComplete ? (
                        <p className="mt-3 text-[12px] text-muted-foreground">
                          You can change these anytime in System Settings.
                        </p>
                      ) : (
                        // The footer is hidden while a row owns the primary
                        // action, so the way out lives here. Without it, "later"
                        // means abandoning the app.
                        <div className="mt-4 flex justify-center">
                          <button onClick={skipPermissionsStep} className={LINK_QUIET}>
                            I'll do this later
                          </button>
                        </div>
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
                {/* Some grants only take hold after a restart, and macOS
                    already offered one that Juno declined on the person's
                    behalf so setup could finish. This is Juno paying that
                    back, at the one moment a restart costs nothing. */}
                {needsRelaunch && (
                  <p className="max-w-[320px] text-center text-[12px] leading-snug text-muted-foreground">
                    {relaunchNames} {awaitingRelaunch.length > 1 ? "need" : "needs"} a
                    restart before Juno can use {awaitingRelaunch.length > 1 ? "them" : "it"}.
                  </p>
                )}
                <button
                  onClick={needsRelaunch ? handleRelaunch : handleNext}
                  disabled={isContinueDisabled}
                  className={`${BTN_PRIMARY} ${isContinueDisabled ? "cursor-not-allowed" : ""}`}
                  aria-label={needsRelaunch ? "Restart Juno" : primaryLabel}
                >
                  {needsRelaunch ? "Restart Juno" : primaryLabel}
                </button>

                {!isSkipHidden && (
                  <button
                    onClick={
                      currentStep === 0
                        ? handleSkip
                        : isDictationOffer
                          ? declineAndSkipDictation
                          : handleSkipStep
                    }
                    className={LINK_QUIET}
                    aria-label={
                      currentStep === 0
                        ? "Skip onboarding"
                        : isDictationOffer
                          ? "Not now"
                          : "Skip this step"
                    }
                  >
                    {currentStep === 0 ? "Set up later" : isDictationOffer ? "Not now" : "Skip"}
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
