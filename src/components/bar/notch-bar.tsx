"use client";

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { EVENTS, UI } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";
import type { NotchGeometry } from "@/types/bar-config";

// ─── Types ───────────────────────────────────────────────

interface BarStateData {
  barState: string;
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

interface UIInteractionEvent {
  element_id: string;
  interaction_type: string;
  data: Record<string, unknown> | null;
  timestamp: number;
}

// ─── Constants ───────────────────────────────────────────

const COMPONENT_ID = UI.ELEMENT_IDS_NOTCH_BAR;

// Fallback geometry while the backend query is in flight (or off-macOS).
const DEFAULT_GEOMETRY: NotchGeometry = {
  has_notch: false,
  notch_width: 200,
  notch_height: 30,
  menu_bar_height: 24,
  canvas_width: 480,
  canvas_height: 180,
};

// Expansion deltas relative to the notch silhouette. These must stay inside
// the canvas slack the backend reserves (280 horizontal / 150 vertical) —
// the window itself is NEVER resized per state; all transitions are CSS-only
// so the shape stays welded to the bezel while animating.
const PEEK_EXTRA_WIDTH = 70;
const PEEK_EXTRA_HEIGHT = 34;
const INPUT_EXTRA_WIDTH = 240;
const INPUT_EXTRA_HEIGHT = 48;

const IDLE_STATES: string[] = [
  UI.BAR_STATES_DEFAULT,
  UI.BAR_STATES_SHRINKING,
  UI.BAR_STATES_DICTATION_READY,
];

const INPUT_STATES: string[] = [
  UI.BAR_STATES_INPUT,
  UI.BAR_STATES_EXPANDING,
];

const NOTCH_KEYFRAMES = `
@keyframes notch-idle {
  0%, 100% { opacity: 0.35; transform: scale(1); }
  50%      { opacity: 0.6; transform: scale(1.1); }
}
@keyframes notch-working {
  0%   { transform: translateX(0) scale(1); }
  25%  { transform: translateX(3px) scale(0.9); }
  50%  { transform: translateX(0) scale(1); }
  75%  { transform: translateX(-3px) scale(0.9); }
  100% { transform: translateX(0) scale(1); }
}
@keyframes notch-listening {
  0%, 100% { opacity: 0.6; transform: scale(1); }
  50%      { opacity: 1; transform: scale(1.25); }
}
`;

// ─── Helpers ─────────────────────────────────────────────

const getLabel = (data: BarStateData): string | null => {
  switch (data.barState) {
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return "listening";
    case UI.BAR_STATES_TRANSCRIBING:
      return data.transcriptionText || "transcribing";
    case UI.BAR_STATES_SPEAKING: {
      const t = data.spokenText;
      return t ? (t.length > 40 ? `${t.slice(0, 40)}…` : t) : "speaking";
    }
    case UI.BAR_STATES_DICTATING:
      return "dictating";
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return data.agentState || "working";
    case UI.BAR_STATES_ERROR:
      return data.currentError || "something went wrong";
    case UI.BAR_STATES_SUCCESS:
      return "done";
    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_STOPPING:
      return "finishing";
    default:
      return null;
  }
};

const StateDot = ({ state, audioLevel }: { state: string; audioLevel: number }) => {
  const dot = "w-[6px] h-[6px] rounded-full shrink-0";

  if (state === UI.BAR_STATES_ERROR)
    return <div className={cn(dot, "bg-[#e8866a]")} />;
  if (state === UI.BAR_STATES_SUCCESS)
    return <div className={cn(dot, "bg-[#7aba8a]")} />;
  if (
    state === UI.BAR_STATES_LISTENING ||
    state === UI.BAR_STATES_ALWAYS_LISTENING ||
    state === UI.BAR_STATES_DICTATING
  )
    return (
      <div
        className={cn(dot, "bg-white")}
        style={{
          animation: "notch-listening 1.5s ease-in-out infinite",
          opacity: Math.max(0.5, audioLevel),
        }}
      />
    );
  if (IDLE_STATES.includes(state) || INPUT_STATES.includes(state))
    return (
      <div
        className={cn(dot, "bg-white/80")}
        style={{ animation: "notch-idle 4s ease-in-out infinite" }}
      />
    );
  return (
    <div
      className={cn(dot, "bg-white/80")}
      style={{ animation: "notch-working 1.1s ease-in-out infinite" }}
    />
  );
};

// ─── Main component ──────────────────────────────────────

export function NotchBar() {
  const [geometry, setGeometry] = useState<NotchGeometry>(DEFAULT_GEOMETRY);
  const [barState, setBarState] = useState<BarStateData>({
    barState: UI.BAR_STATES_DEFAULT,
    inputValue: "",
    lastSubmittedValue: "",
    currentError: null,
    transcriptionText: "",
    spokenText: "",
    voiceMode: UI.VOICE_MODES_IDLE,
    audioLevel: 0,
    isAgentWorking: false,
    isDictationMode: false,
    isAlwaysListening: false,
    agentState: null,
  });
  const [hovered, setHovered] = useState(false);
  const [localInputValue, setLocalInputValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  // Inject keyframes once
  useEffect(() => {
    const id = "notch-bar-keyframes";
    if (!document.getElementById(id)) {
      const style = document.createElement("style");
      style.id = id;
      style.textContent = NOTCH_KEYFRAMES;
      document.head.appendChild(style);
    }
  }, []);

  // Load notch geometry from the backend (single source of truth for the
  // silhouette + canvas dimensions; the backend positions the window itself).
  useEffect(() => {
    let mounted = true;
    invoke<NotchGeometry>("get_notch_geometry")
      .then((geo) => {
        if (mounted) setGeometry(geo);
      })
      .catch((error) => {
        console.error("NotchBar: failed to load notch geometry:", error);
      });
    return () => {
      mounted = false;
    };
  }, []);

  useEventListener<BarStateData>(EVENTS.BAR_STATE_UPDATE, (payload) => {
    if (payload && typeof payload === "object" && "barState" in payload) {
      setBarState(payload);
    }
  });

  // ── Interactions ──

  const sendInteraction = useCallback(
    async (type: string, data?: Record<string, unknown>) => {
      const interaction: UIInteractionEvent = {
        element_id: COMPONENT_ID,
        interaction_type: type,
        data: data ?? null,
        timestamp: Date.now(),
      };
      try {
        await invoke("ui_handle_interaction", {
          elementId: COMPONENT_ID,
          interaction,
        });
      } catch (error) {
        console.error("NotchBar: interaction failed:", error);
      }
    },
    [],
  );

  const handleClick = useCallback(() => {
    sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, [sendInteraction]);

  const handleSubmit = useCallback(
    (e: FormEvent) => {
      e.preventDefault();
      const value = localInputValue.trim();
      if (!value) return;
      sendInteraction(UI.INTERACTION_TYPES_SUBMIT, { value });
      setLocalInputValue("");
    },
    [localInputValue, sendInteraction],
  );

  // Sync backend input value; focus the field when input mode opens
  useEffect(() => {
    setLocalInputValue(barState.inputValue);
  }, [barState.inputValue]);

  const isInput = INPUT_STATES.includes(barState.barState);
  const isIdle = IDLE_STATES.includes(barState.barState);

  useEffect(() => {
    if (barState.barState === UI.BAR_STATES_INPUT) {
      const t = setTimeout(() => inputRef.current?.focus(), 60);
      return () => clearTimeout(t);
    }
  }, [barState.barState]);

  useEffect(() => {
    if (isIdle) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") sendInteraction(UI.INTERACTION_TYPES_ESCAPE);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [isIdle, sendInteraction]);

  // ── Shape dimensions (CSS-only transitions, fixed window canvas) ──

  const label = getLabel(barState);
  const expanded = !isIdle || hovered;

  let shapeWidth = geometry.notch_width;
  let shapeHeight = geometry.notch_height;
  if (isInput) {
    shapeWidth += INPUT_EXTRA_WIDTH;
    shapeHeight += INPUT_EXTRA_HEIGHT;
  } else if (expanded) {
    shapeWidth += PEEK_EXTRA_WIDTH;
    shapeHeight += PEEK_EXTRA_HEIGHT;
  }

  // On notch-less displays the shape is a pill floating inside the menu bar
  // rather than an extension of the hardware cutout.
  const pill = !geometry.has_notch;

  return (
    <div
      data-testid="notch-bar"
      className="h-screen w-screen overflow-hidden bg-transparent flex items-start justify-center"
    >
      <div
        data-testid="notch-shape"
        role="button"
        aria-label="Juno notch bar"
        onClick={isIdle ? handleClick : undefined}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        className={cn(
          "bg-black flex flex-col overflow-hidden select-none",
          "transition-[width,height,border-radius] duration-300 ease-[cubic-bezier(0.32,0.72,0,1)]",
          pill
            ? expanded
              ? "mt-[3px] rounded-[18px]"
              : "mt-[3px] rounded-full"
            : expanded
              ? "rounded-b-2xl"
              : "rounded-b-[10px]",
          isIdle && "cursor-pointer",
        )}
        style={{
          width: shapeWidth,
          height: pill && !expanded ? shapeHeight - 6 : shapeHeight,
        }}
      >
        {/* Spacer covering the hardware cutout — content renders below it */}
        {!pill && (
          <div style={{ height: geometry.notch_height }} className="shrink-0" />
        )}

        {isInput ? (
          <form
            onSubmit={handleSubmit}
            className={cn(
              "flex items-center gap-2.5 px-4 flex-1 min-h-0",
              pill && "py-1",
            )}
          >
            <StateDot state={barState.barState} audioLevel={barState.audioLevel} />
            <input
              ref={inputRef}
              type="text"
              value={localInputValue}
              onChange={(e) => setLocalInputValue(e.target.value)}
              placeholder="Ask Juno"
              className={cn(
                "flex-1 min-w-0 bg-transparent border-none outline-none",
                "text-[13px] text-white/90 placeholder:text-white/25 tracking-[-0.02em]",
              )}
              disabled={barState.barState !== UI.BAR_STATES_INPUT}
            />
            <span
              className={cn(
                "text-[11px] tracking-[0.04em] select-none shrink-0 transition-opacity duration-200",
                localInputValue.trim() ? "text-white/25 opacity-100" : "opacity-0",
              )}
            >
              return
            </span>
          </form>
        ) : (
          <div
            className={cn(
              "flex items-center justify-center gap-2.5 px-4 flex-1 min-h-0",
              "transition-opacity duration-200",
              expanded ? "opacity-100" : "opacity-0",
            )}
          >
            <StateDot state={barState.barState} audioLevel={barState.audioLevel} />
            {label && (
              <span
                data-testid="notch-label"
                className={cn(
                  "text-[12px] tracking-[-0.01em] truncate",
                  barState.barState === UI.BAR_STATES_ERROR
                    ? "text-[#e8866a]/80"
                    : "text-white/50",
                )}
              >
                {label}
              </span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
