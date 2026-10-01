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
import { AnimatePresence, useReducedMotion } from "motion/react";
import { EVENTS, UI, COMMANDS, WINDOW_LABELS } from "@/lib/constants.generated";
import { useWindowSize } from "@/hooks/useWindowSize";
import { useBarDrag } from "@/hooks/useDragWindow";
import { useEventListener } from "@/hooks/useEventListener";
import { useEscapeToIdle } from "@/hooks/useEscapeToIdle";
import { useBarConversation } from "@/hooks/useBarConversation";
import { MixedContentRenderer } from "@/components/ui/mixed-content-renderer";
import { InputControlNotices } from "@/components/input-control/InputControlNotices";
import { drivingLabel, type InputControlStatePayload } from "@/lib/inputControl";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";
import { useLinger } from "./island/useLinger";
import { OrbCanvas, type Frameloop, type OrbDrive } from "./orb/OrbCanvas";
import {
  CaptionApproval,
  CaptionComposer,
  CaptionLine,
  CaptionSheet,
  CaptionSlot,
  useOrbKeyframes,
} from "./orb/OrbCaption";
import {
  CAPTION_GAP,
  HOVER_CLOSE_MS,
  HOVER_OPEN_MS,
  LINGER_MS,
  SHEET_MIN_HEIGHT,
  SHRINK_DELAY_MS,
  STAGE,
  answerKey,
  captionFor,
  hasComponent,
  hasSheetContent,
  isInputState,
  isRestState,
  isVoiceState,
  isWorkingState,
  latestTurn,
  orbLook,
  orbTargets,
  posture as postureFor,
  sheetHeightFor,
  windowSize,
  type Posture,
} from "./orb/orbModel";

/**
 * Presence: the ElevenLabs orb as a presence. The orb is the only object,
 * and everything is told through what it does: dim and breathing at rest,
 * blue and swelling with
 * your voice, green while your words become text, turning while Juno works
 * (faster as steps complete), rippling with Juno's speech, one red flinch
 * when something fails. Words live in one caption slot beneath it, one line
 * at a time like subtitles; the whole answer unfolds in a sheet under that.
 *
 * Spec and the full state table: docs/plans/presence-appearance.md.
 */

/** Backend element id. Rust has no orb id; the orb uses the floating bar's. */
const COMPONENT_ID = UI.ELEMENT_IDS_FLOATING_BAR;
const WINDOW_LABEL = WINDOW_LABELS.FLOATING_BAR;
/** Vertical padding inside the sheet, both sides. */
const SHEET_PAD = 24;

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
    console.error("Orb: interaction failed:", error);
  }
}

/** Escape, reported to Rust. What stopping means is Rust's decision. */
function reportEscape(): void {
  void sendInteraction(UI.INTERACTION_TYPES_ESCAPE);
}

interface Layout {
  posture: Posture;
  sheetHeight: number;
}

interface ElevenLabsOrbBarProps {
  barAppearance?: string;
}

export function ElevenLabsOrbBar(_props: ElevenLabsOrbBarProps) {
  useOrbKeyframes();
  const reducedMotion = useReducedMotion() ?? false;
  const { resizeWindowIfChanged } = useWindowSize(WINDOW_LABEL);
  const chat = useBarConversation();

  // ── What Rust says ──
  const [bar, setBar] = useState<BarStateData>(INITIAL_BAR);
  const [loop, setLoop] = useState<Frameloop>("always");
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let mounted = true;
    listen<BarStateData>(EVENTS.BAR_STATE_UPDATE, (event) => {
      if (!mounted) return;
      const p = event.payload;
      if (p && typeof p === "object" && "barState" in p) {
        setBar(p);
        // Anything Rust says may move the orb; it sleeps again once settled.
        setLoop("always");
      }
    })
      .then((fn) => {
        if (mounted) unlisten = fn;
        else safeCleanupEventListener(fn);
      })
      .catch((e) => console.error("Orb: listener setup failed:", e));
    return () => {
      mounted = false;
      safeCleanupEventListener(unlisten);
    };
  }, []);

  const [driving, setDriving] = useState<InputControlStatePayload | null>(null);
  const [noticeOpen, setNoticeOpen] = useState(false);
  useEventListener<InputControlStatePayload>(EVENTS.INPUT_CONTROL_STATE, (p) => {
    setDriving(p);
    setNoticeOpen(false);
  });
  useEventListener(EVENTS.INPUT_CONTROL_REQUEST, () => setNoticeOpen(true));
  const isDriving = !!driving?.active;

  // ── The current turn ──
  const turn = useMemo(() => latestTurn(chat.messages), [chat.messages]);
  const key = answerKey(turn);
  const answer = turn.answer;
  const visibleText = answer?.content.trim() ?? "";
  const spokenParts = answer?.tts_metadata?.tts_parts ?? [];
  const streaming = !!answer?.isStreaming;
  const working = isWorkingState(bar.barState) || chat.isProcessing;
  const approvalPending = !!turn.approval;

  // ── Lingering: a finished answer stays under the orb for a while ──
  const [lingering, setLingering] = useState(false);
  const [pinned, setPinned] = useState(false);
  const [autoSheet, setAutoSheet] = useState(false);
  const [hoverOpen, setHoverOpen] = useState(false);
  const settle = useCallback(() => {
    setLingering(false);
    setPinned(false);
    setAutoSheet(false);
    setHoverOpen(false);
    setNoticeOpen(false);
  }, []);
  // A reply that arrives while the orb is up lingers. One that was already
  // there when it mounted (history) does not.
  const seenKeyRef = useRef(key);
  useEffect(() => {
    if (key && key !== seenKeyRef.current) {
      seenKeyRef.current = key;
      setLingering(true);
      setPinned(false);
      setAutoSheet(false);
    }
  }, [key]);
  // An answer with a component opens the sheet by itself; the caption alone
  // could not show it.
  useEffect(() => {
    if (lingering && hasComponent(answer)) setAutoSheet(true);
  }, [lingering, answer]);
  // The person is speaking again: the orb must be free.
  useEffect(() => {
    if (isVoiceState(bar.barState)) settle();
  }, [bar.barState, settle]);

  const [hovered, setHovered] = useState(false);
  const [focusWithin, setFocusWithin] = useState(false);
  const paused = hovered || focusWithin;
  const counting = lingering && !streaming && !working && !approvalPending && !noticeOpen;
  useLinger({
    running: counting,
    paused,
    durationMs: LINGER_MS,
    resetKey: `${key}:${visibleText.length}:${spokenParts.length}`,
    onExpire: settle,
  });

  // ── The sheet: hover or click, when there is more than the caption ──
  const hoverTimer = useRef<number | null>(null);
  const clearHoverTimer = () => {
    if (hoverTimer.current) {
      window.clearTimeout(hoverTimer.current);
      hoverTimer.current = null;
    }
  };
  useEffect(() => clearHoverTimer, []);
  const sheetAvailable = (hasSheetContent(answer) && (working || lingering)) || noticeOpen;
  const sheetOpen = sheetAvailable && (pinned || autoSheet || hoverOpen || noticeOpen);

  // Drag from anywhere, land in a gravity well. A tap still reaches the orb:
  // the gesture only becomes a drag past the movement threshold.
  const { dragProps, swallowClickAfterDrag } = useBarDrag({
    displayFollowPaused: sheetOpen || working,
  });

  const onPointerEnter = () => {
    setHovered(true);
    clearHoverTimer();
    hoverTimer.current = window.setTimeout(() => setHoverOpen(true), HOVER_OPEN_MS);
  };
  const onPointerLeave = () => {
    setHovered(false);
    clearHoverTimer();
    hoverTimer.current = window.setTimeout(() => setHoverOpen(false), HOVER_CLOSE_MS);
  };

  // ── Composer ──
  const [input, setInput] = useState("");
  const inputRef = useRef("");
  inputRef.current = input;
  const composerRef = useRef<HTMLInputElement>(null);
  const lineOpen = isInputState(bar.barState);
  useEffect(() => {
    if (bar.inputValue === "" && isRestState(bar.barState)) setInput("");
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
    const t = window.setTimeout(() => composerRef.current?.focus(), 60);
    return () => window.clearTimeout(t);
  }, [lineOpen]);

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
      .catch((e) => console.error("Orb: focus listener failed:", e));
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  // Escape: one behaviour, shared by every appearance (src/lib/barEscape.ts).
  useEscapeToIdle({
    barState: bar.barState,
    working: working,
    overlayOpen: sheetOpen || lingering || noticeOpen,
    composerOpen: lineOpen,
    popupOpen: false,
    collapse: settle,
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

  const idle = isRestState(bar.barState) && !lingering && !noticeOpen;
  const onOrbClick = useCallback(() => {
    if (idle) {
      void sendInteraction(UI.INTERACTION_TYPES_CLICK);
    } else if (sheetAvailable) {
      setPinned((v) => !v);
    }
  }, [idle, sheetAvailable]);

  // ── What the orb does ──
  const look = useMemo(
    () =>
      orbLook({
        state: bar.barState,
        stepsDone: turn.stepsDone,
        approvalPending,
        driving: isDriving,
      }),
    [bar.barState, turn.stepsDone, approvalPending, isDriving],
  );
  const impulseRef = useRef(0);
  const lastMotionRef = useRef(look.motion);
  if (look.motion !== lastMotionRef.current) {
    lastMotionRef.current = look.motion;
    if (look.motion === "flinch" || look.motion === "bloom") impulseRef.current += 1;
  }
  const driveRef = useRef<OrbDrive>({ look, targets: orbTargets(look, 0), impulse: 0 });
  driveRef.current = { look, targets: orbTargets(look, bar.audioLevel), impulse: impulseRef.current };
  useEffect(() => setLoop("always"), [look]);
  const onSettled = useCallback(() => setLoop("demand"), []);

  // ── The caption ──
  const question = turn.question || bar.lastSubmittedValue;
  const caption = captionFor({
    state: bar.barState,
    transcriptionText: bar.transcriptionText,
    transcriptionProvisional: bar.transcriptionProvisional,
    spokenText: bar.spokenText,
    currentError: bar.currentError,
    question,
    answer: answer ? { spokenParts, visibleText, streaming } : null,
    runningTool: turn.runningTool,
    approval: approvalPending ? turn.approval?.content || turn.approval?.tool_name || "do this" : null,
    drivingLabel: isDriving && driving ? drivingLabel(driving) : null,
    lingering,
  });

  // ── The window ──
  const [sheetContentH, setSheetContentH] = useState(SHEET_MIN_HEIGHT);
  const [measureEl, setMeasureEl] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!measureEl || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      setSheetContentH(Math.ceil(entry.contentRect.height) + SHEET_PAD);
    });
    observer.observe(measureEl);
    return () => observer.disconnect();
  }, [measureEl]);
  useEffect(() => {
    if (!sheetOpen) setSheetContentH(SHEET_MIN_HEIGHT);
  }, [sheetOpen]);

  const wanted: Layout = { posture: postureFor(caption, sheetOpen), sheetHeight: sheetHeightFor(sheetContentH) };
  const win = windowSize(wanted.posture, wanted.sheetHeight);

  // Growing: resize the window, then show. Shrinking: hide, then resize once
  // the caption has faded. The orb never moves: its centre is at the same
  // place in every window, and the resize is centre-stable and top-anchored.
  const [shown, setShown] = useState<Layout>({ posture: "orb", sheetHeight: SHEET_MIN_HEIGHT });
  const prevWinRef = useRef(windowSize("orb"));
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
        setShown(wanted);
      })();
    } else {
      setShown(wanted);
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
  }, [win.width, win.height, wanted.posture, wanted.sheetHeight, resizeWindowIfChanged]);
  useEffect(
    () => () => {
      if (shrinkTimerRef.current) window.clearTimeout(shrinkTimerRef.current);
    },
    [],
  );

  // ── Under the orb ──
  let below: ReactNode = null;
  if (shown.posture !== "orb" && caption) {
    if (caption.kind === "composer") {
      below = (
        <CaptionSlot key="composer" reducedMotion={reducedMotion} testId="orb-slot">
          <CaptionComposer
            value={input}
            disabled={bar.barState !== UI.BAR_STATES_INPUT}
            inputRef={composerRef}
            onChange={changeInput}
            onSubmit={submit}
          />
        </CaptionSlot>
      );
    } else if (caption.kind === "approval" && turn.approval) {
      below = (
        <CaptionSlot key="approval" reducedMotion={reducedMotion} testId="orb-slot">
          <CaptionApproval caption={caption} msg={turn.approval} onDecided={chat.handleApprovalUpdate} />
        </CaptionSlot>
      );
    } else if (caption.kind === "words" && shown.posture !== "sheet") {
      // The sheet is the whole answer; the line would only repeat its start.
      below = (
        <CaptionSlot key="words" reducedMotion={reducedMotion} testId="orb-slot">
          <CaptionLine caption={caption} />
        </CaptionSlot>
      );
    }
  }
  const sheet =
    shown.posture === "sheet" ? (
      <CaptionSlot key="sheet" reducedMotion={reducedMotion} testId="orb-sheet-slot">
        <CaptionSheet height={shown.sheetHeight} measureRef={setMeasureEl} reducedMotion={reducedMotion}>
          <InputControlNotices />
          {visibleText ? <MixedContentRenderer content={visibleText} isStreaming={streaming} /> : null}
        </CaptionSheet>
      </CaptionSlot>
    ) : null;

  return (
    <div
      data-testid="orb-bar"
      data-posture={shown.posture}
      data-loop={loop}
      className="relative h-screen w-screen cursor-grab overflow-hidden bg-transparent select-none active:cursor-grabbing"
      onPointerEnter={onPointerEnter}
      onPointerLeave={onPointerLeave}
      onFocusCapture={() => setFocusWithin(true)}
      onBlurCapture={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setFocusWithin(false);
      }}
      {...dragProps}
      onClickCapture={swallowClickAfterDrag}
    >
      {/* The stage: centred, top-anchored, sized once. The orb's centre is at
          (width/2, STAGE/2) in every posture, so nothing under it can move it. */}
      <div
        className="absolute left-1/2 top-0 -translate-x-1/2"
        style={{ width: STAGE, height: STAGE }}
        onClick={onOrbClick}
        data-testid="orb"
        data-motion={look.motion}
        data-tint={look.tint[0]}
      >
        <div
          className="h-full w-full"
          style={{
            animation: "orb-breathe 4s ease-in-out infinite",
            animationPlayState: look.motion === "breathe" ? "running" : "paused",
          }}
        >
          <OrbCanvas drive={driveRef} frameloop={loop} onSettled={onSettled} />
        </div>
      </div>
      <div
        className="absolute left-1/2 flex -translate-x-1/2 flex-col items-center gap-2"
        style={{ top: STAGE + CAPTION_GAP, width: win.width }}
      >
        <AnimatePresence initial={false}>
          {below}
          {sheet}
        </AnimatePresence>
      </div>
    </div>
  );
}
