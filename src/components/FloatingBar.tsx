/**
 * FloatingBar — the default bar appearance.
 *
 * Wispr-Flow-shaped: a very small dark pill while idle, which grows on
 * hover to reveal a mic and a type button. A drag anywhere on the pill
 * (buttons included) moves the window; a click without movement acts on
 * what was clicked. The mic starts a spoken query to the agent, the type
 * button opens the text input focused. Once a query is in flight the pill
 * takes its full width and the conversation pane opens underneath.
 *
 * State comes from the backend UIManager (`bar-state-update`); the only
 * local state is hover, whether the input is open, and the conversation.
 */

import {
  lazy,
  Suspense,
  useEffect,
  useLayoutEffect,
  useMemo,
  useState,
  useCallback,
  useRef,
  useSyncExternalStore,
  FormEvent,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  availableMonitors,
  cursorPosition,
  getCurrentWindow,
} from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ArrowUp, Ear, EarOff, Maximize2, Mic, Square, Type, X } from "lucide-react";
import { VoiceTurnControls } from "@/components/bar/VoiceTurnControls";
import { cancelVoiceTurn, isRecordingTurn, sendVoiceTurn } from "@/lib/voiceTurn";

import { isSendKey, useAutoGrowTextarea } from "@/hooks/useAutoGrowTextarea";
import { useAgentSessions } from "@/hooks/useAgentSessions";
import { useBarConversation } from "@/hooks/useBarConversation";
import { useEventListener } from "@/hooks/useEventListener";
import { useEscapeToIdle } from "@/hooks/useEscapeToIdle";
import { BarFlameBorder } from "@/components/bar/BarFlameBorder";
import { BAR_DEPTH_GLOW } from "@/components/bar/barAppearance";
import { cn } from "@/lib/utils";
import { COMMANDS, EVENTS, UI } from "@/lib/constants.generated";
import { drivingLabel, type InputControlStatePayload } from "@/lib/inputControl";
import { nearestWell, wellForSlot, SLOT, type Well } from "@/lib/snapWells";
import { monitorIndexAt, setDockSlot } from "@/lib/barDock";
import {
  contentPlacement,
  contentRect,
  getSteady,
  registerSteady,
  roomForContent,
  setSteadyFootprint,
  setSteadyLayout,
  steadyLayout,
  subscribeSteady,
  type Rect,
  type SteadyLayout,
  type SteadySpec,
} from "@/lib/steadyFrame";
import {
  applySteadyFrame,
  steadyWells,
  toMonitorRects,
  windowOrigin,
} from "@/hooks/useBarSnapWells";
import { cursorInPoints } from "@/lib/desktopPoints";
import { useBarDrag } from "@/hooks/useDragWindow";
import { AgentRosterStrip } from "./AgentRosterStrip";
import { PillTooltip, PILL_TOOLTIP_DELAY_MS } from "@/components/bar/PillTooltip";
import { TooltipProvider } from "@/components/ui/tooltip";
import { shortcutCaps } from "@/components/settings/KeyCaps";
import type { TriggerHints } from "@/components/onboarding/Onboarding";
import { useConnectivity } from "@/hooks/useConnectivity";
import { useStartupReadiness } from "@/hooks/useStartupReadiness";
import { takeBootBarPosition } from "@/lib/barBoot";
import { DOT_COLORS, dotLabel, dotTone, type Connectivity } from "@/lib/pillStatus";
// The conversation pane renders markdown, maths and diagrams: most of the
// bar's script, and none of it needed to draw the pill. It loads after the
// bar is on screen (`warmChatPane`), so the first open is still instant.
const loadChatPane = () => import("./bar/BarChatPane");
const BarChatPane = lazy(() => loadChatPane().then((m) => ({ default: m.BarChatPane })));
/** How long after the pill is placed its pane's code is fetched. */
const CHAT_PANE_WARM_MS = 1500;
/** Fetch the pane's code in the background, once the pill has been placed. */
function warmChatPane() {
  void loadChatPane().catch((error) => console.debug("FloatingBar: pane preload failed:", error));
}
import type { BarAppearance } from "@/components/bar/barAppearance";

/**
 * UI State enumeration — values emitted by the backend UIManager in
 * BAR_STATE_UPDATE events.
 */
type UIState =
  | typeof UI.BAR_STATES_DEFAULT
  | typeof UI.BAR_STATES_EXPANDING
  | typeof UI.BAR_STATES_INPUT
  | typeof UI.BAR_STATES_SHRINKING
  | typeof UI.BAR_STATES_SUBMITTING
  | typeof UI.BAR_STATES_LOADING
  | typeof UI.BAR_STATES_FINISHING
  | typeof UI.BAR_STATES_SUCCESS
  | typeof UI.BAR_STATES_LISTENING
  | typeof UI.BAR_STATES_ERROR
  | typeof UI.BAR_STATES_TRANSCRIBING
  | typeof UI.BAR_STATES_SPEAKING
  | typeof UI.BAR_STATES_DICTATING
  | typeof UI.BAR_STATES_DICTATION_READY
  | typeof UI.BAR_STATES_ALWAYS_LISTENING
  | typeof UI.BAR_STATES_AGENT_RESPONDING
  | typeof UI.BAR_STATES_STOPPING;

/** The state object emitted by the backend; shared by every bar appearance. */
interface BarStateData {
  barState: UIState;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;
  transcriptionText: string;
  /** True while transcriptionText is a live streaming partial (render dimmed). */
  transcriptionProvisional?: boolean;
  spokenText: string;
  voiceMode: string;
  audioLevel: number;
  isAgentWorking: boolean;
  isDictationMode: boolean;
  isAlwaysListening: boolean;
  agentState: string | null;
}

/** Matches UIInteractionEvent in ui_commands.rs */
interface UIInteractionEvent {
  element_id: string;
  interaction_type: string;
  data: Record<string, any> | null;
  timestamp: number;
}

// === LAYOUT ===

export type BarLayout = "compact" | "hover" | "voice" | "status" | "full";

/**
 * Pill size per layout. The pill grows and shrinks in CSS inside a window that
 * never changes size (see `lib/steadyFrame.ts`); only the footprint around it,
 * the pill plus BAR_PAD on every side, takes the mouse.
 */
export const BAR_LAYOUTS: Record<BarLayout, { width: number; height: number }> = {
  compact: { width: 56, height: 16 },
  // Wider than the three buttons alone need: the status dot is always present
  // at its fixed home (see DOT_HOME_LEFT) even in hover, so the row starts
  // past it. The optional wake-phrase button is added on top via
  // pillExtraWidth.
  hover: { width: 148, height: 34 },
  // Voice and status share a width on purpose. Listening used to open a 220px
  // bar and then, the instant the mic closed, a 419px one, for a status word
  // and a stop button. The extra 200px held nothing, and the jump happened
  // mid-sentence, every time.
  voice: { width: 260, height: 34 },
  status: { width: 260, height: 34 },
  full: { width: 419, height: 44 },
};

/**
 * The room around the pill (for its shadow, and the hover margin), and the
 * fixed vertical band the pill is centred in. One value each, for EVERY layout,
 * so the band's near edge (the top, or the bottom when the pane opens upward)
 * is the same in every state and the pill grows around the band's centre.
 */
export const BAR_PAD = 16;
export const BAR_BAND = 44;

/**
 * The status dot's fixed home, and the left inset every flowing content block
 * (buttons, label, composer) uses to clear it. The dot is absolutely positioned
 * at DOT_HOME_LEFT in EVERY layout, so it never reflows and never teleports:
 * DOT_HOME_LEFT is chosen to centre it in the 56px compact pill, and reused
 * verbatim when the pill opens, so growing from compact reveals the label to
 * the dot's right while the dot itself stays put. CONTENT_LEAD = home + dot +
 * gap. This is the vertical-centring idea applied to the horizontal axis: one
 * fixed origin the pill grows around, instead of a per-layout alignment that
 * flips centre<->left.
 */
export const DOT_HOME_LEFT = 23;
export const CONTENT_LEAD = 38;

export const FLOATING_BAR_DIMENSIONS = {
  ROSTER_STRIP_HEIGHT: 34, // 22px strip + 6px gap + breathing room (LAC-2830 §3)
  PANE_GAP: 8,
  PANE_HEIGHT: 360,
};

/**
 * A mouse-leave is only believed after this long, once the cursor has been
 * checked against what the pill is drawing: a leave can be reported for a
 * cursor that is still over it (the pill growing under a resting cursor).
 */
export const LEAVE_VERIFY_MS = 120;
/** An input blur is only acted on after this long, once focus has settled. */
export const BLUR_SETTLE_MS = 80;

/**
 * Is the cursor over what the pill is drawing right now? Used to verify a
 * mouse-leave. `content` is the drawn footprint in logical pixels from the
 * window's top-left; the window itself is mostly transparent room.
 */
async function cursorInsideContent(content: Rect | null): Promise<boolean> {
  if (!content) return false;
  const w = getCurrentWindow();
  // In global points: the cursor and the window are scaled by different
  // displays' factors in Tauri's raw numbers (see `src/lib/desktopPoints.ts`).
  const [c, p, mons] = await Promise.all([cursorPosition(), windowOrigin(w), availableMonitors()]);
  const cursor = cursorInPoints(c, mons);
  const x = cursor.x - p.x;
  const y = cursor.y - p.y;
  return (
    x >= content.x &&
    x < content.x + content.width &&
    y >= content.y &&
    y < content.y + content.height
  );
}

/** Everything the pill's footprint depends on. */
export interface PillFrame {
  layout: BarLayout;
  paneOpen: boolean;
  rosterVisible: boolean;
  /** Extra height the typed text needs beyond a single line. */
  composerGrowth: number;
  /** Extra width for a control the layout does not always carry. */
  extraWidth: number;
}

/**
 * The pill's footprint for a frame: the pill (or the pane, which is as wide
 * as the full pill) plus BAR_PAD each side, with the roster and pane stacked
 * in the column. This is the part of the window that is drawn and takes the
 * mouse; the rest is transparent and lets clicks through. It is exactly the
 * window the bar used to resize to on every state change.
 */
export function pillFootprint({
  layout,
  paneOpen,
  rosterVisible,
  composerGrowth = 0,
  extraWidth = 0,
  paneHeight = FLOATING_BAR_DIMENSIONS.PANE_HEIGHT,
}: {
  layout: BarLayout;
  paneOpen: boolean;
  rosterVisible: boolean;
  composerGrowth?: number;
  extraWidth?: number;
  paneHeight?: number;
}) {
  const l = BAR_LAYOUTS[layout];
  const d = FLOATING_BAR_DIMENSIONS;
  return {
    width: l.width + Math.max(0, extraWidth) + 2 * BAR_PAD,
    height:
      BAR_BAND +
      Math.max(0, composerGrowth) +
      2 * BAR_PAD +
      (rosterVisible ? d.ROSTER_STRIP_HEIGHT : 0) +
      (paneOpen ? d.PANE_GAP + paneHeight : 0),
  };
}

/**
 * One pill button and the gap beside it. The hover pill is sized for three
 * controls; the wake-phrase toggle is a fourth and only exists when a voice
 * trigger does, so the pill is told to make room for it rather than clipping.
 */
export const BAR_PILL_BUTTON_PX = 32;

/** One line of the bar's composer, and the most it may grow to. */
export const BAR_COMPOSER_LINE_PX = 18;
export const BAR_COMPOSER_MAX_PX = 96;

/**
 * Everything the column holds besides the pane: the pads, the band, the gap,
 * and whatever the composer and roster are taking right now.
 */
function pillColumnReserve(composerGrowth: number, rosterVisible: boolean): number {
  return (
    2 * BAR_PAD +
    BAR_BAND +
    Math.max(0, composerGrowth) +
    (rosterVisible ? FLOATING_BAR_DIMENSIONS.ROSTER_STRIP_HEIGHT : 0) +
    FLOATING_BAR_DIMENSIONS.PANE_GAP
  );
}

/** The pane is never shorter than this, even on a display too small for the full one. */
export const PILL_MIN_PANE_PX = 120;

/**
 * The Pill's steady frame. The window is the largest footprint any state
 * reaches (the full pill with the composer grown, the roster and the pane)
 * and never changes size; wells are computed for the resting compact
 * footprint, so the idle pill sits exactly where it always has.
 */
export const PILL_STEADY_SPEC: SteadySpec = {
  rest: pillFootprint({ layout: "compact", paneOpen: false, rosterVisible: false }),
  max: {
    // One spare pixel so the room either side of the resting footprint is
    // even, and a pill in a centre well sits on whole pixels.
    width: BAR_LAYOUTS.full.width + 2 * BAR_PAD + 1,
    height: pillColumnReserve(BAR_COMPOSER_MAX_PX, true) + FLOATING_BAR_DIMENSIONS.PANE_HEIGHT,
  },
  stage: { width: BAR_LAYOUTS.full.width + 2 * BAR_PAD, height: BAR_BAND + 2 * BAR_PAD },
};

/**
 * The pane's height in this well. Full height wherever there is room for it,
 * which is every well on an ordinary display. Where the window had to be
 * clamped (a middle well on a short laptop screen) the pane gives up only the
 * room the composer's extra lines or the roster are actually using, so nothing
 * is ever clipped and the pill itself never moves.
 */
export function pillPaneHeight(
  layout: SteadyLayout,
  { composerGrowth = 0, rosterVisible = false }: { composerGrowth?: number; rosterVisible?: boolean } = {},
): number {
  return Math.max(
    PILL_MIN_PANE_PX,
    Math.min(
      FLOATING_BAR_DIMENSIONS.PANE_HEIGHT,
      roomForContent(layout) - pillColumnReserve(composerGrowth, rosterVisible),
    ),
  );
}

/** Where the pill draws before its launch placement has landed (the window is still hidden). */
const PILL_FALLBACK_LAYOUT: SteadyLayout = steadyLayout(
  { x: 0, y: 0, width: 0, height: 0, fx: 1, fy: 0, monitorIndex: 0 },
  { position: { x: -1e6, y: -1e6 }, size: { width: 2e6, height: 2e6 }, scaleFactor: 1 },
  PILL_STEADY_SPEC,
);

/** Component name for backend interactions — MUST match backend element ids */
const COMPONENT_ID = UI.ELEMENT_IDS_FLOATING_BAR;

const IDLE_STATES: readonly string[] = [
  UI.BAR_STATES_DEFAULT,
  UI.BAR_STATES_DICTATION_READY,
  UI.BAR_STATES_SHRINKING,
];

const INPUT_STATES: readonly string[] = [
  UI.BAR_STATES_INPUT,
  UI.BAR_STATES_EXPANDING,
];

const VOICE_STATES: readonly string[] = [
  UI.BAR_STATES_LISTENING,
  UI.BAR_STATES_DICTATING,
  UI.BAR_STATES_ALWAYS_LISTENING,
];

// Transcribing sits here on purpose: once the mic closes, the bar shows a
// processing state (STT is finalizing, the agent is about to start) rather
// than the listening look, straight through to the agent's own working state.
const WORKING_STATES: readonly string[] = [
  UI.BAR_STATES_TRANSCRIBING,
  UI.BAR_STATES_SUBMITTING,
  UI.BAR_STATES_LOADING,
  UI.BAR_STATES_AGENT_RESPONDING,
  UI.BAR_STATES_FINISHING,
  UI.BAR_STATES_STOPPING,
];

/**
 * The activity-indicator flame for a bar state: which colour, how bright, and
 * whether it breathes. `null` means no flame (idle). The colour/intensity are a
 * small language:
 *   blue  = Juno's own listening / working    green = your dictation input
 *   red   = error                             dim   = ambient, bright = engaged
 *   pulse = ongoing work (thinking / processing / ambient wait)
 * Named states win over the generic working fallback, because TRANSCRIBING is
 * itself a working state but belongs to the your-input (green) family.
 */
export function flameForState(
  state: string,
  isWorking: boolean,
  sessionColor: string,
): { color: string; intensity: number; pulse: boolean } | null {
  switch (state) {
    case UI.BAR_STATES_DICTATING:
      return { color: "#30D158", intensity: 0.2, pulse: false }; // green: your speech -> text
    case UI.BAR_STATES_TRANSCRIBING:
      return { color: "#30D158", intensity: 0.13, pulse: true }; // green breathe: converting your speech
    case UI.BAR_STATES_LISTENING:
      return { color: "#0A84FF", intensity: 0.2, pulse: false }; // blue: listening to your request
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return { color: "#0A84FF", intensity: 0.07, pulse: true }; // dim blue breathe: ambient wait
    case UI.BAR_STATES_ERROR:
      return { color: "#FF453A", intensity: 0.24, pulse: false }; // red: something failed
    default:
      break;
  }
  if (isWorking) {
    // Agent working: the focused session's own colour, breathing.
    return { color: sessionColor, intensity: 0.22, pulse: true };
  }
  return null;
}

/** Is a point within a rect? Exported for the hover hit-test tests. */
export function pointInRect(
  x: number,
  y: number,
  rect: { left: number; top: number; right: number; bottom: number },
): boolean {
  return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
}

/** The dot's hover target where it is drawn now, or null when it is not drawn. */
function idleDotRect(el: HTMLElement | null): DOMRect | null {
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return r.width === 0 && r.height === 0 ? null : r;
}

/** Which layout a combination of backend state and local UI state gets. */
export function pickLayout({
  state,
  hovered,
  inputOpen,
  paneOpen,
  rosterVisible,
  driving = false,
}: {
  state: string;
  hovered: boolean;
  inputOpen: boolean;
  paneOpen: boolean;
  rosterVisible: boolean;
  /** Juno holds the physical cursor; the bar has to say so in words. */
  driving?: boolean;
}): BarLayout {
  if (driving) return "full";
  if (paneOpen || rosterVisible || inputOpen || INPUT_STATES.includes(state)) return "full";
  if (VOICE_STATES.includes(state)) return "voice";
  if (IDLE_STATES.includes(state)) return hovered ? "hover" : "compact";
  // Working, error, success, speaking: a label and one control, which is the
  // same room listening needs, so the bar does not lurch wider the moment
  // someone stops talking.
  return "status";
}

// Purpose-built motions for the status dot; injected once into <head>.
const BAR_KEYFRAMES = `
@keyframes fbar-idle {
  0%, 100% { opacity: 0.4; transform: scale(1); }
  50%      { opacity: 0.6; transform: scale(1.08); }
}
/* Still loading after launch: a slow, shallow pulse. No scale, no glow; the
   dot only fades. Reduce Motion stops it at the inline 0.6. */
@keyframes fbar-warm {
  0%, 100% { opacity: 0.35; }
  50%      { opacity: 0.9; }
}
@keyframes fbar-breathe {
  0%, 100% { opacity: 0.6; transform: scale(1); }
  50%      { opacity: 1; transform: scale(1.25); }
}
@keyframes fbar-orbit {
  0%   { transform: translateX(0) scale(1); }
  25%  { transform: translateX(4px) scale(0.92); }
  50%  { transform: translateX(0) scale(1); }
  75%  { transform: translateX(-4px) scale(0.92); }
  100% { transform: translateX(0) scale(1); }
}
@keyframes fbar-ripple {
  0%   { box-shadow: 0 0 0 0 rgba(255,255,255,0.14); }
  100% { box-shadow: 0 0 0 8px rgba(255,255,255,0); }
}
@keyframes fbar-shake {
  0%, 100% { transform: translateX(0); }
  20%  { transform: translateX(-2px); }
  40%  { transform: translateX(2px); }
  60%  { transform: translateX(-1px); }
  80%  { transform: translateX(1px); }
}
@keyframes fbar-content-in {
  0%   { opacity: 0; transform: translateY(-4px); }
  100% { opacity: 1; transform: translateY(0); }
}
@keyframes fbar-reveal {
  0%   { opacity: 0; transform: scale(0.85); }
  100% { opacity: 1; transform: scale(1); }
}

/* One guard for every animation the bar injects, rather than a check at each
   call site: Reduce Motion is a system setting, not a per-component choice. */
@media (prefers-reduced-motion: reduce) {
  [class*="fbar-"], [style*="fbar-"] { animation: none !important; }
}
`;

// === STATUS DOT ===
// One dot; state is communicated through motion and colour, not icons. The
// colour says one of three things (see `lib/pillStatus.ts`): fine (neutral),
// listening (system blue), or Juno cannot answer right now (system red).
// Nothing else gets a colour, so red is never confused with a passing error
// and blue is never anything but an open microphone or Juno at the pointer.

/** macOS system blue, the only accent the bar uses. */
const SYSTEM_BLUE = "#0A84FF";

/** The dot's motion for a bar state. Motion says what Juno is doing. */
function dotMotion(state: UIState, voicePaused: boolean): string | undefined {
  switch (state) {
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_ALWAYS_LISTENING:
    case UI.BAR_STATES_DICTATING:
      return "fbar-breathe 1.4s ease-in-out infinite";
    case UI.BAR_STATES_TRANSCRIBING:
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_STOPPING:
      return "fbar-orbit 1.1s ease-in-out infinite";
    case UI.BAR_STATES_SPEAKING:
      return "fbar-ripple 1.4s ease-out infinite";
    case UI.BAR_STATES_ERROR:
      return "fbar-shake 0.4s ease-out";
    case UI.BAR_STATES_SUCCESS:
    case UI.BAR_STATES_INPUT:
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_DICTATION_READY:
      return undefined;
    default:
      // At rest the dot carries whether Juno is listening for its wake
      // phrase: armed keeps the slow idle breath, paused is still.
      return voicePaused ? undefined : "fbar-idle 4s ease-in-out infinite";
  }
}

/** The neutral dot's opacity per state, unchanged from before colour meant status. */
function neutralShade(state: UIState, voicePaused: boolean): string {
  switch (state) {
    case UI.BAR_STATES_TRANSCRIBING:
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_STOPPING:
    case UI.BAR_STATES_SPEAKING:
    case UI.BAR_STATES_ERROR:
      return "rgba(255,255,255,0.8)";
    case UI.BAR_STATES_INPUT:
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_DICTATION_READY:
      return "rgba(255,255,255,0.7)";
    case UI.BAR_STATES_SUCCESS:
      return "#ffffff";
    default:
      return voicePaused ? "rgba(255,255,255,0.3)" : "#ffffff";
  }
}

/** The dot's fill: what `dotTone` decided, or the neutral shade for the state. */
export function statusDotColor(
  state: UIState,
  connectivity: Connectivity | null,
  {
    driving = false,
    voicePaused = false,
    loading = false,
  }: { driving?: boolean; voicePaused?: boolean; loading?: boolean } = {},
): string {
  if (driving) return SYSTEM_BLUE;
  const tone = dotTone(state, connectivity, loading);
  return tone === "neutral" ? neutralShade(state, voicePaused) : DOT_COLORS[tone];
}

export function StatusDot({
  state,
  audioLevel,
  connectivity,
  driving = false,
  voicePaused = false,
  loading = false,
}: {
  state: UIState;
  audioLevel: number;
  connectivity: Connectivity | null;
  driving?: boolean;
  /** A wake phrase is configured, but its engine is paused right now. */
  voicePaused?: boolean;
  /** Just launched: what the first request needs is still loading. */
  loading?: boolean;
}) {
  const tone = driving ? "listening" : dotTone(state, connectivity, loading);
  const listening = tone === "listening" && !driving;
  // One element in every state, the same size, so nothing about the dot
  // moves or reflows when its meaning changes (#511); only the fill and the
  // motion do. The fill cross-fades.
  return (
    <div
      role="img"
      aria-label={dotLabel(state, connectivity, { driving, voicePaused, loading })}
      data-testid={
        driving
          ? "floating-bar-driving-dot"
          : voicePaused && state === UI.BAR_STATES_DEFAULT
            ? "floating-bar-voice-paused"
            : "floating-bar-status-dot"
      }
      data-tone={tone}
      className="size-[7px] shrink-0 rounded-full transition-[background-color] duration-200 ease-out motion-reduce:transition-none"
      style={{
        backgroundColor: statusDotColor(state, connectivity, { driving, voicePaused, loading }),
        // Down is a fact, not a mood: a steady, full-strength red. The resting
        // breath would dim it to half.
        animation: driving
          ? "fbar-orbit 1.1s ease-in-out infinite"
          : tone === "loading"
            ? "fbar-warm 1.6s ease-in-out infinite"
            : tone === "down" && dotMotion(state, voicePaused)?.startsWith("fbar-idle")
              ? undefined
              : dotMotion(state, voicePaused),
        opacity: listening ? Math.max(0.5, audioLevel) : tone === "loading" ? 0.6 : undefined,
      }}
    />
  );
}

/** Real-time audio feedback while a microphone is open. */
function AudioLevelBars({ audioLevel }: { audioLevel: number }) {
  const normalizedLevel = Math.min(Math.max(audioLevel * 100, 0), 100);
  const barCount = Math.ceil(normalizedLevel / 20);
  return (
    <div className="flex shrink-0 items-center gap-0.5" aria-hidden="true">
      {[...Array(5)].map((_, i) => (
        <div
          key={i}
          className={cn(
            "h-2 w-0.5 rounded-full transition-all duration-100",
            i < barCount ? "bg-white/80" : "bg-white/20",
          )}
        />
      ))}
    </div>
  );
}

/**
 * A trigger hint as tooltip detail: the gesture and the key caps, "Hold ⌃⌥".
 * Null when nothing keyboard-shaped is bound.
 */
export function hintDetail(hint: { shortcut: string; gesture: string } | null): string | null {
  if (!hint || !hint.shortcut.trim()) return null;
  const caps = shortcutCaps(hint.shortcut);
  if (caps.length === 0) return null;
  return `${hint.gesture} ${caps.map((cap) => cap.glyph).join("")}`;
}

/** Lower-case status label for the non-input states; null when none applies. */
function statusLabel(state: UIState, data: BarStateData): string | null {
  switch (state) {
    case UI.BAR_STATES_LISTENING:
      return "listening";
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return "always listening";
    case UI.BAR_STATES_TRANSCRIBING:
      return data.transcriptionText || "transcribing";
    case UI.BAR_STATES_DICTATING:
      return "dictating";
    case UI.BAR_STATES_DICTATION_READY:
      return "dictation ready";
    case UI.BAR_STATES_SPEAKING: {
      const t = data.spokenText;
      return t ? (t.length > 40 ? `${t.slice(0, 40)}…` : t) : "speaking";
    }
    case UI.BAR_STATES_SUBMITTING:
      return "sending";
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return data.agentState || "working";
    case UI.BAR_STATES_FINISHING:
      return "finishing";
    case UI.BAR_STATES_STOPPING:
      return "stopping";
    case UI.BAR_STATES_ERROR:
      return data.currentError || "something went wrong";
    case UI.BAR_STATES_SUCCESS:
      return "done";
    default:
      return null;
  }
}

const pillButton =
  "flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-full text-white/60 transition-colors hover:bg-white/[0.12] hover:text-white";

/** The smaller controls that sit inside the text input and the voice row. */
const inputControlButton =
  "flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-full text-white/55 transition-colors hover:bg-white/[0.16] hover:text-white";

// === MAIN COMPONENT ===

export function FloatingBar(_props: { barAppearance?: BarAppearance }) {
  // Inject keyframes once
  useEffect(() => {
    const id = "floating-bar-keyframes";
    if (!document.getElementById(id)) {
      const style = document.createElement("style");
      style.id = id;
      style.textContent = BAR_KEYFRAMES;
      document.head.appendChild(style);
    }
  }, []);

  // === BACKEND-DRIVEN BAR STATE (single source of truth) ===

  const [barState, setBarState] = useState<BarStateData>({
    barState: UI.BAR_STATES_DEFAULT,
    inputValue: "",
    lastSubmittedValue: "",
    currentError: null,
    transcriptionText: "",
    spokenText: "",
    isAgentWorking: false,
    isDictationMode: false,
    isAlwaysListening: false,
    audioLevel: 0,
    voiceMode: UI.VOICE_MODES_IDLE,
    agentState: null,
  });

  const windowLabel = getCurrentWindow().label;

  // === THE STEADY FRAME ===
  //
  // The Pill's window is sized once for its largest state and placed once per
  // well (`lib/steadyFrame.ts`). Every state change below is a CSS transition
  // inside that window; the window itself never moves or resizes for one. That
  // is the fix for the jump: a resize lands in AppKit and in WebKit on
  // different display frames, and nothing anchored to a right or bottom edge
  // survives that. Now there is no resize to survive.
  //
  // The layout (where the window is, where the docked corner sits inside it,
  // which way the pill grows) is recorded by whoever places the window: the
  // launch placement below, the settle after a drag and the display hop in
  // `useBarSnapWells`. The pill draws from it, so the frame and the drawing
  // never disagree about the well.
  useLayoutEffect(() => {
    registerSteady(windowLabel, PILL_STEADY_SPEC);
    return () => {
      registerSteady(windowLabel, null);
      // Another look is taking the window: it gets every mouse event again.
      void invoke(COMMANDS.BAR_SET_BAR_HIT_REGIONS, { regions: null }).catch(() => {});
    };
  }, [windowLabel]);
  const subscribeLayout = useCallback(
    (fn: () => void) => subscribeSteady(windowLabel, fn),
    [windowLabel],
  );
  const readSteady = useCallback(() => getSteady(windowLabel), [windowLabel]);
  const steady = useSyncExternalStore(subscribeLayout, readSteady, readSteady);
  const dockLayout = steady?.layout ?? PILL_FALLBACK_LAYOUT;
  const growUp = dockLayout.growUp;
  const anchorX = dockLayout.anchorX;
  // A swap to a well that grows the other way is drawn hidden for two frames.
  const swapping = steady?.hidden ?? false;
  // The footprint the pill is drawing, logical, from the window's top-left. The
  // hit test and the leave check both read it.
  const contentRef = useRef<Rect | null>(null);
  // Nothing is reported to the hit test until the launch placement has put the
  // window in its well.
  const [placed, setPlaced] = useState(false);

  /**
   * Listen to BAR_STATE_UPDATE directly and to the component-specific DOM
   * event the multi-bar registration system forwards.
   */
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let mounted = true;

    const handleStateUpdate = (payload: any) => {
      if (!mounted) return;
      if (payload && typeof payload === "object" && "barState" in payload) {
        setBarState(payload);
      } else {
        console.error("❌ FloatingBar: Invalid state data received:", payload);
      }
    };

    const domHandler = ((e: CustomEvent) => handleStateUpdate(e.detail)) as EventListener;

    const setupListener = async () => {
      try {
        const fn = await listen<BarStateData>(EVENTS.BAR_STATE_UPDATE, (event) =>
          handleStateUpdate(event.payload),
        );
        if (mounted) unlisten = fn;
        else fn();
        document.addEventListener(`${COMPONENT_ID}-state-update`, domHandler);
      } catch (error) {
        console.error("❌ FloatingBar: Failed to setup event listeners:", error);
      }
    };

    setupListener();

    return () => {
      mounted = false;
      unlisten?.();
      document.removeEventListener(`${COMPONENT_ID}-state-update`, domHandler);
    };
  }, []);

  // === HOVER ===
  //
  // The native tracking area on this window reports enter/leave even while
  // another app is active (NSTrackingActiveAlways), which is when the bar is
  // mostly looked at. DOM mouseenter/leave cover the active-app case too.

  const [hovered, setHovered] = useState(false);
  const leaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Which pill button the forwarded cursor is over. macOS does not route
  // mouse-moved into an inactive window's webview, so CSS :hover never fires
  // while another app is active; the native tracking area forwards the
  // cursor position and we light the button under it ourselves.
  const [hoveredButton, setHoveredButton] = useState<
    "dot" | "mic" | "type" | "chat" | "listen" | null
  >(null);
  const dotRef = useRef<HTMLDivElement>(null);
  // The layout on screen, for the hover callbacks below (set at render).
  const shownLayoutRef = useRef<BarLayout>("compact");
  // Where the dot was in the idle pill when the cursor came to it. Hovering
  // grows the pill away from the edge it is docked to, which carries the dot
  // (on its leading edge) out from under a resting cursor: on a right-docked
  // bar it slid 76pt left and the cursor landed on a button. The dot is what
  // the person pointed at, so it stays the hovered control until the cursor
  // leaves the spot where the dot was.
  const dotLatchRef = useRef<DOMRect | null>(null);
  const micRef = useRef<HTMLButtonElement>(null);
  const typeRef = useRef<HTMLButtonElement>(null);
  const chatRef = useRef<HTMLButtonElement>(null);
  const listenRef = useRef<HTMLButtonElement>(null);
  const moveFrameRef = useRef<number | null>(null);

  const onMouseEnterWindow = useCallback(() => {
    if (leaveTimerRef.current) {
      clearTimeout(leaveTimerRef.current);
      leaveTimerRef.current = null;
    }
    // Read before the pill grows: this is the dot the person can see.
    if (shownLayoutRef.current === "compact") dotLatchRef.current = idleDotRect(dotRef.current);
    setHovered(true);
  }, []);

  // A leave is verified against the cursor before it is believed: growing the
  // window under a resting cursor produces a leave with no re-enter, which
  // would collapse the pill the moment it opened.
  const onMouseLeaveWindow = useCallback(() => {
    if (leaveTimerRef.current) clearTimeout(leaveTimerRef.current);
    leaveTimerRef.current = setTimeout(async () => {
      leaveTimerRef.current = null;
      let inside = false;
      try {
        inside = await cursorInsideContent(contentRef.current);
      } catch (error) {
        console.debug("FloatingBar: cursor check failed:", error);
      }
      if (!inside) {
        dotLatchRef.current = null;
        setHovered(false);
        setHoveredButton(null);
      }
    }, LEAVE_VERIFY_MS);
  }, []);

  // The forwarded cursor position, rAF-throttled, hit-tested against the two
  // buttons. Works whether or not Juno is the active app.
  const onMouseMovedWindow = useCallback((payload: unknown) => {
    const p = payload as { x?: number; y?: number } | null;
    if (!p || typeof p.x !== "number" || typeof p.y !== "number") return;
    const { x, y } = p;
    if (moveFrameRef.current !== null) return;
    moveFrameRef.current = requestAnimationFrame(() => {
      moveFrameRef.current = null;
      const hit = (ref: React.RefObject<HTMLElement | null>) => {
        const el = ref.current;
        if (!el) return false;
        const r = el.getBoundingClientRect();
        if (r.width === 0 && r.height === 0) return false;
        return pointInRect(x, y, r);
      };
      const latch = dotLatchRef.current;
      if (latch && pointInRect(x, y, latch)) {
        setHoveredButton("dot");
        return;
      }
      dotLatchRef.current = null;
      if (shownLayoutRef.current === "compact" && hit(dotRef)) {
        dotLatchRef.current = idleDotRect(dotRef.current);
      }
      setHoveredButton(
        hit(dotRef)
          ? "dot"
          : hit(micRef)
          ? "mic"
          : hit(typeRef)
            ? "type"
            : hit(chatRef)
              ? "chat"
              : hit(listenRef)
                ? "listen"
                : null,
      );
    });
  }, []);

  useEffect(() => () => {
    if (leaveTimerRef.current) clearTimeout(leaveTimerRef.current);
  }, []);

  useEffect(() => {
    let mounted = true;
    const unlisteners: Array<() => void> = [];
    const setup = async () => {
      try {
        const enter = await listen(EVENTS.SYSTEM_MOUSE_ENTERED_WINDOW, onMouseEnterWindow);
        const leave = await listen(EVENTS.SYSTEM_MOUSE_LEFT_WINDOW, onMouseLeaveWindow);
        const moved = await listen<{ x: number; y: number }>(
          EVENTS.SYSTEM_MOUSE_MOVED_WINDOW,
          (event) => onMouseMovedWindow(event.payload),
        );
        if (mounted) unlisteners.push(enter, leave, moved);
        else {
          enter();
          leave();
          moved();
        }
      } catch (error) {
        console.error("❌ FloatingBar: Failed to setup hover listeners:", error);
      }
    };
    setup();
    return () => {
      mounted = false;
      if (moveFrameRef.current !== null) cancelAnimationFrame(moveFrameRef.current);
      unlisteners.forEach((fn) => fn());
    };
  }, [onMouseEnterWindow, onMouseLeaveWindow, onMouseMovedWindow]);

  // === CONVERSATION (same pipeline as the main window) ===

  const chat = useBarConversation();

  // Whether the pane is up is its own answer, not something inferred from the
  // message count. It used to be `messages.length > 0 && !dismissed`, which
  // meant "New chat" emptied the conversation and the pane vanished under the
  // person who had just asked for one.
  const [paneShown, setPaneShown] = useState(false);
  // The full-size chat window shows this same conversation. While it is up the
  // pane stays down rather than saying everything twice.
  const [mainWindowOpen, setMainWindowOpen] = useState(false);
  const userMessageCount = chat.messages.filter((m) => m.role === "user").length;
  const seenUserMessagesRef = useRef(0);
  useEffect(() => {
    if (userMessageCount > seenUserMessagesRef.current) {
      setPaneShown(true);
    }
    seenUserMessagesRef.current = userMessageCount;
  }, [userMessageCount]);

  const paneOpen = paneShown && !mainWindowOpen;

  // A question about taking the mouse belongs on screen. If the person had
  // closed the conversation, bring it back so the prompt is where they are
  // looking; the window is only shown, never focused.
  useEventListener(EVENTS.INPUT_CONTROL_REQUEST, () => setPaneShown(true));

  // A permission Juno needs is asked for on a card inside the pane. Pressing
  // the mic from the idle pill left that card with nowhere to render, so the
  // button did nothing visible at all. Open the pane so the question is where
  // the person is already looking.
  useEventListener(EVENTS.PERMISSIONS_NEEDED, () => setPaneShown(true));

  const dismissPane = useCallback(() => setPaneShown(false), []);

  // Rust says whether a turn Claude started ran in a conversation that is not
  // on screen. While one did, a click anywhere on the pill opens it.
  const [elsewhere, setElsewhere] = useState(false);
  const elsewhereRef = useRef(false);
  elsewhereRef.current = elsewhere;
  useEventListener<{ conversation_id: string | null }>(
    EVENTS.BAR_CONVERSATION_ELSEWHERE,
    (payload) => setElsewhere(Boolean(payload?.conversation_id)),
  );

  /**
   * The chat button on the idle pill: show the conversation here, in the pane.
   *
   * While the full-size window is up the pane is gated shut (the two would say
   * everything twice), so this button used to set a flag that `paneOpen`
   * immediately discarded: pressing it did nothing at all, which is the worst
   * thing a visible control can do. Asking for the pane is asking for the
   * conversation in the bar, so the full-size window is put away first and the
   * pane opens when Rust announces it has gone.
   */
  const reopenPane = useCallback(() => {
    if (mainWindowOpen) {
      void invoke(COMMANDS.WINDOWS_CLOSE_MAIN_WINDOW).catch((error) =>
        console.error("FloatingBar: could not close the main window:", error),
      );
    }
    // Claude ran a turn of its own in another conversation: that is the one
    // to show. Rust loads it and the pane repopulates from its snapshot.
    if (elsewhereRef.current) {
      void invoke(COMMANDS.CONVERSATIONS_OPEN_CLAUDE_TURN_CONVERSATION).catch((error) =>
        console.debug("FloatingBar: could not open the conversation:", error),
      );
    }
    setPaneShown(true);
  }, [mainWindowOpen]);

  const openElsewhereOnClick = useCallback(
    (e: React.MouseEvent) => {
      // The pill's own buttons keep doing what they say.
      if ((e.target as HTMLElement).closest("button")) return;
      reopenPane();
    },
    [reopenPane],
  );

  // The full-size window taking over, and handing back.
  useEventListener(EVENTS.BAR_MAIN_WINDOW_OPENED, () => setMainWindowOpen(true));
  useEventListener(EVENTS.BAR_MAIN_WINDOW_CLOSED, () => setMainWindowOpen(false));

  // The text input is local until submit, so there is no per-keystroke IPC.
  const [inputOpen, setInputOpen] = useState(false);
  const [composerGrowth, setComposerGrowth] = useState(0);

  // Whether a turn is in flight, readable from callbacks that are defined
  // above the derived state that works it out. Kept in step during render.
  const isWorkingRef = useRef(false);

  // A request for the text input that cannot be honoured yet, because a voice
  // or working session is still winding down and the bar closes the input
  // while one is. Consumed by the effect that watches for the bar going quiet.
  const wantsInputWhenClearRef = useRef(false);

  // Starting a new chat means wanting to type, so the pane stays up, empty,
  // with the caret in it. Rotating the backend conversation matters too: the
  // bar used to clear the screen while the agent kept appending to the same
  // conversation and memory buffer.
  //
  // Mid-answer it has to stop that answer first. Rotating under a live stream
  // left the reply arriving into a conversation nobody could see any more, and
  // the input this opens was closed again a frame later by the working state,
  // so "New chat" read as a button that ate the screen and gave nothing back.
  // Stopping is what the person meant by starting again.
  const startNewChat = useCallback(() => {
    void (async () => {
      const wasWorking = isWorkingRef.current;
      if (wasWorking) await chat.stop();
      await invoke(COMMANDS.CONVERSATIONS_NEW_CONVERSATION).catch(() => {});
      chat.startNewChat();
      setPaneShown(true);
      // The stop has been asked for but the backend may still be winding down,
      // and it closes any input that is open while it does. Ask again once it
      // is clear.
      if (wasWorking) wantsInputWhenClearRef.current = true;
      setInputOpen(true);
    })();
  }, [chat.startNewChat, chat.stop]);

  // "New Chat" from the tray or the bar's right-click menu: the same call the
  // pane's + makes. While the full-size window is up it owns the conversation
  // and answers the event itself.
  const mainWindowOpenRef = useRef(mainWindowOpen);
  mainWindowOpenRef.current = mainWindowOpen;
  useEventListener(EVENTS.MENU_NEW_CHAT_REQUESTED, () => {
    if (!mainWindowOpenRef.current) startNewChat();
  });

  // The tray's "Center Floating Bar": the way back when a drag leaves the bar
  // somewhere it cannot be grabbed. Same transaction as the first placement,
  // aimed at the centre well of the display the bar is on.
  useEventListener(EVENTS.MENU_CENTER_FLOATING_BAR_REQUESTED, () => {
    void (async () => {
      try {
        const [pos, mons] = await Promise.all([
          windowOrigin(getCurrentWindow()),
          availableMonitors(),
        ]);
        if (!mons.length) return;
        const rects = toMonitorRects(mons);
        const wells = steadyWells(rects, PILL_STEADY_SPEC);
        const mon = Math.max(0, monitorIndexAt(rects, pos.x, pos.y));
        const target = wellForSlot(SLOT.center, mon, wells);
        const rect = target ? rects[target.monitorIndex] : undefined;
        if (!target || !rect) return;
        const next = steadyLayout(target, rect, PILL_STEADY_SPEC);
        await applySteadyFrame(next);
        setSteadyLayout(windowLabel, next);
        setDockSlot(windowLabel, { fx: target.fx, fy: target.fy });
        await invoke(COMMANDS.BAR_SET_BAR_POSITION, { x: target.x, y: target.y }).catch(() => {});
      } catch (error) {
        console.debug("FloatingBar: center failed:", error);
      }
    })();
  });

  // External open/close of the pane: the tray "Show/Hide Chat" toggles it, so a
  // dismissed conversation can be reopened showing the retained history.
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void (async () => {
      const toggle = await listen(EVENTS.BAR_TOGGLE_PANE, () =>
        setPaneShown((shown) => !shown),
      );
      if (active) unlisten = toggle;
      else toggle();
    })();
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  // === INTERACTIONS ===

  const createInteraction = useCallback(
    (interactionType: string, data?: Record<string, any>): UIInteractionEvent => ({
      element_id: COMPONENT_ID,
      interaction_type: interactionType,
      data: data || null,
      timestamp: Date.now(),
    }),
    [],
  );

  const sendInteraction = useCallback(async (interaction: UIInteractionEvent) => {
    try {
      await invoke(COMMANDS.BAR_UI_HANDLE_INTERACTION, { elementId: COMPONENT_ID, interaction });
    } catch (error) {
      console.error("❌ FloatingBar: Interaction failed:", error);
    }
  }, []);

  /**
   * Bring Juno forward so keystrokes reach this window.
   *
   * The bar accepts the first mouse click (`acceptFirstMouse`), which is what
   * lets an unfocused bar be dragged or clicked in one go, the way native
   * floating panels do. A first-mouse click does not reliably activate the
   * app or make the webview first responder, so anything that expects typing
   * afterwards asks for activation explicitly.
   */
  const activateWindow = useCallback(() => {
    getCurrentWindow()
      .setFocus()
      .catch((error) => console.debug("FloatingBar: window activation failed:", error));
  }, []);

  const [localInputValue, setLocalInputValue] = useState("");
  const localInputValueRef = useRef("");
  localInputValueRef.current = localInputValue;
  // A textarea, not an input: the pill starts one line tall and grows with
  // what is typed, the same way the full composer does. A single-line input
  // cannot hold a newline at all, so Shift+Enter had nothing to insert.
  const composer = useAutoGrowTextarea({
    value: localInputValue,
    maxHeightPx: BAR_COMPOSER_MAX_PX,
    minHeightPx: BAR_COMPOSER_LINE_PX,
  });
  const inputRef = composer.ref;
  // Only what exceeds a single line counts as growth; the band already holds
  // the first line. An empty or absent composer measures to exactly one line,
  // so this is 0 whenever there is nothing to make room for.
  useEffect(() => {
    const grown = Math.max(0, composer.height - BAR_COMPOSER_LINE_PX);
    setComposerGrowth((prev) => (prev === grown ? prev : grown));
  }, [composer.height]);

  useEffect(() => {
    setLocalInputValue(barState.inputValue);
  }, [barState.inputValue]);

  /** The type button: open the input and make sure typing lands in it. */
  const openInput = useCallback(() => {
    activateWindow();
    setInputOpen(true);
  }, [activateWindow]);

  /** Escape / empty blur: back to the pill. */
  const closeInput = useCallback(() => {
    setInputOpen(false);
    setLocalInputValue("");
  }, []);

  /** The X in the input: drop what was typed and go back to the idle pill. */
  const abandonInput = useCallback(() => {
    closeInput();
    // Without this the input reopens immediately, because an open pane keeps
    // the composer up. Closing the pane is what "back to the pill" means.
    setPaneShown(false);
  }, [closeInput]);

  // === CONNECTIVITY AND SHORTCUTS (for the dot and the tooltips) ===
  //
  // Whether Juno can answer is Rust's to say; the dot only shows it.
  const connectivity = useConnectivity();
  const warmingUp = useStartupReadiness();
  // The talk shortcut, named in the mic's tooltip. Read from the live trigger
  // registry, never written here, so the tooltip cannot teach a dead key.
  const [triggerHints, setTriggerHints] = useState<TriggerHints | null>(null);
  const readTriggerHints = useCallback(() => {
    invoke<TriggerHints>(COMMANDS.TRIGGERS_GET_TRIGGER_HINTS)
      .then((hints) => setTriggerHints(hints ?? null))
      .catch((error) => console.debug("FloatingBar: no trigger hints:", error));
  }, []);
  useEffect(() => {
    readTriggerHints();
  }, [readTriggerHints]);
  useEventListener(EVENTS.TRIGGERS_CHANGED, readTriggerHints);
  const talkShortcut = hintDetail(triggerHints?.agent ?? null);

  // === THE WAKE PHRASE ===
  //
  // Two separate facts, both answered by Rust rather than inferred here.
  // `voiceConfigured` is whether a voice trigger exists at all, which is the
  // person's standing intent and lives in the triggers list. `voiceListening`
  // is whether the engine is actually running right now, which is an outcome:
  // it can be false with a trigger enabled because the engine failed to start,
  // or because this bar paused it. A dot that claims Juno is listening when it
  // is not is worse than no dot, so neither is ever guessed.
  const [voiceConfigured, setVoiceConfigured] = useState(false);
  const [voiceListening, setVoiceListening] = useState(false);

  // The startup arming happens while this webview is still loading, so the
  // first voice-trigger-listening event is usually gone before anyone is
  // listening for it. Ask once on mount for the same two facts.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [listening, triggers] = await Promise.all([
          invoke<boolean>(COMMANDS.ALWAYS_LISTENING_GET_ALWAYS_LISTENING_STATUS),
          invoke<Array<{ gesture?: string; enabled?: boolean; phrase?: string | null }>>(
            COMMANDS.TRIGGERS_GET_TRIGGERS,
          ),
        ]);
        if (cancelled) return;
        setVoiceListening(Boolean(listening));
        setVoiceConfigured(
          triggers.some(
            (t) => t.gesture === "say" && t.enabled === true && Boolean(t.phrase?.trim()),
          ),
        );
      } catch (error) {
        console.debug("FloatingBar: could not read the wake phrase state:", error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // The one signal for both. Rust emits it whenever it arms or disarms the
  // engine: at startup, when the triggers change, when this bar pauses or
  // resumes it, and after a coordinated stop re-arms. It carries the outcome,
  // so an engine that failed to start reads as not listening even though the
  // trigger is there, which is the honest thing to draw.
  useEventListener<{ listening?: boolean; phrases?: string[] } | null>(
    EVENTS.VOICE_TRIGGER_LISTENING,
    (payload) => {
      setVoiceListening(Boolean(payload?.listening));
      setVoiceConfigured((payload?.phrases?.length ?? 0) > 0);
    },
  );

  /**
   * Pause or resume the wake phrase, from the bar.
   *
   * This controls the engine, never the trigger. The trigger is the standing
   * intent, so pausing writes nothing to the triggers list: it stops the engine
   * and leaves `always_listening_active` false, and the next launch arms it
   * again from the stored triggers (`apply_stored_voice_triggers` clears that
   * flag first and re-applies). Resuming restarts the same controller, which
   * still holds the wake words it was given.
   *
   * The bar does not move the dot itself. Both commands report what actually
   * happened through `always-listening-mode-changed`, and the dot follows that.
   */
  const toggleVoiceListening = useCallback(() => {
    const command = voiceListening
      ? COMMANDS.ALWAYS_LISTENING_STOP_ALWAYS_LISTENING
      : COMMANDS.ALWAYS_LISTENING_START_ALWAYS_LISTENING;
    void invoke(command).catch((error) =>
      console.error("FloatingBar: could not change the wake phrase engine:", error),
    );
  }, [voiceListening]);

  /** The mic button: a spoken query to the agent (same path as the hotkey). */
  const startTalking = useCallback(async () => {
    try {
      await invoke(COMMANDS.AGENT_AGENT_VOICE, { action: "start" });
    } catch (error) {
      console.error("❌ FloatingBar: failed to start listening:", error);
    }
  }, []);

  /** The mic inside the text input: drop the draft and start listening. */
  const switchToTalking = useCallback(() => {
    closeInput();
    void startTalking();
  }, [closeInput, startTalking]);

  /** Send what was said. This is what the old "Stop" button actually did. */
  const stopTalking = useCallback(() => sendVoiceTurn(), []);

  /**
   * Stop listening and throw it away. Nothing is transcribed, nothing sent.
   *
   * Which cancel that is depends on whose session is open; `cancelVoiceTurn`
   * (src/lib/voiceTurn.ts) routes a dictation through its own cancel event so
   * the dictation state machine unwinds with it, and a spoken query through
   * `agent_voice`. Shared with every other appearance.
   */
  const cancelTalking = useCallback(
    () =>
      cancelVoiceTurn({
        barState: barState.barState,
        isDictationMode: barState.isDictationMode,
      }),
    [barState.barState, barState.isDictationMode],
  );

  /**
   * Changed their mind about talking: drop the audio and open the input.
   *
   * The input cannot simply be opened here. The backend is still in a voice
   * state for the moment it takes the cancel to land, and while it is, the bar
   * both refuses to show the composer and actively closes an input that is
   * open. So the old version set a flag that was thrown away a frame later and
   * the button did nothing at all: the person pressed "type instead", the mic
   * closed, and they were left looking at the idle pill. The wish is recorded
   * here and acted on when Rust says the session is actually over, which is the
   * only thing that knows.
   */
  const switchToTyping = useCallback(async () => {
    wantsInputWhenClearRef.current = true;
    await cancelTalking();
  }, [cancelTalking]);

  // Pictures pasted into the pill, as data URLs, alongside the typed text.
  const [pastedImages, setPastedImages] = useState<string[]>([]);

  /**
   * The one way a message leaves the bar. Typed follow-ups and the empty
   * state's example prompts both come through here, so there is a single
   * answer to "what happens when the bar sends".
   */
  const sendMessage = useCallback(
    async (text: string, images: string[]) => {
      if (images.length > 0) {
        // The bar-state interaction carries only a string, so an attachment
        // goes straight to the agent rather than being quietly dropped.
        await invoke(COMMANDS.AGENT_DISPATCH_QUERY, {
          query: text,
          images,
        }).catch((error) => console.error("FloatingBar: submit failed:", error));
      } else {
        await sendInteraction(
          createInteraction(UI.INTERACTION_TYPES_SUBMIT, { value: text }),
        );
      }
      setLocalInputValue("");
      setPastedImages([]);
    },
    [sendInteraction, createInteraction],
  );

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      // A turn is already on its way. The composer is hidden while one is, but
      // the backend takes a moment to say so, and Enter held down or pressed
      // twice in that window sent the same question twice.
      if (isWorkingRef.current) return;
      const trimmedValue = localInputValue.trim();
      // A picture on its own is a message: "what is this?".
      if (!trimmedValue && pastedImages.length === 0) return;
      await sendMessage(trimmedValue, pastedImages);
    },
    [localInputValue, pastedImages, sendMessage],
  );

  // An example prompt clicked in the pane's empty state. Same contract as the
  // main window: the words always land in the input first, then go out by
  // the path a typed follow-up takes. The buttons are disabled while the
  // backend is connecting; if it is in error, or a turn is already running,
  // the text waits in the input instead of being silently dropped.
  const handleExamplePromptSelect = useCallback(
    (prompt: string) => {
      const trimmedPrompt = prompt.trim();
      if (!trimmedPrompt) return;
      setLocalInputValue(trimmedPrompt);
      setInputOpen(true);
      if (isWorkingRef.current || chat.serverStatus !== "connected") return;
      void sendMessage(trimmedPrompt, pastedImages);
    },
    [chat.serverStatus, pastedImages, sendMessage],
  );

  /** Read pasted images off the clipboard as data URLs. */
  const handlePaste = useCallback((event: React.ClipboardEvent) => {
    const files = Array.from(event.clipboardData?.items ?? [])
      .filter((item) => item.kind === "file" && item.type.startsWith("image/"))
      .map((item) => item.getAsFile())
      .filter((file): file is File => file !== null);
    if (files.length === 0) return;
    event.preventDefault();
    for (const file of files) {
      const reader = new FileReader();
      reader.onload = () => {
        const url = reader.result;
        if (typeof url === "string") setPastedImages((prev) => [...prev, url]);
      };
      reader.readAsDataURL(file);
    }
  }, []);

  const handleFocus = useCallback(async () => {
    await sendInteraction(createInteraction(UI.INTERACTION_TYPES_FOCUS, { isFocused: true }));
  }, [sendInteraction, createInteraction]);

  const handleBlur = useCallback(async () => {
    await sendInteraction(createInteraction(UI.INTERACTION_TYPES_BLUR, { isFocused: false }));
  }, [sendInteraction, createInteraction]);

  /**
   * Input blur, acted on once focus has settled: the window becoming key
   * (webview first responder, caret restored) blurs and refocuses the input
   * within a frame, and that must not fold the pill up. A blur because the
   * whole window lost focus is handled by the window focus listener below.
   */
  const handleInputBlur = useCallback(() => {
    setTimeout(() => {
      if (document.activeElement === inputRef.current) return;
      if (!document.hasFocus()) return;
      if (!localInputValueRef.current.trim()) closeInput();
      void handleBlur();
    }, BLUR_SETTLE_MS);
  }, [closeInput, handleBlur]);

  /**
   * OS-level focus changes (Cmd+Tab, clicking another window). When the
   * window becomes key, make the webview the window's first responder
   * (keystrokes otherwise never reach the page, even with a visible caret)
   * and put the caret in the input if one is showing. The backend is told
   * about focus by the input itself, never by the window, so that a click
   * on the mic — which may activate the window — can't turn into the
   * expand-to-input transition.
   */
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;

    const setup = async () => {
      try {
        const fn = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
          if (!mounted) return;
          if (focused) {
            getCurrentWebview()
              .setFocus()
              .catch((error) => console.debug("FloatingBar: webview focus failed:", error));
            inputRef.current?.focus();
          } else {
            // Left for another app with nothing typed: fold the pill up.
            if (!localInputValueRef.current.trim()) closeInput();
            handleBlur();
          }
        });
        if (mounted) unlisten = fn;
        else fn();
      } catch (error) {
        console.error("FloatingBar: Failed to setup window focus listener:", error);
      }
    };

    setup();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [handleBlur, closeInput]);

  // === DERIVED UI STATE ===

  const currentUiState = barState.barState;
  const isIdle = IDLE_STATES.includes(currentUiState);
  const isInputState = INPUT_STATES.includes(currentUiState);
  const isVoice = VOICE_STATES.includes(currentUiState);
  // The microphone is open. Wider than `isVoice`: live words while you talk
  // arrive as TRANSCRIBING, which is otherwise a working state.
  const isRecording = isRecordingTurn(barState);
  const isWorking =
    (WORKING_STATES.includes(currentUiState) && !isRecording) || chat.isProcessing;
  isWorkingRef.current = isWorking;

  // === JUNO IS DRIVING ===
  //
  // Juno normally works without touching the pointer. While it holds the real
  // cursor the bar says so in plain words and clears the moment it lets go.
  // The Rust side enlarges the system cursor at the same time; this is the
  // other half of that signal.
  const [driving, setDriving] = useState<InputControlStatePayload | null>(null);
  useEventListener<InputControlStatePayload>(EVENTS.INPUT_CONTROL_STATE, (payload) => {
    setDriving(payload?.active ? payload : null);
  });
  const isDriving = driving !== null;

  // Parallel agent sessions (LAC-1432): the roster strip appears below the
  // bar when 2+ agents run, so the window grows to make room for it.
  const { sessions: agentSessions, focusSession } = useAgentSessions();
  const showRosterStrip = agentSessions.length >= 2;

  // === ACTIVITY INDICATOR (bar flame border) ===
  // The border lights up and HOLDS while Juno is doing something; colour and
  // intensity name the mode (see flameForState). The agent-working colour
  // follows the focused session so parallel agents read apart.
  const focusedSessionColor =
    agentSessions.find((s) => s.focused)?.display_color ?? "#0A84FF";
  const flame = flameForState(currentUiState, isWorking, focusedSessionColor);

  // Whether the glowing border shows at all. Read from the same bar config the
  // settings window writes, and kept current from the config-changed event so
  // the toggle takes effect without a restart. On by default.
  const [showGlowBorder, setShowGlowBorder] = useState(true);
  useEffect(() => {
    let cancelled = false;
    void invoke<{ show_glow_border?: boolean }>(COMMANDS.BAR_UI_GET_BAR_CONFIG)
      .then((config) => {
        if (!cancelled && typeof config?.show_glow_border === "boolean") {
          setShowGlowBorder(config.show_glow_border);
        }
      })
      .catch((error) =>
        console.debug("FloatingBar: could not read the glow-border setting:", error),
      );
    return () => {
      cancelled = true;
    };
  }, []);
  useEventListener<{ show_glow_border?: boolean } | null>(
    EVENTS.BAR_CONFIG_CHANGED,
    (payload) => {
      if (typeof payload?.show_glow_border === "boolean") {
        setShowGlowBorder(payload.show_glow_border);
      }
    },
  );

  const layout = pickLayout({
    state: currentUiState,
    hovered,
    inputOpen,
    paneOpen,
    rosterVisible: showRosterStrip,
    driving: isDriving,
  });

  // The way back to the conversation, in every state the chat is not already
  // on screen: listening, working, driving, answering, an error. Closing the
  // chat mid-task used to leave the tray as the only way back.
  const showExpand = !paneOpen && !mainWindowOpen;

  // The hover pill carries a fourth control when there is a wake phrase to
  // pause, and the voice and status pills carry the expand control while the
  // chat is closed, so the pill is told to make room for each. The full pill
  // already has the room, and the steady frame is sized for the full pill, so
  // none of this can outgrow the window.
  const pillExtraWidth =
    (layout === "hover" && voiceConfigured) ||
    ((layout === "voice" || layout === "status") && showExpand)
      ? BAR_PILL_BUTTON_PX
      : 0;

  useEffect(() => {
    if (layout !== "hover") setHoveredButton(null);
  }, [layout]);

  // The input shows when the user opened it, while the backend is in its
  // input states, and between turns with the pane open so a follow-up is one
  // click away. Voice and working states show status instead.
  const showInput =
    !isDriving &&
    !isWorking &&
    !isVoice &&
    (inputOpen || isInputState || (paneOpen && isIdle));
  const label = driving
    ? drivingLabel(driving)
    : statusLabel(currentUiState, barState);

  // Once the input is up, put the caret in it. After a turn ends with the
  // pane open, refocus only if this window is still the one the user is in:
  // element focus in a non-key window is inert and must not steal focus.
  // Keyed on the layout too: the composer mounts with the full pill.
  useEffect(() => {
    if (!showInput || layout !== "full") return;
    if (inputOpen) {
      inputRef.current?.focus();
      return;
    }
    if (!document.hasFocus()) return;
    const t = setTimeout(() => inputRef.current?.focus(), 60);
    return () => clearTimeout(t);
  }, [showInput, inputOpen, layout]);

  // A voice or working state that starts while the input is open (hotkey,
  // wake word) takes over; the input is not waiting underneath.
  useEffect(() => {
    if ((isVoice || isWorking) && inputOpen) closeInput();
  }, [isVoice, isWorking, inputOpen, closeInput]);

  /**
   * The X, wherever it appears, and the Stop square: one meaning.
   *
   * "Stop what is happening and put me back at rest." The person should not
   * have to know whether what is happening is a spoken query, a dictation, a
   * running agent task or a half-typed line, and until now they did: the X in
   * the composer threw away a draft, the X in the voice row cancelled a
   * session, and the two had nothing in common but the glyph.
   *
   * It is one function, not one command, because the stops underneath are not
   * interchangeable. A spoken turn is cancelled through its own cancel, which
   * discards the audio and leaves everything else alone. The coordinated stop
   * (`stop_all_operations`, the one Escape reaches) is the heavier instrument:
   * it clears every subsystem and then re-arms the wake phrase from the stored
   * triggers, which is right for a running task and for an utterance nothing
   * lighter can reach, and wrong for a sentence somebody is still saying into
   * the ordinary mic.
   */
  const stopCurrentActivity = useCallback(() => {
    // A wake-phrase capture is held by the always-listening engine, not by the
    // voice controller every lighter cancel reaches, so stopping that engine is
    // the only thing that drops the utterance. That is safe now: the
    // coordinated stop re-arms from the stored triggers on its way out, so the
    // wake phrase survives being used to cancel one sentence.
    if (currentUiState === UI.BAR_STATES_ALWAYS_LISTENING) {
      void chat.stop();
      return;
    }
    if (isVoice || isRecording) {
      void cancelTalking();
      return;
    }
    if (isDriving || isWorking) {
      void chat.stop();
      return;
    }
    // Nothing is running. The only thing left to put down is the draft, and
    // the pane with it, because an open pane keeps the composer up.
    abandonInput();
  }, [
    currentUiState,
    isVoice,
    isRecording,
    isDriving,
    isWorking,
    cancelTalking,
    chat.stop,
    abandonInput,
  ]);

  // "Type instead" and "New chat", honoured once the session they interrupted
  // is really over. Anything that starts in the meantime (the hotkey, a wake
  // word) outranks the request: the wish was to type instead of *that*
  // session, not instead of whatever the person started next.
  useEffect(() => {
    if (!wantsInputWhenClearRef.current) return;
    if (isVoice || isWorking) return;
    wantsInputWhenClearRef.current = false;
    openInput();
  }, [isVoice, isWorking, openInput]);

  /**
   * Escape: one behaviour, shared by every appearance (src/lib/barEscape.ts).
   *
   * The Pill used to stage this by hand and leave the working case to Rust's
   * passive stop-key monitor. The hook wires both routes, so a press lands
   * whether or not the bar is the focused window, and in every state rather
   * than only while the input or the pane is up.
   */
  const collapseToIdle = useCallback(() => {
    if (inputOpen) void handleBlur();
    closeInput();
    dismissPane();
  }, [inputOpen, handleBlur, closeInput, dismissPane]);
  const reportEscape = useCallback(() => {
    void sendInteraction(createInteraction(UI.INTERACTION_TYPES_ESCAPE));
  }, [sendInteraction, createInteraction]);
  useEscapeToIdle({
    barState: currentUiState,
    working: isWorking,
    overlayOpen: paneOpen,
    composerOpen: inputOpen,
    popupOpen: false,
    collapse: collapseToIdle,
    report: reportEscape,
  });

  // On launch the bar always lands in a well, never at an arbitrary spot.
  // A remembered position is re-snapped to the nearest current well (so it
  // survives a resolution / monitor change); a fresh install with nothing saved
  // defaults to the least-intrusive well: top-right on the monitor the bar
  // opened on. Top-right clears the menu bar and the Dock, unlike the bottom
  // wells which do not yet measure the Dock.
  //
  // This also puts the bar on screen. The window is created hidden at the
  // placeholder frame in tauri.conf.json, because only the webview knows which
  // well the bar was left in; Rust used to show it on a 100ms timer, so the bar
  // appeared at the placeholder frame and then moved and resized itself while
  // the app finished loading, which is the jump people saw. Frame first, then
  // show, and the first thing on screen is already right. `show_bar_when_ready`
  // is called whatever happens above: a bar that cannot work out its well is
  // still better than no bar (and Rust shows it anyway after a few seconds).
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        // Rust wrote the stored well into the page at launch; ask only when
        // it did not (a reload, a preview, an older backend).
        const booted = takeBootBarPosition();
        const saved =
          booted !== undefined
            ? booted
            : await invoke<{ x: number; y: number } | null>(COMMANDS.BAR_GET_BAR_POSITION);
        const [pos, mons] = await Promise.all([
          windowOrigin(getCurrentWindow()),
          availableMonitors(),
        ]);
        if (cancelled || !mons.length) return;

        // Wells are computed for the resting footprint, not the window: the
        // idle pill lands on the well, and the window extends away from it.
        const rects = toMonitorRects(mons);
        const wells = steadyWells(rects, PILL_STEADY_SPEC);
        if (!wells.length) return;

        let target: Well | null = saved ? nearestWell(saved, wells) : null;
        if (!target) {
          // The monitor the window was created on, or the first one.
          const mon = Math.max(0, monitorIndexAt(rects, pos.x, pos.y));
          target = wellForSlot(SLOT.topRight, mon, wells) ?? wells[0];
        }
        if (cancelled || !target) return;

        const mon = rects[target.monitorIndex];
        if (!mon) return;
        // The one frame this window has in this well: placed and sized in a
        // single transaction, and never resized again until the well changes.
        const next = steadyLayout(target, mon, PILL_STEADY_SPEC);
        await applySteadyFrame(next);
        if (cancelled) return;
        setSteadyLayout(windowLabel, next);
        setDockSlot(windowLabel, { fx: target.fx, fy: target.fy });
        try {
          await invoke(COMMANDS.BAR_SET_BAR_POSITION, { x: target.x, y: target.y });
        } catch {
          // best effort; a failed persist just means we re-default next launch
        }
      } catch (error) {
        console.debug("FloatingBar: default/restore well failed:", error);
      } finally {
        if (!cancelled) {
          setPlaced(true);
          // After the reveal has the bar on screen, not during it.
          setTimeout(warmChatPane, CHAT_PANE_WARM_MS);
          await invoke(COMMANDS.BAR_SHOW_BAR_WHEN_READY).catch((error) =>
            console.error("FloatingBar: could not show the bar:", error),
          );
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // === THE FRAME THE PILL DRAWS, AND WHAT TAKES THE MOUSE ===
  //
  // There used to be a resize controller here: grow the window, then let the
  // pill animate into it; shrink the pill, then let the window follow. Every
  // one of those resizes was a chance for AppKit and WebKit to land on
  // different display frames, which is the jump. The window now has room for
  // the largest frame already, so a state change is the CSS transition and
  // nothing else. All that is left is telling the hit test what is drawn.
  const frame = useMemo<PillFrame>(
    () => ({
      layout,
      paneOpen,
      rosterVisible: showRosterStrip,
      // Gated on the composer being on screen, exactly as the pill is, so the
      // footprint and the pill inside it never disagree about how tall it is.
      composerGrowth: showInput ? Math.max(0, composerGrowth) : 0,
      extraWidth: pillExtraWidth,
    }),
    [layout, paneOpen, showRosterStrip, showInput, composerGrowth, pillExtraWidth],
  );
  const paneHeight = pillPaneHeight(dockLayout, frame);
  const footprint = pillFootprint({ ...frame, paneHeight });
  const content = contentRect(dockLayout, footprint);
  contentRef.current = content;

  // A drag shrinks the window to exactly this (`dragLayout`), so the OS drag
  // carries the pill and not the empty room around it. While it does, the
  // content rect above is the whole window, and that is what the hit test is
  // told: the cursor is on the pill for the whole drag by construction.
  useLayoutEffect(() => {
    setSteadyFootprint(windowLabel, { width: footprint.width, height: footprint.height });
  }, [windowLabel, footprint.width, footprint.height]);

  // The footprint is the only part of the window that takes the mouse: the
  // backend flips click-through as the cursor crosses its edge, and reports
  // enter and leave against it. Reported the moment it changes, so a growing
  // pill is clickable before its animation finishes.
  useEffect(() => {
    if (!placed) return;
    void invoke(COMMANDS.BAR_SET_BAR_HIT_REGIONS, {
      regions: [{ x: content.x, y: content.y, width: content.width, height: content.height }],
    }).catch((error) => console.error("FloatingBar: could not set hit regions:", error));
  }, [placed, content.x, content.y, content.width, content.height]);

  // === DRAG ANYWHERE, SNAP INTO A WELL ===
  //
  // One gesture, shared by every appearance (`hooks/useDragWindow.ts`): a
  // mousedown anywhere except text entry arms a drag, moving past the
  // threshold hands the gesture to the OS window drag and swallows the click
  // that would otherwise fire on release, and a press and release without
  // movement is an ordinary click on the button.
  //
  // On release the window glides into the nearest well and the dock store is
  // told where it landed. All of that used to live here, which is why the
  // Pill was the only look with gravity wells; it is now wherever the drag
  // hook is called, so a look added later gets it with no wiring.
  //
  // The display-follow is paused while the chat pane is open or the agent is
  // working: only the idle pill follows the cursor between displays.
  const { dragProps, swallowClickAfterDrag } = useBarDrag({
    displayFollowPaused: paneOpen || isWorking,
  });

  // === RENDER ===

  // Everything is drawn straight from the frame: the window already has room
  // for the largest one, so there is nothing to wait for in either direction.
  // The typed text makes the pill taller; the band grows with it, so the pane
  // slides rather than jumps.
  const shownLayout = frame.layout;
  shownLayoutRef.current = shownLayout;
  const growth = frame.composerGrowth;
  const pill = {
    width: BAR_LAYOUTS[shownLayout].width + frame.extraWidth,
    height: BAR_LAYOUTS[shownLayout].height + growth,
  };
  const band = BAR_BAND + growth;
  const place = contentPlacement(dockLayout);
  // Tooltips open toward the side the pill grows: the steady frame always has
  // the pane's worth of room there, so a tooltip is never clipped by the window.
  const tooltipSide = growUp ? "top" : "bottom";

  // The pane and roster render either below the pill (docked in the top half,
  // growing down) or above it (bottom half, growing up); only the margin side
  // and the render order flip, so build each once and place them by growUp.
  const chatPaneNode = frame.paneOpen ? (
    <div
      className={cn("shrink-0", growUp ? "mb-2" : "mt-2")}
      // The same conversation lives in the full-size window, so handing it
      // over should read as a handover. Without this the pane blinked out of
      // existence the instant that window opened, and blinked back when it
      // closed, with nothing connecting the two.
      style={{ animation: "fbar-reveal 0.18s ease-out both" }}
    >
      <Suspense fallback={null}>
      <BarChatPane
        messages={chat.messages}
        isProcessing={isWorking}
        backendStatus={chat.serverStatus}
        height={paneHeight}
        copiedMessageId={chat.copiedMessageId}
        onCopyResponse={chat.handleCopyResponse}
        onShareResponse={chat.handleShareResponse}
        onExamplePromptSelect={handleExamplePromptSelect}
        onApprovalUpdate={chat.handleApprovalUpdate}
        onContinuationUpdate={chat.handleContinuationUpdate}
        onDismiss={dismissPane}
        onNewChat={startNewChat}
      />
      </Suspense>
    </div>
  ) : null;

  // Parallel-agent roster (LAC-2830 §3): appears when 2+ agents run. Clicking a
  // dot focuses that agent; background sessions keep working.
  const rosterNode = frame.rosterVisible ? (
    <AgentRosterStrip
      sessions={agentSessions}
      onFocus={focusSession}
      className={growUp ? "mb-1.5" : "mt-1.5"}
    />
  ) : null;

  return (
    // The whole window: transparent, never resized per state. Clicks on it
    // outside the column pass through to whatever is underneath (Rust flips
    // the window's mouse handling as the cursor crosses the column's edge).
    <TooltipProvider delayDuration={PILL_TOOLTIP_DELAY_MS} skipDelayDuration={300}>
    <div className="relative h-screen w-screen select-none overflow-hidden">
      <div
        data-testid="floating-bar-column"
        className={cn(
          "absolute flex cursor-grab flex-col active:cursor-grabbing",
          // Pinned to the docked edges: absolute offsets from the window's
          // edges put the column's docked corner on the well, and the
          // alignment keeps the pill and pane flush with that same edge, so
          // the pill grows away from it and the edge itself never moves.
          anchorX === "start" ? "items-start" : anchorX === "end" ? "items-end" : "items-center",
          // Hidden only while a drop swaps the frame to a well that grows the
          // other way; it comes back with a short fade, or at once under
          // Reduce Motion.
          swapping
            ? "opacity-0"
            : "opacity-100 transition-opacity duration-150 ease-out motion-reduce:transition-none",
        )}
        style={{
          padding: BAR_PAD,
          left: place.left,
          right: place.right,
          top: place.top,
          bottom: place.bottom,
          transform: place.translateX ? "translateX(-50%)" : undefined,
        }}
        {...dragProps}
        onClickCapture={swallowClickAfterDrag}
        // Through the same verified path the native hover uses. These used to
        // set `hovered` directly, which is the flicker: a leave can be reported
        // for a cursor still over the pill, so the pill collapsed the instant
        // it opened, re-triggered enter, and oscillated. The verified path
        // checks where the cursor actually is before believing a leave.
        onMouseEnter={onMouseEnterWindow}
        onMouseLeave={onMouseLeaveWindow}
      >
      {/* Docked in the bottom half: the pane and roster open ABOVE the pill
          and nothing runs off the bottom. The pill stays anchored either way. */}
      {growUp && chatPaneNode}
      {growUp && rosterNode}

      {/* The fixed band the pill is centred in. Every layout shares it, so
          the pill's vertical centre never moves when the pill grows or
          shrinks; only the typed text can make it taller, and then it grows
          away from the anchored edge. */}
      <div
        className="flex shrink-0 items-center justify-center transition-[height] duration-200 ease-out motion-reduce:transition-none"
        style={{ height: band }}
      >
      <div
        data-testid="floating-bar"
        data-state={currentUiState}
        onClick={elsewhere ? openElsewhereOnClick : undefined}
        data-layout={shownLayout}
        data-driving={isDriving ? "" : undefined}
        className={cn(
          // `isolate` scopes the flame border's z-index -1 to the pill, so it
          // sits above the pill's dark face but behind its content.
          "relative isolate flex shrink-0 items-center gap-2 rounded-full",
          "border border-white/10 bg-neutral-950/90 text-white backdrop-blur-xl",
          // Juno has the pointer: a hairline in system blue, nothing louder.
          isDriving && "border-[#0A84FF]/70",
          "transition-[width,height,padding] duration-200 ease-out motion-reduce:transition-none",
        )}
        // The dot is absolute at its fixed home inside the pill, and every
        // flowing block starts at CONTENT_LEAD so it clears the dot. No layout
        // flips centre<->left, so nothing shifts inside the pill; the pill
        // itself grows away from the screen edge it is docked at, and the dot
        // rides its left edge. The right pad breathes the trailing controls;
        // compact carries no flowing content, so its right pad is 0.
        style={{
          width: pill.width,
          height: pill.height,
          paddingLeft: CONTENT_LEAD,
          paddingRight: shownLayout === "full" ? 16 : shownLayout === "compact" ? 0 : 8,
          boxShadow: BAR_DEPTH_GLOW,
        }}
      >
        {/* Activity indicator: a lit border whose colour + intensity name the
            mode (dictation, transcription, listening, working, error). Behind
            the content, mounted only while active. */}
        {flame && showGlowBorder && (
          <BarFlameBorder
            color={flame.color}
            intensity={flame.intensity}
            pulse={flame.pulse}
            radius={pill.height / 2}
          />
        )}

        {/* The status dot: Juno's presence, one persistent object. Absolutely
            positioned at its fixed home in EVERY layout (hover included) and
            vertically centred, so it never reflows, never teleports, and never
            flickers out between states — the pill just grows around it and the
            content appears to its right. */}
        <PillTooltip
          label={dotLabel(currentUiState, connectivity, {
            driving: isDriving,
            voicePaused: voiceConfigured && !voiceListening,
            loading: warmingUp,
          })}
          forcedOpen={hoveredButton === "dot"}
          side={tooltipSide}
          // The dot's target sits mid-pill; clear the pill's edge the way the
          // buttons' tooltips do.
          sideOffset={pill.height / 2 - 7.5 + 3}
        >
          {/* A 15px hover target around the 7px dot, offset by its own
              padding so the dot itself sits exactly where it always has. */}
          <div
            ref={dotRef}
            data-testid="floating-bar-dot-target"
            className="absolute top-1/2 -translate-y-1/2 p-1"
            style={{ left: DOT_HOME_LEFT - 4 }}
          >
            <StatusDot
              state={currentUiState}
              audioLevel={barState.audioLevel}
              connectivity={connectivity}
              driving={isDriving}
              voicePaused={voiceConfigured && !voiceListening}
              loading={warmingUp}
            />
          </div>
        </PillTooltip>

        {shownLayout === "compact" ? null : shownLayout === "hover" ? (
          <div
            className="flex items-center gap-1"
            style={{ animation: "fbar-reveal 0.18s ease-out both" }}
          >
            <PillTooltip
              label="Talk to Juno"
              detail={talkShortcut}
              forcedOpen={hoveredButton === "mic"}
              blocked={hoveredButton === "dot"}
              side={tooltipSide}
            >
              <button
                ref={micRef}
                type="button"
                onClick={startTalking}
                aria-label="Talk to Juno"
                title="Talk to Juno"
                data-phover={hoveredButton === "mic" ? "" : undefined}
                className={cn(pillButton, hoveredButton === "mic" && "bg-white/[0.12] text-white")}
              >
                <Mic className="size-3.5" />
              </button>
            </PillTooltip>
            <PillTooltip
              label="Type to Juno"
              forcedOpen={hoveredButton === "type"}
              blocked={hoveredButton === "dot"}
              side={tooltipSide}
            >
              <button
                ref={typeRef}
                type="button"
                onClick={openInput}
                aria-label="Type to Juno"
                title="Type to Juno"
                data-phover={hoveredButton === "type" ? "" : undefined}
                className={cn(pillButton, hoveredButton === "type" && "bg-white/[0.12] text-white")}
              >
                <Type className="size-3.5" />
              </button>
            </PillTooltip>
            {/* Always offered. This used to appear only when a conversation
                had been dismissed, so after "New chat" emptied the history
                there was no way back into the pane at all. */}
            <PillTooltip
              label="Open chat"
              forcedOpen={hoveredButton === "chat"}
              blocked={hoveredButton === "dot"}
              side={tooltipSide}
            >
              <button
                ref={chatRef}
                type="button"
                onClick={reopenPane}
                aria-label="Open chat"
                title="Open chat"
                data-phover={hoveredButton === "chat" ? "" : undefined}
                className={cn(pillButton, hoveredButton === "chat" && "bg-white/[0.12] text-white")}
              >
                <Maximize2 className="size-3.5" />
              </button>
            </PillTooltip>
            {/* The wake phrase, when there is one. A microphone that is open
                all day is worth being able to close for a while without going
                to Settings and taking the trigger away, which is a different
                and more permanent thing to mean. */}
            {voiceConfigured && (
              <PillTooltip
                label={voiceListening ? "Stop listening for the wake phrase" : "Listen for the wake phrase"}
                forcedOpen={hoveredButton === "listen"}
              blocked={hoveredButton === "dot"}
                side={tooltipSide}
              >
              <button
                ref={listenRef}
                type="button"
                onClick={toggleVoiceListening}
                aria-label={
                  voiceListening ? "Stop listening for the wake phrase" : "Listen for the wake phrase"
                }
                title={voiceListening ? "Stop listening" : "Start listening"}
                data-testid="floating-bar-voice-toggle"
                data-listening={voiceListening ? "" : undefined}
                data-phover={hoveredButton === "listen" ? "" : undefined}
                className={cn(
                  pillButton,
                  hoveredButton === "listen" && "bg-white/[0.12] text-white",
                )}
              >
                {voiceListening ? <Ear className="size-3.5" /> : <EarOff className="size-3.5" />}
              </button>
              </PillTooltip>
            )}
          </div>
        ) : showInput && shownLayout === "full" ? (
          <form
            onSubmit={handleSubmit}
            className={cn(
              "flex min-w-0 flex-1 gap-3",
              // Centre while it is one line; once it grows, keep the controls
              // by the first line rather than drifting to the middle.
              composerGrowth > 0 ? "items-start pt-1" : "items-center",
            )}
            style={{ animation: "fbar-content-in 0.2s ease-out both" }}
          >
            <textarea
              ref={composer.attach}
              rows={1}
              value={localInputValue}
              onChange={(e) => setLocalInputValue(e.target.value)}
              onKeyDown={(e) => {
                // Enter sends, Shift+Enter makes a line. A textarea would
                // otherwise insert a newline on both.
                if (isSendKey(e)) {
                  e.preventDefault();
                  void handleSubmit(e as unknown as FormEvent);
                }
              }}
              onPaste={handlePaste}
              onMouseDown={activateWindow}
              onFocus={handleFocus}
              onBlur={handleInputBlur}
              placeholder={paneOpen ? "Follow up…" : "Ask Juno"}
              aria-label="Ask Juno"
              className={cn(
                "min-w-0 flex-1 resize-none cursor-text border-none bg-transparent outline-none",
                "text-[13px] leading-[18px] tracking-[-0.01em] text-white/90 placeholder:text-white/30",
              )}
            />
            {/* Typing used to be Enter or nothing: no way to send by hand, no
                way to reach the mic, and no way out but Escape. */}
            <div className="flex shrink-0 items-center gap-1">
              {/* Not "switch to dictation": dictation types what you say into
                  whatever app has focus, and this mic is the inverse of the
                  "type instead" control next to it. It opens a spoken turn to
                  Juno, the same one the pill's mic opens, so it says so. */}
              <button
                type="button"
                onClick={switchToTalking}
                aria-label="Talk to Juno"
                title="Talk to Juno"
                className={inputControlButton}
              >
                <Mic className="size-3" />
              </button>
              <button
                type="submit"
                aria-label="Send"
                title="Send"
                disabled={!localInputValue.trim()}
                className={cn(
                  inputControlButton,
                  localInputValue.trim()
                    ? "bg-white/[0.16] text-white"
                    : "cursor-default opacity-40",
                )}
              >
                <ArrowUp className="size-3" />
              </button>
              <button
                type="button"
                onClick={stopCurrentActivity}
                aria-label="Close without sending"
                title="Close without sending"
                className={inputControlButton}
              >
                <X className="size-3" />
              </button>
            </div>
          </form>
        ) : (
          <>
            <span
              className={cn(
                "min-w-0 flex-1 truncate text-[13px] tracking-[-0.01em]",
                isDriving
                  ? "text-white/80"
                  : currentUiState === UI.BAR_STATES_ERROR
                    ? "text-[#e8866a]/80"
                    : "text-white/55",
                // Live streaming partial: render provisional, swap to solid on final.
                currentUiState === UI.BAR_STATES_TRANSCRIBING &&
                  barState.transcriptionProvisional &&
                  "italic text-white/40",
              )}
              data-testid="floating-bar-status"
            >
              {isDriving ? `Juno is ${label}` : (label ?? "Ask Juno")}
            </span>
            {/* Watching the pointer move on its own, the question is how to
                make it stop. The stop-key monitor in Rust has always taken
                Escape; nothing ever said so. */}
            {isDriving && (
              <kbd
                className="shrink-0 rounded border border-white/20 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-white/55"
                data-testid="floating-bar-stop-hint"
              >
                esc to stop
              </kbd>
            )}
            {/* Ahead of every stop control, so Stop keeps its place at the
                trailing edge whichever state this is. */}
            {showExpand && (
              <button
                type="button"
                onClick={reopenPane}
                aria-label="Open chat"
                title="Open chat"
                className={inputControlButton}
              >
                <Maximize2 className="size-3" />
              </button>
            )}
            {/* And the same thing to press, for a hand that is already on the
                mouse Juno is holding. The hint stays: it is the faster way. */}
            {isDriving && (
              <button
                type="button"
                onClick={stopCurrentActivity}
                aria-label="Stop Juno"
                title="Stop Juno (Esc)"
                className={inputControlButton}
              >
                <X className="size-3" />
              </button>
            )}
            {(isVoice || isRecording) && <AudioLevelBars audioLevel={barState.audioLevel} />}
            {/* Send, type instead, cancel: in every recording state, live
                words included, from the mapping every appearance shares. A
                wake-phrase capture gets cancel only. */}
            <VoiceTurnControls
              state={barState}
              onSend={() => void stopTalking()}
              onType={() => void switchToTyping()}
              onCancel={stopCurrentActivity}
              buttonClassName={(control) =>
                cn(inputControlButton, control === "send" && "bg-white/[0.16] text-white")
              }
            />
            {isWorking && (
              <button
                type="button"
                onClick={stopCurrentActivity}
                aria-label="Stop Juno"
                title="Stop Juno (Esc)"
                className="flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-full bg-white/[0.08] text-white/60 transition-colors hover:bg-white/[0.16] hover:text-white"
              >
                <Square className="size-2.5 fill-current" />
              </button>
            )}
          </>
        )}
      </div>
      </div>

      {/* Default (top-half) downward growth: roster then pane below the pill. */}
      {!growUp && rosterNode}
      {!growUp && chatPaneNode}
      </div>
    </div>
    </TooltipProvider>
  );
}
