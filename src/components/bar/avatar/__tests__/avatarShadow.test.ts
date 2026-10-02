import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import {
  BUBBLE_DEPTH,
  CONTROLS_HEIGHT,
  GAP,
  PAD,
  REST_SIZE,
  sceneFor,
  windowFor,
  type SceneInput,
} from "../avatarModel";

/** How far each box-shadow layer paints past the shape, per side. */
function reach(shadow: string) {
  return shadow.split(/,(?![^(]*\))/).map((layer) => {
    const [x = 0, y = 0, blur = 0, spread = 0] = (layer.match(/-?[\d.]+px|\b0\b(?![.\d])/g) ?? []).map((n) =>
      parseFloat(n),
    );
    return {
      top: blur + spread - y,
      bottom: blur + spread + y,
      left: blur + spread - x,
      right: blur + spread + x,
    };
  });
}

const REST_INPUT: SceneInput = {
  state: UI.BAR_STATES_DEFAULT,
  transcriptionText: "",
  spokenText: "",
  currentError: null,
  question: "",
  answerOpen: false,
  hasAnswerContent: false,
  approvalPending: false,
};

describe("avatar window fit", () => {
  it("ends every bubble's shadow inside the window padding, so the window edge never clips it", () => {
    for (const r of reach(BUBBLE_DEPTH)) {
      expect(Math.max(r.top, r.bottom, r.left, r.right)).toBeLessThanOrEqual(PAD);
    }
  });

  it("hovering at rest grows the window toward the bubbles only, by the controls' height", () => {
    const scene = sceneFor({ ...REST_INPUT, hovered: true });
    expect(scene.junos).toEqual({ kind: "controls" });
    expect(windowFor(scene, 999)).toMatchObject({ width: REST_SIZE, height: REST_SIZE + GAP + CONTROLS_HEIGHT });
  });

  it("shows the controls only at rest", () => {
    expect(sceneFor({ ...REST_INPUT, hovered: false }).junos).toBeNull();
    expect(sceneFor({ ...REST_INPUT, hovered: true, state: UI.BAR_STATES_LISTENING }).junos).toBeNull();
    expect(sceneFor({ ...REST_INPUT, hovered: true, state: UI.BAR_STATES_LOADING }).junos?.kind).toBe("thought");
    expect(
      sceneFor({ ...REST_INPUT, hovered: true, answerOpen: true, hasAnswerContent: true }).junos?.kind,
    ).toBe("answer");
  });
});
