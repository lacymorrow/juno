import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { motion, useReducedMotion } from "motion/react";
import { ArrowUp, MessageSquare, Mic, Square, Type, Volume2, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { EVENTS, UI, COMMANDS } from "@/lib/constants.generated";
import { useBarDrag } from "@/hooks/useDragWindow";
import { useEventListener } from "@/hooks/useEventListener";
import { openComposerThroughRust, useEscapeToIdle, type ClimbTarget } from "@/hooks/useEscapeToIdle";
import { useBarConversation } from "@/hooks/useBarConversation";
import { useSkillAutocomplete } from "@/hooks/useSkillAutocomplete";
import { isSendKey, useAutoGrowTextarea } from "@/hooks/useAutoGrowTextarea";
import { SkillGhostText, SkillSuggestionList } from "@/components/SkillAutocomplete";
import { MixedContentRenderer, splitHeroContent } from "@/components/ui/mixed-content-renderer";
import { InputControlNotices } from "@/components/input-control/InputControlNotices";
import { drivingLabel, type InputControlStatePayload } from "@/lib/inputControl";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";
import { cancelVoiceTurn, sendVoiceTurn } from "@/lib/voiceTurn";
import { contentPlacement, contentRect } from "@/lib/steadyFrame";
import type { ChatMessage } from "@/types/chat";
import { IslandDot, IslandShell, IslandWords, ISLAND_EASE } from "./IslandShell";
import { LingerRing } from "./LingerRing";
import { useLinger } from "./useLinger";
import { ISLAND_BUTTON_ATTR, useIslandHover } from "./useIslandHover";
import { useIslandFrame } from "./useIslandFrame";
import { clampLevel } from "../voiceLevel";
import {
  COMPOSER_LINE_PX,
  COMPOSER_MAX_TEXT_PX,
  ISLAND_WIDTH,
  LINGER_MS,
  SHADOW_PAD,
  STOP_LABELS,
  answerKey,
  composerHeightFor,
  composerPlaceholder,
  dotFor,
  faceFor,
  isInputState,
  isRecording,
  isRestingState,
  isWorkingState,
  latestTurn,
  lingerShouldRun,
  shapeFor,
  stopControl,
  type Face,
} from "./islandModel";

/**
 * Island: one surface that flips between the idle bar, the composer, an
 * answer and a component.
 *
 * Spec and the full state table: docs/plans/island-appearance.md. In short:
 * a capsule at rest (its controls under the pointer); past rest the island is
 * the text box, whatever Rust is doing, its placeholder saying what that is;
 * an answer streams in above the box, which stays as the follow-up field; a
 * component answer is the hero, edge to edge. The composer is one element for
 * the life of the island, so a draft or a dictation landing in it can never
 * lose its field to a state change (LAC-4240).
 *
 * It lives on the steady frame (`useIslandFrame`): the window never resizes
 * for a state, the shape morphs inside it on one critically damped spring,
 * and the rest of the window lets clicks through.
 */

/** Backend element id for interactions. Must match `ui::element_ids::DYNAMIC_BAR`. */
const COMPONENT_ID = UI.ELEMENT_IDS_DYNAMIC_BAR;

/** The answer's header row: the dot, your question, the close control. */
const HEADER_H = 36;
/** Room under the answer body, above the composer's hairline. */
const BODY_PAD_BOTTOM = 12;
/** One row of the skill suggestions, and the list's own padding. */
const SUGGESTION_ROW_H = 29;
const SUGGESTION_LIST_PAD = 20;
const SUGGESTION_GAP = 8;

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

async function sendInteraction(type: string, data?: Record<string, unknown>): Promise<void> {
  const interaction = {
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

function activateWindow(): void {
  getCurrentWindow()
    .setFocus()
    .catch((error) => console.debug("Island: window activation failed:", error));
}

// === SMALL PARTS ===

/** A tool waiting on Allow or Don't: one row inside the answer. */
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
      className="mb-3 flex items-center gap-2 rounded-[12px] bg-white/[0.06] py-2 pl-3 pr-2"
      data-testid="island-approval"
      role="group"
      aria-label="Permission"
    >
      <p className="min-w-0 flex-1 text-[12.5px] leading-snug text-white/85">Juno wants to {title}</p>
      <button
        type="button"
        disabled={busy}
        onClick={() => void decide("denied")}
        className="h-7 shrink-0 rounded-full px-3 text-[12px] text-white/70 transition-colors hover:bg-white/[0.08] hover:text-white disabled:opacity-50"
      >
        Don&rsquo;t
      </button>
      <button
        type="button"
        disabled={busy}
        onClick={() => void decide("approved")}
        className="h-7 shrink-0 rounded-full bg-[#0A84FF] px-3.5 text-[12px] font-medium text-white transition-colors hover:bg-[#2B93FF] active:bg-[#0071E3] disabled:opacity-50"
      >
        Allow
      </button>
    </div>
  );
}

/** Five bars that follow the mic. They stand in for the mic button while one is open. */
function LevelBars({ level }: { level: number }) {
  const l = Math.sqrt(clampLevel(level));
  const weights = [0.55, 0.85, 1, 0.8, 0.5];
  return (
    <div className="flex h-6 w-6 shrink-0 items-center justify-center gap-[2px]" aria-hidden="true" data-testid="island-level">
      {weights.map((w, i) => (
        <span
          key={i}
          className="block w-[2px] rounded-full bg-white/80"
          style={{ height: 3 + Math.round(11 * l * w), transition: "height 90ms linear" }}
        />
      ))}
    </div>
  );
}

const CONTROL =
  "flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-full text-white/55 transition-colors hover:bg-white/[0.14] hover:text-white";

// === THE BAR ===

export function IslandBar() {
  const reducedMotion = useReducedMotion() ?? false;
  const frame = useIslandFrame();
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
  const recording = isRecording(bar.barState, bar.transcriptionProvisional);
  const working = (isWorkingState(bar.barState) && !recording) || chat.isProcessing;
  const hero = useMemo(
    () => (visibleText ? splitHeroContent(visibleText, streaming) : null),
    [visibleText, streaming],
  );
  const question = turn.question || bar.lastSubmittedValue;

  // ── Local state: the answer, the composer ──
  const [answerOpen, setAnswerOpen] = useState(false);
  const [spokenOpen, setSpokenOpen] = useState(false);
  const [captionOpen, setCaptionOpen] = useState(false);
  // The component answer stepped back from (X or Escape): its key.
  const [steppedBack, setSteppedBack] = useState<string | null>(null);
  const [composerHeld, setComposerHeld] = useState(false);
  const [composerFocused, setComposerFocused] = useState(false);
  // Put away by hand while Rust still holds its input state: stay idle until it lets go.
  const [dismissed, setDismissed] = useState(false);
  const [draft, setDraft] = useState("");
  const dismissedRef = useRef(dismissed);
  dismissedRef.current = dismissed;
  const draftRef = useRef(draft);
  draftRef.current = draft;
  const barRef = useRef(bar);
  barRef.current = bar;

  // A reply that arrives while the island is up opens the answer. One that was
  // already there when it mounted (history) does not.
  const seenKeyRef = useRef(key);
  useEffect(() => {
    if (key && key !== seenKeyRef.current) {
      seenKeyRef.current = key;
      setAnswerOpen(true);
      setSpokenOpen(false);
      setCaptionOpen(false);
    }
  }, [key]);
  useEffect(() => {
    if (turn.approval) setAnswerOpen(true);
  }, [turn.approval]);
  useEventListener(EVENTS.INPUT_CONTROL_REQUEST, () => setAnswerOpen(true));

  // ── The face ──
  const answerShowing = answerOpen && !!(turn.answer || turn.approval);
  const componentShown = !!hero && steppedBack !== key && !turn.approval;
  const dictation = bar.isDictationMode || bar.barState === UI.BAR_STATES_DICTATING;
  const composerEngaged = draft.trim() !== "" || composerFocused;
  const face: Face = faceFor({
    state: bar.barState,
    answerOpen: answerShowing,
    componentShown,
    composerActive: composerEngaged || composerHeld,
    dictation,
    dismissed,
  });

  // Drag from anywhere, land in a gravity well. Not re-homed to another
  // display while an answer is up or Juno works.
  const { dragProps, swallowClickAfterDrag } = useBarDrag({
    displayFollowPaused: face !== "idle" || working,
  });

  // ── The composer: one textarea for the life of the island ──
  const composer = useAutoGrowTextarea({
    value: draft,
    maxHeightPx: COMPOSER_MAX_TEXT_PX,
    minHeightPx: COMPOSER_LINE_PX,
  });
  const composerH = composerHeightFor(composer.height);
  const skill = useSkillAutocomplete({
    value: draft,
    onAccept: setDraft,
    enabled: face !== "idle",
  });
  const focusComposer = useCallback(() => {
    composer.ref.current?.focus();
  }, [composer.ref]);
  // Rust's input state is the composer opened on purpose (a click, the type
  // control, the field itself): put the caret in it.
  useEffect(() => {
    if (!isInputState(bar.barState)) {
      setDismissed(false);
      return;
    }
    if (!dismissedRef.current) focusComposer();
  }, [bar.barState, focusComposer]);

  const changeDraft = useCallback((value: string) => {
    setDraft(value);
    void sendInteraction(UI.INTERACTION_TYPES_INPUT_CHANGE, { value });
  }, []);
  const submit = useCallback(() => {
    // A turn is already on its way; Enter pressed twice must not send twice.
    if (working) return;
    const value = draftRef.current.trim();
    if (!value) return;
    void sendInteraction(UI.INTERACTION_TYPES_SUBMIT, { value });
    setDraft("");
  }, [working]);

  /** Back to the capsule: the answer closes and the composer goes away. */
  const [hoverHeld, setHoverHeld] = useState(false);
  const settle = useCallback(() => {
    setAnswerOpen(false);
    setSpokenOpen(false);
    setComposerHeld(false);
    setHoverHeld(true);
  }, []);
  /**
   * Put the island away by hand (Escape, the close control). At rest the
   * draft goes too, which is what putting it away means; while Rust is busy
   * the press is Rust's to act on and the draft is kept.
   */
  const putAway = useCallback(() => {
    const resting = isRestingState(barRef.current.barState) || isInputState(barRef.current.barState);
    if (!resting) return;
    settle();
    setDismissed(true);
    if (draftRef.current !== "") changeDraft("");
    composer.ref.current?.blur();
    // The composer put Rust in its input state; with nothing typed, tell it
    // the composer blurred so it shrinks to Default.
    if (isInputState(barRef.current.barState)) void sendInteraction(UI.INTERACTION_TYPES_BLUR);
  }, [settle, changeDraft, composer.ref]);
  /** A component's X: step back to the composer, the draft and caret kept. */
  const stepBack = useCallback(() => {
    setSteppedBack(key);
    setAnswerOpen(false);
    setComposerHeld(true);
    focusComposer();
  }, [key, focusComposer]);

  // ── Hover, engagement and the ring ──
  const hover = useIslandHover(frame.region);
  const hovered = hover.hovered;
  useEffect(() => {
    if (!hovered && hoverHeld) setHoverHeld(false);
  }, [hovered, hoverHeld]);
  const [focusWithin, setFocusWithin] = useState(false);
  const [engaged, setEngaged] = useState(false);
  const [ringEpoch, setRingEpoch] = useState(0);
  const paused = hovered || focusWithin;
  const counting = lingerShouldRun({
    face,
    streaming,
    working,
    approvalPending: !!turn.approval,
    composerEngaged,
  });
  const { progress } = useLinger({
    running: counting && !engaged,
    paused,
    durationMs: LINGER_MS,
    resetKey: `${key}:${face}:${visibleText.length}:${spokenText.length}:${ringEpoch}`,
    onExpire: settle,
  });
  useEffect(() => {
    if (engaged && !hovered && !focusWithin) {
      setEngaged(false);
      setRingEpoch((e) => e + 1);
    }
  }, [engaged, hovered, focusWithin]);
  const engage = useCallback(() => setEngaged(true), []);

  // ── OS focus ──
  //
  // Only losing focus is reported. A press on the capsule makes the window
  // key, and reporting that would turn every drag into "open the composer".
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;
    getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (!mounted || focused) return;
        void sendInteraction(UI.INTERACTION_TYPES_BLUR);
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

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void sendInteraction(UI.INTERACTION_TYPES_ENTER);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  // ── The way back to the last answer ──
  const hasTurn = !!(turn.question || turn.answer);
  const openAnswer = useCallback(() => {
    setAnswerOpen(true);
    setSteppedBack(null);
    setSpokenOpen(false);
    setEngaged(false);
    setRingEpoch((e) => e + 1);
  }, []);
  const faceRef = useRef(face);
  faceRef.current = face;
  const hasTurnRef = useRef(hasTurn);
  hasTurnRef.current = hasTurn;
  // The tray's Show/Hide Chat.
  useEventListener(EVENTS.BAR_TOGGLE_PANE, () => {
    const f = faceRef.current;
    if (f === "answer" || f === "component") settle();
    else if (hasTurnRef.current) openAnswer();
  });

  // Escape: one behaviour, shared by every appearance (src/lib/barEscape.ts).
  // A component steps back to the composer; anything else settles.
  useEscapeToIdle({
    barState: bar.barState,
    working: working,
    overlayOpen: face === "answer" || face === "component",
    composerOpen: face === "compose",
    popupOpen: skill.open,
    collapse: () => (faceRef.current === "component" ? stepBack() : putAway()),
    report: reportEscape,
    climb: (to: ClimbTarget) => {
      if (to === "floating" && hasTurn) {
        openAnswer();
        return;
      }
      openComposerThroughRust(() => void sendInteraction(UI.INTERACTION_TYPES_CLICK));
    },
  });

  // ── Controls ──
  const startTalking = useCallback((e: ReactMouseEvent) => {
    e.stopPropagation();
    void invoke(COMMANDS.AGENT_AGENT_VOICE, { action: "start" }).catch((error) =>
      console.error("Island: could not start listening:", error),
    );
  }, []);
  const startTyping = useCallback((e: ReactMouseEvent) => {
    e.stopPropagation();
    // Typing has to land here, so the window is made key on purpose.
    activateWindow();
    void sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, []);
  const showAnswer = useCallback(
    (e: ReactMouseEvent) => {
      e.stopPropagation();
      openAnswer();
    },
    [openAnswer],
  );
  const stopKind = stopControl({
    face,
    state: bar.barState,
    provisional: bar.transcriptionProvisional,
    working,
    driving: isDriving,
  });
  const stop = useCallback(() => {
    if (stopKind === "cancel") {
      void cancelVoiceTurn(barRef.current);
    } else if (stopKind === "stop") {
      void chat.stop();
    } else {
      putAway();
    }
  }, [stopKind, chat, putAway]);

  // ── Size ──
  const [measureEl, setMeasureEl] = useState<HTMLDivElement | null>(null);
  const [bodyH, setBodyH] = useState(0);
  const attachMeasure = useCallback((el: HTMLDivElement | null) => {
    // An exiting layer unmounts after the entering one has attached; keep the newest.
    if (el) setMeasureEl(el);
  }, []);
  useEffect(() => {
    if (!measureEl || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => setBodyH(Math.ceil(entry.contentRect.height)));
    observer.observe(measureEl);
    return () => observer.disconnect();
  }, [measureEl]);
  const hoverShown = face === "idle" && hovered && !hoverHeld;
  const shape = shapeFor(face, {
    hovered: hoverShown,
    composerHeight: composerH,
    contentHeight: HEADER_H + bodyH + BODY_PAD_BOTTOM,
  });

  // ── Where it is drawn, and what takes the mouse ──
  const layout = frame.layout;
  const growUp = layout.growUp;
  const suggestionsH =
    skill.open && face !== "idle" ? skill.suggestions.length * SUGGESTION_ROW_H + SUGGESTION_LIST_PAD : 0;
  const extra = suggestionsH ? SUGGESTION_GAP + suggestionsH : 0;
  const block = { width: shape.width + 2 * SHADOW_PAD, height: shape.height + 2 * SHADOW_PAD + extra };
  const rect = contentRect(layout, block);
  const hit = {
    x: rect.x + SHADOW_PAD,
    y: rect.y + SHADOW_PAD,
    width: shape.width,
    height: shape.height + extra,
  };
  const { report } = frame;
  useLayoutEffect(() => {
    report({ width: block.width, height: block.height }, hit);
  }, [report, block.width, block.height, hit.x, hit.y, hit.width, hit.height]); // eslint-disable-line react-hooks/exhaustive-deps
  const place = contentPlacement(layout);

  // ── Layers ──
  const dot = dotFor(bar.barState, isDriving);
  const layerKey = face === "idle" ? (hoverShown ? "hover" : "idle") : face;
  const placeholder = composerPlaceholder({
    state: bar.barState,
    transcriptionText: bar.transcriptionText,
    transcriptionProvisional: bar.transcriptionProvisional,
    spokenText: bar.spokenText,
    currentError: bar.currentError,
    working,
    answerShown: face === "answer" || face === "component",
    runningTool: turn.runningTool,
    drivingLabel: isDriving && driving ? drivingLabel(driving) : null,
  });

  let layer: ReactNode = null;
  if (layerKey === "idle") {
    layer = (
      <div className="flex h-full w-full items-center justify-center" data-testid="island-capsule">
        <IslandDot look={dot} />
      </div>
    );
  } else if (layerKey === "hover") {
    const control = (name: string) =>
      cn(CONTROL, hover.hoveredButton === name && "bg-white/[0.14] text-white");
    layer = (
      <div className="flex h-full w-full items-center gap-2 pl-3.5 pr-1" data-testid="island-hover">
        <IslandDot look={dot} />
        <div className="ml-auto flex items-center gap-0.5">
          <button
            type="button"
            {...{ [ISLAND_BUTTON_ATTR]: "mic" }}
            onClick={startTalking}
            aria-label="Talk to Juno"
            title="Talk to Juno"
            className={control("mic")}
          >
            <Mic className="size-3.5" />
          </button>
          <button
            type="button"
            {...{ [ISLAND_BUTTON_ATTR]: "type" }}
            onClick={startTyping}
            aria-label="Type to Juno"
            title="Type to Juno"
            className={control("type")}
          >
            <Type className="size-3.5" />
          </button>
          {hasTurn && (
            <button
              type="button"
              {...{ [ISLAND_BUTTON_ATTR]: "answer" }}
              onClick={showAnswer}
              aria-label="Show last answer"
              title="Show last answer"
              className={control("answer")}
            >
              <MessageSquare className="size-3.5" />
            </button>
          )}
        </div>
      </div>
    );
  } else if (layerKey === "answer" || layerKey === "component") {
    const isComponent = layerKey === "component" && hero;
    const caption = (isComponent ? hero.prose : "") || question;
    layer = (
      <div className="dark flex h-full w-full flex-col" data-testid={`island-${layerKey}`}>
        <header className="flex shrink-0 items-center gap-2.5 pl-4 pr-3" style={{ height: HEADER_H }}>
          <IslandDot look={dot} />
          {isComponent && caption ? (
            <button
              type="button"
              onClick={() => {
                engage();
                setCaptionOpen((v) => !v);
              }}
              aria-expanded={captionOpen}
              className="min-w-0 flex-1 truncate text-left text-[13px] tracking-[-0.01em] text-white/55 hover:text-white/80"
              data-testid="island-caption"
            >
              {caption}
            </button>
          ) : (
            <p className="min-w-0 flex-1 truncate text-[13px] tracking-[-0.01em] text-white/45" data-testid="island-question">
              {question}
            </p>
          )}
          {!isComponent && visibleText && spokenText ? (
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
          <LingerRing
            progress={progress}
            counting={counting && !engaged}
            paused={paused}
            onClose={isComponent ? stepBack : putAway}
          />
        </header>
        <div
          data-no-drag
          onScroll={engage}
          className={cn(
            "island-scroll min-h-0 flex-1 cursor-auto select-text overflow-y-auto",
            isComponent ? "px-2" : "px-4",
          )}
          style={{ paddingBottom: BODY_PAD_BOTTOM }}
        >
          <div ref={attachMeasure} className="text-[13px] leading-[1.55] text-white/85">
            {isComponent ? (
              <>
                {captionOpen && hero.prose && (
                  <p className="mb-2 px-2 text-[12.5px] text-white/60" data-testid="island-caption-full">
                    {hero.prose}
                  </p>
                )}
                <div data-testid="island-hero">
                  <MixedContentRenderer content={hero.components} isStreaming={streaming} />
                </div>
              </>
            ) : (
              <>
                {turn.approval && <ApprovalRow msg={turn.approval} onDecided={chat.handleApprovalUpdate} />}
                <InputControlNotices />
                {visibleText ? (
                  <MixedContentRenderer content={visibleText} isStreaming={streaming} />
                ) : spokenText ? (
                  <p className="text-white/70" data-testid="island-spoken-only">
                    {spokenText}
                  </p>
                ) : working && !turn.approval ? (
                  <p className="text-white/45">{turn.runningTool || "Working…"}</p>
                ) : null}
                {visibleText && spokenText && spokenOpen && (
                  <p
                    className="mt-2 border-t border-white/[0.08] pt-2 text-[12px] text-white/55"
                    data-testid="island-spoken"
                  >
                    {spokenText}
                  </p>
                )}
              </>
            )}
          </div>
        </div>
      </div>
    );
  }

  // The content sits between the docked edge and the composer.
  const layerStyle =
    layerKey === "answer" || layerKey === "component"
      ? growUp
        ? { top: composerH, bottom: 0 }
        : { top: 0, bottom: composerH }
      : { top: 0, bottom: 0 };

  const idle = face === "idle";
  const onShapeClick = useCallback(() => {
    if (faceRef.current !== "idle") return;
    activateWindow();
    void sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, []);

  const composerNode = (
    <motion.div
      data-testid="island-composer"
      data-no-drag
      aria-hidden={idle ? true : undefined}
      initial={false}
      animate={{ opacity: idle ? 0 : 1 }}
      transition={
        reducedMotion
          ? ISLAND_EASE
          : idle
            ? { duration: 0.12, ease: "easeIn" }
            : { duration: 0.18, delay: 0.06, ease: "easeOut" }
      }
      className={cn(
        "absolute inset-x-0 flex items-start gap-2 pl-4 pr-2.5",
        idle && "pointer-events-none",
        (face === "answer" || face === "component") && (growUp ? "border-b" : "border-t"),
        "border-white/[0.08]",
      )}
      style={{
        height: composerH,
        paddingTop: (composerH - composer.height) / 2,
        ...(growUp ? { top: 0 } : { bottom: 0 }),
      }}
      onClick={(e) => e.stopPropagation()}
    >
      <div className="relative min-w-0 flex-1">
        <textarea
          ref={composer.attach}
          rows={1}
          value={draft}
          tabIndex={idle ? -1 : 0}
          onChange={(e) => {
            engage();
            changeDraft(e.target.value);
          }}
          onKeyDown={(e) => {
            skill.handleKeyDown(e);
            if (e.defaultPrevented) return;
            if (isSendKey(e)) {
              e.preventDefault();
              submit();
            }
          }}
          onMouseDown={activateWindow}
          onFocus={() => {
            setComposerFocused(true);
            setDismissed(false);
            void sendInteraction(UI.INTERACTION_TYPES_FOCUS, { isFocused: true });
          }}
          onBlur={() => {
            setComposerFocused(false);
            setComposerHeld(false);
            // The window losing focus is reported by the window listener.
            if (typeof document.hasFocus === "function" && !document.hasFocus()) return;
            if (!draftRef.current.trim()) void sendInteraction(UI.INTERACTION_TYPES_BLUR);
          }}
          aria-label={face === "answer" || face === "component" ? "Follow up" : "Ask Juno"}
          placeholder={placeholder.text}
          className={cn(
            "block w-full resize-none border-none bg-transparent outline-none placeholder:text-transparent",
            "text-[13px] leading-[18px] tracking-[-0.01em] text-white/90",
          )}
        />
        {draft === "" && (
          <div className="pointer-events-none absolute inset-x-0 top-0 flex" aria-hidden="true" data-testid="island-placeholder">
            <IslandWords words={placeholder} className="h-[18px] leading-[18px]" />
          </div>
        )}
        <SkillGhostText
          value={draft}
          ghostText={skill.ghostText}
          className="flex h-[18px] items-start text-[13px] leading-[18px] tracking-[-0.01em]"
          ghostClassName="text-white/25"
        />
      </div>
      <div className="flex h-[18px] shrink-0 items-center gap-1">
        {recording ? (
          <LevelBars level={bar.audioLevel} />
        ) : working ? null : (
          <button type="button" onClick={startTalking} aria-label="Talk to Juno" title="Talk to Juno" className={CONTROL}>
            <Mic className="size-3" />
          </button>
        )}
        {recording ? (
          <button
            type="button"
            onClick={() => void sendVoiceTurn()}
            aria-label="Send"
            title="Send what was said"
            className={cn(CONTROL, "bg-white/[0.16] text-white")}
          >
            <ArrowUp className="size-3" />
          </button>
        ) : (
          <button
            type="button"
            onClick={submit}
            aria-label="Send"
            title="Send"
            disabled={!draft.trim() || working}
            className={cn(
              CONTROL,
              draft.trim() && !working ? "bg-[#0A84FF] text-white hover:bg-[#2B93FF]" : "cursor-default opacity-40",
            )}
          >
            <ArrowUp className="size-3" />
          </button>
        )}
        {stopKind && (
          <button
            type="button"
            onClick={stop}
            aria-label={STOP_LABELS[stopKind]}
            title={stopKind === "stop" ? `${STOP_LABELS[stopKind]} (Esc)` : STOP_LABELS[stopKind]}
            className={cn(CONTROL, stopKind === "stop" && "bg-white/[0.08]")}
          >
            {stopKind === "stop" ? <Square className="size-2.5 fill-current" /> : <X className="size-3" />}
          </button>
        )}
      </div>
    </motion.div>
  );

  return (
    <div className="relative h-screen w-screen overflow-hidden bg-transparent select-none">
      <div
        {...dragProps}
        {...hover.pointerProps}
        data-testid="island-stage"
        onFocusCapture={() => setFocusWithin(true)}
        onBlurCapture={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setFocusWithin(false);
        }}
        onClickCapture={(e) => {
          if (swallowClickAfterDrag(e)) return;
          if (face !== "idle" && (e.target as HTMLElement).closest("button, textarea, a")) engage();
        }}
        className={cn(
          "absolute flex cursor-grab active:cursor-grabbing",
          growUp ? "flex-col-reverse" : "flex-col",
          layout.anchorX === "start" ? "items-start" : layout.anchorX === "end" ? "items-end" : "items-center",
        )}
        style={{
          left: place.left,
          right: place.right,
          top: place.top,
          bottom: place.bottom,
          transform: place.translateX ? "translateX(-50%)" : undefined,
          padding: SHADOW_PAD,
          gap: suggestionsH ? SUGGESTION_GAP : 0,
          // A drop into a well that draws differently swaps behind this.
          opacity: frame.hidden ? 0 : 1,
        }}
      >
        <IslandShell
          size={shape}
          layerKey={layerKey}
          layer={layer}
          layerStyle={layerStyle}
          reducedMotion={reducedMotion}
          onClick={idle ? onShapeClick : undefined}
        >
          {composerNode}
        </IslandShell>
        {suggestionsH > 0 && (
          <div style={{ width: ISLAND_WIDTH - 24 }} className="z-20">
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
    </div>
  );
}
