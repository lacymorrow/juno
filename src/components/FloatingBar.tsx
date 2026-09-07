/**
 * FloatingBar.tsx — the default bar appearance, and the reference
 * implementation of the standardized UI API:
 *
 * 1. Event-driven state via BAR_STATE_UPDATE (backend is the source of truth)
 * 2. User interactions via ui_handle_interaction
 * 3. Type-safe inline types aligned with the backend
 * 4. Window resizing owned by the component, top-anchored
 *
 * Shape: a compact dark pill until a query occurs. The moment the backend
 * announces a user message (`user-message-submitted` — typed here, spoken,
 * sent from the main window or a rendered component), the same chat pane the
 * main window renders opens underneath the pill inside this window, dark
 * themed, and the response streams into it. Follow-ups go through the pill's
 * input; the pane stays until dismissed (close, Escape while idle, New chat).
 */

import { useEffect, useState, useCallback, useRef, FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Square } from "lucide-react";

import { useWindowSize } from "@/hooks/useWindowSize";
import { useDragWindow } from "@/hooks/useDragWindow";
import { useAgentSessions } from "@/hooks/useAgentSessions";
import { useBarConversation } from "@/hooks/useBarConversation";
import { cn } from "@/lib/utils";
import { EVENTS, UI } from "@/lib/constants.generated";
import { AgentRosterStrip } from "./AgentRosterStrip";
import { BarChatPane } from "./bar/BarChatPane";
import type { BarAppearance } from "@/components/bar/barAppearance";

// === STANDARDIZED UI API TYPES ===

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

// === COMPONENT CONSTANTS ===

/**
 * Fixed width: the pill never changes width, so nothing jumps horizontally.
 * Height is the only thing that varies, and the window is top-anchored, so
 * growth happens below the pill (see useWindowSize.centerStableResize).
 */
export const FLOATING_BAR_DIMENSIONS = {
  WIDTH: 419,
  BAR_HEIGHT: 44,
  SHADOW_PADDING: 48, // 24px per side, room for the pill/pane shadows
  ROSTER_STRIP_HEIGHT: 34, // 22px strip + 6px gap + breathing room (LAC-2830 §3)
  PANE_GAP: 8,
  PANE_HEIGHT: 360,
};

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
  UI.BAR_STATES_TRANSCRIBING,
  UI.BAR_STATES_DICTATING,
  UI.BAR_STATES_ALWAYS_LISTENING,
];

const WORKING_STATES: readonly string[] = [
  UI.BAR_STATES_SUBMITTING,
  UI.BAR_STATES_LOADING,
  UI.BAR_STATES_AGENT_RESPONDING,
  UI.BAR_STATES_FINISHING,
  UI.BAR_STATES_STOPPING,
];

/**
 * Window height for a given layout. Exported so tests and other bars can
 * assert the exact figures instead of re-deriving them.
 */
export function floatingBarWindowSize({
  paneOpen,
  rosterVisible,
}: {
  paneOpen: boolean;
  rosterVisible: boolean;
}) {
  const d = FLOATING_BAR_DIMENSIONS;
  return {
    width: d.WIDTH + d.SHADOW_PADDING,
    height:
      d.BAR_HEIGHT +
      d.SHADOW_PADDING +
      (rosterVisible ? d.ROSTER_STRIP_HEIGHT : 0) +
      (paneOpen ? d.PANE_GAP + d.PANE_HEIGHT : 0),
  };
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
`;

// === STATUS DOT ===
// One dot; state is communicated through motion and colour, not icons.

function StatusDot({ state, audioLevel }: { state: UIState; audioLevel: number }) {
  const dot = "size-[7px] shrink-0 rounded-full";

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

  // === CONVERSATION (same pipeline as the main window) ===

  const chat = useBarConversation();

  // The pane opens on the first message and reopens whenever a new user
  // message arrives after a dismissal; "New chat" clears everything.
  const [paneDismissed, setPaneDismissed] = useState(false);
  const userMessageCount = chat.messages.filter((m) => m.role === "user").length;
  const seenUserMessagesRef = useRef(0);
  useEffect(() => {
    if (userMessageCount > seenUserMessagesRef.current) {
      setPaneDismissed(false);
    }
    seenUserMessagesRef.current = userMessageCount;
  }, [userMessageCount]);

  const paneOpen = chat.messages.length > 0 && !paneDismissed;

  const dismissPane = useCallback(() => setPaneDismissed(true), []);
  const startNewChat = useCallback(() => {
    chat.startNewChat();
    setPaneDismissed(false);
  }, [chat.startNewChat]);

  // === WINDOW RESIZING ===

  const { resizeWindowIfChanged } = useWindowSize(windowLabel);

  // Parallel agent sessions (LAC-1432): the roster strip appears below the
  // bar when 2+ agents run, so the window grows to make room for it.
  const { sessions: agentSessions, focusSession } = useAgentSessions();
  const showRosterStrip = agentSessions.length >= 2;

  useEffect(() => {
    resizeWindowIfChanged(
      floatingBarWindowSize({ paneOpen, rosterVisible: showRosterStrip }),
    ).catch((error) => console.error("❌ FloatingBar: Failed to resize window:", error));
  }, [paneOpen, showRosterStrip, resizeWindowIfChanged]);

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

  const handleClick = useCallback(async () => {
    await sendInteraction(createInteraction(UI.INTERACTION_TYPES_CLICK));
  }, [sendInteraction, createInteraction]);

  // Input is local until submit — no per-keystroke IPC.
  const [localInputValue, setLocalInputValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setLocalInputValue(barState.inputValue);
  }, [barState.inputValue]);

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
   * OS-level focus changes (Cmd+Tab, clicking another window). The input's
   * own onFocus/onBlur only fire for focus moves inside the webview.
   *
   * The first click on this window while another app is active only makes
   * the window key; the webview never sees it, so the input would need a
   * second click. When the window becomes key, make the webview the window's
   * first responder (keystrokes otherwise never reach the page, even with a
   * visible caret) and focus the input, so one click is enough.
   * (`acceptFirstMouse` is not the answer: it delivers the click to the page
   * but stops it from activating the app, so typing goes to the previous app.)
   */
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;

    const setup = async () => {
      try {
        const fn = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
          if (!mounted) return;
          if (focused) {
            handleFocus();
            getCurrentWebview()
              .setFocus()
              .catch((error) => console.debug("FloatingBar: webview focus failed:", error));
            inputRef.current?.focus();
          } else {
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
  }, [handleFocus, handleBlur]);

  // === DERIVED UI STATE ===

  const currentUiState = barState.barState;
  const isIdle = IDLE_STATES.includes(currentUiState);
  const isInputState = INPUT_STATES.includes(currentUiState);
  const isVoice = VOICE_STATES.includes(currentUiState);
  const isWorking = WORKING_STATES.includes(currentUiState) || chat.isProcessing;

  // The pill is an input whenever nothing is running: idle, the backend's
  // input states, and between turns with the pane open, so a query or a
  // follow-up is one click away (focusing it tells the backend to expand).
  // Voice and working states show status instead. Dictation-ready keeps its
  // label unless the pane is up, since the next thing that happens is speech.
  const showInput =
    !isWorking &&
    (isInputState ||
      currentUiState === UI.BAR_STATES_DEFAULT ||
      currentUiState === UI.BAR_STATES_SHRINKING ||
      (paneOpen && isIdle));
  const label = statusLabel(currentUiState, barState);

  // Refocus the input when a turn ends while this window is still the one
  // the user is in, so the follow-up can be typed straight away. Never steal
  // focus from another app: element focus in a non-key window is inert.
  useEffect(() => {
    if (!showInput || !document.hasFocus()) return;
    const t = setTimeout(() => inputRef.current?.focus(), 60);
    return () => clearTimeout(t);
  }, [showInput]);

  /**
   * Escape while idle closes the pane. Escape while work is in progress is
   * deliberately NOT handled here: the passive stop-key monitor in Rust
   * (platform/stop_key_monitor.rs) sees it and stops everything.
   */
  useEffect(() => {
    if (!paneOpen || isWorking) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") dismissPane();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [paneOpen, isWorking, dismissPane]);

  const onDragMouseDown = useDragWindow();

  // === RENDER ===

  return (
    <div
      className="relative flex h-screen w-screen cursor-grab flex-col items-center overflow-hidden p-6 active:cursor-grabbing"
      onMouseDown={onDragMouseDown}
    >
      <div
        data-testid="floating-bar"
        data-state={currentUiState}
        className={cn(
          "relative flex h-11 w-[419px] shrink-0 items-center gap-3 rounded-full px-4",
          "border border-white/10 bg-neutral-950/90 text-white shadow-2xl backdrop-blur-xl",
          "transition-colors duration-300 ease-in-out",
          isIdle && !showInput && "cursor-pointer",
        )}
        onClick={isIdle && !showInput ? handleClick : undefined}
        role={isIdle && !showInput ? "button" : undefined}
        aria-label={isIdle && !showInput ? "Activate assistant" : undefined}
      >
        <StatusDot state={currentUiState} audioLevel={barState.audioLevel} />

        {showInput ? (
          <form onSubmit={handleSubmit} className="flex min-w-0 flex-1 items-center gap-3">
            <input
              ref={inputRef}
              type="text"
              value={localInputValue}
              onChange={(e) => setLocalInputValue(e.target.value)}
              onFocus={handleFocus}
              onBlur={handleBlur}
              placeholder={paneOpen ? "Follow up…" : "Ask Juno"}
              aria-label="Ask Juno"
              className={cn(
                "min-w-0 flex-1 border-none bg-transparent outline-none",
                "text-[13px] tracking-[-0.01em] text-white/90 placeholder:text-white/30",
              )}
            />
            <span
              className={cn(
                "shrink-0 select-none text-[11px] tracking-[0.04em] text-white/30 transition-opacity duration-200",
                localInputValue.trim() ? "opacity-100" : "opacity-0",
              )}
              aria-hidden="true"
            >
              return
            </span>
          </form>
        ) : isIdle ? (
          <span className="min-w-0 flex-1 truncate text-[13px] tracking-[-0.01em] text-white/35">
            {label ?? "Ask Juno"}
          </span>
        ) : (
          <>
            <span
              className={cn(
                "min-w-0 flex-1 truncate text-[13px] tracking-[-0.01em]",
                currentUiState === UI.BAR_STATES_ERROR ? "text-[#e8866a]/80" : "text-white/55",
              )}
              data-testid="floating-bar-status"
            >
              {label}
            </span>
            {isVoice && <AudioLevelBars audioLevel={barState.audioLevel} />}
            {isWorking && (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  void chat.stop();
                }}
                aria-label="Stop"
                title="Stop (Esc)"
                className="flex size-6 shrink-0 items-center justify-center rounded-full bg-white/[0.08] text-white/60 transition-colors hover:bg-white/[0.16] hover:text-white"
              >
                <Square className="size-2.5 fill-current" />
              </button>
            )}
          </>
        )}
      </div>

      {/* Parallel-agent roster (LAC-2830 §3): appears when 2+ agents run.
          Clicking a dot focuses that agent; background sessions keep working. */}
      {showRosterStrip && (
        <AgentRosterStrip sessions={agentSessions} onFocus={focusSession} className="mt-1.5" />
      )}

      {paneOpen && (
        <div className="mt-2 shrink-0">
          <BarChatPane
            messages={chat.messages}
            isProcessing={isWorking}
            height={FLOATING_BAR_DIMENSIONS.PANE_HEIGHT}
            copyingMessageId={chat.copyingMessageId}
            savingMessageId={chat.savingMessageId}
            onCopyResponse={chat.handleCopyResponse}
            onSaveResponse={chat.handleSaveResponse}
            onApprovalUpdate={chat.handleApprovalUpdate}
            onContinuationUpdate={chat.handleContinuationUpdate}
            onDismiss={dismissPane}
            onNewChat={startNewChat}
          />
        </div>
      )}
    </div>
  );
}
