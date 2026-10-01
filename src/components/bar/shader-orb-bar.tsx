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
import { COMMANDS, EVENTS, UI, WINDOW_LABELS } from "@/lib/constants.generated";
import { useWindowSize } from "@/hooks/useWindowSize";
import { useBarDrag } from "@/hooks/useDragWindow";
import { useEventListener } from "@/hooks/useEventListener";
import { useEscapeToIdle } from "@/hooks/useEscapeToIdle";
import { useBarConversation } from "@/hooks/useBarConversation";
import { MixedContentRenderer } from "@/components/ui/mixed-content-renderer";
import { InputControlNotices } from "@/components/input-control/InputControlNotices";
import { type InputControlStatePayload } from "@/lib/inputControl";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";
import { useLinger } from "./island/useLinger";
import { answerKey, latestTurn } from "./orb/orbModel";
import {
  CaptionApproval,
  CaptionComposer,
  CaptionLine,
  CaptionSheet,
  CaptionSlot,
  useOrbKeyframes,
} from "./orb/OrbCaption";
import { OrbShaderCanvas, type Frameloop, type OrbDrive } from "./shader-orb/OrbShaderCanvas";
import {
  HELD_MS,
  PANEL_GAP,
  SHEET_MIN_HEIGHT,
  SHRINK_DELAY_MS,
  STAGE,
  hasSheet,
  isImpulse,
  isInputState,
  isRestState,
  isVoiceState,
  isWorkingState,
  mustOpenSheet,
  orbLook,
  orbTargets,
  posture as postureFor,
  sheetHeightFor,
  windowSize,
  wordsFor,
  type Posture,
} from "./shader-orb/shaderOrbModel";

/**
 * Orb: one canvas that tells you what Juno is doing.
 *
 * The sphere is the whole interface. It rests small and dim, cools to blue
 * and swells with your voice, turns while Juno works and quickens with every
 * finished step, ripples while Juno speaks, blooms green when it lands and
 * goes red and flinches when it fails. An approval stops it dead.
 *
 * Words are not a running commentary. The panel under the sphere opens only
 * when Juno needs an answer from you, when something failed, when you asked
 * to type, or when you click to read: there is no status line and no
 * transcript, because the sphere already said it. Reading is one click, and
 * the sheet carries the question as well as the answer, so a misheard query
 * is recoverable without a second look.
 *
 * Spec and the full state table: docs/plans/orb-appearance.md.
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

interface ShaderOrbBarProps {
  barAppearance?: string;
}

export function ShaderOrbBar(_props: ShaderOrbBarProps) {
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
        // Anything Rust says may move the sphere; it sleeps again once settled.
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
  const streaming = !!answer?.isStreaming;
  const working = isWorkingState(bar.barState) || chat.isProcessing;
  const approvalPending = !!turn.approval;
  const question = turn.question || bar.lastSubmittedValue;

  // ── An answer nobody has read yet ──
  // The sphere holds a green ember for it, which is what makes the click
  // worth making. Reading it, or speaking again, clears it.
  const [unread, setUnread] = useState(false);
  const [pinned, setPinned] = useState(false);
  const [autoSheet, setAutoSheet] = useState(false);
  const settle = useCallback(() => {
    setUnread(false);
    setPinned(false);
    setAutoSheet(false);
    setNoticeOpen(false);
  }, []);
  // An answer that arrives while the orb is up is unread. One that was
  // already there when it mounted (history) is not.
  const seenKeyRef = useRef(key);
  useEffect(() => {
    if (key && key !== seenKeyRef.current) {
      seenKeyRef.current = key;
      setUnread(true);
      setPinned(false);
      setAutoSheet(false);
    }
  }, [key]);
  // A component cannot be spoken, so an answer carrying one opens the sheet
  // by itself rather than waiting for a click that may never come.
  useEffect(() => {
    if (unread && mustOpenSheet(answer)) setAutoSheet(true);
  }, [unread, answer]);
  // The person is speaking again: the sphere must be free.
  useEffect(() => {
    if (isVoiceState(bar.barState)) settle();
  }, [bar.barState, settle]);

  const [hovered, setHovered] = useState(false);
  const [focusWithin, setFocusWithin] = useState(false);
  const counting = unread && !streaming && !working && !approvalPending && !noticeOpen;
  useLinger({
    running: counting,
    paused: hovered || focusWithin,
    durationMs: HELD_MS,
    resetKey: `${key}:${visibleText.length}`,
    onExpire: settle,
  });

  // ── The sheet: by a click, or by itself for a component ──
  const sheetContent = useMemo(
    () => ({ question, answer, streaming }),
    [question, answer, streaming],
  );
  const sheetAvailable = hasSheet(sheetContent, noticeOpen) && (working || unread || noticeOpen);
  const sheetOpen = sheetAvailable && (pinned || autoSheet || noticeOpen);

  // Drag from anywhere, land in a gravity well. A tap still reaches the
  // sphere: the gesture only becomes a drag past the movement threshold.
  const { dragProps, swallowClickAfterDrag } = useBarDrag({
    displayFollowPaused: sheetOpen || working,
  });

  // ── Composer ──
  const [input, setInput] = useState("");
  const composerRef = useRef<HTMLInputElement>(null);
  const typing = isInputState(bar.barState);
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
    if (!typing) return;
    const t = window.setTimeout(() => composerRef.current?.focus(), 60);
    return () => window.clearTimeout(t);
  }, [typing]);

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
    overlayOpen: sheetOpen || unread || noticeOpen,
    composerOpen: typing,
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

  // One click, one idea: give me the words. With something to read that is
  // the sheet; with nothing to read it is the composer.
  const onOrbClick = useCallback(() => {
    if (sheetAvailable) {
      // `unread` stays set: it is what keeps the sheet available and the
      // linger clock running. The ember lets go because the look asks for
      // `unread && !sheetOpen`, not because the answer stopped existing.
      setAutoSheet(false);
      setPinned((v) => !v);
      return;
    }
    if (isRestState(bar.barState)) void sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, [sheetAvailable, bar.barState]);

  // ── What the sphere does ──
  const look = useMemo(
    () =>
      orbLook({
        state: bar.barState,
        stepsDone: turn.stepsDone,
        approvalPending,
        driving: isDriving,
        answerWaiting: unread && !sheetOpen,
      }),
    [bar.barState, turn.stepsDone, approvalPending, isDriving, unread, sheetOpen],
  );
  const impulseRef = useRef(0);
  const lastMotionRef = useRef(look.motion);
  if (look.motion !== lastMotionRef.current) {
    lastMotionRef.current = look.motion;
    if (isImpulse(look.motion)) impulseRef.current += 1;
  }
  const driveRef = useRef<OrbDrive>({ look, targets: orbTargets(look, 0), impulse: 0 });
  driveRef.current = {
    look,
    targets: orbTargets(look, bar.audioLevel),
    impulse: impulseRef.current,
  };
  useEffect(() => setLoop("always"), [look]);
  const onSettled = useCallback(() => setLoop("demand"), []);

  // ── The words, if any are needed ──
  const words = wordsFor({
    state: bar.barState,
    currentError: bar.currentError,
    approval: approvalPending
      ? turn.approval?.content || turn.approval?.tool_name || "do this"
      : null,
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

  const wanted: Layout = {
    posture: postureFor(words, sheetOpen),
    sheetHeight: sheetHeightFor(sheetContentH),
  };
  const win = windowSize(wanted.posture, wanted.sheetHeight);

  // Growing: resize the window, then show. Shrinking: hide, then resize once
  // the panel has faded. The sphere never moves: its centre is at the same
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

  // ── Under the sphere ──
  let panel: ReactNode = null;
  if (shown.posture === "approval" && words?.kind === "approval" && turn.approval) {
    panel = (
      <CaptionSlot key="approval" reducedMotion={reducedMotion} testId="orb-slot">
        <CaptionApproval
          caption={{ kind: "approval", text: words.text }}
          msg={turn.approval}
          onDecided={chat.handleApprovalUpdate}
        />
      </CaptionSlot>
    );
  } else if (shown.posture === "words" && words?.kind === "composer") {
    panel = (
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
  } else if (shown.posture === "words" && words?.kind === "error") {
    panel = (
      <CaptionSlot key="error" reducedMotion={reducedMotion} testId="orb-slot">
        <CaptionLine caption={{ kind: "words", text: words.text, tone: "error" }} />
      </CaptionSlot>
    );
  } else if (shown.posture === "sheet") {
    panel = (
      <CaptionSlot key="sheet" reducedMotion={reducedMotion} testId="orb-sheet-slot">
        <CaptionSheet height={shown.sheetHeight} measureRef={setMeasureEl} reducedMotion={reducedMotion}>
          <InputControlNotices />
          {question ? (
            <p data-testid="orb-sheet-question" className="mb-2 text-white/45">
              {question}
            </p>
          ) : null}
          {visibleText ? <MixedContentRenderer content={visibleText} isStreaming={streaming} /> : null}
        </CaptionSheet>
      </CaptionSlot>
    );
  }

  return (
    <div
      data-testid="orb-bar"
      data-posture={shown.posture}
      data-loop={loop}
      className="relative h-screen w-screen cursor-grab overflow-hidden bg-transparent select-none active:cursor-grabbing"
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onFocusCapture={() => setFocusWithin(true)}
      onBlurCapture={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setFocusWithin(false);
      }}
      {...dragProps}
      onClickCapture={swallowClickAfterDrag}
    >
      {/* The stage: centred, top-anchored, sized once. The sphere's centre is
          at (width/2, STAGE/2) in every posture, so nothing under it moves it. */}
      <div
        className="absolute left-1/2 top-0 -translate-x-1/2"
        style={{ width: STAGE, height: STAGE }}
        onClick={onOrbClick}
        data-testid="orb"
        data-motion={look.motion}
        data-hue={look.hue}
        data-mono={look.mono}
      >
        {/* The resting breath is CSS, so the WebGL loop can sleep through it
            and the sphere still reads as alive on an idle desk. */}
        <div
          className="h-full w-full"
          style={{
            animation: "orb-breathe 4.4s ease-in-out infinite",
            animationPlayState: look.motion === "ember" ? "running" : "paused",
          }}
        >
          <OrbShaderCanvas drive={driveRef} frameloop={loop} onSettled={onSettled} size={STAGE} />
        </div>
      </div>
      <div
        className="absolute left-1/2 flex -translate-x-1/2 flex-col items-center"
        style={{ top: STAGE + PANEL_GAP, width: win.width }}
      >
        <AnimatePresence initial={false}>{panel}</AnimatePresence>
      </div>
    </div>
  );
}
