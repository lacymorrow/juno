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
import { ArrowUp, MessageSquare, Mic, Square, Type, X } from "lucide-react";

import { useWindowSize } from "@/hooks/useWindowSize";
import { useAgentSessions } from "@/hooks/useAgentSessions";
import { useBarConversation } from "@/hooks/useBarConversation";
import { useEventListener } from "@/hooks/useEventListener";
import { cn } from "@/lib/utils";
import { EVENTS, UI } from "@/lib/constants.generated";
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

export type BarLayout = "compact" | "hover" | "voice" | "full";

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
  hover: { width: 132, height: 34, band: 34, pad: 16 },
  voice: { width: 220, height: 34, band: 34, pad: 16 },
  full: { width: 419, height: 44, band: 44, pad: 24 },
};

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
}: {
  layout: BarLayout;
  paneOpen: boolean;
  rosterVisible: boolean;
}) {
  const l = BAR_LAYOUTS[layout];
  const d = FLOATING_BAR_DIMENSIONS;
  return {
    width: l.width + 2 * l.pad,
    height:
      l.band +
      2 * l.pad +
      (rosterVisible ? d.ROSTER_STRIP_HEIGHT : 0) +
      (paneOpen ? d.PANE_GAP + d.PANE_HEIGHT : 0),
    anchorY: l.pad + l.band / 2,
  };
}

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
      else resolve();
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
  // Working, error, success, speaking: room for a label and a control.
  return "full";
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
`;

// === STATUS DOT ===
// One dot; state is communicated through motion and colour, not icons.

/** macOS system blue, the only accent the bar uses. */
const SYSTEM_BLUE = "#0A84FF";

function StatusDot({
  state,
  audioLevel,
  driving = false,
}: {
  state: UIState;
  audioLevel: number;
  driving?: boolean;
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
      return (
        <div
          className={cn(dot, "bg-white")}
          style={{ animation: "fbar-idle 4s ease-in-out infinite" }}
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
  const [hoveredButton, setHoveredButton] = useState<"mic" | "type" | "chat" | null>(null);
  const micRef = useRef<HTMLButtonElement>(null);
  const typeRef = useRef<HTMLButtonElement>(null);
  const chatRef = useRef<HTMLButtonElement>(null);
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
        hit(micRef) ? "mic" : hit(typeRef) ? "type" : hit(chatRef) ? "chat" : null,
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
  const reopenPane = useCallback(() => setPaneShown(true), []);

  // The full-size window taking over, and handing back.
  useEventListener(EVENTS.BAR_MAIN_WINDOW_OPENED, () => setMainWindowOpen(true));
  useEventListener(EVENTS.BAR_MAIN_WINDOW_CLOSED, () => setMainWindowOpen(false));

  // The text input is local until submit, so there is no per-keystroke IPC.
  const [inputOpen, setInputOpen] = useState(false);

  // Starting a new chat means wanting to type, so the pane stays up, empty,
  // with the caret in it. Rotating the backend conversation matters too: the
  // bar used to clear the screen while the agent kept appending to the same
  // conversation and memory buffer.
  const startNewChat = useCallback(() => {
    void invoke("new_conversation").catch(() => {});
    chat.startNewChat();
    setPaneShown(true);
    setInputOpen(true);
  }, [chat.startNewChat]);

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
  const inputRef = useRef<HTMLInputElement>(null);

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

  /** Stop listening and throw it away. Nothing is transcribed, nothing sent. */
  const cancelTalking = useCallback(async () => {
    try {
      await invoke("agent_voice", { action: "cancel" });
    } catch (error) {
      console.error("❌ FloatingBar: failed to cancel listening:", error);
    }
  }, []);

  /** Changed their mind about talking: drop the audio and open the input. */
  const switchToTyping = useCallback(async () => {
    await cancelTalking();
    openInput();
  }, [cancelTalking, openInput]);

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      const trimmedValue = localInputValue.trim();
      if (!trimmedValue) return;
      await sendInteraction(
        createInteraction(UI.INTERACTION_TYPES_SUBMIT, { value: trimmedValue }),
      );
      setLocalInputValue("");
    },
    [localInputValue, sendInteraction, createInteraction],
  );

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

  const layout = pickLayout({
    state: currentUiState,
    hovered,
    inputOpen,
    paneOpen,
    rosterVisible: showRosterStrip,
    driving: isDriving,
  });

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

  // On launch the bar always lands in a well, never at an arbitrary spot.
  // A remembered position is re-snapped to the nearest current well (so it
  // survives a resolution / monitor change); a fresh install with nothing saved
  // defaults to the least-intrusive well: top-right on the monitor the bar
  // opened on. Top-right clears the menu bar and the Dock, unlike the bottom
  // wells which do not yet measure the Dock. Runs once, independent of the
  // first resize so it does not fight window sizing.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const win = getCurrentWindow();
        const saved = await invoke<{ x: number; y: number } | null>("get_bar_position");
        const [logical, pos, mons] = await Promise.all([
          logicalWindowSize(win),
          win.outerPosition(),
          availableMonitors(),
        ]);
        if (cancelled || !mons.length) return;

        const wells = computeWells(toMonitorRects(mons), { ...logical, includeCenter: true });
        if (!wells.length) return;

        let target: Well | null = saved ? nearestWell(saved, wells) : null;
        if (!target) {
          // The monitor the window currently sits on (fall back to the first).
          const monIndex = mons.findIndex(
            (m) =>
              pos.x >= m.position.x &&
              pos.x < m.position.x + m.size.width &&
              pos.y >= m.position.y &&
              pos.y < m.position.y + m.size.height,
          );
          const mon = monIndex >= 0 ? monIndex : 0;
          target = wellForSlot(SLOT.topRight, mon, wells) ?? wells[0];
        }
        if (cancelled || !target) return;

        await win.setPosition(new PhysicalPosition(target.x, target.y));
        currentSlotRef.current = { fx: target.fx, fy: target.fy };
        try {
          await invoke("set_bar_position", { x: target.x, y: target.y });
        } catch {
          // best effort; a failed persist just means we re-default next launch
        }
      } catch (error) {
        console.debug("FloatingBar: default/restore well failed:", error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // === WINDOW RESIZING ===

  const { resizeWindowIfChanged } = useWindowSize(windowLabel);
  const lastWindowRef = useRef<{ width: number; height: number } | null>(null);

  useEffect(() => {
    const next = floatingBarWindowSize({ layout, paneOpen, rosterVisible: showRosterStrip });
    const prev = lastWindowRef.current;
    lastWindowRef.current = { width: next.width, height: next.height };
    // growUp is only sent when set, so the downward path's resize config (and
    // its tests) stay byte-for-byte identical.
    const config = growUp ? { ...next, growUp: true } : next;
    const apply = () =>
      resizeWindowIfChanged(config).catch((error) =>
        console.error("❌ FloatingBar: Failed to resize window:", error),
      );
    // Growing: make room first, then the pill animates into it. Shrinking:
    // let the pill animate down before the window snaps around it.
    const shrinking = prev !== null && next.width <= prev.width && next.height <= prev.height;
    if (!shrinking) {
      apply();
      return;
    }
    const t = setTimeout(apply, SHRINK_DELAY_MS);
    return () => clearTimeout(t);
  }, [layout, paneOpen, showRosterStrip, growUp, resizeWindowIfChanged]);

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
  // The drag-well slot the bar currently occupies (col/row), so it can re-home
  // to the same slot on another display when the cursor moves there.
  const currentSlotRef = useRef<WellSlot | null>(null);
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

  const pill = BAR_LAYOUTS[layout];
  const pad = BAR_LAYOUTS[layout].pad;
  const band = BAR_LAYOUTS[layout].band;

  // The pane and roster render either below the pill (default, growing down) or
  // above it (docked in the bottom half, growing up); only the margin side and
  // the render order flip, so build each once and place them by `growUp`.
  const chatPaneNode = paneOpen ? (
    <div className={cn("shrink-0", growUp ? "mb-2" : "mt-2")}>
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
      className="relative flex h-screen w-screen cursor-grab select-none flex-col items-center overflow-hidden active:cursor-grabbing"
      style={{ padding: pad }}
      onMouseDownCapture={onRootMouseDown}
      onMouseMove={onRootMouseMove}
      onMouseUp={onRootMouseUp}
      onClickCapture={onRootClickCapture}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
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
          "relative flex shrink-0 items-center rounded-full",
          "border border-white/10 bg-neutral-950/90 text-white backdrop-blur-xl",
          // Juno has the pointer: a hairline in system blue, nothing louder.
          isDriving && "border-[#0A84FF]/70",
          "transition-[width,height,padding] duration-200 ease-out",
          layout === "compact" ? "shadow-lg" : "shadow-2xl",
          // Idle layouts centre their single child so the compact dot and the
          // hover buttons occupy the same centre — the swap cross-fades in
          // place instead of the dot teleporting to the left edge.
          layout === "compact" || layout === "hover" ? "justify-center" : "gap-2",
          layout === "full" ? "px-4" : layout === "compact" ? "px-0" : "px-2",
        )}
        style={{ width: pill.width, height: pill.height }}
      >
        {/* The status dot lives in the compact idle pill and in the layouts
            that carry real status (voice, working, input). Hover shows only
            the buttons, so nothing shifts sideways when the pill grows. */}
        {layout !== "hover" && (
          <StatusDot
            state={currentUiState}
            audioLevel={barState.audioLevel}
            driving={isDriving}
          />
        )}

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
          </div>
        ) : showInput ? (
          <form
            onSubmit={handleSubmit}
            className="flex min-w-0 flex-1 items-center gap-3"
            style={{ animation: "fbar-content-in 0.2s ease-out both" }}
          >
            <input
              ref={inputRef}
              type="text"
              value={localInputValue}
              onChange={(e) => setLocalInputValue(e.target.value)}
              onMouseDown={activateWindow}
              onFocus={handleFocus}
              onBlur={handleInputBlur}
              placeholder={paneOpen ? "Follow up…" : "Ask Juno"}
              aria-label="Ask Juno"
              className={cn(
                "min-w-0 flex-1 cursor-text border-none bg-transparent outline-none",
                "text-[13px] tracking-[-0.01em] text-white/90 placeholder:text-white/30",
              )}
            />
            {/* Typing used to be Enter or nothing: no way to send by hand, no
                way to reach the mic, and no way out but Escape. */}
            <div className="flex shrink-0 items-center gap-1">
              <button
                type="button"
                onClick={switchToTalking}
                aria-label="Switch to dictation"
                title="Switch to dictation"
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
                onClick={abandonInput}
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
                  onClick={() => void cancelTalking()}
                  aria-label="Cancel without sending"
                  title="Cancel without sending"
                  className={inputControlButton}
                >
                  <X className="size-3" />
                </button>
              </div>
            )}
            {isWorking && (
              <button
                type="button"
                onClick={() => void chat.stop()}
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
