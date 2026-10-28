import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type ReactElement,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import {
  ArrowUp,
  Ear,
  EarOff,
  Maximize2,
  Mic,
  Minimize2,
  Plus,
  Settings,
  Square,
  Volume2,
  X,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { EVENTS, UI, COMMANDS } from "@/lib/constants.generated";
import { useBarDrag } from "@/hooks/useDragWindow";
import { useEventListener } from "@/hooks/useEventListener";
import { useTypeToFocus } from "@/hooks/useTypeToFocus";
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
import { contentPlacement, contentRect, roomForContent } from "@/lib/steadyFrame";
import type { ChatMessage } from "@/types/chat";
import { IslandDot, IslandShell, IslandWords, ISLAND_EASE, ISLAND_GLOW } from "./IslandShell";
import { LingerRing } from "./LingerRing";
import { useLinger } from "./useLinger";
import { ISLAND_BUTTON_ATTR, useIslandHover } from "./useIslandHover";
import { useIslandFrame } from "./useIslandFrame";
import { clampLevel } from "../voiceLevel";
import { flameForState, useShowGlowBorder } from "../barFlame";
import { useAgentSessions } from "@/hooks/useAgentSessions";
import { useBarDragging } from "@/hooks/useBarSnapWells";
import { useConnectivity } from "@/hooks/useConnectivity";
import { useStartupReadiness } from "@/hooks/useStartupReadiness";
import { dispatchWithImages, usePastedImages } from "@/hooks/usePastedImages";
import { useWakePhrase } from "@/hooks/useWakePhrase";
import { dotLabel } from "@/lib/pillStatus";
import { ChatContainerV2 } from "@/components/chat/ChatContainerV2";
import { AgentRosterStrip } from "@/components/AgentRosterStrip";
import { TooltipProvider } from "@/components/ui/tooltip";
import { PillTooltip, PILL_TOOLTIP_DELAY_MS } from "../PillTooltip";
import { useTalkShortcut } from "../useTalkShortcut";
import {
  COMPOSER_LINE_PX,
  COMPOSER_MAX_TEXT_PX,
  COMPOSER_PAD_Y,
  ROSTER_GAP,
  ROSTER_HEIGHT,
  THUMB_GAP,
  THUMB_PX,
  canSend,
  chatSizeInWindow,
  escapeIsRusts,
  islandEscapeStep,
  rosterShown,
  rosterWidthFor,
  ISLAND_WIDTH,
  LINGER_MS,
  SHADOW_PAD,
  STOP_LABELS,
  SYSTEM_RED,
  answerKey,
  composeIsCompact,
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
/** A control the native tracking area says is under the cursor, where CSS :hover cannot see. */
const CONTROL_LIT = "bg-white/[0.14] text-white";

/** Attributes that name a control for the forwarded hover (`useIslandHover`). */
const named = (name: string) => ({ [ISLAND_BUTTON_ATTR]: name });

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

  // Whether Juno can answer, and whether it is still warming up: Rust's
  // answers (`connectivity.rs`, `readiness.rs`), never derived here.
  const connectivity = useConnectivity();
  const warmingUp = useStartupReadiness();
  const wake = useWakePhrase("Island");

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

  // The Pill's activity flame, same language: green for your dictation, blue
  // for listening, red for an error, the focused session's colour while Juno
  // works. Off when the glow-border setting is off.
  const { sessions: agentSessions, focusSession } = useAgentSessions();
  const showGlowBorder = useShowGlowBorder();
  const sessionColor = agentSessions.find((s) => s.focused)?.display_color ?? "#0A84FF";
  const flame = showGlowBorder ? flameForState(bar.barState, working, sessionColor) : null;
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
  // Put away by hand (Escape, the close control): the capsule, with any draft
  // kept in the box until the composer is opened again or typed into.
  const [dismissed, setDismissed] = useState(false);
  const [draft, setDraft] = useState("");
  // The whole conversation, grown out of the same shape (the chat face).
  const [chatOpen, setChatOpen] = useState(false);
  // Pictures pasted into the composer, sent with the text (the Pill's rule).
  const pasted = usePastedImages();
  const imagesRef = useRef(pasted.images);
  imagesRef.current = pasted.images;
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
  // A new turn puts the last answer away. The answer on screen belongs to the
  // last question, and between the end of a spoken turn and Rust announcing
  // the new question (the final decode, then Sending) it was still the latest
  // turn, so it came back: talking to Juno showed her last reply. It goes once
  // the turn is really under way, the spoken words being decoded or anything
  // being sent; the new answer opens itself when it arrives. A spoken turn
  // cancelled while listening leaves it, and so does a dictation, whose final
  // decode arrives without the dictation flag (the field it types into may be
  // the follow-up under the answer).
  const dictatingNow = bar.isDictationMode || bar.barState === UI.BAR_STATES_DICTATING;
  const spokenTurnRef = useRef(false);
  useEffect(() => {
    const state = bar.barState;
    if (dictatingNow) spokenTurnRef.current = false;
    else if (state === UI.BAR_STATES_LISTENING) spokenTurnRef.current = true;
    const decodingSpokenTurn =
      state === UI.BAR_STATES_TRANSCRIBING && !bar.transcriptionProvisional && spokenTurnRef.current;
    if (decodingSpokenTurn || state === UI.BAR_STATES_SUBMITTING) {
      spokenTurnRef.current = false;
      setAnswerOpen(false);
    }
  }, [bar.barState, bar.transcriptionProvisional, dictatingNow]);
  useEventListener(EVENTS.INPUT_CONTROL_REQUEST, () => setAnswerOpen(true));

  // ── The face ──
  const answerShowing = answerOpen && !!(turn.answer || turn.approval);
  const componentShown = !!hero && steppedBack !== key && !turn.approval;
  const dictation = bar.isDictationMode || bar.barState === UI.BAR_STATES_DICTATING;
  // A pasted picture is a draft: the box is not the small bar while one waits.
  const composerEngaged = draft.trim() !== "" || composerFocused || pasted.images.length > 0;
  const face: Face = faceFor({
    state: bar.barState,
    answerOpen: answerShowing,
    componentShown,
    composerActive: composerEngaged || composerHeld,
    dictation,
    dismissed,
    chatOpen,
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
  const composerH = composerHeightFor(composer.height, pasted.images.length);
  const skill = useSkillAutocomplete({
    value: draft,
    onAccept: setDraft,
    enabled: face !== "idle",
  });
  const focusComposer = useCallback(() => {
    composer.ref.current?.focus();
  }, [composer.ref]);
  // Type to focus: a key with nothing editable focused starts the message.
  useTypeToFocus({ focus: focusComposer, recording: recording || dictatingNow });
  // Rust entering its input state is the composer opened on purpose (a
  // click, the type control, the Dock): bring a put-away draft back and put
  // the caret in it. Rust still in its input state just after the box was
  // put away is only Rust catching up, and the box stays away.
  const prevBarStateRef = useRef(bar.barState);
  useEffect(() => {
    const was = prevBarStateRef.current;
    prevBarStateRef.current = bar.barState;
    if (!isInputState(bar.barState)) return;
    if (!isInputState(was)) setDismissed(false);
    else if (dismissedRef.current) return;
    focusComposer();
  }, [bar.barState, focusComposer]);

  const changeDraft = useCallback((value: string) => {
    setDraft(value);
    void sendInteraction(UI.INTERACTION_TYPES_INPUT_CHANGE, { value });
  }, []);
  const submit = useCallback(() => {
    const value = draftRef.current.trim();
    const images = imagesRef.current;
    // A turn is already on its way; Enter pressed twice must not send twice.
    // A picture on its own is a message ("what is this?").
    if (!canSend({ draft: value, images: images.length, working })) return;
    if (images.length > 0) void dispatchWithImages(value, images, "Island");
    else void sendInteraction(UI.INTERACTION_TYPES_SUBMIT, { value });
    setDraft("");
    pasted.clear();
  }, [working, pasted]);

  /** Back to the capsule: the answer closes and the composer goes away. */
  const [hoverHeld, setHoverHeld] = useState(false);
  const settle = useCallback(() => {
    setChatOpen(false);
    setAnswerOpen(false);
    setSpokenOpen(false);
    setComposerHeld(false);
    setHoverHeld(true);
  }, []);
  /**
   * Put the island away by hand (Escape, the close control): back to the
   * capsule, whatever the draft. The draft stays in the box, out of sight,
   * and is there again when the composer opens or the person starts typing
   * (Lacy, 2026-10-17). Whether a press of Escape reaches this at all while
   * Rust is busy is `collapseOneRung`'s call, not this one's: the close
   * controls always do what they say.
   */
  const putAway = useCallback(() => {
    settle();
    setDismissed(true);
    composer.ref.current?.blur();
    // The composer put Rust in its input state, and Rust keeps it there while
    // its copy of the text is non-empty. Empty Rust's copy (only a mirror; the
    // draft lives here), then say the box blurred, in that order, so Rust
    // shrinks to Default.
    if (!isInputState(barRef.current.barState)) return;
    if (draftRef.current === "") void sendInteraction(UI.INTERACTION_TYPES_BLUR);
    else
      void sendInteraction(UI.INTERACTION_TYPES_INPUT_CHANGE, { value: "" }).then(() =>
        sendInteraction(UI.INTERACTION_TYPES_BLUR),
      );
  }, [settle, composer.ref]);
  const putAwayRef = useRef(putAway);
  putAwayRef.current = putAway;
  /** A component's X: step back to the composer, the draft and caret kept. */
  const stepBack = useCallback(() => {
    setSteppedBack(key);
    setAnswerOpen(false);
    setComposerHeld(true);
    focusComposer();
  }, [key, focusComposer]);

  /**
   * Out of the chat face, one rung: back to the answer it grew from, or to
   * the composer when there is no answer to go back to (`islandEscapeStep`).
   */
  const leaveChat = useCallback(
    (to: Face) => {
      setChatOpen(false);
      if (to === "compose") {
        setAnswerOpen(false);
        setComposerHeld(true);
        focusComposer();
      } else {
        setAnswerOpen(true);
      }
    },
    [focusComposer],
  );

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
    // A draft that was put away is not a follow-up being written.
    composerEngaged: composerFocused || (!dismissed && composerEngaged),
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
  //
  // Going somewhere else (another app, the desktop, another Juno window) puts
  // a composer nobody is using away: the island is the idle capsule again.
  // Rust's press-outside event covers the case where no focus change reaches
  // the page (the box was opened without the window ever becoming key). An
  // answer, a component, the chat or anything Juno is doing stays: those are
  // being shown, and the ring or Escape puts them away.
  const leftForElsewhere = useCallback(() => {
    if (faceRef.current !== "compose") return;
    const state = barRef.current.barState;
    if (!isInputState(state) && !isRestingState(state)) return;
    // A draft is kept, out of sight, the same as Escape.
    putAwayRef.current();
  }, []);
  useEventListener(EVENTS.SYSTEM_MOUSE_PRESSED_OUTSIDE, leftForElsewhere);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;
    getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (!mounted || focused) return;
        // Left for another app with nothing typed: a composer held open
        // after a step back folds away.
        if (!draftRef.current.trim()) setComposerHeld(false);
        void sendInteraction(UI.INTERACTION_TYPES_BLUR);
        leftForElsewhere();
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
  const hasAnswer = !!(turn.answer || turn.approval);
  const hasAnswerRef = useRef(hasAnswer);
  hasAnswerRef.current = hasAnswer;
  const openAnswer = useCallback(() => {
    setChatOpen(false);
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
  /** Into the chat face: the whole conversation, from an answer or the tray. */
  const openChat = useCallback(() => {
    setChatOpen(true);
    setEngaged(false);
  }, []);
  // Show/Hide Chat (the bar's right-click menu): the chat face, or back to
  // the capsule. Before any turn there is no chat to show, so it opens the
  // composer, the same as the hover capsule's Open chat.
  useEventListener(EVENTS.BAR_TOGGLE_PANE, () => {
    if (faceRef.current === "chat") settle();
    else if (hasTurnRef.current) openChat();
    else {
      activateWindow();
      void sendInteraction(UI.INTERACTION_TYPES_CLICK);
    }
  });

  // ── New chat: the tray, the right-click menu, Cmd+N, the + in a header ──
  //
  // The Pill's rule: stop an answer still arriving (rotating under a live
  // stream left it landing in a conversation nobody could see), start a fresh
  // conversation in Rust, clear the screen, and leave the caret in the box.
  // The chat face stays the chat face, empty with its example prompts.
  const workingRef = useRef(working);
  workingRef.current = working;
  const mainWindowOpenRef = useRef(false);
  useEventListener(EVENTS.BAR_MAIN_WINDOW_OPENED, () => {
    mainWindowOpenRef.current = true;
  });
  useEventListener(EVENTS.BAR_MAIN_WINDOW_CLOSED, () => {
    mainWindowOpenRef.current = false;
  });
  const startNewChat = useCallback(() => {
    void (async () => {
      if (workingRef.current) await chat.stop();
      await invoke(COMMANDS.CONVERSATIONS_NEW_CONVERSATION).catch(() => {});
      chat.startNewChat();
      setAnswerOpen(false);
      setSpokenOpen(false);
      setCaptionOpen(false);
      setSteppedBack(null);
      setDismissed(false);
      if (faceRef.current !== "chat") setComposerHeld(true);
      activateWindow();
      focusComposer();
    })();
  }, [chat.stop, chat.startNewChat, focusComposer]); // eslint-disable-line react-hooks/exhaustive-deps
  // While the full-size window is up it owns the conversation and answers
  // the menu itself.
  useEventListener(EVENTS.MENU_NEW_CHAT_REQUESTED, () => {
    if (!mainWindowOpenRef.current) startNewChat();
  });

  /**
   * One rung down the island's own layers: chat to the answer (or the
   * composer), a component to the composer, anything else to the capsule.
   */
  const collapseOneRung = useCallback(() => {
    // Rust busy (a dictation, a turn, a reply being spoken): that is the
    // newest rung, and this press is reported to Rust, which ends it. The
    // island's own faces wait for the next press, so one press is one rung.
    if (escapeIsRusts(barRef.current.barState)) return;
    const from = faceRef.current;
    const to = islandEscapeStep(from, { answerShown: hasAnswerRef.current });
    if (from === "chat") leaveChat(to);
    else if (to === "compose") stepBack();
    else putAway();
  }, [leaveChat, stepBack, putAway]);

  // Escape: one behaviour, shared by every appearance (src/lib/barEscape.ts).
  // A component steps back to the composer; anything else settles.
  useEscapeToIdle({
    barState: bar.barState,
    working: working,
    overlayOpen: face === "answer" || face === "component" || face === "chat",
    composerOpen: face === "compose",
    popupOpen: skill.open,
    collapse: collapseOneRung,
    report: reportEscape,
    climb: (to: ClimbTarget) => {
      if (to === "floating" && hasTurn) {
        openAnswer();
        return;
      }
      openComposerThroughRust(() => void sendInteraction(UI.INTERACTION_TYPES_CLICK));
    },
    // The Island's large chat is the big chat: every route that opened the
    // chat window opens it, empty with its example prompts before any turn.
    openFullChat: openChat,
  });

  // ── Controls ──
  const startTalking = useCallback((e: ReactMouseEvent) => {
    e.stopPropagation();
    void invoke(COMMANDS.AGENT_AGENT_VOICE, { action: "start" }).catch((error) =>
      console.error("Island: could not start listening:", error),
    );
  }, []);
  // The hover expand climbs one size: the small chat (the last answer), whose
  // own expand opens the full chat. Before any turn there is no answer to
  // show, so it opens the composer, where a chat starts.
  const expandFromHover = useCallback(
    (e: ReactMouseEvent) => {
      e.stopPropagation();
      if (hasTurnRef.current) {
        openAnswer();
        return;
      }
      activateWindow();
      void sendInteraction(UI.INTERACTION_TYPES_CLICK);
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

  // ── "See dictated text as you speak" ──
  // Read at mount and again as each recording starts or ends, so a change in
  // Settings lands on the next turn without a listener of its own.
  const [livePartials, setLivePartials] = useState(false);
  useEffect(() => {
    let cancelled = false;
    void invoke<boolean>(COMMANDS.STT_MODELS_GET_LIVE_PARTIAL_TRANSCRIPTION)
      .then((on) => {
        if (!cancelled && typeof on === "boolean") setLivePartials(on);
      })
      .catch((error) => console.debug("Island: could not read the live dictation setting:", error));
    return () => {
      cancelled = true;
    };
  }, [recording]);
  const compact = composeIsCompact({
    face,
    state: bar.barState,
    provisional: bar.transcriptionProvisional,
    composerActive: composerEngaged || composerHeld,
    livePartials,
  });

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
  // While the island is carried the capsule keeps the look it had at the
  // press. The drag window is that shape (`dragLayout`), so a leave reported
  // as the window shrinks and moves under the cursor must not fold the hover
  // controls away mid-drag: the shape would shrink inside a window that
  // cannot follow it, and the spot pressed would slide off the cursor.
  const dragging = useBarDragging();
  const hoverLive = face === "idle" && hovered && !hoverHeld;
  const hoverAtPressRef = useRef(hoverLive);
  if (!dragging) hoverAtPressRef.current = hoverLive;
  const hoverShown = dragging ? hoverAtPressRef.current : hoverLive;
  const layout = frame.layout;
  const growUp = layout.growUp;
  // Talk and Open chat always; the wake phrase toggle when one is set. Typing
  // is the capsule itself: a click anywhere on it opens the composer.
  const hoverButtons = 2 + (wake.configured ? 1 : 0);
  const shape = shapeFor(face, {
    hovered: hoverShown,
    hoverButtons,
    chat: chatSizeInWindow(layout.size, roomForContent(layout)),
    compact,
    composerHeight: composerH,
    contentHeight: HEADER_H + bodyH + BODY_PAD_BOTTOM,
  });

  // ── Where it is drawn, and what takes the mouse ──
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
  // The roster: a bubble of the same black beside the shape, on the side away
  // from a side-docked edge, level with the docked edge. Apple's split island:
  // a second activity is a second small shape, never a change to the first.
  // Out of the way while the island is carried (the drag window is the shape).
  const rosterVisible = rosterShown(agentSessions.length) && !dragging;
  const rosterW = rosterWidthFor(agentSessions.length);
  const rosterOnLeft = layout.anchorX === "end";
  const rosterHit = rosterVisible
    ? {
        x: rosterOnLeft ? hit.x - ROSTER_GAP - rosterW : hit.x + shape.width + ROSTER_GAP,
        y: growUp ? hit.y + hit.height - ROSTER_HEIGHT : hit.y,
        width: rosterW,
        height: ROSTER_HEIGHT,
      }
    : null;
  const { report } = frame;
  useLayoutEffect(() => {
    report({ width: block.width, height: block.height }, rosterHit ? [hit, rosterHit] : [hit]);
  }, [report, block.width, block.height, hit.x, hit.y, hit.width, hit.height, rosterHit?.x, rosterHit?.y, rosterHit?.width]); // eslint-disable-line react-hooks/exhaustive-deps
  const place = contentPlacement(layout);
  // Tooltips open toward the side the island grows: the frame always has room there.
  const tooltipSide = growUp ? "top" : "bottom";

  // ── Tooltips: the Pill's, for every control ──
  const lit = hover.hoveredButton;
  // Talk names its key, as the Pill's does ("Hold ⌃⌥").
  const talkShortcut = useTalkShortcut();
  const tip = (name: string, label: string, control: ReactElement<{ title?: string }>, detail?: string | null) => (
    <PillTooltip
      label={label}
      detail={detail}
      forcedOpen={lit === name}
      blocked={lit === "dot" && name !== "dot"}
      side={tooltipSide}
    >
      {control}
    </PillTooltip>
  );

  // ── Layers ──
  const dot = dotFor(bar.barState, isDriving, connectivity);
  const dotText = dotLabel(bar.barState, connectivity, {
    driving: isDriving,
    voicePaused: wake.configured && !wake.listening,
    loading: warmingUp,
  });
  // The dot with its label: a 15px target around the 7px dot.
  const dotTarget = tip(
    "dot",
    dotText,
    <span
      {...named("dot")}
      role="img"
      aria-label={dotText}
      data-testid="island-dot-target"
      data-tone={dot.color === SYSTEM_RED ? "down" : undefined}
      className="flex size-[15px] shrink-0 items-center justify-center"
    >
      <IslandDot look={dot} />
    </span>,
  );
  const layerKey = face === "idle" ? (hoverShown ? "hover" : "idle") : face;
  const placeholder = composerPlaceholder({
    state: bar.barState,
    // Words while you speak only when the person asked to see them.
    transcriptionText: recording && !livePartials ? "" : bar.transcriptionText,
    transcriptionProvisional: bar.transcriptionProvisional,
    spokenText: bar.spokenText,
    currentError: bar.currentError,
    working,
    // An empty chat (opened from the hover capsule before any turn) asks, it does not follow up.
    answerShown: face === "answer" || face === "component" || (face === "chat" && hasTurn),
    runningTool: turn.runningTool,
    drivingLabel: isDriving && driving ? drivingLabel(driving) : null,
  });

  const wakeLabel = wake.listening ? "Stop listening for the wake phrase" : "Listen for the wake phrase";
  // The Pill pane's +: a fresh conversation, the caret in the box.
  const newChatControl = tip(
    "new-chat",
    "New chat",
    <button
      type="button"
      {...named("new-chat")}
      onClick={(e) => {
        e.stopPropagation();
        startNewChat();
      }}
      aria-label="New chat"
      title="New chat"
      data-testid="island-new-chat"
      className={cn(CONTROL, "text-white/40", lit === "new-chat" && CONTROL_LIT)}
    >
      <Plus className="size-3.5" />
    </button>,
  );

  let layer: ReactNode = null;
  if (layerKey === "idle") {
    layer = (
      <div className="flex h-full w-full items-center justify-center" data-testid="island-capsule">
        <IslandDot look={dot} />
      </div>
    );
  } else if (layerKey === "hover") {
    const control = (name: string) => cn(CONTROL, lit === name && CONTROL_LIT);
    layer = (
      <div className="flex h-full w-full items-center gap-2 pl-2.5 pr-1" data-testid="island-hover">
        {dotTarget}
        <div className="ml-auto flex items-center gap-0.5">
          {tip(
            "mic",
            "Talk to Juno",
            <button type="button" {...named("mic")} onClick={startTalking} aria-label="Talk to Juno" title="Talk to Juno" className={control("mic")}>
              <Mic className="size-3.5" />
            </button>,
            talkShortcut,
          )}
          {/* The Pill's hover expand, always offered: one size up, the small
              chat (or the composer before any turn), never straight to the
              full chat. */}
          {tip(
            "chat",
            "Open chat",
            <button
              type="button"
              {...named("chat")}
              onClick={expandFromHover}
              aria-label="Open chat"
              title="Open chat"
              data-testid="island-hover-expand"
              className={control("chat")}
            >
              <Maximize2 className="size-3.5" />
            </button>,
          )}
          {/* The wake phrase, when there is one: pause an all-day microphone
              for a while without taking the trigger away in Settings. */}
          {wake.configured &&
            tip(
              "listen",
              wakeLabel,
              <button
                type="button"
                {...named("listen")}
                onClick={(e) => {
                  e.stopPropagation();
                  wake.toggle();
                }}
                aria-label={wakeLabel}
                title={wakeLabel}
                aria-pressed={wake.listening}
                data-testid="island-wake-toggle"
                data-listening={wake.listening ? "" : undefined}
                className={control("listen")}
              >
                {wake.listening ? <Ear className="size-3.5" /> : <EarOff className="size-3.5" />}
              </button>,
            )}
        </div>
      </div>
    );
  } else if (layerKey === "chat") {
    layer = (
      <div className="dark flex h-full w-full flex-col" data-testid="island-chat">
        <header className="flex shrink-0 items-center gap-2.5 pl-2.5 pr-3" style={{ height: HEADER_H }}>
          {dotTarget}
          <div className="flex-1" />
          {newChatControl}
          {tip(
            "settings",
            "Settings",
            <button
              type="button"
              {...named("settings")}
              onClick={(e) => {
                e.stopPropagation();
                void invoke(COMMANDS.WINDOWS_OPEN_SETTINGS_WINDOW).catch((error) =>
                  console.error("Island: could not open Settings:", error),
                );
              }}
              aria-label="Settings"
              title="Settings"
              className={cn(CONTROL, "text-white/40", lit === "settings" && CONTROL_LIT)}
            >
              <Settings className="size-3.5" />
            </button>,
          )}
          {tip(
            "collapse",
            hasAnswer ? "Back to the answer" : "Back to the text box",
            <button
              type="button"
              {...named("collapse")}
              onClick={(e) => {
                e.stopPropagation();
                leaveChat(islandEscapeStep("chat", { answerShown: hasAnswer }));
              }}
              aria-label={hasAnswer ? "Back to the answer" : "Back to the text box"}
              title={hasAnswer ? "Back to the answer" : "Back to the text box"}
              className={cn(CONTROL, "text-white/40", lit === "collapse" && CONTROL_LIT)}
            >
              <Minimize2 className="size-3.5" />
            </button>,
          )}
        </header>
        {/* select-text undoes the root's select-none so replies read like any Mac text. */}
        <div data-no-drag className="flex min-h-0 flex-1 cursor-auto select-text flex-col" onScroll={engage}>
          <ChatContainerV2
            conversation={chat.messages}
            copiedMessageId={chat.copiedMessageId}
            onCopyResponse={chat.handleCopyResponse}
            onShareResponse={chat.handleShareResponse}
            onExamplePromptSelect={(prompt) => {
              changeDraft(prompt);
              focusComposer();
            }}
            backendStatus={chat.serverStatus}
            onApprovalUpdate={chat.handleApprovalUpdate}
            onContinuationUpdate={chat.handleContinuationUpdate}
            className="island-scroll bg-transparent"
            contentClassName="gap-4 px-4 py-3 text-[13px]"
          />
        </div>
      </div>
    );
  } else if (layerKey === "answer" || layerKey === "component") {
    const isComponent = layerKey === "component" && hero;
    const caption = (isComponent ? hero.prose : "") || question;
    layer = (
      <div className="dark flex h-full w-full flex-col" data-testid={`island-${layerKey}`} data-surface="island">
        <header className="flex shrink-0 items-center gap-2.5 pl-2.5 pr-3" style={{ height: HEADER_H }}>
          {dotTarget}
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
          {!isComponent && visibleText && spokenText
            ? tip(
                "spoken",
                "Spoken aloud",
                <button
                  type="button"
                  {...named("spoken")}
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
                    lit === "spoken" && !spokenOpen && "bg-white/[0.08] text-white/80",
                  )}
                >
                  <Volume2 className="size-3.5" />
                </button>,
              )
            : null}
          {tip(
            "expand",
            "Show the conversation",
            <button
              type="button"
              {...named("expand")}
              onClick={(e) => {
                e.stopPropagation();
                openChat();
              }}
              aria-label="Show the conversation"
              title="Show the conversation"
              data-testid="island-expand"
              className={cn(CONTROL, "text-white/40", lit === "expand" && CONTROL_LIT)}
            >
              <Maximize2 className="size-3" />
            </button>,
          )}
          {tip(
            "close",
            isComponent ? "Back to the text box" : "Close",
            <LingerRing
              {...named("close")}
              title={isComponent ? "Back to the text box" : "Close"}
              progress={progress}
              counting={counting && !engaged}
              paused={paused}
              onClose={isComponent ? stepBack : putAway}
            />,
          )}
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

  // The composer is always the bottom row, whichever way the shape grows from
  // its anchor: the content sits above it.
  const beside = layerKey === "answer" || layerKey === "component" || layerKey === "chat";
  const layerStyle = beside ? { top: 0, bottom: composerH } : { top: 0, bottom: 0 };

  const idle = face === "idle";
  const onShapeClick = useCallback(() => {
    if (faceRef.current !== "idle") return;
    activateWindow();
    void sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, []);

  const sendable = canSend({ draft, images: pasted.images.length, working });
  const textBlockH = composerHeightFor(composer.height);
  const composerNode = (
    <motion.div
      data-testid="island-composer"
      data-compact={compact ? "true" : undefined}
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
        "absolute inset-x-0 flex flex-col pl-4 pr-2.5",
        idle && "pointer-events-none",
        beside && "border-t",
        "border-white/[0.08]",
      )}
      style={{
        height: composerH,
        paddingTop: pasted.images.length ? COMPOSER_PAD_Y : (textBlockH - composer.height) / 2,
        bottom: 0,
      }}
      onClick={(e) => e.stopPropagation()}
    >
      {pasted.images.length > 0 && (
        <div className="flex shrink-0 items-center gap-1.5" style={{ height: THUMB_PX, marginBottom: THUMB_GAP }} data-testid="island-thumbs">
          {pasted.images.map((src, i) => (
            <div key={`${i}-${src.length}`} className="relative shrink-0 overflow-hidden rounded-[7px] ring-1 ring-white/15" style={{ width: THUMB_PX, height: THUMB_PX }}>
              <img src={src} alt={`Pasted image ${i + 1}`} className="size-full object-cover" draggable={false} />
              {tip(
                `remove-${i}`,
                "Remove image",
                <button
                  type="button"
                  {...named(`remove-${i}`)}
                  onClick={() => pasted.remove(i)}
                  aria-label={`Remove image ${i + 1}`}
                  title="Remove image"
                  className={cn(
                    "absolute right-0.5 top-0.5 flex size-3.5 items-center justify-center rounded-full bg-black/75 text-white/80 hover:text-white",
                    lit === `remove-${i}` && "text-white",
                  )}
                >
                  <X className="size-2" />
                </button>,
              )}
            </div>
          ))}
        </div>
      )}
      <div className="flex items-start gap-2">
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
            onPaste={pasted.onPaste}
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
              // A draft kept through a put-away: Rust's mirror was emptied
              // then, so Cmd+Enter (which sends Rust's copy) needs it back.
              if (draftRef.current !== "") {
                void sendInteraction(UI.INTERACTION_TYPES_INPUT_CHANGE, { value: draftRef.current });
              }
            }}
            onBlur={() => {
              setComposerFocused(false);
              // The window losing focus is reported by the window listener.
              if (typeof document.hasFocus === "function" && !document.hasFocus()) return;
              if (!draftRef.current.trim()) void sendInteraction(UI.INTERACTION_TYPES_BLUR);
            }}
            aria-label={beside && (face !== "chat" || hasTurn) ? "Follow up" : "Ask Juno"}
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
            tip(
              "compose-mic",
              "Talk to Juno",
              <button type="button" {...named("compose-mic")} onClick={startTalking} aria-label="Talk to Juno" title="Talk to Juno" className={cn(CONTROL, lit === "compose-mic" && CONTROL_LIT)}>
                <Mic className="size-3" />
              </button>,
              talkShortcut,
            )
          )}
          {recording
            ? tip(
                "send",
                "Send what was said",
                <button
                  type="button"
                  {...named("send")}
                  onClick={() => void sendVoiceTurn()}
                  aria-label="Send"
                  title="Send what was said"
                  className={cn(CONTROL, "bg-white/[0.16] text-white")}
                >
                  <ArrowUp className="size-3" />
                </button>,
              )
            : compact
              ? null
              : tip(
                  "send",
                  "Send",
                  <button
                    type="button"
                    {...named("send")}
                    onClick={submit}
                    aria-label="Send"
                    title="Send"
                    disabled={!sendable}
                    className={cn(
                      CONTROL,
                      sendable ? "bg-[#0A84FF] text-white hover:bg-[#2B93FF]" : "cursor-default opacity-40",
                    )}
                  >
                    <ArrowUp className="size-3" />
                  </button>,
                )}
          {stopKind &&
            tip(
              "stop",
              stopKind === "stop" ? `${STOP_LABELS[stopKind]} (Esc)` : STOP_LABELS[stopKind],
              <button
                type="button"
                {...named("stop")}
                onClick={stop}
                aria-label={STOP_LABELS[stopKind]}
                title={stopKind === "stop" ? `${STOP_LABELS[stopKind]} (Esc)` : STOP_LABELS[stopKind]}
                className={cn(CONTROL, stopKind === "stop" && "bg-white/[0.08]", lit === "stop" && CONTROL_LIT)}
              >
                {stopKind === "stop" ? <Square className="size-2.5 fill-current" /> : <X className="size-3" />}
              </button>,
            )}
        </div>
      </div>
    </motion.div>
  );

  const rosterNode = (
    <AnimatePresence initial={false}>
      {rosterVisible && (
        <motion.div
          key="roster"
          data-testid="island-roster"
          initial={{ opacity: 0, scale: 0.85 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.85 }}
          transition={reducedMotion ? ISLAND_EASE : { duration: 0.18, ease: "easeOut" }}
          className="absolute flex items-center justify-center rounded-full bg-black"
          style={{
            width: rosterW,
            height: ROSTER_HEIGHT,
            boxShadow: ISLAND_GLOW,
            [rosterOnLeft ? "right" : "left"]: `calc(100% + ${ROSTER_GAP}px)`,
            [growUp ? "bottom" : "top"]: 0,
          }}
        >
          <AgentRosterStrip
            sessions={agentSessions}
            onFocus={focusSession}
            tooltipSide={tooltipSide}
            className="gap-2 border-0 bg-transparent p-0 backdrop-blur-none"
          />
        </motion.div>
      )}
    </AnimatePresence>
  );

  return (
    <TooltipProvider delayDuration={PILL_TOOLTIP_DELAY_MS} skipDelayDuration={300}>
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
          {/* The shape and its roster bubble move as one: the bubble rides the
              shape's side, so the shape itself never moves for it. */}
          <div className="relative shrink-0">
            <IslandShell
              size={shape}
              layerKey={layerKey}
              layer={layer}
              layerStyle={layerStyle}
              reducedMotion={reducedMotion}
              flame={flame}
              onClick={idle ? onShapeClick : undefined}
            >
              {composerNode}
            </IslandShell>
            {rosterNode}
          </div>
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
    </TooltipProvider>
  );
}
