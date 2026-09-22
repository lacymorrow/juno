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

import { useEffect, useState, useCallback, useRef, FormEvent, MouseEvent as ReactMouseEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import {
  availableMonitors,
  cursorPosition,
  getCurrentWindow,
  PhysicalPosition,
} from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ArrowUp, Ear, EarOff, MessageSquare, Mic, Square, Type, X } from "lucide-react";

import { useWindowSize, resetWindowAnchor } from "@/hooks/useWindowSize";
import { isSendKey, useAutoGrowTextarea } from "@/hooks/useAutoGrowTextarea";
import { useAgentSessions } from "@/hooks/useAgentSessions";
import { useBarConversation } from "@/hooks/useBarConversation";
import { useEventListener } from "@/hooks/useEventListener";
import { BarFlameBorder } from "@/components/bar/BarFlameBorder";
import { BAR_DEPTH_GLOW } from "@/components/bar/barAppearance";
import { cn } from "@/lib/utils";
import { COMMANDS, EVENTS, UI } from "@/lib/constants.generated";
import { drivingLabel, type InputControlStatePayload } from "@/lib/inputControl";
import {
  computeWells,
  nearestWell,
  wellForSlot,
  easeOutCubic,
  SLOT,
  type MonitorRect,
  type Well,
  type WellSlot,
} from "@/lib/snapWells";
import { AgentRosterStrip } from "./AgentRosterStrip";
import { BarChatPane } from "./bar/BarChatPane";
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
 * Pill size per layout plus the transparent padding around it (room for the
 * shadow). The window is exactly pill + padding, so the invisible part of
 * the window that catches clicks meant for what's behind it stays small.
 */
// `height` is the pill's own height; `band` is the fixed vertical slot the
// pill is centred in. compact keeps a tiny pill but shares the idle band with
// hover/voice, so growing from compact to hover changes only the width — the
// window height and vertical anchor never move, and nothing jumps vertically
// when the (delayed) shrink resizes the window.
export const BAR_LAYOUTS: Record<
  BarLayout,
  { width: number; height: number; band: number; pad: number }
> = {
  compact: { width: 56, height: 16, band: 34, pad: 16 },
  // Wider than the three buttons alone need: the status dot is now always
  // present at its fixed home (see DOT_HOME_LEFT) even in hover, so the row
  // starts past it. The optional wake-phrase button is still added on top via
  // pillExtraWidth.
  hover: { width: 148, height: 34, band: 34, pad: 16 },
  // Voice and status share a width and a band on purpose. Listening used to
  // open a 220px bar and then, the instant the mic closed, a 419px one, for a
  // status word and a stop button. The extra 200px held nothing, and the jump
  // happened mid-sentence, every time.
  voice: { width: 260, height: 34, band: 34, pad: 16 },
  status: { width: 260, height: 34, band: 34, pad: 16 },
  full: { width: 419, height: 44, band: 44, pad: 24 },
};

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

/** Drag starts once the mouse has moved this far from where it went down. */
export const DRAG_THRESHOLD_PX = 4;
/** A shrinking window waits for the pill's size animation before it snaps. */
export const SHRINK_DELAY_MS = 220;
/**
 * A mouse-leave is only believed after this long, once the cursor has been
 * checked against the window: resizing the window under a resting cursor
 * makes AppKit/WebKit report a leave that never happened.
 */
export const LEAVE_VERIFY_MS = 120;
/** An input blur is only acted on after this long, once focus has settled. */
export const BLUR_SETTLE_MS = 80;

/** Is the cursor over this window right now? Used to verify a mouse-leave. */
async function cursorInsideWindow(): Promise<boolean> {
  const w = getCurrentWindow();
  const [c, p, s] = await Promise.all([cursorPosition(), w.outerPosition(), w.outerSize()]);
  return c.x >= p.x && c.x < p.x + s.width && c.y >= p.y && c.y < p.y + s.height;
}

/**
 * Window size for a layout. `anchorY` is the pill's vertical centre, which
 * `useWindowSize` keeps at the same screen position across resizes, so the
 * pill grows around itself and the pane grows downward from it.
 */
export function floatingBarWindowSize({
  layout,
  paneOpen,
  rosterVisible,
  composerGrowth = 0,
  extraWidth = 0,
}: {
  layout: BarLayout;
  paneOpen: boolean;
  rosterVisible: boolean;
  /** Extra height the typed text needs beyond a single line. */
  composerGrowth?: number;
  /** Extra width for a control this layout does not always carry. */
  extraWidth?: number;
}) {
  const l = BAR_LAYOUTS[layout];
  const d = FLOATING_BAR_DIMENSIONS;
  // The window is sized to its contents, so a pill that grows without telling
  // the window would simply be clipped by it.
  const band = l.band + Math.max(0, composerGrowth);
  return {
    width: l.width + Math.max(0, extraWidth) + 2 * l.pad,
    height:
      band +
      2 * l.pad +
      (rosterVisible ? d.ROSTER_STRIP_HEIGHT : 0) +
      (paneOpen ? d.PANE_GAP + d.PANE_HEIGHT : 0),
    anchorY: l.pad + band / 2,
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

/** Settle animation: min/max duration, and the travel below which it's skipped. */
export const SNAP_MIN_MS = 160;
export const SNAP_MAX_MS = 340;
export const SNAP_MIN_TRAVEL_PX = 2;

/**
 * Animate a window's top-left from `from` to `to` (physical px) with an
 * ease-out, so a released bar glides into its well instead of teleporting.
 */
async function animateWindowTo(
  win: ReturnType<typeof getCurrentWindow>,
  from: { x: number; y: number },
  to: { x: number; y: number },
): Promise<void> {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  if (Math.abs(dx) + Math.abs(dy) < SNAP_MIN_TRAVEL_PX) return;
  const duration = Math.min(
    SNAP_MAX_MS,
    Math.max(SNAP_MIN_MS, Math.hypot(dx, dy) * 0.35),
  );
  const start = performance.now();
  await new Promise<void>((resolve) => {
    const step = () => {
      const t = Math.min(1, (performance.now() - start) / duration);
      const e = easeOutCubic(t);
      void win.setPosition(
        new PhysicalPosition(
          Math.round(from.x + dx * e),
          Math.round(from.y + dy * e),
        ),
      );
      if (t < 1) requestAnimationFrame(step);
      else {
        // The glide moved the window frame by frame without the resize cache
        // seeing any of it. Drop the stale baseline so the next resize anchors
        // from where the bar actually landed, not where it was before the drag.
        resetWindowAnchor(win.label);
        resolve();
      }
    };
    requestAnimationFrame(step);
  });
}

/** Component name for backend interactions — MUST match backend element ids */
const COMPONENT_ID = "floating-bar";

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
// One dot; state is communicated through motion and colour, not icons.

/** macOS system blue, the only accent the bar uses. */
const SYSTEM_BLUE = "#0A84FF";

function StatusDot({
  state,
  audioLevel,
  driving = false,
  voicePaused = false,
}: {
  state: UIState;
  audioLevel: number;
  driving?: boolean;
  /** A wake phrase is configured, but its engine is paused right now. */
  voicePaused?: boolean;
}) {
  const dot = "size-[7px] shrink-0 rounded-full";

  // Juno has the physical cursor. That outranks every other status: the same
  // travelling motion as working, in system blue, so it reads as Juno moving
  // the pointer rather than as a fault.
  if (driving) {
    return (
      <div
        data-testid="floating-bar-driving-dot"
        className={cn(dot)}
        style={{
          backgroundColor: SYSTEM_BLUE,
          animation: "fbar-orbit 1.1s ease-in-out infinite",
        }}
      />
    );
  }

  switch (state) {
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_ALWAYS_LISTENING:
    case UI.BAR_STATES_DICTATING:
      return (
        <div className="relative flex shrink-0 items-center justify-center">
          <div
            className={cn(dot, "relative z-10 bg-white")}
            style={{
              animation: "fbar-breathe 1.4s ease-in-out infinite",
              opacity: Math.max(0.5, audioLevel),
            }}
          />
        </div>
      );
    case UI.BAR_STATES_TRANSCRIBING:
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_STOPPING:
      return (
        <div
          className={cn(dot, "bg-white/80")}
          style={{ animation: "fbar-orbit 1.1s ease-in-out infinite" }}
        />
      );
    case UI.BAR_STATES_SPEAKING:
      return (
        <div
          className={cn(dot, "bg-white/80")}
          style={{ animation: "fbar-ripple 1.4s ease-out infinite" }}
        />
      );
    case UI.BAR_STATES_ERROR:
      return (
        <div
          className={cn(dot, "bg-[#e8866a]")}
          style={{ animation: "fbar-shake 0.4s ease-out" }}
        />
      );
    case UI.BAR_STATES_SUCCESS:
      return <div className={cn(dot, "bg-[#7aba8a]")} />;
    case UI.BAR_STATES_INPUT:
    case UI.BAR_STATES_EXPANDING:
      return <div className={cn(dot, "bg-white/70")} />;
    case UI.BAR_STATES_DICTATION_READY:
      return <div className={cn(dot, "bg-[#e8b36a]")} />;
    default:
      // At rest the dot is the only thing on screen, so it carries whether Juno
      // is listening for its wake phrase. Armed keeps the slow idle breath;
      // paused is dimmer and still, because a dot that moves the same way in
      // both states tells a person nothing about whether they can be heard.
      return (
        <div
          data-testid={voicePaused ? "floating-bar-voice-paused" : undefined}
          className={cn(dot, voicePaused ? "bg-white/30" : "bg-white")}
          style={
            voicePaused ? undefined : { animation: "fbar-idle 4s ease-in-out infinite" }
          }
        />
      );
  }
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

/** Tauri monitors as the plain rects the well math takes. */
const toMonitorRects = (
  mons: Array<{
    position: { x: number; y: number };
    size: { width: number; height: number };
    scaleFactor: number;
  }>,
): MonitorRect[] =>
  mons.map((m) => ({
    position: { x: m.position.x, y: m.position.y },
    size: { width: m.size.width, height: m.size.height },
    scaleFactor: m.scaleFactor,
  }));

/**
 * Which monitor a physical point is on, or -1 when it is on none of them (the
 * gap between two displays of different heights is a real place a window's
 * corner can sit). Callers decide what "none" means for them.
 */
const monitorIndexAt = (
  mons: Array<{ position: { x: number; y: number }; size: { width: number; height: number } }>,
  x: number,
  y: number,
): number =>
  mons.findIndex(
    (m) =>
      x >= m.position.x &&
      x < m.position.x + m.size.width &&
      y >= m.position.y &&
      y < m.position.y + m.size.height,
  );

/**
 * The window's size in logical pixels. Wells are computed from this, never
 * from the physical size, because the physical footprint changes with the
 * display's pixel density and a well computed for the wrong one lands the
 * bar off the far edge of a Retina screen.
 */
const logicalWindowSize = async (win: ReturnType<typeof getCurrentWindow>) => {
  const [size, scale] = await Promise.all([win.outerSize(), win.scaleFactor()]);
  return { windowWidth: size.width / scale, windowHeight: size.height / scale };
};

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
    "mic" | "type" | "chat" | "listen" | null
  >(null);
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
        inside = await cursorInsideWindow();
      } catch (error) {
        console.debug("FloatingBar: cursor check failed:", error);
      }
      if (!inside) {
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
      const hit = (ref: React.RefObject<HTMLButtonElement>) => {
        const el = ref.current;
        if (!el) return false;
        const r = el.getBoundingClientRect();
        if (r.width === 0 && r.height === 0) return false;
        return pointInRect(x, y, r);
      };
      setHoveredButton(
        hit(micRef)
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
    setPaneShown(true);
  }, [mainWindowOpen]);

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
      await invoke("new_conversation").catch(() => {});
      chat.startNewChat();
      setPaneShown(true);
      // The stop has been asked for but the backend may still be winding down,
      // and it closes any input that is open while it does. Ask again once it
      // is clear.
      if (wasWorking) wantsInputWhenClearRef.current = true;
      setInputOpen(true);
    })();
  }, [chat.startNewChat, chat.stop]);

  // Arm the global Escape monitor only while the pane is open, so Escape can
  // dismiss the pane even when the bar is not focused (the backend emits
  // BAR_DISMISS_PANE when nothing is running). The ledger is idempotent.
  useEffect(() => {
    void invoke("set_bar_pane_open", { open: paneOpen }).catch(() => {});
  }, [paneOpen]);
  useEffect(
    () => () => {
      void invoke("set_bar_pane_open", { open: false }).catch(() => {});
    },
    [],
  );

  // External open/close of the pane: the tray "Show/Hide Chat" toggles it (so a
  // dismissed conversation can be reopened, showing the retained history), and a
  // global Escape dismisses it.
  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    let active = true;
    void (async () => {
      const dismiss = await listen(EVENTS.BAR_DISMISS_PANE, () => dismissPane());
      const toggle = await listen(EVENTS.BAR_TOGGLE_PANE, () =>
        setPaneShown((shown) => !shown),
      );
      if (active) {
        unlisteners.push(dismiss, toggle);
      } else {
        dismiss();
        toggle();
      }
    })();
    return () => {
      active = false;
      unlisteners.forEach((fn) => fn());
    };
  }, [dismissPane]);

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
      await invoke("ui_handle_interaction", { elementId: COMPONENT_ID, interaction });
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
          invoke<boolean>("get_always_listening_status"),
          invoke<Array<{ method?: string; enabled?: boolean; phrase?: string | null }>>(
            COMMANDS.TRIGGERS_GET_TRIGGERS,
          ),
        ]);
        if (cancelled) return;
        setVoiceListening(Boolean(listening));
        setVoiceConfigured(
          triggers.some(
            (t) => t.method === "voice" && t.enabled === true && Boolean(t.phrase?.trim()),
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
      ? "stop_always_listening_mode"
      : "start_always_listening_mode";
    void invoke(command).catch((error) =>
      console.error("FloatingBar: could not change the wake phrase engine:", error),
    );
  }, [voiceListening]);

  /** The mic button: a spoken query to the agent (same path as the hotkey). */
  const startTalking = useCallback(async () => {
    try {
      await invoke("agent_voice", { action: "start" });
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
  const stopTalking = useCallback(async () => {
    try {
      await invoke("agent_voice", { action: "stop" });
    } catch (error) {
      console.error("❌ FloatingBar: failed to stop listening:", error);
    }
  }, []);

  /**
   * Stop listening and throw it away. Nothing is transcribed, nothing sent.
   *
   * Which cancel that is depends on whose session is open. `agent_voice` speaks
   * for a spoken query to the agent, and it was used for every voice state, so
   * cancelling a dictation session reached into the shared voice controller
   * without telling the dictation state machine anything: the mic went quiet
   * while `dictation_active`, the dictation monitor and the escape registration
   * all still believed a session was running, and a key trigger that was still
   * held simply put it back. From the person's side the X did nothing. A
   * dictation session is cancelled through its own event, which Rust's
   * `handle_dictation_cancel` uses to unwind all of that in one place.
   *
   * `isDictationMode` rather than the bar state is the discriminator on
   * purpose: an open dictation mic shows as LISTENING, not DICTATING (the UI
   * manager's `handle_dictation_started` sets Listening for either kind of
   * session), so the state alone would miss the case this exists for.
   */
  const isDictationSession =
    barState.isDictationMode || barState.barState === UI.BAR_STATES_DICTATING;
  const cancelTalking = useCallback(async () => {
    try {
      if (isDictationSession) {
        // The bar's own state settles from the BAR_STATE_UPDATE that Rust emits
        // when it puts dictation mode down, exactly as the agent path does.
        await emit(EVENTS.DICTATION_TRANSCRIPTION_CANCEL);
        return;
      }
      await invoke("agent_voice", { action: "cancel" });
    } catch (error) {
      console.error("❌ FloatingBar: failed to cancel listening:", error);
    }
  }, [isDictationSession]);

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
      if (pastedImages.length > 0) {
        // The bar-state interaction carries only a string, so an attachment
        // goes straight to the agent rather than being quietly dropped.
        await invoke(COMMANDS.AGENT_DISPATCH_QUERY, {
          query: trimmedValue,
          images: pastedImages,
        }).catch((error) => console.error("FloatingBar: submit failed:", error));
      } else {
        await sendInteraction(
          createInteraction(UI.INTERACTION_TYPES_SUBMIT, { value: trimmedValue }),
        );
      }
      setLocalInputValue("");
      setPastedImages([]);
    },
    [localInputValue, pastedImages, sendInteraction, createInteraction],
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
  const isWorking = WORKING_STATES.includes(currentUiState) || chat.isProcessing;
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

  const layout = pickLayout({
    state: currentUiState,
    hovered,
    inputOpen,
    paneOpen,
    rosterVisible: showRosterStrip,
    driving: isDriving,
  });

  // The hover pill carries a fourth control when there is a wake phrase to
  // pause, so the pill and its window are both told to make room for it. Every
  // other layout is unchanged, and so is a bar with no voice trigger.
  const pillExtraWidth = layout === "hover" && voiceConfigured ? BAR_PILL_BUTTON_PX : 0;

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
  useEffect(() => {
    if (!showInput) return;
    if (inputOpen) {
      inputRef.current?.focus();
      return;
    }
    if (!document.hasFocus()) return;
    const t = setTimeout(() => inputRef.current?.focus(), 60);
    return () => clearTimeout(t);
  }, [showInput, inputOpen]);

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
    if (isVoice) {
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
   * Escape while idle closes the input, then the pane. Escape while work is
   * in progress is deliberately NOT handled here: the passive stop-key
   * monitor in Rust (platform/stop_key_monitor.rs) sees it and stops
   * everything.
   */
  useEffect(() => {
    if (isWorking || (!inputOpen && !paneOpen)) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (inputOpen) {
        closeInput();
        void handleBlur();
      } else {
        dismissPane();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [inputOpen, paneOpen, isWorking, closeInput, handleBlur, dismissPane]);

  // === DOCK-AWARE GROWTH DIRECTION ===
  //
  // When the bar is docked in the bottom half of its display, its chat pane
  // (and the roster strip) open ABOVE the pill so they never run off the
  // bottom; the window then grows upward while the pill stays put. In the top
  // half it keeps the original downward growth. Recomputed whenever the pane or
  // roster opens and after a snap settles, since the dock can change.
  const [growUp, setGrowUp] = useState(false);

  const recomputeGrowUp = useCallback(async () => {
    try {
      const win = getCurrentWindow();
      const [pos, size, mons] = await Promise.all([
        win.outerPosition(),
        win.outerSize(),
        availableMonitors(),
      ]);
      const centerX = pos.x + size.width / 2;
      const centerY = pos.y + size.height / 2;
      const mon =
        mons.find(
          (m) =>
            centerX >= m.position.x &&
            centerX < m.position.x + m.size.width &&
            centerY >= m.position.y &&
            centerY < m.position.y + m.size.height,
        ) ?? mons[0];
      if (!mon) return;
      setGrowUp(centerY >= mon.position.y + mon.size.height / 2);
    } catch (error) {
      console.debug("FloatingBar: growUp recompute failed:", error);
    }
  }, []);

  useEffect(() => {
    if (paneOpen || showRosterStrip) void recomputeGrowUp();
  }, [paneOpen, showRosterStrip, recomputeGrowUp]);

  // The drag-well slot the bar currently occupies (col/row), so it can re-home
  // to the same slot on another display when the cursor moves there.
  const currentSlotRef = useRef<WellSlot | null>(null);

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
        const saved = await invoke<{ x: number; y: number } | null>("get_bar_position");
        const [pos, mons] = await Promise.all([
          getCurrentWindow().outerPosition(),
          availableMonitors(),
        ]);
        if (cancelled || !mons.length) return;

        // The size the bar is about to be, not the size the window happens to
        // have been created at: a well is computed for a footprint, and the
        // resize effect is racing this one to apply exactly this size.
        const initial = floatingBarWindowSize({
          layout: "compact",
          paneOpen: false,
          rosterVisible: false,
        });
        const wells = computeWells(toMonitorRects(mons), {
          windowWidth: initial.width,
          windowHeight: initial.height,
          includeCenter: true,
        });
        if (!wells.length) return;

        let target: Well | null = saved ? nearestWell(saved, wells) : null;
        if (!target) {
          // The monitor the window was created on, or the first one.
          const mon = Math.max(0, monitorIndexAt(mons, pos.x, pos.y));
          target = wellForSlot(SLOT.topRight, mon, wells) ?? wells[0];
        }
        if (cancelled || !target) return;

        // Move and size in one transaction, so even a bar that is somehow
        // already visible cannot show an intermediate frame.
        await invoke("set_bar_frame", {
          x: target.x,
          y: target.y,
          width: initial.width,
          height: initial.height,
        });
        // Placed at launch outside the resize path; start the baseline here.
        resetWindowAnchor(windowLabel);
        currentSlotRef.current = { fx: target.fx, fy: target.fy };
        try {
          await invoke("set_bar_position", { x: target.x, y: target.y });
        } catch {
          // best effort; a failed persist just means we re-default next launch
        }
      } catch (error) {
        console.debug("FloatingBar: default/restore well failed:", error);
      } finally {
        if (!cancelled) {
          await invoke("show_bar_when_ready").catch((error) =>
            console.error("FloatingBar: could not show the bar:", error),
          );
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // === WINDOW RESIZING ===

  const { resizeWindowIfChanged } = useWindowSize(windowLabel);
  const lastWindowRef = useRef<{ width: number; height: number } | null>(null);

  // Vertical centering, the exact analogue of the pill's horizontal centering.
  // With no pane/roster the window is symmetric top-to-bottom, so the pill sits
  // at the window's vertical middle — which is the anchor the frame is pinned
  // to — and any pad/band/anchor change (compact<->full) is absorbed by the
  // centering the same way the centred window absorbs a width change: the pill
  // grows around its own middle instead of drifting. With a pane the window is
  // asymmetric (the pane fills one side), so the pill pins to its near edge
  // instead. Driven off the APPLIED frame, not the target: on a shrink the
  // window is held for SHRINK_DELAY_MS, so flipping this with the target would
  // dump the pill into the middle of a still-tall pane window for that beat.
  const [vcenter, setVcenter] = useState(true);

  useEffect(() => {
    const next = floatingBarWindowSize({
      layout,
      paneOpen,
      rosterVisible: showRosterStrip,
      // Gated on the composer being on screen, exactly as the pill is, so the
      // window and the pill inside it never disagree about how tall it is.
      composerGrowth: showInput ? composerGrowth : 0,
      extraWidth: pillExtraWidth,
    });
    const prev = lastWindowRef.current;
    lastWindowRef.current = { width: next.width, height: next.height };

    const apply = () => {
      // Centre-stable, both axes: the window grows and shrinks around the pill
      // rather than around a screen edge.
      //
      // A previous attempt anchored growth to the well the bar is docked in,
      // to stop the chat pane walking the bar out of that well a little at a
      // time. It fixed that and broke the case that happens a hundred times a
      // day. The pill is centred in its window, so pinning the docked edge
      // moves the pill by half the size change, in one frame, while the pill's
      // own width is still animating: every hover jumped the pill sideways and
      // every collapse jumped it back, and the collapse jumped after the shrink
      // delay, with nothing connecting the two. Reverted rather than patched,
      // because the walk is a slow annoyance and this was a fast one.
      const config = growUp ? { ...next, growUp: true } : next;
      resizeWindowIfChanged(config).catch((error) =>
        console.error("❌ FloatingBar: Failed to resize window:", error),
      );
      // Follow the frame we just applied: centre only when it is symmetric
      // (no pane / roster on one side). Flips in lockstep with the window, so
      // the pill's vertical origin never disagrees with the frame mid-move.
      setVcenter(!paneOpen && !showRosterStrip);
    };

    // Growing: make room first, then the pill animates into it. Shrinking:
    // let the pill animate down before the window snaps around it.
    const shrinking = prev !== null && next.width <= prev.width && next.height <= prev.height;
    if (!shrinking) {
      apply();
      return;
    }
    const t = setTimeout(apply, SHRINK_DELAY_MS);
    return () => clearTimeout(t);
  }, [
    layout,
    paneOpen,
    showRosterStrip,
    growUp,
    composerGrowth,
    showInput,
    pillExtraWidth,
    resizeWindowIfChanged,
  ]);

  // === DRAG ANYWHERE, SNAP INTO A WELL ===
  //
  // A mousedown anywhere except the text input and the chat body arms a
  // drag; moving past the threshold hands the gesture to the OS window drag
  // and swallows the click that would otherwise fire on release. A press
  // and release without movement is an ordinary click on the button.
  //
  // The bar drags anywhere, but on release it glides into the nearest well —
  // the tidy anchor points around every display (corners + edge-midpoints).
  // The user aims roughly; the wells make it land deliberately.

  const dragArmRef = useRef<{ x: number; y: number } | null>(null);
  const draggedRef = useRef(false);
  // Set the moment a drag hands off to the OS; consumed once on release to
  // settle the bar into the nearest well.
  const snapArmedRef = useRef(false);
  const snapAnimatingRef = useRef(false);
  // Whether the snap-well drop indicator overlay is currently shown, so we
  // hide it exactly once on release regardless of which settle path fires.
  const snapOverlayShownRef = useRef(false);

  /**
   * On release after a drag, glide the bar into the nearest well — the tidy
   * anchor points around every display (corners and edge-midpoints). The bar
   * still drags anywhere; the wells only decide where it lands.
   */
  const settleIntoWell = useCallback(async () => {
    // Hide the drop indicator overlay on any release path, even if the settle
    // below no-ops (drag not armed, or already consumed by another path).
    if (snapOverlayShownRef.current) {
      snapOverlayShownRef.current = false;
      void emit("snap-wells-hide");
    }
    if (!snapArmedRef.current || snapAnimatingRef.current) return;
    snapArmedRef.current = false;
    try {
      const win = getCurrentWindow();
      const [pos, logical, monitors] = await Promise.all([
        win.outerPosition(),
        logicalWindowSize(win),
        availableMonitors(),
      ]);
      if (!monitors.length) return;
      const wells = computeWells(toMonitorRects(monitors), { ...logical, includeCenter: true });
      const target = nearestWell({ x: pos.x, y: pos.y }, wells);
      if (!target) return;
      snapAnimatingRef.current = true;
      await animateWindowTo(win, { x: pos.x, y: pos.y }, { x: target.x, y: target.y });
      currentSlotRef.current = { fx: target.fx, fy: target.fy };
      // Remember where it landed so the bar reopens here next launch, and
      // re-derive the growth direction since the dock may have changed.
      try {
        await invoke("set_bar_position", { x: target.x, y: target.y });
      } catch (error) {
        console.debug("FloatingBar: persist well failed:", error);
      }
      void recomputeGrowUp();
    } catch (error) {
      console.debug("FloatingBar: settle into well failed:", error);
    } finally {
      snapAnimatingRef.current = false;
    }
  }, [recomputeGrowUp]);

  // The cursor moved to another display (backend poll): re-home the compact pill
  // to the same drag-well slot on that display, so it is always where the user
  // is looking. Only the idle pill follows — a dragging bar or an open chat pane
  // is left where it is.
  const handleCursorDisplayChange = useCallback(
    async ({ x, y }: { x: number; y: number }) => {
      if (paneOpen || isWorking) return;
      if (snapArmedRef.current || snapAnimatingRef.current) return;
      const slot = currentSlotRef.current;
      if (!slot) return;
      const contains = (
        m: { position: { x: number; y: number }; size: { width: number; height: number } },
        px: number,
        py: number,
      ) =>
        px >= m.position.x &&
        px < m.position.x + m.size.width &&
        py >= m.position.y &&
        py < m.position.y + m.size.height;
      try {
        const win = getCurrentWindow();
        const [logical, pos, mons] = await Promise.all([
          logicalWindowSize(win),
          win.outerPosition(),
          availableMonitors(),
        ]);
        if (!mons.length) return;
        const targetIdx = mons.findIndex((m) => contains(m, x, y));
        if (targetIdx < 0) return;
        // Already on the cursor's display: nothing to do.
        const barIdx = mons.findIndex((m) => contains(m, pos.x, pos.y));
        if (barIdx === targetIdx) return;
        const wells = computeWells(toMonitorRects(mons), { ...logical, includeCenter: true });
        // The same place on the new display: same corner, same padding, its
        // own size and pixel density, so centre stays centre and a corner
        // stays a corner instead of the old physical coordinates landing
        // somewhere else (or off-screen) on a display of a different shape.
        const target = wellForSlot(slot, targetIdx, wells);
        if (!target) return;
        await win.setPosition(new PhysicalPosition(target.x, target.y));
        // Moved to another display outside any resize; forget the baseline.
        resetWindowAnchor(win.label);
        currentSlotRef.current = { fx: target.fx, fy: target.fy };
        try {
          await invoke("set_bar_position", { x: target.x, y: target.y });
        } catch {
          // best effort persist
        }
        void recomputeGrowUp();
      } catch (error) {
        console.debug("FloatingBar: cursor-follow move failed:", error);
      }
    },
    [paneOpen, isWorking, recomputeGrowUp],
  );

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    let active = true;
    void (async () => {
      const fn = await listen<{ x: number; y: number }>(
        EVENTS.BAR_CURSOR_DISPLAY_CHANGED,
        (event) => void handleCursorDisplayChange(event.payload),
      );
      if (active) unlisteners.push(fn);
      else fn();
    })();
    return () => {
      active = false;
      unlisteners.forEach((fn) => fn());
    };
  }, [handleCursorDisplayChange]);

  const onRootMouseDown = useCallback((e: ReactMouseEvent) => {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    draggedRef.current = false;
    if (target.closest("input, textarea, [data-no-drag]")) return;
    dragArmRef.current = { x: e.clientX, y: e.clientY };
  }, []);

  const onRootMouseMove = useCallback((e: ReactMouseEvent) => {
    const start = dragArmRef.current;
    if (!start) return;
    if (
      Math.abs(e.clientX - start.x) + Math.abs(e.clientY - start.y) <
      DRAG_THRESHOLD_PX
    )
      return;
    dragArmRef.current = null;
    draggedRef.current = true;
    snapArmedRef.current = true;
    e.preventDefault();
    const win = getCurrentWindow();
    // Show the snap-well drop indicator overlay for the length of the drag.
    if (!snapOverlayShownRef.current) {
      snapOverlayShownRef.current = true;
      logicalWindowSize(win)
        .then((logical) => emit("snap-wells-show", logical))
        .catch((error) =>
          console.debug("FloatingBar: snap-wells-show failed:", error),
        );
    }
    win
      .startDragging()
      .catch((error) => console.debug("FloatingBar: startDragging failed:", error));
  }, []);

  const onRootMouseUp = useCallback(() => {
    dragArmRef.current = null;
  }, []);

  const onRootClickCapture = useCallback(
    (e: ReactMouseEvent) => {
      if (!draggedRef.current) return;
      draggedRef.current = false;
      e.stopPropagation();
      e.preventDefault();
      void settleIntoWell();
    },
    [settleIntoWell],
  );

  // A drag that ends off the pill (fast flick, release outside) never fires a
  // click, so a window-level mouseup is the reliable settle trigger; whichever
  // path fires first disarms the other.
  useEffect(() => {
    const onUp = () => void settleIntoWell();
    window.addEventListener("mouseup", onUp, true);
    return () => window.removeEventListener("mouseup", onUp, true);
  }, [settleIntoWell]);

  // === RENDER ===

  // The typed text makes the pill taller, and the window was already sized for
  // it by floatingBarWindowSize. These two were not: they stayed at the layout's
  // fixed height, so the window grew around a 34px pill and the textarea was
  // clipped inside it, which looked like the growth not working at all.
  //
  // Only while the composer is actually on screen: belt and braces next to the
  // hook giving up its measured height when the textarea detaches, so a pill
  // left tall by a race is still impossible.
  const growth = showInput ? Math.max(0, composerGrowth) : 0;
  const pill = {
    ...BAR_LAYOUTS[layout],
    width: BAR_LAYOUTS[layout].width + pillExtraWidth,
    height: BAR_LAYOUTS[layout].height + growth,
  };
  const pad = BAR_LAYOUTS[layout].pad;
  const band = BAR_LAYOUTS[layout].band + growth;

  // The pane and roster render either below the pill (default, growing down) or
  // above it (docked in the bottom half, growing up); only the margin side and
  // the render order flip, so build each once and place them by `growUp`.
  const chatPaneNode = paneOpen ? (
    <div
      className={cn("shrink-0", growUp ? "mb-2" : "mt-2")}
      // The same conversation lives in the full-size window, so handing it
      // over should read as a handover. Without this the pane blinked out of
      // existence the instant that window opened, and blinked back when it
      // closed, with nothing connecting the two.
      style={{ animation: "fbar-reveal 0.18s ease-out both" }}
    >
      <BarChatPane
        messages={chat.messages}
        isProcessing={isWorking}
        height={FLOATING_BAR_DIMENSIONS.PANE_HEIGHT}
        copiedMessageId={chat.copiedMessageId}
        onCopyResponse={chat.handleCopyResponse}
        onShareResponse={chat.handleShareResponse}
        onApprovalUpdate={chat.handleApprovalUpdate}
        onContinuationUpdate={chat.handleContinuationUpdate}
        onDismiss={dismissPane}
        onNewChat={startNewChat}
      />
    </div>
  ) : null;

  // Parallel-agent roster (LAC-2830 §3): appears when 2+ agents run. Clicking a
  // dot focuses that agent; background sessions keep working.
  const rosterNode = showRosterStrip ? (
    <AgentRosterStrip
      sessions={agentSessions}
      onFocus={focusSession}
      className={growUp ? "mb-1.5" : "mt-1.5"}
    />
  ) : null;

  return (
    <div
      className={cn(
        "relative flex h-screen w-screen cursor-grab select-none flex-col items-center overflow-hidden active:cursor-grabbing",
        // Vertical origin: centre the pill in a symmetric window so pad/band/
        // anchor changes are absorbed (the pill grows around its middle); pin
        // to the near edge when a pane fills one side. Mirrors the horizontal
        // centring that already makes width changes smooth. See `vcenter`.
        vcenter ? "justify-center" : growUp ? "justify-end" : "justify-start",
      )}
      style={{ padding: pad }}
      onMouseDownCapture={onRootMouseDown}
      onMouseMove={onRootMouseMove}
      onMouseUp={onRootMouseUp}
      onClickCapture={onRootClickCapture}
      // Through the same verified path the native tracking area uses. These
      // used to set `hovered` directly, which is the flicker: growing the
      // window under a resting cursor fires a DOM mouseleave with no
      // re-enter, so the pill collapsed the instant it opened, re-triggered
      // enter, and oscillated. The native path already debounces a leave and
      // checks where the cursor actually is; bypassing it here undid that.
      onMouseEnter={onMouseEnterWindow}
      onMouseLeave={onMouseLeaveWindow}
    >
      {/* Docked in the bottom half: the pane and roster open ABOVE the pill so
          the window grows upward and nothing runs off the bottom. The pill
          stays anchored either way. */}
      {growUp && chatPaneNode}
      {growUp && rosterNode}

      {/* Fixed-height band the pill is centred in. compact and hover share
          the same band, so the pill's vertical centre never moves and the
          window's height/anchor stay put when the pill grows or shrinks — only
          the width changes, which is centre-stable and animates cleanly. */}
      <div
        className="flex shrink-0 items-center justify-center transition-[height] duration-200 ease-out"
        style={{ height: band }}
      >
      <div
        data-testid="floating-bar"
        data-state={currentUiState}
        data-layout={layout}
        data-driving={isDriving ? "" : undefined}
        className={cn(
          // `isolate` scopes the flame border's z-index -1 to the pill, so it
          // sits above the pill's dark face but behind its content.
          "relative isolate flex shrink-0 items-center gap-2 rounded-full",
          "border border-white/10 bg-neutral-950/90 text-white backdrop-blur-xl",
          // Juno has the pointer: a hairline in system blue, nothing louder.
          isDriving && "border-[#0A84FF]/70",
          "transition-[width,height,padding] duration-200 ease-out",
        )}
        // Horizontal origin: the dot is absolute at its fixed home, and every
        // flowing block starts at CONTENT_LEAD so it clears the dot. No layout
        // flips centre<->left any more, so nothing shifts sideways when the
        // pill grows or shrinks — only the width changes, around the fixed dot.
        // The right pad breathes the trailing controls; compact carries no
        // flowing content, so its right pad is 0.
        style={{
          width: pill.width,
          height: pill.height,
          paddingLeft: CONTENT_LEAD,
          paddingRight: layout === "full" ? 16 : layout === "compact" ? 0 : 8,
          boxShadow: BAR_DEPTH_GLOW,
        }}
      >
        {/* Activity indicator: a lit border whose colour + intensity name the
            mode (dictation, transcription, listening, working, error). Behind
            the content, mounted only while active. */}
        {flame && (
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
        <div
          className="pointer-events-none absolute top-1/2 -translate-y-1/2"
          style={{ left: DOT_HOME_LEFT }}
        >
          <StatusDot
            state={currentUiState}
            audioLevel={barState.audioLevel}
            driving={isDriving}
            voicePaused={voiceConfigured && !voiceListening}
          />
        </div>

        {layout === "compact" ? null : layout === "hover" ? (
          <div
            className="flex items-center gap-1"
            style={{ animation: "fbar-reveal 0.18s ease-out both" }}
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
            {/* Always offered. This used to appear only when a conversation
                had been dismissed, so after "New chat" emptied the history
                there was no way back into the pane at all. */}
            <button
              ref={chatRef}
              type="button"
              onClick={reopenPane}
              aria-label="Open chat"
              title="Open chat"
              data-phover={hoveredButton === "chat" ? "" : undefined}
              className={cn(pillButton, hoveredButton === "chat" && "bg-white/[0.12] text-white")}
            >
              <MessageSquare className="size-3.5" />
            </button>
            {/* The wake phrase, when there is one. A microphone that is open
                all day is worth being able to close for a while without going
                to Settings and taking the trigger away, which is a different
                and more permanent thing to mean. */}
            {voiceConfigured && (
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
            )}
          </div>
        ) : showInput ? (
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
            {isVoice && <AudioLevelBars audioLevel={barState.audioLevel} />}
            {/* Three answers, not one. The single "Stop" here finalised the
                audio and submitted it, so the only way to abandon a sentence
                was to say it and then stop the agent. */}
            {isVoice && currentUiState !== UI.BAR_STATES_ALWAYS_LISTENING && (
              <div className="flex shrink-0 items-center gap-1">
                <button
                  type="button"
                  onClick={() => void stopTalking()}
                  aria-label="Send"
                  title="Send"
                  className={cn(inputControlButton, "bg-white/[0.16] text-white")}
                >
                  <ArrowUp className="size-3" />
                </button>
                <button
                  type="button"
                  onClick={() => void switchToTyping()}
                  aria-label="Type instead"
                  title="Type instead"
                  className={inputControlButton}
                >
                  <Type className="size-3" />
                </button>
                <button
                  type="button"
                  onClick={stopCurrentActivity}
                  aria-label="Cancel without sending"
                  title="Cancel without sending"
                  className={inputControlButton}
                >
                  <X className="size-3" />
                </button>
              </div>
            )}
            {/* A wake phrase landed and Juno is taking down what follows. There
                is nothing to send here (the engine decides when the sentence
                ends) and nothing to switch to, but there is something to stop,
                and the rule is that when there is, the X is there. */}
            {currentUiState === UI.BAR_STATES_ALWAYS_LISTENING && (
              <button
                type="button"
                onClick={stopCurrentActivity}
                aria-label="Cancel without sending"
                title="Cancel without sending"
                className={inputControlButton}
              >
                <X className="size-3" />
              </button>
            )}
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
  );
}
