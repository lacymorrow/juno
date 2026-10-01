import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useReducedMotion } from "motion/react";
import { Volume2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { EVENTS, UI, COMMANDS, WINDOW_LABELS } from "@/lib/constants.generated";
import { useWindowSize } from "@/hooks/useWindowSize";
import { useBarDrag } from "@/hooks/useDragWindow";
import { useEventListener } from "@/hooks/useEventListener";
import { useEscapeToIdle } from "@/hooks/useEscapeToIdle";
import { useBarConversation } from "@/hooks/useBarConversation";
import { useSkillAutocomplete } from "@/hooks/useSkillAutocomplete";
import { SkillGhostText, SkillSuggestionList } from "@/components/SkillAutocomplete";
import { MixedContentRenderer } from "@/components/ui/mixed-content-renderer";
import { InputControlNotices } from "@/components/input-control/InputControlNotices";
import { drivingLabel, type InputControlStatePayload } from "@/lib/inputControl";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";
import type { ChatMessage } from "@/types/chat";
import { IslandDot, IslandShell, IslandWords } from "./IslandShell";
import { LingerRing } from "./LingerRing";
import { useLinger } from "./useLinger";
import {
  ISLAND_SIZES,
  LINGER_MS,
  SHADOW_PAD,
  SHRINK_DELAY_MS,
  answerKey,
  dotFor,
  isIdleState,
  isInputState,
  isVoiceState,
  isWorkingState,
  islandSize,
  latestTurn,
  posture as postureFor,
  ringShouldRun,
  windowSize,
  wordsFor,
  type IslandSize,
} from "./islandModel";

/**
 * Island: one window that grows to hold the answer, then settles back.
 *
 * Spec and the full state table: docs/plans/island-appearance.md. In short:
 * a capsule at rest; an ear while you speak, with your words inside; a line
 * to type in; a status line while Juno works; and a card that grows downward
 * for the answer, with a ring that drains until the island settles again.
 * There is no chat pane. The island shows the current turn; history lives in
 * the main window.
 */

/** Backend element id for interactions. Must match `ui::element_ids::DYNAMIC_BAR`. */
const COMPONENT_ID = UI.ELEMENT_IDS_DYNAMIC_BAR;
const WINDOW_LABEL = WINDOW_LABELS.FLOATING_BAR;

/** Header and footer heights inside the card, for the content measurement. */
const CARD_HEADER_H = 36;
const CARD_FOOTER_H = 40;
/** Vertical padding around the card body. */
const CARD_BODY_PAD = 8;

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
    console.error("Island: interaction failed:", error);
  }
}

/** Escape, reported to Rust. What stopping means is Rust's decision. */
function reportEscape(): void {
  void sendInteraction(UI.INTERACTION_TYPES_ESCAPE);
}

// === THE APPROVAL ROW ===

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
      console.error("Island: approval failed:", error);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div
      className="mb-3 rounded-[12px] bg-white/[0.06] px-3 py-2.5"
      data-testid="island-approval"
      role="group"
      aria-label="Permission"
    >
      <p className="text-[12.5px] leading-snug text-white/85">Juno wants to {title}</p>
      <div className="mt-2 flex items-center gap-2">
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

export function IslandBar() {
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
      .catch((e) => console.error("Island: listener setup failed:", e));
    return () => {
      mounted = false;
      safeCleanupEventListener(unlisten);
    };
  }, []);

  const [driving, setDriving] = useState<InputControlStatePayload | null>(null);
  useEventListener<InputControlStatePayload>(EVENTS.INPUT_CONTROL_STATE, setDriving);
  const isDriving = !!driving?.active;

  // ── The current turn ──
  const turn = useMemo(() => latestTurn(chat.messages), [chat.messages]);
  const key = answerKey(turn);
  const answer = turn.answer;
  const visibleText = answer?.content.trim() ?? "";
  const spokenText =
    answer?.tts_metadata?.total_spoken_text?.trim() ||
    answer?.tts_metadata?.tts_parts?.map((p) => p.trim()).join(" ") ||
    "";
  const streaming = !!answer?.isStreaming;
  const working = isWorkingState(bar.barState) || chat.isProcessing;

  // ── The card ──
  const [cardOpen, setCardOpen] = useState(false);
  const [spokenOpen, setSpokenOpen] = useState(false);

  // Drag from anywhere, land in a gravity well. The card being open is the
  // island's "busy": it is not re-homed to another display mid-answer.
  const { dragProps, swallowClickAfterDrag } = useBarDrag({
    displayFollowPaused: cardOpen || working,
  });
  // Read through refs so a close from a timer sees the current state.
  const barRef = useRef(bar);
  barRef.current = bar;
  const inputRef = useRef("");
  const closeCard = useCallback(() => {
    setCardOpen(false);
    setSpokenOpen(false);
    // Focus put Rust in its input state while the card was up. With nothing
    // typed, tell it the composer blurred so it shrinks to Default and the
    // island settles to the capsule rather than the line.
    if (isInputState(barRef.current.barState) && inputRef.current.trim() === "") {
      void sendInteraction(UI.INTERACTION_TYPES_BLUR);
    }
  }, []);

  // A reply that arrives while the island is up opens the card. A reply that
  // was already there when it mounted (history) does not.
  const seenKeyRef = useRef(key);
  useEffect(() => {
    if (key && key !== seenKeyRef.current) {
      seenKeyRef.current = key;
      setCardOpen(true);
      setSpokenOpen(false);
    }
  }, [key]);
  useEffect(() => {
    if (turn.approval) setCardOpen(true);
  }, [turn.approval]);
  useEventListener(EVENTS.INPUT_CONTROL_REQUEST, () => setCardOpen(true));
  // The person is speaking again: the ear must be free.
  useEffect(() => {
    if (isVoiceState(bar.barState)) closeCard();
  }, [bar.barState, closeCard]);

  // ── Engagement and the ring ──
  const [hovered, setHovered] = useState(false);
  const [focusWithin, setFocusWithin] = useState(false);
  const [engaged, setEngaged] = useState(false);
  const [ringEpoch, setRingEpoch] = useState(0);
  const paused = hovered || focusWithin;
  const counting = ringShouldRun({
    cardOpen,
    streaming,
    working,
    approvalPending: !!turn.approval,
  });
  const { progress } = useLinger({
    running: counting && !engaged,
    paused,
    durationMs: LINGER_MS,
    resetKey: `${key}:${visibleText.length}:${spokenText.length}:${ringEpoch}`,
    onExpire: closeCard,
  });
  // Once the pointer has left and nothing inside has focus, an engaged card
  // starts a fresh ring.
  useEffect(() => {
    if (engaged && !hovered && !focusWithin) {
      setEngaged(false);
      setRingEpoch((e) => e + 1);
    }
  }, [engaged, hovered, focusWithin]);
  const engage = useCallback(() => setEngaged(true), []);

  // ── Composer ──
  const [input, setInput] = useState("");
  inputRef.current = input;
  const lineInputRef = useRef<HTMLInputElement>(null);
  const followUpRef = useRef<HTMLInputElement>(null);
  const lineOpen = isInputState(bar.barState);
  const skill = useSkillAutocomplete({
    value: input,
    onAccept: setInput,
    enabled: lineOpen || cardOpen,
  });
  // Rust clears its copy when it shrinks; follow it. It is never followed
  // while typing, so a slow echo cannot clobber a fast typist.
  useEffect(() => {
    if (bar.inputValue === "" && isIdleState(bar.barState)) setInput("");
  }, [bar.inputValue, bar.barState]);
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
    if (!lineOpen) return;
    const target = cardOpen ? followUpRef.current : lineInputRef.current;
    const t = window.setTimeout(() => target?.focus(), 60);
    return () => window.clearTimeout(t);
  }, [lineOpen, cardOpen]);

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
      .catch((e) => console.error("Island: focus listener failed:", e));
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  // Escape: one behaviour, shared by every appearance (src/lib/barEscape.ts).
  useEscapeToIdle({
    barState: bar.barState,
    working: working,
    overlayOpen: cardOpen,
    composerOpen: lineOpen,
    popupOpen: skill.open,
    collapse: closeCard,
    report: reportEscape,
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        void sendInteraction(UI.INTERACTION_TYPES_ENTER);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const idle = isIdleState(bar.barState) && !cardOpen;
  const onIslandClick = useCallback(() => {
    if (idle) void sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, [idle]);

  // ── Posture and size ──
  const posture = postureFor({ state: bar.barState, cardOpen, driving: isDriving });
  const [cardContentH, setCardContentH] = useState(ISLAND_SIZES.card.minHeight);
  // The card's content, measured through a callback ref: the body mounts
  // after the posture switches, so a plain ref would be empty when the
  // observer effect first ran.
  const [measureEl, setMeasureEl] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!measureEl || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      const h = entry.contentRect.height;
      setCardContentH(CARD_HEADER_H + CARD_BODY_PAD * 2 + Math.ceil(h) + CARD_FOOTER_H);
    });
    observer.observe(measureEl);
    return () => observer.disconnect();
  }, [measureEl]);
  // A closed card forgets its height, so the next one opens small and grows.
  useEffect(() => {
    if (!cardOpen) setCardContentH(ISLAND_SIZES.card.minHeight);
  }, [cardOpen]);
  const target = islandSize(posture, cardContentH);
  const suggestionExtra =
    skill.open && posture === "line" ? skill.suggestions.length * 29 + 20 : 0;
  const win = windowSize(target, suggestionExtra);

  // Window protocol, same as Pill: growing, resize then spring; shrinking,
  // spring then resize once it has settled. Both through one backend call so
  // position and size land in the same frame.
  const [shell, setShell] = useState<IslandSize>(() => islandSize("capsule"));
  const prevWinRef = useRef(windowSize(islandSize("capsule")));
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
        setShell(target);
      })();
    } else {
      setShell(target);
      shrinkTimerRef.current = window.setTimeout(() => {
        void resizeWindowIfChanged(win);
        prevWinRef.current = win;
      }, SHRINK_DELAY_MS);
    }
    return () => {
      cancelled = true;
    };
    // The sizes are value objects; their fields are the real dependencies.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [win.width, win.height, target.width, target.height, target.radius, resizeWindowIfChanged]);
  useEffect(
    () => () => {
      if (shrinkTimerRef.current) window.clearTimeout(shrinkTimerRef.current);
    },
    [],
  );

  // ── Words and dot ──
  const dot = dotFor(bar.barState, isDriving);
  // The dot swells with your voice while the island listens; any other
  // state keeps it at its own size.
  const dotLevel = isVoiceState(bar.barState) ? bar.audioLevel : 0;
  const question = turn.question || bar.lastSubmittedValue;
  const words = wordsFor({
    state: bar.barState,
    transcriptionText: bar.transcriptionText,
    transcriptionProvisional: bar.transcriptionProvisional,
    spokenText: bar.spokenText,
    currentError: bar.currentError,
    question,
    runningTool: turn.runningTool,
    drivingLabel: isDriving && driving ? drivingLabel(driving) : null,
  });

  // ── Layers ──
  let layer: ReactNode;
  if (posture === "capsule") {
    layer = (
      <div className="flex h-full w-full items-center justify-center" data-testid="island-capsule">
        <IslandDot look={dot} level={dotLevel} />
      </div>
    );
  } else if (posture === "line") {
    layer = (
      <form
        onSubmit={submit}
        className="flex h-full w-full items-center gap-2.5 pl-4 pr-3.5"
        data-testid="island-line"
      >
        <IslandDot look={dot} level={dotLevel} />
        <div className="relative min-w-0 flex-1">
          <input
            ref={lineInputRef}
            type="text"
            value={input}
            onChange={(e) => changeInput(e.target.value)}
            onKeyDown={skill.handleKeyDown}
            aria-label="Ask Juno"
            placeholder="Ask Juno"
            disabled={bar.barState !== UI.BAR_STATES_INPUT}
            className={cn(
              "w-full bg-transparent text-[13px] tracking-[-0.01em] text-white/90 outline-none",
              "placeholder:text-white/25",
            )}
          />
          <SkillGhostText
            value={input}
            ghostText={skill.ghostText}
            className="flex items-center text-[13px] tracking-[-0.01em]"
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
    );
  } else if (posture === "card") {
    const headerWords = words ?? (question ? { text: question, tone: "dim" as const } : null);
    layer = (
      <div className="dark flex h-full w-full flex-col" data-testid="island-card">
        <header
          className="flex shrink-0 items-center gap-2.5 pl-4 pr-2"
          style={{ height: CARD_HEADER_H }}
        >
          <IslandDot look={dot} level={dotLevel} />
          {headerWords ? (
            <IslandWords words={headerWords} />
          ) : (
            <span className="min-w-0 flex-1" />
          )}
          <LingerRing progress={progress} counting={counting && !engaged} paused={paused} onClose={closeCard} />
        </header>
        <div
          data-no-drag
          onScroll={engage}
          className="island-scroll min-h-0 flex-1 cursor-auto select-text overflow-y-auto px-4"
          style={{ paddingTop: CARD_BODY_PAD, paddingBottom: CARD_BODY_PAD }}
        >
          <div ref={setMeasureEl} className="text-[13px] leading-[1.55] text-white/85">
            {turn.approval && (
              <ApprovalRow msg={turn.approval} onDecided={chat.handleApprovalUpdate} />
            )}
            <InputControlNotices />
            {visibleText ? (
              <MixedContentRenderer content={visibleText} isStreaming={streaming} />
            ) : spokenText ? (
              <p className="italic text-white/70" data-testid="island-spoken-only">
                {spokenText}
              </p>
            ) : working && !turn.approval ? (
              <p className="text-white/45">{turn.runningTool || "Working"}</p>
            ) : null}
            {visibleText && spokenText && spokenOpen && (
              <p
                className="mt-2 border-t border-white/[0.08] pt-2 text-[12px] italic text-white/55"
                data-testid="island-spoken"
              >
                {spokenText}
              </p>
            )}
          </div>
        </div>
        <footer
          className="flex shrink-0 items-center gap-2 pl-3 pr-3"
          style={{ height: CARD_FOOTER_H }}
        >
          {visibleText && spokenText ? (
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
                spokenOpen ? "bg-white/[0.12] text-white/85" : "text-white/40 hover:bg-white/[0.08] hover:text-white/80",
              )}
            >
              <Volume2 className="size-3.5" />
            </button>
          ) : null}
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
                ref={followUpRef}
                type="text"
                value={input}
                onChange={(e) => {
                  engage();
                  changeInput(e.target.value);
                }}
                onKeyDown={skill.handleKeyDown}
                aria-label="Follow up"
                placeholder="Follow up"
                className={cn(
                  "h-7 w-full rounded-full bg-white/[0.06] px-3 text-[12px] tracking-[-0.01em] text-white/85 outline-none",
                  "placeholder:text-white/25 focus:bg-white/[0.09]",
                )}
              />
              <SkillGhostText
                value={input}
                ghostText={skill.ghostText}
                className="flex h-7 items-center px-3 text-[12px] tracking-[-0.01em]"
                ghostClassName="text-white/25"
              />
            </div>
          </form>
        </footer>
      </div>
    );
  } else {
    // ear and status
    layer = (
      <div className="flex h-full w-full items-center gap-2 pl-3.5 pr-3.5" data-testid={`island-${posture}`}>
        <IslandDot look={dot} level={dotLevel} />
        {words && <IslandWords words={words} />}
      </div>
    );
  }

  return (
    <div
      className="relative h-screen w-screen cursor-grab overflow-hidden bg-transparent select-none active:cursor-grabbing"
      {...dragProps}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onFocusCapture={() => setFocusWithin(true)}
      onBlurCapture={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setFocusWithin(false);
      }}
      onClickCapture={(e) => {
        if (swallowClickAfterDrag(e)) return;
        if (cardOpen && (e.target as HTMLElement).closest("button, input, a")) engage();
      }}
    >
      {/* Centred, not left-anchored: the window grows first (centre-stable),
          so an island pinned to the left edge would jump left by half the
          added width before it springs open. Centred, it grows around itself. */}
      <div className="absolute left-1/2 -translate-x-1/2" style={{ top: SHADOW_PAD }}>
        <IslandShell
          size={shell}
          layerKey={posture}
          reducedMotion={reducedMotion}
          onClick={idle ? onIslandClick : undefined}
        >
          {layer}
        </IslandShell>
      </div>
      {posture === "line" && skill.open && (
        <div
          className="absolute left-1/2 z-20 -translate-x-1/2"
          style={{ top: SHADOW_PAD + shell.height + 8, width: ISLAND_SIZES.line.width - 24 }}
        >
          <SkillSuggestionList
            variant="bar"
            suggestions={skill.suggestions}
            selectedIndex={skill.selectedIndex}
            onSelect={skill.accept}
            onHighlight={skill.setSelectedIndex}
          />
        </div>
      )}
    </div>
  );
}
