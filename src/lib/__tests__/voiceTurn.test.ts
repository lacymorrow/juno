import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import { isRecordingTurn, voiceTurnControls } from "@/lib/voiceTurn";

describe("voiceTurnControls", () => {
  // Every state in which the microphone is open for a turn the person can
  // still send. Send must be in each one: losing it the moment live words
  // arrived is the bug this mapping exists to end.
  const recording = [
    { barState: UI.BAR_STATES_LISTENING },
    { barState: UI.BAR_STATES_DICTATING },
    { barState: UI.BAR_STATES_TRANSCRIBING, transcriptionProvisional: true },
  ];

  it.each(recording)("offers send, type instead and cancel while recording: %o", (state) => {
    expect(isRecordingTurn(state)).toBe(true);
    expect(voiceTurnControls(state)).toEqual(["send", "type", "cancel"]);
  });

  it("offers nothing once the mic has closed and the final decode is running", () => {
    const closed = { barState: UI.BAR_STATES_TRANSCRIBING, transcriptionProvisional: false };
    expect(isRecordingTurn(closed)).toBe(false);
    expect(voiceTurnControls(closed)).toEqual([]);
  });

  it("offers cancel only for a wake-phrase capture", () => {
    expect(voiceTurnControls({ barState: UI.BAR_STATES_ALWAYS_LISTENING })).toEqual(["cancel"]);
  });

  it.each([
    UI.BAR_STATES_DEFAULT,
    UI.BAR_STATES_INPUT,
    UI.BAR_STATES_LOADING,
    UI.BAR_STATES_AGENT_RESPONDING,
    UI.BAR_STATES_ERROR,
  ])("offers no voice controls in %s", (barState) => {
    expect(voiceTurnControls({ barState })).toEqual([]);
  });
});
