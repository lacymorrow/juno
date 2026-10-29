import { describe, expect, it } from "vitest";
import { flameForState } from "../barFlame";
import { UI } from "@/lib/constants.generated";

const BLUE = "#0A84FF";
const GREEN = "#30D158";

describe("flameForState", () => {
  it("transcribes in the colour the words were heard in", () => {
    expect(flameForState(UI.BAR_STATES_TRANSCRIBING, true, BLUE, true)?.color).toBe(GREEN);
    expect(flameForState(UI.BAR_STATES_TRANSCRIBING, true, BLUE, false)?.color).toBe(BLUE);
  });

  it("asking Juno never shows the dictation colour", () => {
    for (const state of [
      UI.BAR_STATES_LISTENING,
      UI.BAR_STATES_TRANSCRIBING,
      UI.BAR_STATES_LOADING,
      UI.BAR_STATES_AGENT_RESPONDING,
    ]) {
      expect(flameForState(state, true, BLUE, false)?.color).not.toBe(GREEN);
    }
  });
});
