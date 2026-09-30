import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";
import {
  HAIRLINE,
  SHADOW_PAD,
  STUDIO_SIZES,
  SYSTEM_BLUE,
  SYSTEM_GREEN,
  SYSTEM_RED,
  answerKey,
  commitTranscript,
  counterFor,
  formatCounter,
  lineFor,
  posture,
  replyChannels,
  spokenIsInVisible,
  scriptFor,
  scriptHeightFor,
  smoothLevel,
  speechEnvelope,
  studioSize,
  tapeShouldRun,
  teleprompterText,
  waveFor,
  windowSize,
} from "../studioModel";

const EVERY_STATE = Object.entries(UI)
  .filter(([k]) => k.startsWith("BAR_STATES_"))
  .map(([, v]) => v as string);

describe("posture", () => {
  it("rests as a deck, opens a take to speak, a type to write, work while Juno works", () => {
    const at = (state: string) => posture({ state, scriptOpen: false });
    expect(at(UI.BAR_STATES_DEFAULT)).toBe("deck");
    expect(at(UI.BAR_STATES_DICTATION_READY)).toBe("deck");
    expect(at(UI.BAR_STATES_SHRINKING)).toBe("deck");
    expect(at(UI.BAR_STATES_LISTENING)).toBe("take");
    expect(at(UI.BAR_STATES_DICTATING)).toBe("take");
    expect(at(UI.BAR_STATES_TRANSCRIBING)).toBe("take");
    expect(at(UI.BAR_STATES_ALWAYS_LISTENING)).toBe("take");
    expect(at(UI.BAR_STATES_INPUT)).toBe("type");
    expect(at(UI.BAR_STATES_EXPANDING)).toBe("type");
    for (const s of [
      UI.BAR_STATES_SUBMITTING,
      UI.BAR_STATES_LOADING,
      UI.BAR_STATES_AGENT_RESPONDING,
      UI.BAR_STATES_FINISHING,
      UI.BAR_STATES_SUCCESS,
      UI.BAR_STATES_SPEAKING,
      UI.BAR_STATES_ERROR,
      UI.BAR_STATES_STOPPING,
    ]) {
      expect(at(s)).toBe("work");
    }
  });

  it("maps every state Rust can send to a posture", () => {
    for (const state of EVERY_STATE) {
      expect(["deck", "take", "type", "work", "script"]).toContain(posture({ state, scriptOpen: false }));
    }
  });

  it("an open script wins over everything, driving wins over the type", () => {
    expect(posture({ state: UI.BAR_STATES_INPUT, scriptOpen: true })).toBe("script");
    expect(posture({ state: UI.BAR_STATES_LISTENING, scriptOpen: true })).toBe("script");
    expect(posture({ state: UI.BAR_STATES_INPUT, scriptOpen: false, driving: true })).toBe("work");
  });
});

describe("size", () => {
  it("take and work share a size so the studio does not lurch when you stop talking", () => {
    expect(studioSize("take")).toEqual(studioSize("work"));
  });

  it("the window is the studio plus shadow room, and can hold suggestions below", () => {
    expect(windowSize(studioSize("deck"))).toEqual({
      width: STUDIO_SIZES.deck.width + 2 * SHADOW_PAD,
      height: STUDIO_SIZES.deck.height + 2 * SHADOW_PAD,
    });
    expect(windowSize(studioSize("type"), 60).height).toBe(STUDIO_SIZES.type.height + 2 * SHADOW_PAD + 60);
  });

  it("the script follows its content in steps, within its range", () => {
    expect(scriptHeightFor(10)).toBe(STUDIO_SIZES.script.minHeight);
    expect(scriptHeightFor(201)).toBe(208);
    expect(scriptHeightFor(9999)).toBe(STUDIO_SIZES.script.maxHeight);
    expect(studioSize("script", 201).height).toBe(208);
  });
});

describe("wave", () => {
  it("is green and live while you speak, blue while Juno speaks, a hairline at rest", () => {
    for (const s of [UI.BAR_STATES_LISTENING, UI.BAR_STATES_DICTATING, UI.BAR_STATES_TRANSCRIBING]) {
      expect(waveFor({ state: s })).toEqual({ color: SYSTEM_GREEN, opacity: 1, mode: "live" });
    }
    expect(waveFor({ state: UI.BAR_STATES_SPEAKING, spokenText: "Hello" }).mode).toBe("speech");
    expect(waveFor({ state: UI.BAR_STATES_SPEAKING, spokenText: "Hello" }).color).toBe(SYSTEM_BLUE);
    expect(waveFor({ state: UI.BAR_STATES_SPEAKING }).mode).toBe("flat");
    expect(waveFor({ state: UI.BAR_STATES_DEFAULT })).toEqual({ color: HAIRLINE, opacity: 1, mode: "flat" });
  });

  it("an open mic drifts blue; failure is the only red; driving is flat", () => {
    expect(waveFor({ state: UI.BAR_STATES_ALWAYS_LISTENING })).toEqual({
      color: SYSTEM_BLUE,
      opacity: 0.7,
      mode: "drift",
    });
    expect(waveFor({ state: UI.BAR_STATES_ERROR })).toEqual({ color: SYSTEM_RED, opacity: 1, mode: "shake" });
    for (const state of EVERY_STATE) {
      if (state === UI.BAR_STATES_ERROR) continue;
      expect(waveFor({ state }).color).not.toBe(SYSTEM_RED);
    }
    expect(waveFor({ state: UI.BAR_STATES_LISTENING, driving: true }).mode).toBe("flat");
  });

  it("settles green when the take ends and when the work is done", () => {
    expect(waveFor({ state: UI.BAR_STATES_SUBMITTING }).mode).toBe("settle");
    expect(waveFor({ state: UI.BAR_STATES_FINISHING })).toEqual({ color: SYSTEM_GREEN, opacity: 1, mode: "settle" });
    expect(waveFor({ state: UI.BAR_STATES_SUCCESS }).mode).toBe("settle");
  });

  it("has a look for every state", () => {
    for (const state of EVERY_STATE) {
      const w = waveFor({ state });
      expect(w.color).toBeTruthy();
      expect(w.opacity).toBeGreaterThan(0);
    }
  });
});

describe("level", () => {
  it("attacks fast and releases slowly", () => {
    const up = smoothLevel(0, 1, 50);
    const down = smoothLevel(1, 0, 50);
    expect(up).toBeGreaterThan(0.6);
    expect(1 - down).toBeLessThan(0.4);
    expect(smoothLevel(0.5, 0.5, 50)).toBeCloseTo(0.5);
  });

  it("clamps what Rust sends", () => {
    expect(smoothLevel(0, 4, 1000)).toBeLessThanOrEqual(1);
    expect(smoothLevel(0.5, -1, 1000)).toBeGreaterThanOrEqual(0);
  });

  it("the speech envelope is a voice, not a fault: never zero while there is text, always in range", () => {
    expect(speechEnvelope("", 100)).toBe(0);
    expect(speechEnvelope("   ", 100)).toBe(0);
    let min = 1;
    let max = 0;
    for (let t = 0; t < 4000; t += 25) {
      const v = speechEnvelope("Done. The draft is with Maya.", t);
      min = Math.min(min, v);
      max = Math.max(max, v);
    }
    expect(min).toBeGreaterThanOrEqual(0.12);
    expect(max).toBeLessThanOrEqual(1);
    expect(max - min).toBeGreaterThan(0.3);
  });

  it("two sentences do not move the same way, and one sentence always moves the same way", () => {
    const a = speechEnvelope("Sunny and 72.", 300);
    const b = speechEnvelope("Raining and 51.", 300);
    expect(a).not.toBeCloseTo(b, 3);
    expect(speechEnvelope("Sunny and 72.", 300)).toBe(a);
  });
});

describe("line", () => {
  const base = {
    transcriptionText: "",
    spokenText: "",
    currentError: null,
    question: "Send it",
  };

  it("names what is happening until words arrive, then steps aside for the teleprompter", () => {
    expect(lineFor({ ...base, state: UI.BAR_STATES_LISTENING })).toEqual({ text: "Listening", tone: "dim" });
    expect(lineFor({ ...base, state: UI.BAR_STATES_DICTATING })).toEqual({ text: "Dictating", tone: "dim" });
    expect(lineFor({ ...base, state: UI.BAR_STATES_TRANSCRIBING })).toEqual({ text: "Transcribing", tone: "dim" });
    expect(lineFor({ ...base, state: UI.BAR_STATES_LISTENING, transcriptionText: "Send" })).toBeNull();
    expect(lineFor({ ...base, state: UI.BAR_STATES_ALWAYS_LISTENING })).toEqual({ text: "Mic open", tone: "dim" });
  });

  it("keeps what you said in view while Juno works, and reads a tool as a stage direction", () => {
    expect(lineFor({ ...base, state: UI.BAR_STATES_SUBMITTING })).toEqual({ text: "Send it", tone: "dim" });
    expect(lineFor({ ...base, state: UI.BAR_STATES_LOADING })).toEqual({ text: "Send it", tone: "dim" });
    expect(lineFor({ ...base, state: UI.BAR_STATES_LOADING, runningTool: "open Mail" })).toEqual({
      text: "Juno runs open Mail",
      tone: "direction",
    });
    expect(lineFor({ ...base, state: UI.BAR_STATES_SUBMITTING, question: "" })).toEqual({
      text: "Sending",
      tone: "dim",
    });
  });

  it("speaks in blue, fails in red, and says when it is done or stopping", () => {
    expect(lineFor({ ...base, state: UI.BAR_STATES_SPEAKING, spokenText: "Here you go." })).toEqual({
      text: "Here you go.",
      tone: "speech",
    });
    expect(lineFor({ ...base, state: UI.BAR_STATES_SPEAKING })).toEqual({ text: "Speaking", tone: "dim" });
    expect(lineFor({ ...base, state: UI.BAR_STATES_ERROR, currentError: "No network" })).toEqual({
      text: "No network",
      tone: "error",
    });
    expect(lineFor({ ...base, state: UI.BAR_STATES_ERROR })).toEqual({
      text: "Something went wrong",
      tone: "error",
    });
    expect(lineFor({ ...base, state: UI.BAR_STATES_FINISHING })).toEqual({ text: "Done", tone: "plain" });
    expect(lineFor({ ...base, state: UI.BAR_STATES_SUCCESS })).toEqual({ text: "Done", tone: "plain" });
    expect(lineFor({ ...base, state: UI.BAR_STATES_STOPPING })).toEqual({ text: "Stopping", tone: "dim" });
  });

  it("driving wins over every state", () => {
    expect(lineFor({ ...base, state: UI.BAR_STATES_LISTENING, drivingLabel: "using the mouse in Mail" })).toEqual({
      text: "Juno is using the mouse in Mail",
      tone: "direction",
    });
  });

  it("is silent at rest and while typing", () => {
    for (const s of [
      UI.BAR_STATES_DEFAULT,
      UI.BAR_STATES_DICTATION_READY,
      UI.BAR_STATES_SHRINKING,
      UI.BAR_STATES_INPUT,
      UI.BAR_STATES_EXPANDING,
    ]) {
      expect(lineFor({ ...base, state: s })).toBeNull();
    }
  });
});

describe("teleprompter", () => {
  it("final words are solid; the tail after the last commit is provisional", () => {
    let committed = commitTranscript("", "Send the", true);
    expect(committed).toBe("");
    expect(teleprompterText("Send the", true, committed)).toEqual({ final: "", provisional: "Send the" });

    committed = commitTranscript(committed, "Send the draft", false);
    expect(committed).toBe("Send the draft");
    expect(teleprompterText("Send the draft", false, committed)).toEqual({
      final: "Send the draft",
      provisional: "",
    });

    committed = commitTranscript(committed, "Send the draft to May", true);
    expect(committed).toBe("Send the draft");
    expect(teleprompterText("Send the draft to May", true, committed)).toEqual({
      final: "Send the draft",
      provisional: " to May",
    });
  });

  it("a rewrite that no longer starts with the commit is all provisional", () => {
    const committed = commitTranscript("Send the draft", "Sent the draft to", true);
    expect(committed).toBe("");
    expect(teleprompterText("Sent the draft to", true, committed)).toEqual({
      final: "",
      provisional: "Sent the draft to",
    });
  });

  it("empty text clears the commit", () => {
    expect(commitTranscript("Send", "", false)).toBe("");
    expect(teleprompterText("", false, "Send")).toEqual({ final: "", provisional: "" });
  });
});

describe("tape", () => {
  it("counts while Rust or the conversation works, holds when it stops, is absent at rest", () => {
    expect(counterFor({ state: UI.BAR_STATES_SUBMITTING, processing: false, scriptOpen: false })).toBe("running");
    expect(counterFor({ state: UI.BAR_STATES_LOADING, processing: false, scriptOpen: false })).toBe("running");
    expect(counterFor({ state: UI.BAR_STATES_AGENT_RESPONDING, processing: false, scriptOpen: true })).toBe("running");
    expect(counterFor({ state: UI.BAR_STATES_DEFAULT, processing: true, scriptOpen: true })).toBe("running");
    expect(counterFor({ state: UI.BAR_STATES_DEFAULT, processing: false, scriptOpen: true })).toBe("held");
    for (const s of [
      UI.BAR_STATES_SPEAKING,
      UI.BAR_STATES_FINISHING,
      UI.BAR_STATES_SUCCESS,
      UI.BAR_STATES_ERROR,
      UI.BAR_STATES_STOPPING,
    ]) {
      expect(counterFor({ state: s, processing: false, scriptOpen: false })).toBe("held");
    }
    for (const s of [
      UI.BAR_STATES_DEFAULT,
      UI.BAR_STATES_LISTENING,
      UI.BAR_STATES_TRANSCRIBING,
      UI.BAR_STATES_INPUT,
      UI.BAR_STATES_ALWAYS_LISTENING,
    ]) {
      expect(counterFor({ state: s, processing: false, scriptOpen: false })).toBe("off");
    }
  });

  it("reads like a tape counter", () => {
    expect(formatCounter(0)).toBe("00:00.0");
    expect(formatCounter(1234)).toBe("00:01.2");
    expect(formatCounter(61_999)).toBe("01:01.9");
    expect(formatCounter(-5)).toBe("00:00.0");
  });

  it("runs out only when the script is complete and nothing is waiting", () => {
    const base = { scriptOpen: true, streaming: false, working: false, approvalPending: false };
    expect(tapeShouldRun(base)).toBe(true);
    expect(tapeShouldRun({ ...base, scriptOpen: false })).toBe(false);
    expect(tapeShouldRun({ ...base, streaming: true })).toBe(false);
    expect(tapeShouldRun({ ...base, working: true })).toBe(false);
    expect(tapeShouldRun({ ...base, approvalPending: true })).toBe(false);
  });
});

describe("script", () => {
  const messages: ChatMessage[] = [
    { role: "user", content: "Old question", timestamp: 1 },
    { role: "assistant", content: "Old answer", messageId: "old", timestamp: 2 },
    { role: "user", content: "Open the page", timestamp: 3 },
    { role: "tool_call_request", content: "open the page in Safari", tool_name: "browser", tool_id: "t1", success: true },
    { role: "tool_call_request", content: "read the title", tool_name: "browser", tool_id: "t2" },
    {
      role: "assistant",
      content: "The title is Hello.",
      messageId: "m1",
      timestamp: 4,
      tts_metadata: { has_spoken_content: true, tts_parts: ["It says hello."], total_spoken_text: "It says hello." },
    },
  ];

  it("is the current take only, with every tool since the question as a stage direction", () => {
    const s = scriptFor(messages);
    expect(s.question).toBe("Open the page");
    expect(s.answer?.messageId).toBe("m1");
    expect(s.approval).toBeNull();
    expect(s.runningTool).toBe("read the title");
    expect(s.directions.map((d) => [d.kind, d.text])).toEqual([
      ["done", "open the page in Safari"],
      ["running", "read the title"],
    ]);
  });

  it("a pending approval is a direction and the approval", () => {
    const s = scriptFor([
      { role: "user", content: "Q", timestamp: 1 },
      { role: "tool_call_request", content: "send the email", tool_id: "t3", approval_state: "pending" },
    ]);
    expect(s.approval?.tool_id).toBe("t3");
    expect(s.directions[0]).toMatchObject({ kind: "approval", text: "send the email" });
    expect(s.runningTool).toBeNull();
  });

  it("ignores notices and empty replies, and keys a reply by its id", () => {
    const s = scriptFor([
      { role: "user", content: "Q", timestamp: 1 },
      { role: "assistant", content: "Welcome to Juno", notice: true, messageId: "n" },
      { role: "assistant", content: "  ", messageId: "empty" },
    ]);
    expect(s.answer).toBeNull();
    expect(answerKey(s)).toBe("");
    expect(answerKey(scriptFor(messages))).toBe("m1");
  });

  it("separates what was said aloud from what was shown", () => {
    const s = scriptFor(messages);
    expect(replyChannels(s.answer)).toEqual({ spoken: "It says hello.", visible: "The title is Hello." });
    expect(replyChannels(null)).toEqual({ spoken: "", visible: "" });
  });

  it("does not read the spoken sentence twice when the notes already carry it", () => {
    const reply = (content: string, spoken: string): ChatMessage => ({
      role: "assistant",
      content,
      messageId: "m",
      tts_metadata: { has_spoken_content: true, tts_parts: [spoken], total_spoken_text: spoken },
    });
    // Juno spoke the first sentence of what it shows: the notes are the line.
    expect(
      replyChannels(reply("Done. The draft is with Maya.\n\nI will let you know.", "Done. The draft is with Maya.")),
    ).toEqual({ spoken: "", visible: "Done. The draft is with Maya.\n\nI will let you know." });
    // Case, spacing and trailing punctuation do not make it a different sentence.
    expect(replyChannels(reply("done, the draft  is with maya", "Done, the draft is with Maya!")).spoken).toBe("");
    // A sentence the person cannot read anywhere else keeps its own line.
    expect(replyChannels(reply("The title is Hello.", "It says hello.")).spoken).toBe("It says hello.");
    // Spoken only: the spoken text is all there is.
    expect(replyChannels(reply("", "Done.")).spoken).toBe("Done.");
    expect(spokenIsInVisible("", "anything")).toBe(false);
  });
});
