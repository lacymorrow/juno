import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import { POSTURE_SIZES, cardHeightFor, posture, postureSize, ringShouldRun } from "../notchPosture";

const EVERY_STATE = Object.entries(UI)
  .filter(([k]) => k.startsWith("BAR_STATES_"))
  .map(([, v]) => v as string);

// The Notch keeps the posture model the Island used before it became one
// surface with four faces. These pin it unchanged.

describe("notch posture", () => {
  it("rests as a capsule, opens an ear to speak, a line to type, a status line to work", () => {
    const at = (state: string) => posture({ state, cardOpen: false });
    expect(at(UI.BAR_STATES_DEFAULT)).toBe("capsule");
    expect(at(UI.BAR_STATES_DICTATION_READY)).toBe("capsule");
    expect(at(UI.BAR_STATES_SHRINKING)).toBe("capsule");
    expect(at(UI.BAR_STATES_LISTENING)).toBe("ear");
    expect(at(UI.BAR_STATES_DICTATING)).toBe("ear");
    expect(at(UI.BAR_STATES_ALWAYS_LISTENING)).toBe("ear");
    expect(at(UI.BAR_STATES_INPUT)).toBe("line");
    expect(at(UI.BAR_STATES_EXPANDING)).toBe("line");
    for (const s of [
      UI.BAR_STATES_TRANSCRIBING,
      UI.BAR_STATES_SUBMITTING,
      UI.BAR_STATES_LOADING,
      UI.BAR_STATES_AGENT_RESPONDING,
      UI.BAR_STATES_FINISHING,
      UI.BAR_STATES_SUCCESS,
      UI.BAR_STATES_SPEAKING,
      UI.BAR_STATES_ERROR,
      UI.BAR_STATES_STOPPING,
    ]) {
      expect(at(s)).toBe("status");
    }
  });

  it("maps every state Rust can send to a posture", () => {
    for (const state of EVERY_STATE) {
      expect(["capsule", "ear", "line", "status", "card"]).toContain(posture({ state, cardOpen: false }));
    }
  });

  it("an open card wins over everything, driving wins over the line", () => {
    expect(posture({ state: UI.BAR_STATES_INPUT, cardOpen: true })).toBe("card");
    expect(posture({ state: UI.BAR_STATES_LISTENING, cardOpen: true })).toBe("card");
    expect(posture({ state: UI.BAR_STATES_INPUT, cardOpen: false, driving: true })).toBe("status");
  });

  it("only an idle shape answers the pointer with its controls", () => {
    expect(posture({ state: UI.BAR_STATES_DEFAULT, cardOpen: false, hovered: true })).toBe("hover");
    expect(posture({ state: UI.BAR_STATES_LISTENING, cardOpen: false, hovered: true })).toBe("ear");
    expect(posture({ state: UI.BAR_STATES_DEFAULT, cardOpen: true, hovered: true })).toBe("card");
  });

  it("ear and status share a size; the card follows its content in steps", () => {
    expect(postureSize("ear")).toEqual(postureSize("status"));
    expect(cardHeightFor(10)).toBe(POSTURE_SIZES.card.minHeight);
    expect(cardHeightFor(150)).toBe(152);
    expect(cardHeightFor(9000)).toBe(POSTURE_SIZES.card.maxHeight);
    expect(postureSize("card", 150).height).toBe(152);
  });

  it("counts the ring only once the answer is complete, Juno is idle and nothing waits on you", () => {
    expect(ringShouldRun({ cardOpen: true, streaming: false, working: false, approvalPending: false })).toBe(true);
    expect(ringShouldRun({ cardOpen: true, streaming: true, working: false, approvalPending: false })).toBe(false);
    expect(ringShouldRun({ cardOpen: true, streaming: false, working: false, approvalPending: true })).toBe(false);
    expect(ringShouldRun({ cardOpen: false, streaming: false, working: false, approvalPending: false })).toBe(false);
  });
});
