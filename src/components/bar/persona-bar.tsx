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
import { availableMonitors, getCurrentWindow } from "@tauri-apps/api/window";
import { useReducedMotion } from "motion/react";
import { Volume2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { COMMANDS, EVENTS, UI } from "@/lib/constants.generated";
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
import { answerKey, latestTurn } from "./island/islandModel";
import { LingerRing } from "./island/LingerRing";
import { useLinger } from "./island/useLinger";
import { AvatarHead } from "./avatar/AvatarHead";
import { Bubble, ThinkingMark } from "./avatar/Bubble";
import {
  ANSWER_MAX_HEIGHT,
  GAP,
  QUOTE_MAX_WIDTH,
  HEAD,
  LINGER_MS,
  PAD,
  SHRINK_DELAY_MS,
  STACK_STEP,
  isIdleState,
  isInputState,
  isVoiceState,
  isWorkingState,
  answerParts,
  sceneFor,
  windowFor,
  lingerShouldRun,
  type JunoBubble,
  type Scene,
  type YourBubble,
} from "./avatar/avatarModel";

/**
 * Avatar: a character you talk to.
 *
 * Spec and the full state table: docs/plans/avatar-appearance.md. In short:
 * a small head that blinks at rest; it leans in while you speak and your
 * words fill a bubble on your side; it thinks in a thought bubble with the
 * running tool named inside; it talks, and the answer (text and components)
 * sits in its bubble; it holds up a question when a tool needs you; it
 * winces at a failure; it nods when done. The head is the anchor: the window
 * grows around it and the bubbles never push it.
 */

/** Backend element id for interactions. Must match `ui::element_ids::FLOATING_BAR`. */
const COMPONENT_ID = "floating-bar";
const WINDOW_LABEL = "floating-bar";

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
    console.error("Avatar: interaction failed:", error);
  }
}

/** Escape, reported to Rust. What stopping means is Rust's decision. */
function reportEscape(): void {
  void sendInteraction(UI.INTERACTION_TYPES_ESCAPE);
}

/** Whether the window's centre is in the bottom half of its display. Bubbles
 *  then rise above the head so they never run off the bottom. */
async function isDockedLow(): Promise<boolean> {
  const win = getCurrentWindow();
  const [pos, size, monitors] = await Promise.all([win.outerPosition(), win.outerSize(), availableMonitors()]);
  const centerX = pos.x + size.width / 2;
  const centerY = pos.y + size.height / 2;
  const monitor =
    monitors.find(
      (m) =>
        centerX >= m.position.x &&
        centerX < m.position.x + m.size.width &&
        centerY >= m.position.y &&
        centerY < m.position.y + m.size.height,
    ) ?? monitors[0];
  if (!monitor) return false;
  return centerY >= monitor.position.y + monitor.size.height / 2;
}

// === THE QUESTION ===

function Question({
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
      console.error("Avatar: approval failed:", error);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div data-testid="avatar-question" role="group" aria-label="Permission">
      <p className="text-white/90">Can I {title}?</p>
      <div className="mt-2.5 flex items-center gap-2">
        <button
          type="button"
          disabled={busy}
          onClick={() => void decide("approved")}
          className={cn(
            "h-7 rounded-full bg-[#0A84FF] px-3.5 text-[12px] font-medium text-white",
            "transition-colors hover:bg-[#2B93FF] active:bg-[#0071E3] disabled:opacity-50",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/60",
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
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#0A84FF]/70",
          )}
        >
          Don&rsquo;t
        </button>
      </div>
    </div>
  );
}

// === THE BAR ===

interface PersonaBarProps {
  barAppearance?: string;
}

export function PersonaBar(_props: PersonaBarProps) {
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
      .catch((e) => console.error("Avatar: listener setup failed:", e));
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
  const hasAnswerContent = visibleText.length > 0 || spokenText.length > 0;
  const streaming = !!answer?.isStreaming;
  const working = isWorkingState(bar.barState) || chat.isProcessing;

  // Drag from anywhere, land in a gravity well. A tap still reaches the head:
  // the gesture only becomes a drag past the movement threshold.
  const { dragProps, swallowClickAfterDrag } = useBarDrag({
    displayFollowPaused: working,
  });

  // ── Juno's bubble: open and close ──
  const [answerOpen, setAnswerOpen] = useState(false);
  const [noticeOpen, setNoticeOpen] = useState(false);
  const [spokenOpen, setSpokenOpen] = useState(true);
  const barRef = useRef(bar);
  barRef.current = bar;
  const inputRef = useRef("");
  const closeAnswer = useCallback(() => {
    setAnswerOpen(false);
    setNoticeOpen(false);
    setSpokenOpen(true);
    // Focus put Rust in its input state while the bubble was up. With nothing
    // typed, tell it the composer blurred so it settles back to rest.
    if (isInputState(barRef.current.barState) && inputRef.current.trim() === "") {
      void sendInteraction(UI.INTERACTION_TYPES_BLUR);
    }
  }, []);

  // A reply that arrives while the avatar is up opens the bubble. A reply
  // that was already there when it mounted (history) does not.
  const seenKeyRef = useRef(key);
  useEffect(() => {
    if (key && key !== seenKeyRef.current) {
      seenKeyRef.current = key;
      setAnswerOpen(true);
      setSpokenOpen(true);
    }
  }, [key]);
  useEffect(() => {
    if (turn.approval) setAnswerOpen(true);
  }, [turn.approval]);
  useEventListener(EVENTS.INPUT_CONTROL_REQUEST, () => setNoticeOpen(true));
  // The person is speaking again: the last answer leaves.
  useEffect(() => {
    if (isVoiceState(bar.barState)) closeAnswer();
  }, [bar.barState, closeAnswer]);

  // ── Composer ──
  const [input, setInput] = useState("");
  inputRef.current = input;
  const composerRef = useRef<HTMLInputElement>(null);
  const composerOpen = isInputState(bar.barState);
  const skill = useSkillAutocomplete({ value: input, onAccept: setInput, enabled: composerOpen });
  // Rust clears its copy when it shrinks; follow it, never while typing.
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
    if (bar.barState !== UI.BAR_STATES_INPUT) return;
    const t = window.setTimeout(() => composerRef.current?.focus(), 60);
    return () => window.clearTimeout(t);
  }, [bar.barState]);

  // ── Engagement and the linger ──
  const [hovered, setHovered] = useState(false);
  const [focusWithin, setFocusWithin] = useState(false);
  const [engaged, setEngaged] = useState(false);
  const [lingerEpoch, setLingerEpoch] = useState(0);
  const paused = hovered || focusWithin;
  const counting = lingerShouldRun({
    answerOpen: answerOpen && hasAnswerContent,
    streaming,
    working,
    approvalPending: !!turn.approval,
    composerOpen,
  });
  const { progress } = useLinger({
    running: counting && !engaged,
    paused,
    durationMs: LINGER_MS,
    resetKey: `${key}:${visibleText.length}:${spokenText.length}:${lingerEpoch}`,
    onExpire: closeAnswer,
  });
  useEffect(() => {
    if (engaged && !hovered && !focusWithin) {
      setEngaged(false);
      setLingerEpoch((e) => e + 1);
    }
  }, [engaged, hovered, focusWithin]);
  const engage = useCallback(() => setEngaged(true), []);

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
      .catch((e) => console.error("Avatar: focus listener failed:", e));
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  // Escape: one behaviour, shared by every appearance (src/lib/barEscape.ts).
  useEscapeToIdle({
    barState: bar.barState,
    working: working,
    overlayOpen: answerOpen || noticeOpen,
    composerOpen: composerOpen,
    popupOpen: skill.open,
    collapse: closeAnswer,
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
  const onHeadClick = useCallback(() => {
    if (idle) void sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, [idle]);

  // ── The scene ──
  const question = turn.question || bar.lastSubmittedValue;
  const scene: Scene = sceneFor({
    state: bar.barState,
    transcriptionText: bar.transcriptionText,
    transcriptionProvisional: bar.transcriptionProvisional,
    spokenText: bar.spokenText,
    currentError: bar.currentError,
    question,
    runningTool: turn.runningTool,
    drivingLabel: isDriving && driving ? drivingLabel(driving) : null,
    answerOpen,
    hasAnswerContent,
    approvalPending: !!turn.approval,
  });
  // A cursor-control notice with nothing else to say gets a plain bubble of
  // its own. It hides itself once the notice is answered (`empty:hidden`).
  const junos: JunoBubble | null = scene.junos ?? (noticeOpen ? { kind: "notice" as const } : null);
  const yours: YourBubble | null = scene.yours;
  const open = yours !== null || junos !== null;

  // ── Direction ──
  // Docked in the bottom half of the display, bubbles rise above the head.
  // Decided each time the avatar opens from rest; at rest the head sits at
  // the same spot either way, so the flip itself never moves it.
  const [facingUp, setFacingUp] = useState(false);

  // ── The stack, measured ──
  const [stackH, setStackH] = useState(0);
  const [stackEl, setStackEl] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!stackEl || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => setStackH(Math.ceil(entry.contentRect.height)));
    observer.observe(stackEl);
    return () => observer.disconnect();
  }, [stackEl]);
  const box = windowFor({ ...scene, yours, junos }, open ? stackH : 0);
  // While an answer streams, one step of headroom so the bubble's next line
  // lands inside the window rather than under its edge.
  const win = {
    width: box.width,
    height: box.height + (open && streaming ? STACK_STEP * 2 : 0),
    anchorY: box.anchorY,
  };

  // ── Window protocol ──
  // Growing: resize first, then show the bubble. Shrinking: the bubble is
  // already gone from the scene; resize once its exit has played. Both land
  // position and size in one backend call, anchored on the head's centre.
  const [applied, setApplied] = useState({ width: 0, height: 0 });
  const appliedRef = useRef(applied);
  const facingUpRef = useRef(facingUp);
  const shrinkTimerRef = useRef<number | null>(null);
  useEffect(() => {
    const prev = appliedRef.current;
    const growing = win.width > prev.width || win.height > prev.height;
    const same = win.width === prev.width && win.height === prev.height;
    if (shrinkTimerRef.current) {
      window.clearTimeout(shrinkTimerRef.current);
      shrinkTimerRef.current = null;
    }
    if (same) return;
    let cancelled = false;
    if (growing) {
      void (async () => {
        let up = facingUpRef.current;
        // Opening from rest: decide which way the bubbles go.
        if (open && prev.width < win.width) {
          try {
            up = await isDockedLow();
          } catch {
            up = false;
          }
          if (cancelled) return;
          facingUpRef.current = up;
          setFacingUp(up);
        }
        await resizeWindowIfChanged({ ...win, growUp: up });
        if (cancelled) return;
        appliedRef.current = win;
        setApplied(win);
      })();
    } else {
      shrinkTimerRef.current = window.setTimeout(() => {
        void resizeWindowIfChanged({ ...win, growUp: facingUpRef.current });
        appliedRef.current = win;
        setApplied(win);
      }, SHRINK_DELAY_MS);
    }
    return () => {
      cancelled = true;
    };
    // The box is a value object; its fields are the real dependencies.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [win.width, win.height, resizeWindowIfChanged]);
  useEffect(
    () => () => {
      if (shrinkTimerRef.current) window.clearTimeout(shrinkTimerRef.current);
    },
    [],
  );
  const shown = applied.width >= win.width && applied.height >= win.height;

  // ── Bubbles ──
  let yourNode: ReactNode = null;
  if (yours?.kind === "composer") {
    yourNode = (
      <Bubble
        tail="you"
        facingUp={facingUp}
        edge="plain"
        shown={shown}
        className="self-end"
        data-testid="avatar-composer"
      >
        <form onSubmit={submit} className="flex items-center gap-2 px-3.5 py-2" data-no-drag>
          <div className="relative min-w-0" style={{ width: 220 }}>
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
              disabled={!yours.ready}
              className={cn(
                "w-full bg-transparent text-[13px] tracking-[-0.01em] text-white/90 outline-none",
                "placeholder:text-white/30 disabled:opacity-60",
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
      </Bubble>
    );
  } else if (yours?.kind === "words") {
    const edge = yours.tone === "dictation" ? "dictation" : yours.tone === "live" || yours.tone === "provisional" ? "listen" : "plain";
    const withAnswer = junos !== null;
    yourNode = (
      <Bubble
        tail="you"
        facingUp={facingUp}
        edge={withAnswer ? "plain" : edge}
        shown={shown}
        className="self-end"
        data-testid="avatar-yours"
        data-tone={yours.tone}
        // Once Juno's bubble is up, your line steps aside so the tail under
        // the head has a clear path: a short, dimmed quote on your side.
        style={withAnswer ? { maxWidth: QUOTE_MAX_WIDTH } : undefined}
      >
        <p
          className={cn(
            "px-3.5 py-2",
            withAnswer && "truncate",
            yours.tone === "dim" ? "text-white/55" : yours.tone === "provisional" ? "italic text-white/65" : "text-white/90",
          )}
        >
          {yours.text}
        </p>
      </Bubble>
    );
  }

  let junoNode: ReactNode = null;
  if (junos) {
    const notices = <InputControlNotices className="px-0 pt-0" />;
    if (junos.kind === "thought") {
      junoNode = (
        <Bubble tail="thought" facingUp={facingUp} shown={shown} className="self-center" data-testid="avatar-thought">
          <div className="flex items-center gap-2.5 px-4 py-2.5">
            <ThinkingMark />
            <span className="text-white/70">{junos.text}</span>
          </div>
          {notices}
        </Bubble>
      );
    } else if (junos.kind === "speech") {
      junoNode = (
        <Bubble tail="head" facingUp={facingUp} shown={shown} className="self-center" data-testid="avatar-speech">
          <p className="px-3.5 py-2 text-white/90">{junos.text}</p>
          {notices}
        </Bubble>
      );
    } else if (junos.kind === "error") {
      junoNode = (
        <Bubble
          tail="head"
          facingUp={facingUp}
          edge="error"
          shown={shown}
          className="self-center"
          data-testid="avatar-error"
          role="alert"
        >
          <p className="px-3.5 py-2 text-white/90">{junos.text}</p>
          {notices}
        </Bubble>
      );
    } else if (junos.kind === "question" && turn.approval) {
      junoNode = (
        <Bubble tail="head" facingUp={facingUp} shown={shown} className="self-center" data-testid="avatar-ask">
          <div className="px-3.5 py-2.5" data-no-drag>
            <Question msg={turn.approval} onDecided={chat.handleApprovalUpdate} />
            {notices}
          </div>
        </Bubble>
      );
    } else if (junos.kind === "notice") {
      junoNode = (
        <Bubble tail="head" facingUp={facingUp} shown={shown} className="self-center empty:hidden" data-testid="avatar-notice">
          <div className="px-3.5 py-2.5 empty:hidden" data-no-drag>
            {notices}
          </div>
        </Bubble>
      );
    } else if (junos.kind === "answer") {
      const parts = answerParts(visibleText, spokenText);
      const both = parts.foldable;
      const spokenShown = parts.spoken !== null && (!both || spokenOpen);
      junoNode = (
        <Bubble tail="head" facingUp={facingUp} shown={shown} className="self-center" data-testid="avatar-answer">
          <div className="absolute right-2 top-2 z-10 flex items-center gap-1">
            {both && (
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
                  "flex size-[22px] shrink-0 items-center justify-center rounded-full transition-colors",
                  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#0A84FF]/70",
                  spokenOpen ? "bg-white/[0.12] text-white/85" : "text-white/40 hover:bg-white/[0.08] hover:text-white/80",
                )}
              >
                <Volume2 className="size-3.5" />
              </button>
            )}
            <LingerRing progress={progress} counting={counting && !engaged} paused={paused} onClose={closeAnswer} />
          </div>
          <div
            data-no-drag
            onScroll={engage}
            className="av-bubble-scroll cursor-auto select-text overflow-y-auto px-3.5 py-2.5"
            style={{ maxHeight: ANSWER_MAX_HEIGHT, paddingRight: both ? 60 : 36 }}
          >
            {spokenShown && (
              <p className="text-white/90" data-testid="avatar-spoken">
                {parts.spoken}
              </p>
            )}
            {parts.notes && (
              <div
                className={cn("text-white/85", spokenShown && "mt-2 border-t border-white/[0.08] pt-2")}
                data-testid="avatar-notes"
              >
                <MixedContentRenderer content={parts.notes} isStreaming={streaming} />
              </div>
            )}
            {notices}
          </div>
        </Bubble>
      );
    }
  }

  const stackOffset = PAD + HEAD + GAP;

  return (
    <div
      className="relative h-screen w-screen overflow-hidden bg-transparent select-none"
      data-testid="avatar-root"
      data-open={open ? "true" : "false"}
      data-facing-up={facingUp ? "true" : "false"}
      {...dragProps}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onFocusCapture={() => setFocusWithin(true)}
      onBlurCapture={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setFocusWithin(false);
      }}
      onClickCapture={(e) => {
        if (swallowClickAfterDrag(e)) return;
        if (answerOpen && (e.target as HTMLElement).closest("button, input, a")) engage();
      }}
    >
      {/* The head is centred and a fixed distance from the still edge. The
          window resizes centre-stable and anchored on that point, so the
          head never moves on screen while a bubble comes or goes. */}
      <div
        className="absolute left-1/2 -translate-x-1/2"
        style={facingUp ? { bottom: PAD } : { top: PAD }}
      >
        <AvatarHead
          look={scene.head}
          facingUp={facingUp}
          reducedMotion={reducedMotion}
          level={isVoiceState(bar.barState) ? bar.audioLevel : 0}
          onClick={idle ? onHeadClick : undefined}
          className={idle ? "cursor-pointer" : "cursor-grab active:cursor-grabbing"}
        />
      </div>
      <div
        ref={setStackEl}
        data-testid="avatar-stack"
        className={cn("dark absolute flex flex-col gap-2.5", !open && "hidden")}
        style={{
          left: PAD,
          right: PAD,
          ...(facingUp ? { bottom: stackOffset } : { top: stackOffset }),
        }}
      >
        {yourNode}
        {yours?.kind === "composer" && skill.open && (
          <div className="self-end" style={{ width: 252 }}>
            <SkillSuggestionList
              variant="bar"
              suggestions={skill.suggestions}
              selectedIndex={skill.selectedIndex}
              onSelect={skill.accept}
              onHighlight={skill.setSelectedIndex}
            />
          </div>
        )}
        {junoNode}
      </div>
    </div>
  );
}
