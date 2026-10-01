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
import type { VoiceAIBarProps } from "@/types/voice-ai";
import { useLinger } from "./island/useLinger";
import {
  CloseMark,
  StudioLine,
  StudioShell,
  Tape,
  TapeCounter,
  Teleprompter,
  WaveStrip,
} from "./studio/StudioShell";
import { useTapeCounter, useWaveSamples } from "./studio/studioHooks";
import {
  LINGER_MS,
  SHADOW_PAD,
  SHRINK_DELAY_MS,
  STUDIO_SIZES,
  answerKey,
  commitTranscript,
  counterFor,
  isIdleState,
  isInputState,
  isVoiceState,
  isWorkingState,
  lineFor,
  posture as postureFor,
  replyChannels,
  scriptFor,
  studioSize,
  tapeShouldRun,
  teleprompterText,
  waveFor,
  windowSize,
  type Direction,
  type StudioSize,
} from "./studio/studioModel";

/**
 * Studio: a recording studio for your voice.
 *
 * Spec and the full state table: docs/plans/studio-appearance.md. In short:
 * a light deck at rest with a flat hairline; a take while you speak, the
 * waveform following your voice in green and your words rolling up a
 * teleprompter; a type to write in; work while Juno works, with a tape
 * counter that counts and holds; and a script that grows downward for the
 * answer: "You" and "Juno" lines, notes beneath, stage directions in italics
 * for tools and approvals. There is no chat pane. The studio shows the
 * current take; history lives in the main window.
 */

/** Backend element id for interactions. Must match `ui::element_ids::VOICE_AI_BAR`. */
const COMPONENT_ID = UI.ELEMENT_IDS_VOICE_AI_BAR;
const WINDOW_LABEL = WINDOW_LABELS.FLOATING_BAR;

/** Header and footer heights inside the script, for the content measurement. */
const SCRIPT_HEADER_H = 40;
const SCRIPT_FOOTER_H = 40;
/** Vertical padding around the script body. */
const SCRIPT_BODY_PAD = 10;

/** The strip's size per posture. */
const STRIP = {
  deck: { width: 88, height: 14 },
  take: { width: STUDIO_SIZES.take.width - 28, height: 22 },
  type: { width: 40, height: 14 },
  script: { width: 120, height: 18 },
};

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
    console.error("Studio: interaction failed:", error);
  }
}

// === THE SCRIPT'S PIECES ===

/** A speaker name above a line, the way a screenplay sets it. */
function Speaker({ name }: { name: string }) {
  return (
    <p className="m-0 mb-0.5 select-none text-[10px] font-medium uppercase tracking-[0.12em] text-black/40">
      {name}
    </p>
  );
}

/** A stage direction: what happened between the lines, in italics. */
function StageDirection({ direction }: { direction: Direction }) {
  const verb = direction.kind === "running" ? "runs" : "ran";
  return (
    <p
      className={cn("m-0 my-1.5 text-[12px] italic leading-snug", direction.kind === "running" ? "text-black/55" : "text-black/35")}
      data-testid="studio-direction"
      data-kind={direction.kind}
    >
      Juno {verb} {direction.text}
    </p>
  );
}

/** A tool waiting on you, as a stage direction with the two answers. */
function ApprovalDirection({
  msg,
  onDecided,
}: {
  msg: ChatMessage;
  onDecided: (toolId: string, state: "approved" | "denied") => void;
}) {
  const [busy, setBusy] = useState(false);
  const text = (msg.content || msg.tool_name || "do this").trim().replace(/\.$/, "");
  const decide = async (state: "approved" | "denied") => {
    if (!msg.tool_id || busy) return;
    setBusy(true);
    try {
      const command = state === "approved" ? "approve_tool_execution" : "deny_tool_execution";
      const ok = await invoke<boolean>(command, { toolId: msg.tool_id });
      if (ok) onDecided(msg.tool_id, state);
    } catch (error) {
      console.error("Studio: approval failed:", error);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="my-1.5" data-testid="studio-approval" role="group" aria-label="Permission">
      <p className="m-0 text-[12px] italic leading-snug text-black/55">Juno asks to {text}</p>
      <div className="mt-1.5 flex items-center gap-2">
        <button
          type="button"
          disabled={busy}
          onClick={() => void decide("approved")}
          className={cn(
            "h-6 rounded-full bg-[#0A84FF] px-3 text-[12px] font-medium text-white",
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
            "h-6 rounded-full px-3 text-[12px] text-black/60",
            "transition-colors hover:bg-black/[0.06] hover:text-black disabled:opacity-50",
          )}
        >
          Don&rsquo;t
        </button>
      </div>
    </div>
  );
}

// === THE BAR ===

export function VoiceAIBar(_props: VoiceAIBarProps = {}) {
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
      .catch((e) => console.error("Studio: listener setup failed:", e));
    return () => {
      mounted = false;
      safeCleanupEventListener(unlisten);
    };
  }, []);

  const [driving, setDriving] = useState<InputControlStatePayload | null>(null);
  useEventListener<InputControlStatePayload>(EVENTS.INPUT_CONTROL_STATE, setDriving);
  const isDriving = !!driving?.active;

  // ── The current take ──
  const script = useMemo(() => scriptFor(chat.messages), [chat.messages]);
  const key = answerKey(script);
  const { spoken, visible } = replyChannels(script.answer);
  const streaming = !!script.answer?.isStreaming;
  const working = isWorkingState(bar.barState) || chat.isProcessing;

  // ── The script ──
  const [scriptOpen, setScriptOpen] = useState(false);

  // Drag from anywhere, land in a gravity well.
  const { dragProps, swallowClickAfterDrag } = useBarDrag({
    displayFollowPaused: scriptOpen || working,
  });

  // Read through refs so a close from a timer sees the current state.
  const barRef = useRef(bar);
  barRef.current = bar;
  const inputRef = useRef("");
  const closeScript = useCallback(() => {
    setScriptOpen(false);
    // Focus put Rust in its input state while the script was up. With
    // nothing typed, tell it the composer blurred so it shrinks to Default
    // and the studio settles to the deck rather than the type.
    if (isInputState(barRef.current.barState) && inputRef.current.trim() === "") {
      void sendInteraction(UI.INTERACTION_TYPES_BLUR);
    }
  }, []);

  // A reply that arrives while the studio is up opens the script. A reply
  // that was already there when it mounted (history) does not.
  const seenKeyRef = useRef(key);
  useEffect(() => {
    if (key && key !== seenKeyRef.current) {
      seenKeyRef.current = key;
      setScriptOpen(true);
    }
  }, [key]);
  useEffect(() => {
    if (script.approval) setScriptOpen(true);
  }, [script.approval]);
  useEventListener(EVENTS.INPUT_CONTROL_REQUEST, () => setScriptOpen(true));
  // The person is speaking again: the take must be free.
  useEffect(() => {
    if (isVoiceState(bar.barState)) closeScript();
  }, [bar.barState, closeScript]);

  // ── Engagement and the tape ──
  const [hovered, setHovered] = useState(false);
  const [focusWithin, setFocusWithin] = useState(false);
  const [engaged, setEngaged] = useState(false);
  const [tapeEpoch, setTapeEpoch] = useState(0);
  const paused = hovered || focusWithin;
  const counting = tapeShouldRun({
    scriptOpen,
    streaming,
    working,
    approvalPending: !!script.approval,
  });
  const { progress } = useLinger({
    running: counting && !engaged,
    paused,
    durationMs: LINGER_MS,
    resetKey: `${key}:${visible.length}:${spoken.length}:${tapeEpoch}`,
    onExpire: closeScript,
  });
  // Once the pointer has left and nothing inside has focus, an engaged
  // script starts a fresh tape.
  useEffect(() => {
    if (engaged && !hovered && !focusWithin) {
      setEngaged(false);
      setTapeEpoch((e) => e + 1);
    }
  }, [engaged, hovered, focusWithin]);
  const engage = useCallback(() => setEngaged(true), []);

  // ── Composer ──
  const [input, setInput] = useState("");
  inputRef.current = input;
  const typeInputRef = useRef<HTMLInputElement>(null);
  const followUpRef = useRef<HTMLInputElement>(null);
  const typeOpen = isInputState(bar.barState);
  const skill = useSkillAutocomplete({
    value: input,
    onAccept: setInput,
    enabled: typeOpen || scriptOpen,
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
    if (!typeOpen) return;
    const target = scriptOpen ? followUpRef.current : typeInputRef.current;
    const t = window.setTimeout(() => target?.focus(), 60);
    return () => window.clearTimeout(t);
  }, [typeOpen, scriptOpen]);

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
      .catch((e) => console.error("Studio: focus listener failed:", e));
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
        } else if (scriptOpen) {
          closeScript();
        } else if (typeOpen) {
          void sendInteraction(UI.INTERACTION_TYPES_ESCAPE);
        }
      } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        void sendInteraction(UI.INTERACTION_TYPES_ENTER);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [working, scriptOpen, typeOpen, closeScript]);

  const idle = isIdleState(bar.barState) && !scriptOpen;
  const onDeckClick = useCallback(() => {
    if (idle) void sendInteraction(UI.INTERACTION_TYPES_CLICK);
  }, [idle]);

  // ── Posture and size ──
  const posture = postureFor({ state: bar.barState, scriptOpen, driving: isDriving });
  const [scriptContentH, setScriptContentH] = useState(STUDIO_SIZES.script.minHeight);
  // The script's content, measured through a callback ref: the body mounts
  // after the posture switches, so a plain ref would be empty when the
  // observer effect first ran.
  const [measureEl, setMeasureEl] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!measureEl || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      const h = entry.contentRect.height;
      setScriptContentH(SCRIPT_HEADER_H + SCRIPT_BODY_PAD * 2 + Math.ceil(h) + (typeOpen ? SCRIPT_FOOTER_H : 0));
    });
    observer.observe(measureEl);
    return () => observer.disconnect();
  }, [measureEl, typeOpen]);
  // A closed script forgets its height, so the next one opens small and grows.
  useEffect(() => {
    if (!scriptOpen) setScriptContentH(STUDIO_SIZES.script.minHeight);
  }, [scriptOpen]);
  const target = studioSize(posture, scriptContentH);
  const suggestionExtra =
    skill.open && posture === "type" ? skill.suggestions.length * 29 + 20 : 0;
  const win = windowSize(target, suggestionExtra);

  // Window protocol, same as Pill and Island: growing, resize then spring;
  // shrinking, spring then resize once it has settled. Both through one
  // backend call so position and size land in the same frame.
  const [shell, setShell] = useState<StudioSize>(() => studioSize("deck"));
  const prevWinRef = useRef(windowSize(studioSize("deck")));
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

  // ── The wave, the words, the tape ──
  const wave = waveFor({ state: bar.barState, driving: isDriving, spokenText: bar.spokenText });
  const samples = useWaveSamples({
    mode: wave.mode,
    level: bar.audioLevel,
    spokenText: bar.spokenText,
    reducedMotion,
  });
  const question = script.question || bar.lastSubmittedValue;
  const line = lineFor({
    state: bar.barState,
    transcriptionText: bar.transcriptionText,
    spokenText: bar.spokenText,
    currentError: bar.currentError,
    question,
    runningTool: script.runningTool,
    drivingLabel: isDriving && driving ? drivingLabel(driving) : null,
  });
  // What the engine has committed so far, for the teleprompter's split.
  const [committed, setCommitted] = useState("");
  useEffect(() => {
    if (!isVoiceState(bar.barState)) {
      setCommitted("");
      return;
    }
    setCommitted((prev) => commitTranscript(prev, bar.transcriptionText, !!bar.transcriptionProvisional));
  }, [bar.barState, bar.transcriptionText, bar.transcriptionProvisional]);
  const prompter = teleprompterText(bar.transcriptionText, !!bar.transcriptionProvisional, committed);
  const counterMode = counterFor({ state: bar.barState, processing: chat.isProcessing, scriptOpen });
  const counterMs = useTapeCounter(counterMode);

  // ── Layers ──
  let layer: ReactNode;
  if (posture === "deck") {
    layer = (
      <div className="flex h-full w-full items-center justify-center" data-testid="studio-deck">
        <WaveStrip samples={samples} look={wave} width={STRIP.deck.width} height={STRIP.deck.height} />
      </div>
    );
  } else if (posture === "type") {
    layer = (
      <form
        onSubmit={submit}
        className="flex h-full w-full items-center gap-3 pl-3.5 pr-3.5"
        data-testid="studio-type"
      >
        <WaveStrip samples={samples} look={wave} width={STRIP.type.width} height={STRIP.type.height} />
        <div className="relative min-w-0 flex-1">
          <input
            ref={typeInputRef}
            type="text"
            value={input}
            onChange={(e) => changeInput(e.target.value)}
            onKeyDown={skill.handleKeyDown}
            aria-label="Ask Juno"
            placeholder="Ask Juno"
            disabled={bar.barState !== UI.BAR_STATES_INPUT}
            className={cn(
              "w-full bg-transparent text-[13px] tracking-[-0.01em] text-black/85 outline-none",
              "placeholder:text-black/30",
            )}
          />
          <SkillGhostText
            value={input}
            ghostText={skill.ghostText}
            className="flex items-center text-[13px] tracking-[-0.01em]"
            ghostClassName="text-black/30"
          />
        </div>
        <span
          className={cn(
            "shrink-0 select-none text-[11px] tracking-[0.04em] text-black/30 transition-opacity duration-150",
            input.trim() ? "opacity-100" : "opacity-0",
          )}
          aria-hidden="true"
        >
          return
        </span>
      </form>
    );
  } else if (posture === "script") {
    // The header carries a stage direction or a failure. Not the spoken
    // sentence: the Juno line already has it, and the blue strip says Juno
    // is speaking.
    const headerLine = line && (line.tone === "direction" || line.tone === "error") ? line : null;
    layer = (
      <div className="flex h-full w-full flex-col" data-testid="studio-script">
        <header
          className="flex shrink-0 items-center gap-3 pl-3.5 pr-2.5"
          style={{ height: SCRIPT_HEADER_H }}
        >
          <WaveStrip samples={samples} look={wave} width={STRIP.script.width} height={STRIP.script.height} />
          {headerLine ? <StudioLine line={headerLine} className="text-[12px]" /> : <span className="min-w-0 flex-1" />}
          <TapeCounter ms={counterMs} mode={counterMode} />
          <CloseMark onClose={closeScript} />
        </header>
        <Tape progress={progress} counting={counting && !engaged} paused={paused} />
        <div
          data-no-drag
          onScroll={engage}
          className="studio-scroll min-h-0 flex-1 cursor-auto select-text overflow-y-auto px-3.5"
          style={{ paddingTop: SCRIPT_BODY_PAD, paddingBottom: SCRIPT_BODY_PAD }}
        >
          <div ref={setMeasureEl} className="text-[13px] leading-[1.5] text-black/85">
            {question && (
              <div className="mb-2.5" data-testid="studio-you">
                <Speaker name="You" />
                <p className="m-0">{question}</p>
              </div>
            )}
            {script.directions.map((d) =>
              d.kind === "approval" ? (
                <ApprovalDirection key={d.message.tool_id ?? d.text} msg={d.message} onDecided={chat.handleApprovalUpdate} />
              ) : (
                <StageDirection key={d.message.tool_id ?? `${d.kind}:${d.text}`} direction={d} />
              ),
            )}
            <InputControlNotices className="px-0" />
            {spoken || visible ? (
              <div data-testid="studio-juno">
                <Speaker name="Juno" />
                {spoken ? (
                  <>
                    <p className="m-0" data-testid="studio-spoken">
                      {spoken}
                    </p>
                    {visible && (
                      <div
                        className="mt-1.5 border-l border-black/[0.12] pl-2.5 text-[12.5px] text-black/70"
                        data-testid="studio-notes"
                      >
                        <MixedContentRenderer content={visible} isStreaming={streaming} />
                      </div>
                    )}
                  </>
                ) : (
                  <div data-testid="studio-notes">
                    <MixedContentRenderer content={visible} isStreaming={streaming} />
                  </div>
                )}
              </div>
            ) : working && !script.approval ? (
              <p className="m-0 text-[12px] italic text-black/45">
                {script.runningTool ? `Juno runs ${script.runningTool}` : "Juno is working"}
              </p>
            ) : null}
          </div>
        </div>
        {typeOpen && (
          <footer className="flex shrink-0 items-center gap-2 px-3" style={{ height: SCRIPT_FOOTER_H }}>
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
                    "h-7 w-full rounded-full bg-black/[0.05] px-3 text-[12px] tracking-[-0.01em] text-black/85 outline-none",
                    "placeholder:text-black/30 focus:bg-black/[0.07]",
                  )}
                />
                <SkillGhostText
                  value={input}
                  ghostText={skill.ghostText}
                  className="flex h-7 items-center px-3 text-[12px] tracking-[-0.01em]"
                  ghostClassName="text-black/30"
                />
              </div>
            </form>
          </footer>
        )}
      </div>
    );
  } else {
    // take and work: the strip on top, the words beneath.
    const showPrompter = posture === "take" && !line && (prompter.final || prompter.provisional);
    layer = (
      <div className="flex h-full w-full flex-col justify-center gap-2 px-3.5" data-testid={`studio-${posture}`}>
        <WaveStrip samples={samples} look={wave} width={STRIP.take.width} height={STRIP.take.height} />
        <div className="flex min-w-0 items-end gap-3">
          {showPrompter ? (
            <Teleprompter final={prompter.final} provisional={prompter.provisional} />
          ) : line ? (
            <StudioLine line={line} />
          ) : (
            <span className="min-w-0 flex-1" />
          )}
          <TapeCounter ms={counterMs} mode={counterMode} className="mb-px" />
        </div>
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
        if (scriptOpen && (e.target as HTMLElement).closest("button, input, a")) engage();
      }}
    >
      {/* Centred, not left-anchored: the window grows first (centre-stable),
          so a studio pinned to the left edge would jump left by half the
          added width before it springs open. Centred, it grows around itself. */}
      <div className="absolute left-1/2 -translate-x-1/2" style={{ top: SHADOW_PAD }}>
        <StudioShell
          size={shell}
          layerKey={posture}
          reducedMotion={reducedMotion}
          onClick={idle ? onDeckClick : undefined}
        >
          {layer}
        </StudioShell>
      </div>
      {posture === "type" && skill.open && (
        <div
          className="absolute left-1/2 z-20 -translate-x-1/2"
          style={{ top: SHADOW_PAD + shell.height + 8, width: STUDIO_SIZES.type.width - 24 }}
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
