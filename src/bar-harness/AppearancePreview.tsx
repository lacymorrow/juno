/**
 * Live preview of one bar appearance (route `/__bar-preview`).
 *
 * The settings window frames this route to show a person what each look does
 * before they pick it. It runs the REAL bar components through `BarHost` on the
 * same fake Tauri layer the state harness uses, so nothing here can move,
 * resize or focus a real window: every call the bar makes lands in the shim.
 *
 * A short script walks the bar through the moments that matter (resting,
 * listening, dictating, done) by emitting the same `bar-state-update` events
 * Rust would, then loops. With `?motion=reduced` it holds a single frame.
 *
 * Query: `appearance=<bar_appearance value>`, `motion=reduced` (optional),
 * `state=<bar state>` to hold one frame (the bench and the docs screenshots),
 * `demo=card` to play one full turn (question, answer with a component) once,
 * `demo=script` to play a turn with a tool and a spoken answer (the studio's
 * clip), `demo=approval` to hold a tool waiting on Allow or Don't,
 * `start=manual` to hold the script until `window.__junoBenchStart()` is
 * called (so a recording begins on the first beat, not on a blank page), and
 * `bg=<css color>` to paint a background instead of transparent.
 */

import { Component, useEffect, useLayoutEffect, useMemo, useState, type ReactNode } from "react";
import { emit } from "@tauri-apps/api/event";
import { EVENTS, UI } from "@/lib/constants.generated";
import { BarHost } from "@/components/bar/BarHost";
import { appearanceEntry } from "@/components/bar/appearanceCatalog";
import { harness, installBarHarnessTauri, setPreviewAppearance } from "./barHarnessTauri";

// The sentence the preview "dictates". Short enough to fit every look, long
// enough to show words arriving.
const DEMO_SENTENCE = "Send the draft to Maya and ask for Friday.";

interface Beat {
  state: string;
  /** How long this beat holds, in ms. */
  hold: number;
}

// One loop of the preview. Durations are tuned so the whole cycle reads in
// about eight seconds, which is roughly how long a person lingers on a picker.
const SCRIPT: Beat[] = [
  { state: UI.BAR_STATES_DEFAULT, hold: 1400 },
  { state: UI.BAR_STATES_LISTENING, hold: 1600 },
  { state: UI.BAR_STATES_DICTATING, hold: 2600 },
  { state: UI.BAR_STATES_TRANSCRIBING, hold: 700 },
  { state: UI.BAR_STATES_SUCCESS, hold: 1100 },
];

interface Frame {
  barState: string;
  audioLevel: number;
  transcriptionText: string;
}

interface FrameExtra {
  lastSubmittedValue?: string;
  currentError?: string | null;
  /** The sentence Rust is speaking aloud, for looks that draw Juno's voice. */
  spokenText?: string;
}

function emitFrame(
  { barState, audioLevel, transcriptionText }: Frame,
  extra: FrameExtra = {},
): void {
  void emit(EVENTS.BAR_STATE_UPDATE, {
    barState,
    inputValue: "",
    lastSubmittedValue: extra.lastSubmittedValue ?? "",
    currentError: extra.currentError ?? null,
    transcriptionText,
    spokenText: extra.spokenText ?? "",
    voiceMode: UI.VOICE_MODES_IDLE,
    audioLevel,
    isAgentWorking: false,
    isDictationMode: barState === UI.BAR_STATES_DICTATING,
    isAlwaysListening: false,
    agentState: null,
  });
}

/** What the preview tells the settings window that frames it. */
export type PreviewMessage = { type: "juno-bar-preview"; status: "ready" | "failed"; appearance: string };

function post(status: PreviewMessage["status"], appearance: string): void {
  if (window.parent === window) return;
  const message: PreviewMessage = { type: "juno-bar-preview", status, appearance };
  window.parent.postMessage(message, window.location.origin);
}

/**
 * A bar that throws must not leave a silent blank stage. The boundary reports
 * the failure to the settings window so it can say so, and logs the cause.
 */
class PreviewBoundary extends Component<
  { appearance: string; children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch(error: unknown) {
    console.error(`[bar-preview] ${this.props.appearance} failed to render:`, error);
    post("failed", this.props.appearance);
  }
  render() {
    return this.state.failed ? null : this.props.children;
  }
}

function readQuery(): {
  appearance: string;
  reducedMotion: boolean;
  holdState: string | null;
  demo: string | null;
  manualStart: boolean;
  background: string | null;
} {
  const params = new URLSearchParams(window.location.search);
  const appearance = appearanceEntry(params.get("appearance")).value;
  const reducedMotion =
    params.get("motion") === "reduced" ||
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  return {
    appearance,
    reducedMotion,
    holdState: params.get("state"),
    demo: params.get("demo"),
    manualStart: params.get("start") === "manual",
    background: params.get("bg"),
  };
}

declare global {
  interface Window {
    /** Bench hook: starts a `start=manual` preview's script. */
    __junoBenchStart?: () => void;
  }
}

/** One frame for a held state, with words where the state would show them. */
function heldFrame(state: string): Frame & FrameExtra {
  switch (state) {
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_DICTATING:
    case UI.BAR_STATES_TRANSCRIBING:
      return { barState: state, audioLevel: 0.5, transcriptionText: DEMO_SENTENCE };
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_AGENT_RESPONDING:
    case UI.BAR_STATES_FINISHING:
      return { barState: state, audioLevel: 0, transcriptionText: "", lastSubmittedValue: DEMO_SENTENCE };
    case UI.BAR_STATES_ERROR:
      return { barState: state, audioLevel: 0, transcriptionText: "", currentError: "Connection unavailable" };
    case UI.BAR_STATES_SPEAKING:
      return { barState: state, audioLevel: 0, transcriptionText: "", spokenText: DEMO_SPOKEN };
    default:
      return { barState: state, audioLevel: 0, transcriptionText: "" };
  }
}

/** The canned answer `demo=card` plays: a sentence, a component, a sentence. */
const DEMO_ANSWER =
  "Done. The draft is with Maya and I asked for Friday.\n\n" +
  '<TaskSummaryCard title="Sent" tasks={[{ "label": "Draft to Maya", "done": true }, { "label": "Ask for Friday", "done": true }]} />\n\n' +
  "I will let you know when she replies.";
const DEMO_SPOKEN = "Done. The draft is with Maya and I asked for Friday.";

/** Play one turn through the same events Rust would send, then rest. */
function playCardDemo(later: (fn: () => void, ms: number) => void): void {
  const messageId = "preview-demo";
  const now = Date.now();
  const frame = (barState: string) =>
    emitFrame({ barState, audioLevel: 0, transcriptionText: "" }, { lastSubmittedValue: DEMO_SENTENCE });
  later(() => {
    void emit(EVENTS.MESSAGES_USER_MESSAGE_SUBMITTED, { content: DEMO_SENTENCE, timestamp: now });
    frame(UI.BAR_STATES_SUBMITTING);
  }, 0);
  later(() => frame(UI.BAR_STATES_LOADING), 500);
  later(() => {
    frame(UI.BAR_STATES_AGENT_RESPONDING);
    void emit(EVENTS.STREAMING_STREAM_START, { message_id: messageId });
  }, 1100);
  const words = DEMO_ANSWER.split(" ");
  words.forEach((word, i) => {
    later(() => {
      void emit(EVENTS.STREAMING_TEXT_STREAM, {
        message_id: messageId,
        chunk: (i === 0 ? "" : " ") + word,
        tts_content: i === 0 ? DEMO_SPOKEN : undefined,
      });
    }, 1200 + i * 45);
  });
  const end = 1200 + words.length * 45 + 200;
  later(() => {
    void emit(EVENTS.STREAMING_STREAM_END, { message_id: messageId, complete_text: DEMO_ANSWER });
    void emit(EVENTS.AGENT_ACTIVE, false);
    frame(UI.BAR_STATES_FINISHING);
  }, end);
  later(() => frame(UI.BAR_STATES_DEFAULT), end + 300);
}

/** What `demo=script` and `demo=approval` ask. */
const DEMO_SCRIPT_QUESTION = "Find the draft to Maya and send it.";
const DEMO_SCRIPT_ANSWER =
  "Sent. Maya has the draft and I asked her for Friday.\n\n" +
  '<TaskSummaryCard title="Sent" tasks={[{ "label": "Draft to Maya", "done": true }, { "label": "Ask for Friday", "done": true }]} />';
const DEMO_SCRIPT_SPOKEN = "Sent. Maya has the draft.";

/** The tool the script demos run, through the same events Rust would send. */
function emitTool(content: string, done: boolean): void {
  void emit(EVENTS.AGENT_EVENT, {
    type: done ? "tool_call_result" : "tool_call_request",
    payload: done ? { tool_name: "mail", success: true, content: "Found it" } : { tool_name: "mail", content },
  });
}

/**
 * One turn with a tool between the lines and a spoken answer: submit, a tool
 * runs and finishes, the answer streams with a component, Juno speaks the
 * first sentence, then rest.
 */
function playScriptDemo(later: (fn: () => void, ms: number) => void): void {
  const messageId = "preview-script";
  const now = Date.now();
  const frame = (barState: string, extra: FrameExtra = {}) =>
    emitFrame(
      { barState, audioLevel: 0, transcriptionText: "" },
      { lastSubmittedValue: DEMO_SCRIPT_QUESTION, ...extra },
    );
  later(() => {
    void emit(EVENTS.MESSAGES_USER_MESSAGE_SUBMITTED, { content: DEMO_SCRIPT_QUESTION, timestamp: now });
    frame(UI.BAR_STATES_SUBMITTING);
  }, 0);
  later(() => frame(UI.BAR_STATES_LOADING), 500);
  later(() => emitTool("find the draft to Maya", false), 900);
  later(() => emitTool("find the draft to Maya", true), 2300);
  later(() => {
    frame(UI.BAR_STATES_AGENT_RESPONDING);
    void emit(EVENTS.STREAMING_STREAM_START, { message_id: messageId });
  }, 2600);
  const words = DEMO_SCRIPT_ANSWER.split(" ");
  words.forEach((word, i) => {
    later(() => {
      void emit(EVENTS.STREAMING_TEXT_STREAM, {
        message_id: messageId,
        chunk: (i === 0 ? "" : " ") + word,
        tts_content: i === 0 ? DEMO_SCRIPT_SPOKEN : undefined,
      });
    }, 2700 + i * 45);
  });
  const end = 2700 + words.length * 45 + 200;
  later(() => {
    void emit(EVENTS.STREAMING_STREAM_END, { message_id: messageId, complete_text: DEMO_SCRIPT_ANSWER });
    frame(UI.BAR_STATES_SPEAKING, { spokenText: DEMO_SCRIPT_SPOKEN });
  }, end);
  later(() => {
    void emit(EVENTS.AGENT_ACTIVE, false);
    frame(UI.BAR_STATES_FINISHING);
  }, end + 2600);
  later(() => frame(UI.BAR_STATES_DEFAULT), end + 2900);
}

/** A tool waiting on Allow or Don't, held so a still can show it. */
function playApprovalDemo(later: (fn: () => void, ms: number) => void): void {
  const now = Date.now();
  later(() => {
    void emit(EVENTS.MESSAGES_USER_MESSAGE_SUBMITTED, { content: DEMO_SCRIPT_QUESTION, timestamp: now });
    emitFrame(
      { barState: UI.BAR_STATES_LOADING, audioLevel: 0, transcriptionText: "" },
      { lastSubmittedValue: DEMO_SCRIPT_QUESTION },
    );
  }, 0);
  later(() => emitTool("find the draft to Maya", false), 300);
  later(() => emitTool("find the draft to Maya", true), 600);
  later(() => {
    // Sent again: a bar subscribes some time after its first paint, and the
    // still must show the tape counting behind the request.
    emitFrame(
      { barState: UI.BAR_STATES_LOADING, audioLevel: 0, transcriptionText: "" },
      { lastSubmittedValue: DEMO_SCRIPT_QUESTION },
    );
    void emit("tool-approval-request", {
      tool_name: "mail",
      tool_id: "preview-approval",
      tool_input: {},
      description: "send the draft to Maya",
      timestamp: Date.now(),
    });
  }, 900);
}

function useHarnessWindow() {
  const [snap, setSnap] = useState(() => {
    const { frame, driven } = harness.snapshot();
    return { frame, driven };
  });
  useEffect(
    () =>
      harness.subscribe(() => {
        const { frame, driven } = harness.snapshot();
        setSnap({ frame, driven });
      }),
    [],
  );
  return snap;
}

/**
 * Natural size of the mounted bar, for looks that never size a window. Takes
 * a callback ref: the wrapper mounts after the shim is ready, so a plain ref
 * would still be empty when the observer effect first ran.
 */
function useNaturalSize() {
  const [el, setEl] = useState<HTMLDivElement | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    if (!el || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setSize({ width, height });
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [el]);
  return { ref: setEl, size };
}

export default function AppearancePreview() {
  const { appearance, reducedMotion, holdState, demo, manualStart, background } = useMemo(
    readQuery,
    [],
  );

  // With `start=manual` the script waits for the bench to say go.
  const [started, setStarted] = useState(!manualStart);
  useEffect(() => {
    if (!manualStart) return;
    window.__junoBenchStart = () => setStarted(true);
    return () => {
      delete window.__junoBenchStart;
    };
  }, [manualStart]);

  // The shim must exist before any bar effect calls `listen` or `invoke`, and
  // the host must read the requested appearance on its first config fetch.
  const [ready, setReady] = useState(false);
  useLayoutEffect(() => {
    setPreviewAppearance(appearance);
    installBarHarnessTauri();
    document.documentElement.style.background = background ?? "transparent";
    document.body.style.background = background ?? "transparent";
    setReady(true);
  }, [appearance]);

  // Drive the script. Listening breathes the audio level; dictating reveals
  // the sentence a word at a time. Everything else is a single emit.
  useEffect(() => {
    if (!ready || !started) return;

    let cancelled = false;
    const timers: number[] = [];
    const later = (fn: () => void, ms: number) => {
      timers.push(window.setTimeout(fn, ms));
    };
    const stop = () => {
      cancelled = true;
      timers.forEach((id) => window.clearTimeout(id));
    };

    if (holdState) {
      const { lastSubmittedValue, currentError, spokenText, ...frame } = heldFrame(holdState);
      // Re-sent every 400ms: a bar subscribes some time after its first paint
      // (the host fetches its config first), and a held frame must still land.
      const hold = () => {
        if (cancelled) return;
        emitFrame(frame, { lastSubmittedValue, currentError, spokenText });
        later(hold, 400);
      };
      hold();
      return stop;
    }

    if (demo === "card") {
      playCardDemo(later);
      return stop;
    }
    if (demo === "script") {
      playScriptDemo(later);
      return stop;
    }
    if (demo === "approval") {
      playApprovalDemo(later);
      return stop;
    }

    if (reducedMotion) {
      emitFrame({
        barState: UI.BAR_STATES_DICTATING,
        audioLevel: 0.5,
        transcriptionText: DEMO_SENTENCE,
      });
      return;
    }

    const playBeat = (index: number) => {
      if (cancelled) return;
      const beat = SCRIPT[index % SCRIPT.length];
      const next = () => playBeat(index + 1);

      if (beat.state === UI.BAR_STATES_LISTENING) {
        // A gentle breath, 10 Hz, so meters and orbs have something to follow.
        const start = performance.now();
        const tick = () => {
          if (cancelled) return;
          const t = (performance.now() - start) / 1000;
          const level = 0.35 + 0.3 * Math.sin(t * 5) + 0.1 * Math.sin(t * 13);
          emitFrame({ barState: beat.state, audioLevel: level, transcriptionText: "" });
          if (performance.now() - start < beat.hold) later(tick, 100);
          else next();
        };
        tick();
        return;
      }

      if (beat.state === UI.BAR_STATES_DICTATING) {
        const words = DEMO_SENTENCE.split(" ");
        const step = beat.hold / (words.length + 2);
        words.forEach((_, i) => {
          later(() => {
            emitFrame({
              barState: beat.state,
              audioLevel: 0.5,
              transcriptionText: words.slice(0, i + 1).join(" "),
            });
          }, step * i);
        });
        later(next, beat.hold);
        return;
      }

      emitFrame({
        barState: beat.state,
        audioLevel: 0,
        transcriptionText: beat.state === UI.BAR_STATES_TRANSCRIBING ? DEMO_SENTENCE : "",
      });
      later(next, beat.hold);
    };

    playBeat(0);
    return stop;
  }, [ready, started, reducedMotion, holdState, demo]);

  const { frame, driven } = useHarnessWindow();
  const { ref: naturalRef, size: natural } = useNaturalSize();

  // Tell the framing window once the bar has painted at a real size. Two
  // frames later, not at once: a WebGL canvas has a size before it has drawn
  // anything, and the picker fades the frame in on this message.
  const painted = driven ? frame.width > 0 : natural.width > 0;
  useEffect(() => {
    if (!ready || !painted) return;
    let cancelled = false;
    const outer = requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (!cancelled) post("ready", appearance);
      });
    });
    return () => {
      cancelled = true;
      cancelAnimationFrame(outer);
    };
  }, [ready, painted, appearance]);

  // A bar that sizes its own window (the pill, the island, the studio) gets a
  // window of exactly that size, like the harness gives it. A bar that never
  // does (the app bar, the orbs, the avatar) is laid out at its natural size.
  // Either way the result is centred and shrunk only if it would not fit, so
  // every look is shown at the size a person would actually see it.
  const [viewport, setViewport] = useState({ w: window.innerWidth, h: window.innerHeight });
  useEffect(() => {
    const onResize = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  const box = driven ? { width: frame.width, height: frame.height } : natural;
  const scale =
    box.width > 0 && box.height > 0
      ? Math.min(1, viewport.w / box.width, viewport.h / box.height)
      : 1;
  const left = (viewport.w - box.width * scale) / 2;
  const top = (viewport.h - box.height * scale) / 2;

  if (!ready) return null;

  return (
    <div
      className="relative h-screen w-screen overflow-hidden"
      data-appearance={appearance}
      data-preview-ready="true"
      data-preview-started={started ? "true" : "false"}
      aria-hidden="true"
    >
      <PreviewBoundary appearance={appearance}>
        {driven ? (
          <>
            <style>{`.jbp-window > div{width:100%!important;height:100%!important;min-width:0!important;min-height:0!important;}`}</style>
            <div
              className="jbp-window absolute origin-top-left"
              style={{ left, top, width: frame.width, height: frame.height, transform: `scale(${scale})` }}
            >
              <BarHost />
            </div>
          </>
        ) : (
          <div
            ref={naturalRef}
            className="absolute origin-top-left inline-block"
            style={{ left, top, transform: `scale(${scale})` }}
          >
            <BarHost />
          </div>
        )}
      </PreviewBoundary>
    </div>
  );
}
