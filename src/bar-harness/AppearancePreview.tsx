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
 * `demo=spoken` to play a spoken turn (you speak, a tool runs, the answer is
 * spoken sentence by sentence while it streams, Juno speaks, then rest),
 * `demo=ring` (two tools then a short answer), `demo=allow` (a tool waiting on
 * Allow) and `demo=break` (two tools then a failure) for the Halo,
 * `start=manual` to hold the script until `window.__junoBenchStart()` is
 * called (so a recording begins on the first beat, not on a blank page),
 * `bg=<css color>` to paint a background instead of transparent,
 * `theme=light|dark` to hold a look that follows the system appearance on one
 * of them, `demo=steps` to play a turn with tool steps and an approval (the
 * Bar's timeline), `demo=fail` to play a turn whose step fails, `pin=frame`
 * to place the window by the x/y the bar asked for (so a look whose window
 * grows around an anchor is seen holding still, as on hardware, instead of
 * being re-centred on every resize), and `dock=low` to park the fake window
 * in the bottom half of the display before the bar mounts (looks that grow
 * upward when docked low can then be seen doing so).
 */

import { Component, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { emit } from "@tauri-apps/api/event";
import { EVENTS, UI } from "@/lib/constants.generated";
import { BarHost } from "@/components/bar/BarHost";
import { appearanceEntry } from "@/components/bar/appearanceCatalog";
import { harness, installBarHarnessTauri, setPreviewAppearance, setPreviewTheme } from "./barHarnessTauri";

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
  theme: "light" | "dark" | null;
  pinFrame: boolean;
  dockLow: boolean;
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
    theme: params.get("theme") === "dark" ? "dark" : params.get("theme") === "light" ? "light" : null,
    pinFrame: params.get("pin") === "frame",
    dockLow: params.get("dock") === "low",
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
    void emit(EVENTS.TOOLS_APPROVAL_REQUEST, {
      tool_name: "mail",
      tool_id: "preview-approval",
      tool_input: {},
      description: "send the draft to Maya",
      timestamp: Date.now(),
    });
  }, 900);
}

/** The question `demo=steps` asks, and what Juno says back. */
const STEPS_QUESTION = "Do I need a coat in Charlotte this morning?";
const STEPS_ANSWER =
  "Yes, bring a coat. It is 41 degrees in Charlotte right now and the wind is from the north.\n\n" +
  '<WeatherCard location="Charlotte, NC" temperature={41} unit="F" condition="cloudy" high={58} low={39} wind="12 mph N" />\n\n' +
  "It warms up to 58 by mid afternoon.";
const STEPS_SPOKEN = "Yes, bring a coat. It is 41 degrees in Charlotte right now.";

/**
 * Play one turn with steps: the question, a tool that runs and finishes, a
 * tool that waits on Allow or Don't (answered on its own after a moment when
 * nobody clicks), then the answer. With `fail`, the second tool fails and Rust
 * reports an error instead.
 */
function playStepsDemo(later: (fn: () => void, ms: number) => void, fail: boolean): void {
  const messageId = "preview-steps";
  const now = Date.now();
  const frame = (barState: string, currentError: string | null = null) =>
    emitFrame({ barState, audioLevel: 0, transcriptionText: "" }, { lastSubmittedValue: STEPS_QUESTION, currentError });
  const tool = (type: "tool_call_request" | "tool_call_result", payload: Record<string, unknown>) =>
    void emit(EVENTS.AGENT_EVENT, { type, payload });

  later(() => {
    void emit(EVENTS.MESSAGES_USER_MESSAGE_SUBMITTED, { content: STEPS_QUESTION, timestamp: now });
    frame(UI.BAR_STATES_SUBMITTING);
  }, 0);
  later(() => frame(UI.BAR_STATES_LOADING), 500);
  later(() => tool("tool_call_request", { tool_name: "browser", content: "Opening weather.com" }), 1100);
  later(() => tool("tool_call_result", { tool_name: "browser", success: true, content: "Opened" }), 2600);
  later(() => {
    void emit(EVENTS.TOOLS_APPROVAL_REQUEST, {
      tool_name: "computer",
      tool_id: "preview-approval",
      tool_input: {},
      description: "read the forecast on this page",
      timestamp: Date.now(),
    });
  }, 3000);
  const resolved = 6200;
  later(() => {
    tool("tool_call_result", {
      tool_name: "computer",
      success: !fail,
      content: fail ? "The page timed out" : "Read the forecast",
    });
    if (fail) frame(UI.BAR_STATES_ERROR, "The page timed out");
  }, resolved);
  if (fail) {
    later(() => frame(UI.BAR_STATES_DEFAULT), resolved + 4000);
    return;
  }
  later(() => {
    frame(UI.BAR_STATES_AGENT_RESPONDING);
    void emit(EVENTS.STREAMING_STREAM_START, { message_id: messageId });
  }, resolved + 400);
  const words = STEPS_ANSWER.split(" ");
  words.forEach((word, i) => {
    later(() => {
      void emit(EVENTS.STREAMING_TEXT_STREAM, {
        message_id: messageId,
        chunk: (i === 0 ? "" : " ") + word,
        tts_content: i === 0 ? STEPS_SPOKEN : undefined,
      });
    }, resolved + 500 + i * 45);
  });
  const end = resolved + 500 + words.length * 45 + 200;
  later(() => {
    void emit(EVENTS.STREAMING_STREAM_END, { message_id: messageId, complete_text: STEPS_ANSWER });
    void emit(EVENTS.AGENT_ACTIVE, false);
    frame(UI.BAR_STATES_FINISHING);
  }, end);
  later(() => frame(UI.BAR_STATES_DEFAULT), end + 300);
}

/** The answer `demo=spoken` speaks, one sentence per streamed part. */
const SPOKEN_PARTS = [
  "Done. The draft is with Maya.",
  "I asked her for Friday and flagged it as a priority.",
];

/**
 * Play one spoken turn: your words arrive while the orb hears you, a tool
 * runs, the answer streams with its spoken sentences, Juno says them with a
 * moving audio level, then everything rests. Same events Rust would send.
 */
function playSpokenDemo(later: (fn: () => void, ms: number) => void): void {
  const messageId = "preview-spoken";
  const frame = (barState: string, audioLevel = 0, transcriptionText = "", spokenText = "") =>
    void emit(EVENTS.BAR_STATE_UPDATE, {
      barState,
      inputValue: "",
      lastSubmittedValue: DEMO_SENTENCE,
      currentError: null,
      transcriptionText,
      spokenText,
      voiceMode: UI.VOICE_MODES_IDLE,
      audioLevel,
      isAgentWorking: false,
      isDictationMode: false,
      isAlwaysListening: false,
      agentState: null,
    });
  // Hearing you: a breathing level and words arriving.
  const words = DEMO_SENTENCE.split(" ");
  const hearing = 2000;
  for (let t = 0; t < hearing; t += 80) {
    later(() => {
      const level = 0.4 + 0.3 * Math.sin(t / 160) + 0.15 * Math.sin(t / 55);
      const spoken = Math.min(words.length, Math.floor((t / hearing) * (words.length + 1)));
      frame(UI.BAR_STATES_LISTENING, level, words.slice(0, spoken).join(" "));
    }, t);
  }
  later(() => frame(UI.BAR_STATES_TRANSCRIBING, 0, DEMO_SENTENCE), hearing);
  const ask = hearing + 500;
  later(() => {
    void emit(EVENTS.MESSAGES_USER_MESSAGE_SUBMITTED, { content: DEMO_SENTENCE, timestamp: Date.now() });
    frame(UI.BAR_STATES_SUBMITTING);
  }, ask);
  later(() => frame(UI.BAR_STATES_LOADING), ask + 400);
  // One tool step, so the orb has a reason to quicken.
  later(() => {
    void emit(EVENTS.AGENT_EVENT, {
      type: "tool_call_request",
      payload: { tool_name: "mail", content: "Sending the draft to Maya" },
    });
  }, ask + 700);
  later(() => {
    void emit(EVENTS.AGENT_EVENT, {
      type: "tool_call_result",
      payload: { tool_name: "mail", success: true, content: "Sent." },
    });
  }, ask + 2000);
  // The answer streams, one spoken sentence per part.
  const answerAt = ask + 2300;
  later(() => {
    frame(UI.BAR_STATES_AGENT_RESPONDING);
    void emit(EVENTS.STREAMING_STREAM_START, { message_id: messageId });
  }, answerAt);
  let cursor = answerAt + 100;
  SPOKEN_PARTS.forEach((part, index) => {
    const partWords = part.split(" ");
    partWords.forEach((word, i) => {
      later(() => {
        void emit(EVENTS.STREAMING_TEXT_STREAM, {
          message_id: messageId,
          chunk: (index === 0 && i === 0 ? "" : " ") + word,
          tts_content: i === 0 ? part : undefined,
        });
      }, cursor + i * 60);
    });
    cursor += partWords.length * 60 + 300;
  });
  const fullAnswer = SPOKEN_PARTS.join(" ");
  later(() => {
    void emit(EVENTS.STREAMING_STREAM_END, { message_id: messageId, complete_text: fullAnswer });
  }, cursor);
  // Juno says it: the level moves with the speech.
  const speakAt = cursor + 100;
  const speakFor = 2600;
  for (let t = 0; t < speakFor; t += 80) {
    later(() => {
      const level = 0.45 + 0.35 * Math.sin(t / 90) * Math.sin(t / 310 + 1);
      const part = t < speakFor / 2 ? SPOKEN_PARTS[0] : SPOKEN_PARTS[1];
      frame(UI.BAR_STATES_SPEAKING, Math.max(0, level), "", part);
    }, speakAt + t);
  }
  later(() => {
    void emit(EVENTS.AGENT_ACTIVE, false);
    frame(UI.BAR_STATES_FINISHING);
  }, speakAt + speakFor);
  later(() => frame(UI.BAR_STATES_DEFAULT), speakAt + speakFor + 300);
}

/**
 * The Halo's demos: a ring measures a turn, so its clips need tools that
 * finish (ticks), a short answer (inside the ring), a tool waiting on Allow,
 * and a failure after some work (the gap where it broke). Same events Rust
 * would send; nothing is run.
 */
const RING_QUESTION = "How long until my next meeting?";

type Later = (fn: () => void, ms: number) => void;

/**
 * A frame that keeps being sent every 400ms until the next one replaces it.
 * A lazy-loaded bar (the Halo) subscribes after a demo's first beats have
 * fired; the hold mode re-sends for the same reason.
 */
function frameKeeper(later: Later, lastSubmittedValue: string) {
  let current: { barState: string; currentError: string | null } | null = null;
  const tick = () => {
    if (current) emitFrame({ ...current, audioLevel: 0, transcriptionText: "" }, { lastSubmittedValue, currentError: current.currentError });
    later(tick, 400);
  };
  later(tick, 400);
  return (barState: string, currentError: string | null = null) => {
    current = { barState, currentError };
    emitFrame({ barState, audioLevel: 0, transcriptionText: "" }, { lastSubmittedValue, currentError });
  };
}

function toolCall(later: Later, at: number, content: string, took: number): void {
  later(() => {
    void emit(EVENTS.AGENT_EVENT, { type: "tool_call_request", payload: { tool_name: content, content } });
  }, at);
  later(() => {
    void emit(EVENTS.AGENT_EVENT, {
      type: "tool_call_result",
      payload: { tool_name: content, content: "Done", success: true },
    });
  }, at + took);
}

/** One turn with two tools, then a short answer the ring can hold whole. */
function playRingDemo(later: Later): void {
  const messageId = "preview-ring";
  const frame = frameKeeper(later, RING_QUESTION);
  later(() => {
    void emit(EVENTS.MESSAGES_USER_MESSAGE_SUBMITTED, { content: RING_QUESTION, timestamp: Date.now() });
    frame(UI.BAR_STATES_SUBMITTING);
  }, 0);
  later(() => frame(UI.BAR_STATES_LOADING), 500);
  toolCall(later, 900, "Checking your calendar", 1100);
  toolCall(later, 2300, "Reading the invite", 900);
  later(() => {
    frame(UI.BAR_STATES_AGENT_RESPONDING);
    void emit(EVENTS.STREAMING_STREAM_START, { message_id: messageId });
    void emit(EVENTS.STREAMING_TEXT_STREAM, { message_id: messageId, chunk: "42 min", tts_content: "Forty-two minutes." });
  }, 3500);
  later(() => {
    void emit(EVENTS.STREAMING_STREAM_END, { message_id: messageId, complete_text: "42 min" });
    void emit(EVENTS.AGENT_ACTIVE, false);
    frame(UI.BAR_STATES_FINISHING);
  }, 3900);
  later(() => frame(UI.BAR_STATES_DEFAULT), 4200);
}

/** A tool waiting on Allow or Don't, after one tick. Holds there. */
function playAllowDemo(later: Later): void {
  const question = "Open the invite in Safari";
  const frame = frameKeeper(later, question);
  later(() => {
    void emit(EVENTS.MESSAGES_USER_MESSAGE_SUBMITTED, { content: question, timestamp: Date.now() });
    frame(UI.BAR_STATES_SUBMITTING);
  }, 0);
  later(() => frame(UI.BAR_STATES_LOADING), 400);
  toolCall(later, 600, "Reading the invite", 700);
  later(() => {
    void emit("tool-approval-request", {
      tool_name: "browser",
      tool_id: "preview-allow",
      tool_input: {},
      description: "open the invite in Safari",
      timestamp: Date.now(),
    });
  }, 1500);
}

/** Two tools finish, the third fails: the ring breaks where it was. */
function playBreakDemo(later: Later): void {
  const frame = frameKeeper(later, RING_QUESTION);
  later(() => {
    void emit(EVENTS.MESSAGES_USER_MESSAGE_SUBMITTED, { content: RING_QUESTION, timestamp: Date.now() });
    frame(UI.BAR_STATES_SUBMITTING);
  }, 0);
  later(() => frame(UI.BAR_STATES_LOADING), 400);
  toolCall(later, 700, "Checking your calendar", 900);
  toolCall(later, 1800, "Reading the invite", 800);
  later(() => {
    void emit(EVENTS.AGENT_ACTIVE, false);
    frame(UI.BAR_STATES_ERROR, "Calendar did not answer");
  }, 2900);
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
  const { appearance, reducedMotion, holdState, demo, manualStart, background, theme, pinFrame, dockLow } =
    useMemo(readQuery, []);

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
    setPreviewTheme(theme);
    installBarHarnessTauri();
    if (dockLow) {
      const { monitor, frame } = harness.snapshot();
      harness.setFrame({ y: Math.round(monitor.height * 0.8) - frame.height });
    }
    document.documentElement.style.background = background ?? "transparent";
    document.body.style.background = background ?? "transparent";
    setReady(true);
  }, [appearance, theme, dockLow]);

  // Drive the script. Listening breathes the audio level; dictating reveals
  // the sentence a word at a time. Everything else is a single emit.
  useEffect(() => {
    if (!ready || !started) return;

    let cancelled = false;
    const timers: number[] = [];
    const later = (fn: () => void, ms: number) => {
      if (cancelled) return;
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

    if (demo === "spoken") {
      playSpokenDemo(later);
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
    if (demo === "steps" || demo === "fail") {
      playStepsDemo(later, demo === "fail");
      return stop;
    }
    if (demo === "ring") {
      playRingDemo(later);
      return stop;
    }
    if (demo === "allow") {
      playAllowDemo(later);
      return stop;
    }
    if (demo === "break") {
      playBreakDemo(later);
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
  // Pinned to the frame, the window is never shrunk to fit: a scaled box
  // would move its anchor, which is the very thing `pin=frame` shows.
  const scale =
    !pinFrame && box.width > 0 && box.height > 0
      ? Math.min(1, viewport.w / box.width, viewport.h / box.height)
      : 1;
  let left = (viewport.w - box.width * scale) / 2;
  let top = (viewport.h - box.height * scale) / 2;
  // `pin=frame`: the first driven frame is seated near the top of the stage
  // (near the bottom with `dock=low`) so a window that grows has room to; after
  // that the box follows the x/y the bar asked for, so the window moves on the
  // stage exactly as the OS would move it.
  const originRef = useRef<{ dx: number; dy: number } | null>(null);
  if (pinFrame && driven) {
    if (!originRef.current) {
      const seatTop = dockLow ? viewport.h * 0.82 - box.height : viewport.h * 0.18;
      originRef.current = { dx: left - frame.x, dy: seatTop - frame.y };
    }
    left = frame.x + originRef.current.dx;
    top = frame.y + originRef.current.dy;
  }

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
