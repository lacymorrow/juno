"use client";

import {
  useState,
  useEffect,
  useCallback,
  useMemo,
  useRef,
  type FormEvent,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cn } from "@/lib/utils";
import {
  DynamicIsland,
  DynamicIslandProvider,
  useDynamicIslandSize,
  type SizePresets,
} from "@/components/ui/dynamic-island";
import { EVENTS, UI } from "@/lib/constants.generated";
import { useWindowSize } from "@/hooks/useWindowSize";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";
import type { BarAppearance } from "@/components/bar/barAppearance";
import { MixedContentRenderer } from "@/components/ui/mixed-content-renderer";

// ─── Utilities ───────────────────────────────────────────

function debounce<T extends (...args: any[]) => any>(
  func: T,
  delay: number,
) {
  let timeoutId: NodeJS.Timeout;
  const debounced = (...args: Parameters<T>) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => func(...args), delay);
  };
  debounced.cancel = () => clearTimeout(timeoutId);
  return debounced;
}

// ─── Types ───────────────────────────────────────────────

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
  | typeof UI.BAR_STATES_AGENT_RESPONDING;

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

interface UIInteractionEvent {
  element_id: string;
  interaction_type: string;
  data: Record<string, any> | null;
  timestamp: number;
}

// ─── Constants ───────────────────────────────────────────

const SHADOW_PADDING = 48;
const COMPONENT_ID = "dynamic-bar";

// Custom keyframes — injected once into <head>.
// These replace Tailwind's default animate-pulse / animate-spin
// with organic, purpose-built motions for each state.
const BAR_KEYFRAMES = `
@keyframes bar-idle {
  0%, 100% { opacity: 0.4; transform: scale(1); }
  50%      { opacity: 0.55; transform: scale(1.08); }
}
@keyframes bar-orbit {
  0%   { transform: translateX(0) scale(1); }
  25%  { transform: translateX(4px) scale(0.92); }
  50%  { transform: translateX(0) scale(1); }
  75%  { transform: translateX(-4px) scale(0.92); }
  100% { transform: translateX(0) scale(1); }
}
@keyframes bar-shimmer {
  0%   { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}
@keyframes bar-flash {
  0%   { opacity: 1; transform: scale(1.5); }
  100% { opacity: 0.4; transform: scale(1); }
}
@keyframes bar-shake {
  0%, 100% { transform: translateX(0); }
  20%  { transform: translateX(-2px); }
  40%  { transform: translateX(2px); }
  60%  { transform: translateX(-1px); }
  80%  { transform: translateX(1px); }
}
@media (prefers-reduced-motion: reduce) {
  @keyframes bar-idle { 0%, 100% { opacity: 0.45; } }
  @keyframes bar-orbit { 0%, 100% { transform: none; } }
  @keyframes bar-shimmer { 0%, 100% { background-position: 0 0; } }
  @keyframes bar-flash { 0%, 100% { transform: none; } }
  @keyframes bar-shake { 0%, 100% { transform: none; } }
}
`;

// ─── Audio bars ──────────────────────────────────────────
// 3 thin reactive bars for audio states. Height tracks audioLevel.
// Replaces the dot for listening/speaking — immediately reads as "sound."

const AUDIO_BAR_RATIOS = [0.5, 1.0, 0.65];
const AUDIO_BAR_MIN = 4;
const AUDIO_BAR_MAX = 14;

const AudioBars = ({
  level,
  opacity = 0.55,
}: {
  level: number;
  opacity?: number;
}) => (
  <div className="flex items-center gap-[2px] shrink-0" style={{ opacity }}>
    {AUDIO_BAR_RATIOS.map((ratio, i) => (
      <div
        key={i}
        className="w-[1.5px] rounded-full bg-white transition-all duration-[120ms] ease-out"
        style={{
          height: `${AUDIO_BAR_MIN + (AUDIO_BAR_MAX - AUDIO_BAR_MIN) * ratio * Math.max(0.15, level)}px`,
        }}
      />
    ))}
  </div>
);

// ─── State indicator ─────────────────────────────────────
// Renders the appropriate visual for each state category:
//   Dot → idle, input, finishing (passive)
//   AudioBars → listening, speaking, dictating (audio)
//   Orbiting dot → processing, transcribing (computing)
//   Colored dot → error, success (terminal)

const StateIndicator = ({
  state,
  audioLevel,
}: {
  state: UIState;
  audioLevel: number;
}) => {
  const dot = "w-[7px] h-[7px] rounded-full shrink-0";

  switch (state) {
    // ── Passive: dot ──
    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_DICTATION_READY:
      return (
        <div
          className={cn(dot, "bg-white")}
          style={{ animation: "bar-idle 4s ease-in-out infinite" }}
        />
      );

    case UI.BAR_STATES_INPUT:
    case UI.BAR_STATES_EXPANDING:
      return <div className={cn(dot, "bg-white/70")} />;

    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_SHRINKING:
      return (
        <div className={cn(dot, "bg-white/25 transition-opacity duration-500")} />
      );

    // ── Audio: bars ──
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return <AudioBars level={audioLevel} opacity={0.65} />;

    case UI.BAR_STATES_SPEAKING:
      return <AudioBars level={audioLevel} opacity={0.4} />;

    case UI.BAR_STATES_DICTATING:
      return <AudioBars level={audioLevel} opacity={0.55} />;

    // ── Computing: orbiting dot ──
    case UI.BAR_STATES_TRANSCRIBING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return (
        <div
          className={cn(dot, "bg-white/80")}
          style={{ animation: "bar-orbit 1.1s ease-in-out infinite" }}
        />
      );

    // ── Terminal: colored dot ──
    case UI.BAR_STATES_ERROR:
      return (
        <div
          className={cn(dot, "bg-[#e8866a]")}
          style={{ animation: "bar-shake 0.4s ease-out" }}
        />
      );

    case UI.BAR_STATES_SUCCESS:
      return (
        <div
          className={cn(dot, "bg-[#7aba8a]")}
          style={{ animation: "bar-flash 0.6s ease-out forwards" }}
        />
      );

    default:
      return <div className={cn(dot, "bg-white/35")} />;
  }
};

// ─── State → island size preset ──────────────────────────

const getIslandSize = (state: UIState): SizePresets => {
  switch (state) {
    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_DICTATION_READY:
      return "default";
    case UI.BAR_STATES_INPUT:
    case UI.BAR_STATES_EXPANDING:
      return "long";
    case UI.BAR_STATES_TRANSCRIBING:
      return "compactLong";
    case UI.BAR_STATES_AGENT_RESPONDING:
      return "medium";
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return "large";
    default:
      return "compact";
  }
};

// State label — lowercase, no trailing ellipsis.
// Returns null for states that don't need a label.
const getLabel = (state: UIState, data: BarStateData): string | null => {
  switch (state) {
    case UI.BAR_STATES_LISTENING:
      return "listening";
    case UI.BAR_STATES_TRANSCRIBING:
      return data.transcriptionText || "transcribing";
    case UI.BAR_STATES_SPEAKING: {
      const t = data.spokenText;
      return t ? (t.length > 32 ? t.slice(0, 32) + "\u2026" : t) : "speaking";
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
      return "finishing";
    default:
      return null;
  }
};

// State → window dimensions (must match island size presets + padding)
const getDimensions = (state: UIState) => {
  let w = 150, h = 44;
  switch (state) {
    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_DICTATION_READY:
      break; // 150 × 44
    case UI.BAR_STATES_TRANSCRIBING:
      w = 300; h = 56;
      break;
    case UI.BAR_STATES_INPUT:
    case UI.BAR_STATES_EXPANDING:
      w = 371; h = 84;
      break;
    case UI.BAR_STATES_AGENT_RESPONDING:
      w = 371; h = 210;
      break;
    case UI.BAR_STATES_ALWAYS_LISTENING:
      w = 371; h = 84;
      break;
    default:
      w = 235; h = 44;
  }
  return { width: w + SHADOW_PADDING, height: h + SHADOW_PADDING };
};

// ─── Main component ──────────────────────────────────────

const DynamicBarContent = (_props: { barAppearance?: BarAppearance }) => {
  const { setSize } = useDynamicIslandSize();

  // Inject keyframes once
  useEffect(() => {
    const id = "dynamic-bar-keyframes";
    if (!document.getElementById(id)) {
      const style = document.createElement("style");
      style.id = id;
      style.textContent = BAR_KEYFRAMES;
      document.head.appendChild(style);
    }
  }, []);

  // ── State ──

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

  const [agentResponseContent, setAgentResponseContent] = useState<
    string | null
  >(null);
  const [localInputValue, setLocalInputValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  // ── Event listeners ──

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let mounted = true;
    const setup = async () => {
      try {
        const fn = await listen<BarStateData>(
          EVENTS.BAR_STATE_UPDATE,
          (event) => {
            if (!mounted) return;
            const p = event.payload;
            if (p && typeof p === "object" && "barState" in p) setBarState(p);
          },
        );
        if (mounted) {
          unlisten = fn;
        } else {
          safeCleanupEventListener(fn);
        }
      } catch (e) {
        console.error("DynamicBar: listener setup failed:", e);
      }
    };
    setup();
    return () => {
      mounted = false;
      safeCleanupEventListener(unlisten);
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let mounted = true;
    const setup = async () => {
      try {
        unlisten = await listen<{
          message_id: string;
          complete_text: string;
          is_jsx?: boolean;
          agent_state?: string;
        }>(EVENTS.STREAMING_STREAM_END, (event) => {
          if (!mounted) return;
          if (event.payload.complete_text)
            setAgentResponseContent(event.payload.complete_text);
        });
      } catch (e) {
        console.error("DynamicBar: stream-end listener failed:", e);
      }
    };
    setup();
    return () => {
      mounted = false;
      safeCleanupEventListener(unlisten);
    };
  }, []);

  // Clear response on return to idle
  useEffect(() => {
    if (barState.barState === UI.BAR_STATES_DEFAULT)
      setAgentResponseContent(null);
  }, [barState.barState]);

  // ── Island size ──

  useEffect(() => {
    setSize(getIslandSize(barState.barState));
  }, [barState.barState, setSize]);

  // ── Window resize ──

  const { resizeWindowIfChanged } = useWindowSize("floating-bar");

  const debouncedResize = useMemo(
    () =>
      debounce((state: BarStateData) => {
        resizeWindowIfChanged(getDimensions(state.barState));
      }, 100),
    [resizeWindowIfChanged],
  );

  // Clean up debounce timer on unmount
  useEffect(() => {
    return () => debouncedResize.cancel();
  }, [debouncedResize]);

  useEffect(() => {
    // Skip when agent content is displayed — its own effect handles sizing
    if (
      agentResponseContent &&
      (barState.barState === UI.BAR_STATES_AGENT_RESPONDING ||
        barState.barState === UI.BAR_STATES_SUCCESS ||
        barState.barState === UI.BAR_STATES_FINISHING)
    )
      return;
    debouncedResize(barState);
  }, [barState, debouncedResize, agentResponseContent]);

  // ── Interactions ──

  const createInteraction = useCallback(
    (type: string, data?: Record<string, any>): UIInteractionEvent => ({
      element_id: COMPONENT_ID,
      interaction_type: type,
      data: data || null,
      timestamp: Date.now(),
    }),
    [],
  );

  const sendInteraction = useCallback(
    async (interaction: UIInteractionEvent) => {
      try {
        await invoke("ui_handle_interaction", {
          elementId: COMPONENT_ID,
          interaction,
        });
      } catch (e) {
        console.error("DynamicBar: interaction failed:", e);
      }
    },
    [],
  );

  const handleIslandClick = useCallback(async () => {
    await sendInteraction(createInteraction(UI.INTERACTION_TYPES_CLICK));
  }, [sendInteraction, createInteraction]);

  // Input sync & focus
  useEffect(() => {
    setLocalInputValue(barState.inputValue);
  }, [barState.inputValue]);

  useEffect(() => {
    if (barState.barState === UI.BAR_STATES_INPUT) {
      const t = setTimeout(() => inputRef.current?.focus(), 60);
      return () => clearTimeout(t);
    }
  }, [barState.barState]);

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      const v = localInputValue.trim();
      if (!v) return;
      await sendInteraction(
        createInteraction(UI.INTERACTION_TYPES_SUBMIT, { value: v }),
      );
      setLocalInputValue("");
    },
    [localInputValue, sendInteraction, createInteraction],
  );

  const handleInputChange = useCallback((v: string) => {
    setLocalInputValue(v);
  }, []);

  const handleFocus = useCallback(async () => {
    await sendInteraction(createInteraction(UI.INTERACTION_TYPES_FOCUS));
  }, [sendInteraction, createInteraction]);

  const handleBlur = useCallback(async () => {
    await sendInteraction(createInteraction(UI.INTERACTION_TYPES_BLUR));
  }, [sendInteraction, createInteraction]);

  // Keyboard shortcuts
  useEffect(() => {
    if (barState.barState === UI.BAR_STATES_DEFAULT) return;

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape")
        sendInteraction(createInteraction(UI.INTERACTION_TYPES_ESCAPE));
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey))
        sendInteraction(createInteraction(UI.INTERACTION_TYPES_ENTER));
    };

    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [barState.barState]);

  // ── Derived state ──

  const showAgentContent =
    agentResponseContent &&
    (barState.barState === UI.BAR_STATES_AGENT_RESPONDING ||
      barState.barState === UI.BAR_STATES_SUCCESS ||
      barState.barState === UI.BAR_STATES_FINISHING);

  const isProcessing =
    barState.barState === UI.BAR_STATES_LOADING ||
    barState.barState === UI.BAR_STATES_SUBMITTING ||
    barState.barState === UI.BAR_STATES_TRANSCRIBING ||
    barState.barState === UI.BAR_STATES_AGENT_RESPONDING;

  const isError = barState.barState === UI.BAR_STATES_ERROR;
  const isSuccess = barState.barState === UI.BAR_STATES_SUCCESS;

  useEffect(() => {
    if (showAgentContent) {
      setSize("medium");
      resizeWindowIfChanged({
        width: 371 + SHADOW_PADDING,
        height: 210 + SHADOW_PADDING,
      });
    }
  }, [showAgentContent, setSize, resizeWindowIfChanged]);

  const isInputState =
    barState.barState === UI.BAR_STATES_INPUT ||
    barState.barState === UI.BAR_STATES_EXPANDING;

  const isIdle =
    barState.barState === UI.BAR_STATES_DEFAULT ||
    barState.barState === UI.BAR_STATES_DICTATION_READY;

  const label = getLabel(barState.barState, barState);

  // ── Render ──

  return (
    <div className="h-full w-full relative p-6" data-tauri-drag-region>
      <div
        className="flex items-center justify-center h-full"
        data-tauri-drag-region
      >
        {isInputState ? (
          // ── Input mode ──
          <DynamicIsland id="ai-chatbot-panel">
            <form
              onSubmit={handleSubmit}
              className={cn(
                "flex items-center w-full h-full px-5 gap-3",
                "transition-opacity duration-300",
                barState.barState === UI.BAR_STATES_INPUT
                  ? "opacity-100"
                  : "opacity-0",
              )}
            >
              <StateIndicator
                state={barState.barState}
                audioLevel={barState.audioLevel}
              />
              <div className="flex-1 min-w-0 flex flex-col justify-center">
                <input
                  ref={inputRef}
                  type="text"
                  value={localInputValue}
                  onChange={(e) => handleInputChange(e.target.value)}
                  onFocus={handleFocus}
                  onBlur={handleBlur}
                  className={cn(
                    "w-full bg-transparent border-none outline-none",
                    "text-[13px] text-white/90 placeholder:text-white/20",
                    "tracking-[-0.02em] pb-1.5",
                  )}
                  disabled={barState.barState !== UI.BAR_STATES_INPUT}
                  autoFocus
                />
                <div className="h-px bg-white/[0.06]" />
              </div>
              <span
                className={cn(
                  "text-[11px] tracking-[0.04em] transition-opacity duration-200 select-none shrink-0",
                  localInputValue.trim()
                    ? "text-white/25 opacity-100"
                    : "opacity-0",
                )}
              >
                return
              </span>
            </form>
          </DynamicIsland>
        ) : (
          // ── All other states ──
          <button
            type="button"
            onClick={handleIslandClick}
            className="cursor-pointer bg-transparent p-0 m-0 border-0"
            aria-label="Activate assistant"
          >
            <DynamicIsland id="ai-chatbot-panel">
              {showAgentContent ? (
                // Agent response
                <div className="p-4 overflow-y-auto max-h-[200px] text-white/80 text-[13px] leading-[1.6] tracking-[-0.01em]">
                  <MixedContentRenderer content={agentResponseContent} />
                </div>
              ) : (
                // State-aware wrapper — background communicates state family
                <div
                  className={cn(
                    "w-full h-full transition-colors duration-300",
                    isError && "bg-[#e8866a]/[0.07]",
                    isSuccess && "bg-[#7aba8a]/[0.07]",
                  )}
                  style={
                    isProcessing
                      ? {
                          background:
                            "linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.025) 50%, transparent 100%)",
                          backgroundSize: "200% 100%",
                          animation: "bar-shimmer 2.5s ease-in-out infinite",
                        }
                      : undefined
                  }
                >
                  {barState.barState === UI.BAR_STATES_ALWAYS_LISTENING ? (
                    // Always-listening — two lines
                    <div className="flex flex-col items-center justify-center h-full w-full gap-2.5 px-5">
                      <div className="flex items-center gap-3">
                        <StateIndicator
                          state={barState.barState}
                          audioLevel={barState.audioLevel}
                        />
                        <span className="text-white/50 text-[13px] tracking-[-0.02em]">
                          listening
                        </span>
                      </div>
                      <span className="text-white/20 text-[11px] tracking-[0.02em]">
                        say &ldquo;hey juno&rdquo;
                      </span>
                    </div>
                  ) : isIdle ? (
                    // Idle — just the indicator
                    <div className="flex items-center justify-center h-full w-full">
                      <StateIndicator
                        state={barState.barState}
                        audioLevel={barState.audioLevel}
                      />
                    </div>
                  ) : (
                    // Active states — indicator + label
                    <div className="flex items-center h-full w-full px-5 gap-3">
                      <StateIndicator
                        state={barState.barState}
                        audioLevel={barState.audioLevel}
                      />
                      {label && (
                        <span
                          className={cn(
                            "text-[13px] tracking-[-0.02em] truncate",
                            isError
                              ? "text-[#e8866a]/80"
                              : "text-white/40",
                          )}
                        >
                          {label}
                        </span>
                      )}
                    </div>
                  )}
                </div>
              )}
            </DynamicIsland>
          </button>
        )}
      </div>
    </div>
  );
};

// ─── Export ───────────────────────────────────────────────

export function DynamicBar({
  barAppearance,
}: {
  barAppearance?: BarAppearance;
}) {
  return (
    <DynamicIslandProvider initialSize="default">
      <div
        className="h-full w-full bg-transparent overflow-hidden"
        data-tauri-drag-region
      >
        <DynamicBarContent barAppearance={barAppearance} />
      </div>
    </DynamicIslandProvider>
  );
}
