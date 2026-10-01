import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
} from "react";
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
import { useEscapeToIdle } from "@/hooks/useEscapeToIdle";
import { useBarConversation } from "@/hooks/useBarConversation";
import { useSkillAutocomplete } from "@/hooks/useSkillAutocomplete";
import { useSystemTheme } from "@/hooks/useSystemTheme";
import { SkillGhostText, SkillSuggestionList } from "@/components/SkillAutocomplete";
import { MixedContentRenderer } from "@/components/ui/mixed-content-renderer";
import { InputControlNotices } from "@/components/input-control/InputControlNotices";
import { drivingLabel, type InputControlStatePayload } from "@/lib/inputControl";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";
import { useLinger } from "./island/useLinger";
import { Bead, NarratorDot, RailBar, Words, paletteFor } from "./narrator/NarratorStrip";
import {
  BAR_HEIGHT,
  BAR_RADIUS,
  BAR_WIDTH,
  DRAIN_MS,
  SHADOW_PAD,
  SHEET_MIN_HEIGHT,
  SHRINK_DELAY_MS,
  SYSTEM_BLUE,
  answerKey,
  dotFor,
  drainShouldRun,
  firstSentence,
  isIdleState,
  isInputState,
  isVoiceState,
  isWorkingState,
  latestTurn,
  lineFor,
  pendingStep,
  posture as postureFor,
  railFor,
  sheetHeightFor,
  sheetWanted,
  windowSize,
  type Segment,
  type Step,
} from "./narrator/narratorModel";

/**
 * Bar: the narrator. One wide strip that tells the turn left to right.
 *
 * Spec and the full state table: docs/plans/bar-appearance.md. In short: what
 * you said sits at the left; each step Juno takes is a bead on the line with
 * its words; the answer's first sentence ends the line. A rail along the
 * bottom edge fills as the turn goes and drains once it is done. The full
 * answer slides down from the strip's bottom edge as a sheet when one line
 * cannot hold it. The strip never changes width, so nothing on it can jump.
 */

/** Backend element id for interactions. Must match `ui::element_ids::APP_BAR`. */
const COMPONENT_ID = UI.ELEMENT_IDS_APP_BAR;
const WINDOW_LABEL = WINDOW_LABELS.FLOATING_BAR;

/** Padding inside the sheet, above and below its body. */
const SHEET_PAD_TOP = BAR_RADIUS + 10;
const SHEET_PAD_BOTTOM = 10;
/** The spoken-text row and the speaker control, when the sheet has them. */
const SHEET_FOOTER_H = 30;

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
    console.error("Bar: interaction failed:", error);
  }
}

/** Escape, reported to Rust. What stopping means is Rust's decision. */
function reportEscape(): void {
  void sendInteraction(UI.INTERACTION_TYPES_ESCAPE);
}

// === ALLOW / DON'T, INLINE ON THE LINE ===

function ApprovalButtons({
  step,
  theme,
  onDecided,
}: {
  step: Step;
  theme: "light" | "dark";
  onDecided: (toolId: string, state: "approved" | "denied") => void;
}) {
  const [busy, setBusy] = useState(false);
  const decide = async (state: "approved" | "denied") => {
    if (!step.toolId || busy) return;
    setBusy(true);
    try {
      const command = state === "approved" ? "approve_tool_execution" : "deny_tool_execution";
      const ok = await invoke<boolean>(command, { toolId: step.toolId });
      if (ok) onDecided(step.toolId, state);
    } catch (error) {
      console.error("Bar: approval failed:", error);
    } finally {
      setBusy(false);
    }
  };
  const quiet = theme === "dark" ? "text-white/70 hover:bg-white/[0.08] hover:text-white" : "text-black/60 hover:bg-black/[0.06] hover:text-black";
  return (
    <span className="flex shrink-0 items-center gap-1" data-testid="bar-approval-buttons" role="group" aria-label="Permission">
      <button
        type="button"
        disabled={busy}
        onClick={() => void decide("approved")}
        className={cn(
          "h-6 rounded-full px-2.5 text-[12px] font-medium text-white",
          "transition-colors hover:bg-[#2B93FF] active:bg-[#0071E3] disabled:opacity-50",
        )}
        style={{ backgroundColor: SYSTEM_BLUE }}
      >
        Allow
      </button>
      <button
        type="button"
        disabled={busy}
        onClick={() => void decide("denied")}
        className={cn("h-6 rounded-full px-2.5 text-[12px] transition-colors disabled:opacity-50", quiet)}
      >
        Don&rsquo;t
      </button>
    </span>
  );
}

// === THE BAR ===

export function AppBar() {
  const theme = useSystemTheme();
  const palette = paletteFor(theme);
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
      .catch((e) => console.error("Bar: listener setup failed:", e));
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
  const answerLine = useMemo(() => firstSentence(visibleText), [visibleText]);
  const spokenOnly = !visibleText && !!spokenText;
  const streaming = !!answer?.isStreaming;
  const speaking = bar.barState === UI.BAR_STATES_SPEAKING;
  const working = isWorkingState(bar.barState) || chat.isProcessing;
  const pending = pendingStep(turn);

  // ── The sheet ──
  const [sheetOpen, setSheetOpen] = useState(false);
  const [spokenOpen, setSpokenOpen] = useState(false);

  // Drag from anywhere, land in a gravity well. A wide strip is exactly the
  // look the old cursor-centre highlight got wrong; the grab offset the drag
  // hook sends fixes it.
  const { dragProps, swallowClickAfterDrag } = useBarDrag({
    displayFollowPaused: sheetOpen || working,
  });
  const barRef = useRef(bar);
  barRef.current = bar;
  const inputRef = useRef("");
  const closeSheet = useCallback(() => {
    setSheetOpen(false);
    setSpokenOpen(false);
  }, []);
  /** Escape: the sheet goes away and the lingering answer line stops holding
   *  the strip open, so the Bar is back to its resting line in one press. */
  const collapse = useCallback(() => {
    closeSheet();
    setDrained(true);
  }, [closeSheet]);

  // An answer that arrives while the Bar is up opens the sheet when the line
  // cannot hold it. One that was already there when it mounted (history) does
  // not. A short answer that grows past one line opens it as it grows.
  const seenKeyRef = useRef(key);
  const wantsSheet = sheetWanted({ visibleText, streaming });
  useEffect(() => {
    if (key && key !== seenKeyRef.current) {
      seenKeyRef.current = key;
      setSpokenOpen(false);
      if (wantsSheet) setSheetOpen(true);
    } else if (key && key === seenKeyRef.current && streaming && wantsSheet) {
      setSheetOpen(true);
    }
  }, [key, wantsSheet, streaming]);
  useEventListener(EVENTS.INPUT_CONTROL_REQUEST, () => setSheetOpen(true));
  // The person is speaking again: the line is theirs. (Always listening is a
  // resting state that happens to listen; it keeps the sheet.)
  useEffect(() => {
    if (isVoiceState(bar.barState)) closeSheet();
  }, [bar.barState, closeSheet]);

  // ── Engagement and the drain ──
  const [hovered, setHovered] = useState(false);
  const [focusWithin, setFocusWithin] = useState(false);
  const [engaged, setEngaged] = useState(false);
  const [drainEpoch, setDrainEpoch] = useState(0);
  const paused = hovered || focusWithin;
  const hasAnswer = answerLine.length > 0 || spokenOnly;
  const draining = drainShouldRun({
    hasAnswer,
    streaming,
    working: working || speaking,
    approvalPending: !!pending,
  });
  const [drained, setDrained] = useState(false);
  const { progress: drainProgress } = useLinger({
    running: draining && !engaged && !drained,
    paused,
    durationMs: DRAIN_MS,
    resetKey: `${key}:${visibleText.length}:${spokenText.length}:${drainEpoch}`,
    onExpire: () => {
      setDrained(true);
      closeSheet();
    },
  });
  // A new answer, or new growth of this one, gets a fresh drain.
  useEffect(() => {
    setDrained(false);
  }, [key, visibleText.length, spokenText.length]);
  // Once the pointer has left and nothing inside has focus, an engaged Bar
  // starts a fresh drain.
  useEffect(() => {
    if (engaged && !hovered && !focusWithin) {
      setEngaged(false);
      setDrainEpoch((e) => e + 1);
    }
  }, [engaged, hovered, focusWithin]);
  const engage = useCallback(() => setEngaged(true), []);

  // ── Composer ──
  const [input, setInput] = useState("");
  inputRef.current = input;
  const composerRef = useRef<HTMLInputElement>(null);
  const composing = isInputState(bar.barState);
  const skill = useSkillAutocomplete({ value: input, onAccept: setInput, enabled: composing });
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
    if (!composing) return;
    const t = window.setTimeout(() => composerRef.current?.focus(), 60);
    return () => window.clearTimeout(t);
  }, [composing]);

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
      .catch((e) => console.error("Bar: focus listener failed:", e));
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  // Escape: one behaviour, shared by every appearance (src/lib/barEscape.ts).
  useEscapeToIdle({
    barState: bar.barState,
    working: working,
    // The draining rail is a timer, and WCAG 2.2.1 says a timer that
    // dismisses content must be stoppable: while it counts, Escape stops it.
    overlayOpen: sheetOpen || spokenOpen || (draining && !drained),
    composerOpen: composing,
    popupOpen: skill.open,
    collapse: collapse,
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

  const idle = isIdleState(bar.barState);
  const onStripClick = useCallback(() => {
    if (idle) void sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, [idle]);

  // ── The line ──
  const posture = postureFor({ state: bar.barState, driving: isDriving });
  const question = turn.question || bar.lastSubmittedValue;
  const segments = useMemo(
    () =>
      lineFor({
        state: bar.barState,
        transcriptionText: bar.transcriptionText,
        transcriptionProvisional: bar.transcriptionProvisional,
        spokenText: bar.spokenText || spokenText,
        currentError: bar.currentError,
        question,
        turn,
        drivingLabel: isDriving && driving ? drivingLabel(driving) : null,
        answerLine,
        spokenOnly,
      }),
    [bar, question, turn, isDriving, driving, answerLine, spokenOnly, spokenText],
  );
  const dot = dotFor(bar.barState, theme, isDriving);
  const rail = railFor({
    state: bar.barState,
    audioLevel: bar.audioLevel,
    segments,
    answerLength: visibleText.length,
    streaming,
    drain: draining && !engaged && !drained ? drainProgress : drained ? 0 : null,
  });

  // The rail reaches a bead: measured on screen after each render of the line,
  // so the fill lands on the bead wherever the words put it.
  const stripRef = useRef<HTMLDivElement>(null);
  const [beadX, setBeadX] = useState<number | null>(null);
  const lastBead = (() => {
    for (let i = segments.length - 1; i >= 0; i -= 1) if (segments[i].bead) return i;
    return null;
  })();
  const beadTarget =
    rail.kind === "toBead" || rail.kind === "failed" ? rail.index : rail.kind === "streaming" ? lastBead : null;
  useLayoutEffect(() => {
    const strip = stripRef.current;
    if (!strip || beadTarget === null) {
      setBeadX(null);
      return;
    }
    const el = strip.querySelector<HTMLElement>(`[data-bead-index="${beadTarget}"]`);
    if (!el) {
      setBeadX(null);
      return;
    }
    const x = el.getBoundingClientRect().left - strip.getBoundingClientRect().left;
    setBeadX(Math.round(x));
  }, [beadTarget, segments, posture]);

  // ── The sheet's height ──
  const [sheetContentH, setSheetContentH] = useState(SHEET_MIN_HEIGHT);
  const [measureEl, setMeasureEl] = useState<HTMLDivElement | null>(null);
  const sheetHasFooter = !!visibleText && !!spokenText;
  useEffect(() => {
    if (!measureEl || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      const h = entry.contentRect.height;
      setSheetContentH(SHEET_PAD_TOP + Math.ceil(h) + SHEET_PAD_BOTTOM + (sheetHasFooter ? SHEET_FOOTER_H : 0));
    });
    observer.observe(measureEl);
    return () => observer.disconnect();
  }, [measureEl, sheetHasFooter]);
  useEffect(() => {
    if (!sheetOpen) setSheetContentH(SHEET_MIN_HEIGHT);
  }, [sheetOpen]);
  const sheetH = sheetOpen ? sheetHeightFor(sheetContentH) : 0;
  const suggestionExtra = skill.open && composing ? skill.suggestions.length * 29 + 20 : 0;
  const win = windowSize(sheetH, suggestionExtra);

  // ── Window protocol ──
  // Growing: resize the window, then slide the sheet. Shrinking: slide the
  // sheet up, then shrink the window once it has gone. The window is anchored
  // at its top and never changes width, so the strip itself never moves.
  const [shown, setShown] = useState({ open: false, height: 0 });
  const [ready, setReady] = useState(false);
  const prevWinRef = useRef<{ width: number; height: number } | null>(null);
  const shrinkTimerRef = useRef<number | null>(null);
  useEffect(() => {
    const prev = prevWinRef.current;
    if (shrinkTimerRef.current) {
      window.clearTimeout(shrinkTimerRef.current);
      shrinkTimerRef.current = null;
    }
    let cancelled = false;
    const target = { open: sheetOpen, height: sheetH };
    if (prev === null) {
      // First paint: size the window before the strip shows at all, so it is
      // never seen clipped inside the window the last look left behind.
      void (async () => {
        await resizeWindowIfChanged(win);
        if (cancelled) return;
        prevWinRef.current = win;
        setShown(target);
        setReady(true);
      })();
    } else if (win.height > prev.height) {
      void (async () => {
        await resizeWindowIfChanged(win);
        if (cancelled) return;
        prevWinRef.current = win;
        setShown(target);
      })();
    } else {
      setShown(target);
      shrinkTimerRef.current = window.setTimeout(() => {
        void resizeWindowIfChanged(win);
        prevWinRef.current = win;
      }, SHRINK_DELAY_MS);
    }
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [win.width, win.height, sheetOpen, sheetH, resizeWindowIfChanged]);
  useEffect(
    () => () => {
      if (shrinkTimerRef.current) window.clearTimeout(shrinkTimerRef.current);
    },
    [],
  );

  // ── Render ──
  const ink = palette.ink;
  const hairline = `0 0 0 0.5px ${palette.edge}`;
  const railLength = BAR_WIDTH;
  const listening = posture === "listen";
  const restEmpty = posture === "rest" && segments.length === 0;

  const renderSegment = (segment: Segment, i: number) => {
    const isLast = i === segments.length - 1;
    const bead = segment.bead ? (
      <span data-bead-index={i} className="flex shrink-0 items-center">
        <Bead segment={segment} theme={theme} title={segment.collapsed ? segment.text : undefined} />
      </span>
    ) : null;
    if (segment.collapsed) {
      return (
        <span key={i} className="flex shrink-0 items-center" data-testid="bar-step-collapsed">
          {bead}
        </span>
      );
    }
    const isAnswer = segment.kind === "answer" && !!visibleText;
    return (
      <span
        key={i}
        className={cn(
          "flex min-w-0 items-center gap-2",
          // The newest segment is the one being read: it always gets room.
          // An approval needs enough for its two buttons; a failure, for why.
          isLast
            ? segment.kind === "approval" || segment.failed
              ? "min-w-[62%] flex-1"
              : "min-w-[38%] flex-1"
            : segment.kind === "question"
              ? pending
                ? "max-w-[28%] shrink"
                : "max-w-[60%] shrink"
              : "max-w-[40%] shrink",
        )}
      >
        {bead}
        <Words
          segment={segment}
          theme={theme}
          tail={listening}
          className="flex-1"
          onClick={
            isAnswer
              ? () => {
                  engage();
                  setSheetOpen((v) => !v);
                }
              : undefined
          }
          ariaLabel={isAnswer ? (sheetOpen ? "Hide the full answer" : "Show the full answer") : undefined}
          ariaExpanded={isAnswer ? sheetOpen : undefined}
        />
        {segment.kind === "approval" && segment.step && (
          <ApprovalButtons step={segment.step} theme={theme} onDecided={chat.handleApprovalUpdate} />
        )}
      </span>
    );
  };

  const composer = (
    <form onSubmit={submit} className="relative flex min-w-0 flex-1 items-center" data-testid="bar-composer">
      <input
        ref={composerRef}
        type="text"
        value={input}
        onChange={(e) => changeInput(e.target.value)}
        onKeyDown={skill.handleKeyDown}
        aria-label="Ask Juno"
        placeholder={turn.question ? "Follow up" : "Ask Juno"}
        disabled={bar.barState !== UI.BAR_STATES_INPUT}
        className="w-full bg-transparent text-[13px] tracking-[-0.01em] outline-none placeholder:opacity-40"
        style={{ color: ink }}
      />
      <SkillGhostText
        value={input}
        ghostText={skill.ghostText}
        className="flex items-center text-[13px] tracking-[-0.01em]"
        ghostClassName="opacity-30"
      />
    </form>
  );

  // In the compose posture the question slot is the text field; the rest of
  // the line (beads, the answer) stays where it was.
  const lineChildren = (() => {
    if (posture === "compose") {
      const rest = segments.filter((s) => s.kind !== "question");
      return [<span key="composer" className={cn("flex min-w-0 items-center", rest.length ? "min-w-[180px] flex-1" : "flex-1")}>{composer}</span>, ...rest.map((s) => renderSegment(s, segments.indexOf(s)))];
    }
    if (restEmpty) {
      return (
        <span key="empty" className="min-w-0 flex-1 text-[13px] leading-none tracking-[-0.01em]" style={{ color: ink, opacity: 0.35 }} data-testid="bar-empty">
          Ask Juno
        </span>
      );
    }
    return segments.map(renderSegment);
  })();

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
        if (sheetOpen && (e.target as HTMLElement).closest("button, input, a")) engage();
      }}
      data-testid="bar-root"
      data-posture={posture}
      data-theme={theme}
      data-sheet={shown.open ? "open" : "closed"}
      style={{ opacity: ready ? 1 : 0, transition: "opacity 120ms ease" }}
    >
      {/* The sheet slides from under the strip's bottom edge. Its top is tucked
          under the strip's corner radius so the two read as one object. */}
      <div
        className="absolute overflow-hidden"
        style={{
          left: SHADOW_PAD,
          top: SHADOW_PAD + BAR_HEIGHT - BAR_RADIUS,
          width: BAR_WIDTH,
          height: shown.height + BAR_RADIUS,
          pointerEvents: shown.open ? "auto" : "none",
        }}
        data-testid="bar-sheet-clip"
      >
        <motion.div
          data-testid="bar-sheet"
          data-open={shown.open ? "true" : "false"}
          initial={false}
          animate={{ y: shown.open ? 0 : -(shown.height + BAR_RADIUS + 8), opacity: shown.open ? 1 : 0 }}
          transition={reducedMotion ? { duration: 0.15, ease: "easeOut" } : { type: "spring", stiffness: 380, damping: 36, mass: 1 }}
          className="absolute left-0 top-0 flex w-full flex-col"
          style={{
            height: shown.height + BAR_RADIUS,
            backgroundColor: palette.sheet,
            borderRadius: `0 0 ${BAR_RADIUS}px ${BAR_RADIUS}px`,
            boxShadow: `${hairline}, ${palette.shadow}`,
            color: ink,
          }}
        >
          <div
            data-no-drag
            onScroll={engage}
            className={cn("narrator-scroll min-h-0 flex-1 cursor-auto select-text overflow-y-auto px-4", theme === "dark" && "dark")}
            style={{ paddingTop: SHEET_PAD_TOP, paddingBottom: SHEET_PAD_BOTTOM }}
          >
            <div ref={setMeasureEl} className="text-[13px] leading-[1.55]" style={{ color: ink }}>
              <InputControlNotices className="px-0 pt-0" />
              {visibleText ? (
                <MixedContentRenderer content={visibleText} isStreaming={streaming} />
              ) : spokenText ? (
                <p className="italic opacity-70" data-testid="bar-spoken-only">
                  {spokenText}
                </p>
              ) : null}
              {visibleText && spokenText && spokenOpen && (
                <p className="mt-2 border-t pt-2 text-[12px] italic opacity-60" style={{ borderColor: palette.edge }} data-testid="bar-spoken">
                  {spokenText}
                </p>
              )}
            </div>
          </div>
          {sheetHasFooter && (
            <footer className="flex shrink-0 items-center px-3" style={{ height: SHEET_FOOTER_H }}>
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
                  theme === "dark"
                    ? spokenOpen ? "bg-white/[0.12] text-white/85" : "text-white/40 hover:bg-white/[0.08] hover:text-white/80"
                    : spokenOpen ? "bg-black/[0.08] text-black/80" : "text-black/40 hover:bg-black/[0.05] hover:text-black/75",
                )}
              >
                <Volume2 className="size-3.5" />
              </button>
            </footer>
          )}
        </motion.div>
      </div>

      {/* The strip. Fixed size; only what is on it changes. */}
      <div
        ref={stripRef}
        data-testid="bar-strip"
        onClick={idle ? onStripClick : undefined}
        className={cn("absolute z-10 overflow-hidden", idle && "cursor-pointer")}
        style={{
          left: SHADOW_PAD,
          top: SHADOW_PAD,
          width: BAR_WIDTH,
          height: BAR_HEIGHT,
          borderRadius: BAR_RADIUS,
          backgroundColor: palette.surface,
          boxShadow: `${hairline}, ${palette.shadow}`,
          color: ink,
          transition: "background-color 200ms ease",
        }}
      >
        <div className="flex h-full w-full items-center gap-2 pl-3.5 pr-3.5" data-testid="bar-line">
          <NarratorDot look={dot} />
          {lineChildren}
        </div>
        <RailBar rail={rail} beadX={beadX} length={railLength} palette={palette} reducedMotion={reducedMotion} />
      </div>

      {composing && skill.open && (
        <div
          className="absolute z-20"
          style={{ left: SHADOW_PAD + 12, top: SHADOW_PAD + BAR_HEIGHT + 8, width: BAR_WIDTH - 24 }}
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
