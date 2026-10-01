import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { motion, useReducedMotion } from "motion/react";
import { Volume2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { EVENTS, UI, COMMANDS, WINDOW_LABELS } from "@/lib/constants.generated";
import { useWindowSize } from "@/hooks/useWindowSize";
import { useBarDrag } from "@/hooks/useDragWindow";
import { useEventListener } from "@/hooks/useEventListener";
import { useBarConversation } from "@/hooks/useBarConversation";
import { useSkillAutocomplete } from "@/hooks/useSkillAutocomplete";
import { SkillGhostText, SkillSuggestionList } from "@/components/SkillAutocomplete";
import { MixedContentRenderer } from "@/components/ui/mixed-content-renderer";
import { InputControlNotices } from "@/components/input-control/InputControlNotices";
import { drivingLabel, type InputControlStatePayload } from "@/lib/inputControl";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";
import type { ChatMessage } from "@/types/chat";
import { useLinger } from "../island/useLinger";
import { HaloRing, RING_SVG } from "./HaloRing";
import {
  CLOSE_MS,
  LINGER_MS,
  RING_BOX,
  SHADOW_PAD,
  SHEET_BOTTOM_PAD,
  SHEET_MAX_CONTENT,
  SHEET_RADIUS,
  SHEET_TOP_INSET,
  SHEET_WIDTH,
  SHRINK_DELAY_MS,
  answerKey,
  answerText,
  captionFor,
  gapAngle,
  insideFontSize,
  isIdleState,
  isInputState,
  isVoiceState,
  isWorkingState,
  latestTurn,
  lingerShouldRun,
  placementFor,
  plainInside,
  ringFor,
  sheetHeight,
  tickAngles,
  windowFor,
  type Caption,
  type WindowSize,
} from "./haloModel";

/**
 * Halo: a ring that measures the turn.
 *
 * Spec and the full state table: docs/plans/halo-appearance.md. In short: a
 * thin ring at rest; it fills with your level as you speak; an arc travels it
 * while Juno works and leaves a tick for every tool that finishes; it pulses
 * while Juno speaks; it closes once when the turn is done; it breaks red
 * where something failed. A short answer sits inside the ring. A long one
 * slides out on a sheet from under it. The ring never moves: the window
 * grows around it and downward from it.
 */

/** Backend element id for interactions. Halo shares the floating bar's. */
const COMPONENT_ID = UI.ELEMENT_IDS_FLOATING_BAR;
const WINDOW_LABEL = WINDOW_LABELS.FLOATING_BAR;

/** The dark shape's hairline plus depth, so it reads on a black wallpaper too. */
const SHEET_SHADOW =
  "0 0 0 0.5px rgba(255,255,255,0.12), 0 1px 2px rgba(0,0,0,0.5), 0 10px 28px rgba(0,0,0,0.4)";

const SPRING = { type: "spring" as const, stiffness: 380, damping: 34, mass: 1 };
const EASE = { duration: 0.15, ease: "easeOut" as const };

interface BarStateData {
  barState: string;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;
  transcriptionText: string;
  transcriptionProvisional?: boolean;
  spokenText: string;
  voiceMode: string;
  audioLevel: number;
  isAgentWorking: boolean;
  isDictationMode: boolean;
  isAlwaysListening: boolean;
  agentState: string | null;
}

const INITIAL_BAR: BarStateData = {
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
};

interface Interaction {
  element_id: string;
  interaction_type: string;
  data: Record<string, unknown> | null;
  timestamp: number;
}

async function sendInteraction(type: string, data?: Record<string, unknown>): Promise<void> {
  const interaction: Interaction = {
    element_id: COMPONENT_ID,
    interaction_type: type,
    data: data ?? null,
    timestamp: Date.now(),
  };
  try {
    await invoke(COMMANDS.BAR_UI_HANDLE_INTERACTION, { elementId: COMPONENT_ID, interaction });
  } catch (error) {
    console.error("Halo: interaction failed:", error);
  }
}

// === THE CAPTION ===

const TONE_CLASS: Record<Caption["tone"], string> = {
  live: "text-white/90",
  dim: "text-white/50",
  provisional: "text-white/55 italic",
  error: "text-[#FF6961]",
  plain: "text-white/75",
};

/** The line under the ring. Up to three lines; a long dictation keeps its end. */
function HaloCaption({ caption }: { caption: Caption }) {
  return (
    <p
      data-testid="halo-caption"
      data-tone={caption.tone}
      className={cn(
        "line-clamp-3 text-center text-[13px] leading-[1.4] tracking-[-0.01em]",
        TONE_CLASS[caption.tone],
      )}
    >
      {caption.text}
    </p>
  );
}

// === THE APPROVAL ===

function ApprovalRow({
  msg,
  onDecided,
}: {
  msg: ChatMessage;
  onDecided: (toolId: string, state: "approved" | "denied") => void;
}) {
  const [busy, setBusy] = useState(false);
  const title = (msg.content || msg.tool_name || "do this").trim().replace(/\.$/, "");
  const decide = async (state: "approved" | "denied") => {
    if (!msg.tool_id || busy) return;
    setBusy(true);
    try {
      const command = state === "approved" ? "approve_tool_execution" : "deny_tool_execution";
      const ok = await invoke<boolean>(command, { toolId: msg.tool_id });
      if (ok) onDecided(msg.tool_id, state);
    } catch (error) {
      console.error("Halo: approval failed:", error);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-col items-center gap-2.5" data-testid="halo-approval" role="group" aria-label="Permission">
      <p className="text-center text-[13px] leading-snug text-white/85">Juno wants to {title}</p>
      <div className="flex items-center gap-2">
        <button
          type="button"
          disabled={busy}
          onClick={() => void decide("approved")}
          className={cn(
            "h-7 rounded-full bg-[#0A84FF] px-3.5 text-[12px] font-medium text-white",
            "transition-colors hover:bg-[#2B93FF] active:bg-[#0071E3] disabled:opacity-50",
          )}
        >
          Allow
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => void decide("denied")}
          className={cn(
            "h-7 rounded-full px-3.5 text-[12px] text-white/70",
            "transition-colors hover:bg-white/[0.08] hover:text-white disabled:opacity-50",
          )}
        >
          Don&rsquo;t
        </button>
      </div>
    </div>
  );
}

// === THE BAR ===

export function HaloBar() {
  const reducedMotion = useReducedMotion() ?? false;
  const { resizeWindowIfChanged } = useWindowSize(WINDOW_LABEL);
  const chat = useBarConversation();

  // ── What Rust says ──
  const [bar, setBar] = useState<BarStateData>(INITIAL_BAR);
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let mounted = true;
    listen<BarStateData>(EVENTS.BAR_STATE_UPDATE, (event) => {
      if (!mounted) return;
      const p = event.payload;
      if (p && typeof p === "object" && "barState" in p) setBar(p);
    })
      .then((fn) => {
        if (mounted) unlisten = fn;
        else safeCleanupEventListener(fn);
      })
      .catch((e) => console.error("Halo: listener setup failed:", e));
    return () => {
      mounted = false;
      safeCleanupEventListener(unlisten);
    };
  }, []);
  const state = bar.barState;

  const [driving, setDriving] = useState<InputControlStatePayload | null>(null);
  useEventListener<InputControlStatePayload>(EVENTS.INPUT_CONTROL_STATE, setDriving);
  const isDriving = !!driving?.active;

  // ── The current turn ──
  const turn = useMemo(() => latestTurn(chat.messages), [chat.messages]);
  const key = answerKey(turn);
  const { visible, spoken } = answerText(turn.answer);
  const text = visible || spoken;
  const streaming = !!turn.answer?.isStreaming;
  const working = isWorkingState(state) || chat.isProcessing;
  const approval = turn.approval;

  // ── The answer on stage ──
  // A reply that arrives while the ring is up is shown. A reply that was
  // already there when it mounted (history) is not.
  const [shownKey, setShownKey] = useState("");
  const [spokenOpen, setSpokenOpen] = useState(false);
  const seenKeyRef = useRef(key);
  useEffect(() => {
    if (key && key !== seenKeyRef.current) {
      seenKeyRef.current = key;
      setShownKey(key);
      setSpokenOpen(false);
    }
  }, [key]);
  const barRef = useRef(bar);
  barRef.current = bar;
  const inputRef = useRef("");
  const dismiss = useCallback(() => {
    setShownKey("");
    setSpokenOpen(false);
    // Focus put Rust in its input state while the answer was up. With nothing
    // typed, tell it the composer blurred so it settles to Default.
    if (isInputState(barRef.current.barState) && inputRef.current.trim() === "") {
      void sendInteraction(UI.INTERACTION_TYPES_BLUR);
    }
  }, []);
  const answerOnStage = !!key && shownKey === key;
  const placement = answerOnStage ? placementFor({ text, streaming }) : "none";
  const answerShowing = placement !== "none";
  // The person is speaking again: the ring must be free.
  useEffect(() => {
    if (isVoiceState(state)) dismiss();
  }, [state, dismiss]);

  // ── Juno asking for the cursor ──
  const [askOpen, setAskOpen] = useState(false);
  useEventListener(EVENTS.INPUT_CONTROL_REQUEST, () => setAskOpen(true));
  useEffect(() => {
    if (isDriving || isVoiceState(state)) setAskOpen(false);
  }, [isDriving, state]);

  // ── The close ──
  // Finishing lasts 300ms in Rust; the ring shows its full close for CLOSE_MS
  // whatever comes next, unless the person speaks or something fails.
  const [closing, setClosing] = useState(false);
  const closeTimerRef = useRef<number | null>(null);
  useEffect(() => {
    const done = state === UI.BAR_STATES_FINISHING || state === UI.BAR_STATES_SUCCESS;
    const cut = isVoiceState(state) || state === UI.BAR_STATES_ERROR;
    if (!done && !cut) return;
    if (closeTimerRef.current) window.clearTimeout(closeTimerRef.current);
    closeTimerRef.current = null;
    setClosing(done);
    if (done) {
      closeTimerRef.current = window.setTimeout(() => {
        closeTimerRef.current = null;
        setClosing(false);
      }, CLOSE_MS);
    }
  }, [state]);
  useEffect(
    () => () => {
      if (closeTimerRef.current) window.clearTimeout(closeTimerRef.current);
    },
    [],
  );

  // ── Engagement and the linger ──
  const [hovered, setHovered] = useState(false);
  const [focusWithin, setFocusWithin] = useState(false);
  const [engaged, setEngaged] = useState(false);
  const [lingerEpoch, setLingerEpoch] = useState(0);
  const paused = hovered || focusWithin;
  const counting = lingerShouldRun({
    answerShowing,
    streaming,
    working,
    approvalPending: !!approval,
  });
  const { progress } = useLinger({
    running: counting && !engaged,
    paused,
    durationMs: LINGER_MS,
    resetKey: `${key}:${text.length}:${lingerEpoch}`,
    onExpire: dismiss,
  });
  useEffect(() => {
    if (engaged && !hovered && !focusWithin) {
      setEngaged(false);
      setLingerEpoch((e) => e + 1);
    }
  }, [engaged, hovered, focusWithin]);
  const engage = useCallback(() => setEngaged(true), []);

  // ── Composer ──
  const [input, setInput] = useState("");
  inputRef.current = input;
  const composerRef = useRef<HTMLInputElement>(null);
  const composerOpen = isInputState(state) && !approval;
  const skill = useSkillAutocomplete({ value: input, onAccept: setInput, enabled: composerOpen });
  useEffect(() => {
    if (bar.inputValue === "" && isIdleState(state)) setInput("");
  }, [bar.inputValue, state]);
  const changeInput = useCallback((value: string) => {
    setInput(value);
    void sendInteraction(UI.INTERACTION_TYPES_INPUT_CHANGE, { value });
  }, []);
  const submit = useCallback(
    (event?: FormEvent) => {
      event?.preventDefault();
      event?.stopPropagation();
      const value = input.trim();
      if (!value) return;
      void sendInteraction(UI.INTERACTION_TYPES_SUBMIT, { value });
      setInput("");
    },
    [input],
  );
  useEffect(() => {
    if (!composerOpen) return;
    const t = window.setTimeout(() => composerRef.current?.focus(), 60);
    return () => window.clearTimeout(t);
  }, [composerOpen]);

  // ── OS focus, keys, click ──
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;
    getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (!mounted) return;
        void sendInteraction(focused ? UI.INTERACTION_TYPES_FOCUS : UI.INTERACTION_TYPES_BLUR);
      })
      .then((fn) => {
        if (mounted) unlisten = fn;
        else fn();
      })
      .catch((e) => console.error("Halo: focus listener failed:", e));
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (working) {
          void sendInteraction(UI.INTERACTION_TYPES_ESCAPE);
        } else if (answerShowing) {
          dismiss();
        } else if (composerOpen) {
          void sendInteraction(UI.INTERACTION_TYPES_ESCAPE);
        }
      } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        void sendInteraction(UI.INTERACTION_TYPES_ENTER);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [working, answerShowing, composerOpen, dismiss]);

  const idle = isIdleState(state) && !answerShowing && !approval;
  const onRingClick = useCallback(() => {
    if (idle) void sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, [idle]);

  // ── The ring ──
  const lingerProgress = counting && !engaged ? progress : null;
  const ring = ringFor({
    state,
    audioLevel: bar.audioLevel,
    completedTools: turn.completedTools,
    runningTool: !!turn.runningTool,
    approvalPending: !!approval,
    driving: isDriving,
    closing,
    lingerProgress,
  });
  // Hover on the idle ring brightens it, the way a button lifts.
  const look = idle && hovered ? { ...ring, opacity: Math.min(1, ring.opacity + 0.25) } : ring;
  const ticks = tickAngles(turn.completedTools);
  const gapAt = gapAngle(turn.completedTools);

  // ── The words ──
  const question = turn.question || bar.lastSubmittedValue;
  const caption = captionFor({
    state,
    transcriptionText: bar.transcriptionText,
    transcriptionProvisional: bar.transcriptionProvisional,
    spokenText: bar.spokenText,
    currentError: bar.currentError,
    question,
    runningTool: turn.runningTool,
    drivingLabel: isDriving && driving ? drivingLabel(driving) : null,
    answerShowing,
    approvalPending: !!approval,
  });
  const inside = approval ? "Allow?" : placement === "inside" ? plainInside(text) : null;

  // ── The sheet and the window ──
  const sheetOpen = !!caption || composerOpen || !!approval || placement === "below" || askOpen;

  // Drag from anywhere, land in a gravity well.
  const { dragProps, swallowClickAfterDrag } = useBarDrag({
    displayFollowPaused: sheetOpen || working,
  });

  const [contentH, setContentH] = useState(0);
  const [measureEl, setMeasureEl] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!measureEl || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      const h = Math.ceil(entry.contentRect.height);
      setContentH(h);
      // The cursor notice answered itself and nothing else is under the ring.
      if (h === 0) setAskOpen(false);
    });
    observer.observe(measureEl);
    return () => observer.disconnect();
  }, [measureEl]);
  useEffect(() => {
    if (!sheetOpen) setContentH(0);
  }, [sheetOpen]);
  const win = windowFor(sheetOpen, contentH);
  const targetSheetH = sheetOpen ? sheetHeight(contentH) : 0;

  // Window protocol, same as Pill and Island: growing, resize then animate;
  // shrinking, animate then resize once the sheet has folded. The ring's
  // centre is the same point in both windows, so it does not move.
  const [sheetH, setSheetH] = useState(0);
  const prevWinRef = useRef<WindowSize>(windowFor(false));
  const shrinkTimerRef = useRef<number | null>(null);
  useEffect(() => {
    const prev = prevWinRef.current;
    const growing = win.width > prev.width || win.height > prev.height;
    if (shrinkTimerRef.current) {
      window.clearTimeout(shrinkTimerRef.current);
      shrinkTimerRef.current = null;
    }
    let cancelled = false;
    if (growing) {
      void (async () => {
        await resizeWindowIfChanged(win);
        if (cancelled) return;
        prevWinRef.current = win;
        setSheetH(targetSheetH);
      })();
    } else {
      setSheetH(targetSheetH);
      shrinkTimerRef.current = window.setTimeout(() => {
        void resizeWindowIfChanged(win);
        prevWinRef.current = win;
      }, SHRINK_DELAY_MS);
    }
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [win.width, win.height, targetSheetH, resizeWindowIfChanged]);
  useEffect(
    () => () => {
      if (shrinkTimerRef.current) window.clearTimeout(shrinkTimerRef.current);
    },
    [],
  );

  const showSpokenToggle = placement === "below" && !!visible && !!spoken;

  return (
    <div
      className="relative h-screen w-screen cursor-grab overflow-hidden bg-transparent select-none active:cursor-grabbing"
      data-testid="halo"
      data-sheet={sheetOpen ? "open" : "closed"}
      {...dragProps}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onFocusCapture={() => setFocusWithin(true)}
      onBlurCapture={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setFocusWithin(false);
      }}
      onClickCapture={(e) => {
        if (swallowClickAfterDrag(e)) return;
        if (answerShowing && (e.target as HTMLElement).closest("button, input, a")) engage();
      }}
    >
      {/* The sheet slides out from under the disc. It is centred and grows
          downward from the disc's centre, so the disc never moves. */}
      <motion.div
        data-testid="halo-sheet"
        className="dark absolute left-1/2 -translate-x-1/2 overflow-hidden bg-black text-white"
        style={{
          top: SHADOW_PAD + RING_BOX / 2,
          width: SHEET_WIDTH,
          borderRadius: SHEET_RADIUS,
          boxShadow: sheetH > 0 ? SHEET_SHADOW : "none",
          willChange: "height",
        }}
        initial={false}
        animate={{ height: sheetH, opacity: sheetH > 0 ? 1 : 0 }}
        transition={reducedMotion ? EASE : SPRING}
      >
        <div
          data-no-drag
          className="halo-scroll cursor-auto select-text overflow-y-auto px-5"
          style={{
            paddingTop: SHEET_TOP_INSET,
            paddingBottom: SHEET_BOTTOM_PAD,
            maxHeight: SHEET_TOP_INSET + SHEET_MAX_CONTENT + SHEET_BOTTOM_PAD,
          }}
          onScroll={engage}
        >
          <div ref={setMeasureEl} className="flex flex-col gap-3">
            {approval && <ApprovalRow msg={approval} onDecided={chat.handleApprovalUpdate} />}
            <InputControlNotices />
            {caption && <HaloCaption caption={caption} />}
            {placement === "below" && (
              <div className="text-[13px] leading-[1.55] text-white/85" data-testid="halo-panel">
                {visible ? (
                  <MixedContentRenderer content={visible} isStreaming={streaming} />
                ) : (
                  <p className="italic text-white/70" data-testid="halo-spoken-only">
                    {spoken}
                  </p>
                )}
                {showSpokenToggle && spokenOpen && (
                  <p
                    className="mt-2 border-t border-white/[0.08] pt-2 text-[12px] italic text-white/55"
                    data-testid="halo-spoken"
                  >
                    {spoken}
                  </p>
                )}
              </div>
            )}
            {(showSpokenToggle || composerOpen) && (
              <div className="flex items-center gap-2">
                {showSpokenToggle && (
                  <button
                    type="button"
                    onClick={() => {
                      engage();
                      setSpokenOpen((v) => !v);
                    }}
                    aria-pressed={spokenOpen}
                    aria-label="Spoken aloud"
                    title="Spoken aloud"
                    className={cn(
                      "flex size-6 shrink-0 items-center justify-center rounded-full transition-colors",
                      spokenOpen
                        ? "bg-white/[0.12] text-white/85"
                        : "text-white/40 hover:bg-white/[0.08] hover:text-white/80",
                    )}
                  >
                    <Volume2 className="size-3.5" />
                  </button>
                )}
                {composerOpen && (
                  <form onSubmit={submit} className="relative flex min-w-0 flex-1 items-center gap-2">
                    {skill.open && (
                      <div className="absolute bottom-full left-0 right-0 z-20 mb-1.5">
                        <SkillSuggestionList
                          variant="bar"
                          suggestions={skill.suggestions}
                          selectedIndex={skill.selectedIndex}
                          onSelect={skill.accept}
                          onHighlight={skill.setSelectedIndex}
                        />
                      </div>
                    )}
                    <div className="relative min-w-0 flex-1">
                      <input
                        ref={composerRef}
                        type="text"
                        value={input}
                        onChange={(e) => {
                          engage();
                          changeInput(e.target.value);
                        }}
                        onKeyDown={skill.handleKeyDown}
                        aria-label="Ask Juno"
                        placeholder="Ask Juno"
                        disabled={state !== UI.BAR_STATES_INPUT}
                        className={cn(
                          "h-8 w-full rounded-full bg-white/[0.06] px-3.5 text-[13px] tracking-[-0.01em] text-white/90 outline-none",
                          "placeholder:text-white/25 focus:bg-white/[0.09]",
                        )}
                      />
                      <SkillGhostText
                        value={input}
                        ghostText={skill.ghostText}
                        className="flex h-8 items-center px-3.5 text-[13px] tracking-[-0.01em]"
                        ghostClassName="text-white/25"
                      />
                    </div>
                    <span
                      className={cn(
                        "shrink-0 select-none text-[11px] tracking-[0.04em] text-white/25 transition-opacity duration-150",
                        input.trim() ? "opacity-100" : "opacity-0",
                      )}
                      aria-hidden="true"
                    >
                      return
                    </span>
                  </form>
                )}
              </div>
            )}
          </div>
        </div>
      </motion.div>

      {/* The ring, centred, at a fixed top. It is the anchor everything else
          grows around. */}
      <div
        className="absolute left-1/2 top-0 -translate-x-1/2"
        style={{ width: RING_SVG, height: RING_SVG }}
        onClick={idle ? onRingClick : undefined}
        role={idle ? "button" : undefined}
        aria-label={idle ? "Ask Juno" : undefined}
        data-testid="halo-disc"
        data-idle={idle ? "true" : "false"}
      >
        <HaloRing look={look} ticks={ticks} gapAt={gapAt} reducedMotion={reducedMotion} />
        {inside && (
          <div
            className="pointer-events-none absolute flex items-center justify-center text-center"
            style={{ left: SHADOW_PAD, top: SHADOW_PAD, width: RING_BOX, height: RING_BOX }}
          >
            <span
              data-testid="halo-inside"
              className="line-clamp-2 px-3 font-medium leading-[1.15] tracking-[-0.02em] text-white/90 tabular-nums"
              style={{ fontSize: approval ? 14 : insideFontSize(inside) }}
            >
              {inside}
            </span>
          </div>
        )}
      </div>
      <style>{`
        .halo-scroll { scrollbar-width: thin; scrollbar-color: rgba(255,255,255,0.22) transparent; }
        .halo-scroll::-webkit-scrollbar { width: 6px; }
        .halo-scroll::-webkit-scrollbar-track { background: transparent; }
        .halo-scroll::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.22); border-radius: 3px; }
      `}</style>
    </div>
  );
}
