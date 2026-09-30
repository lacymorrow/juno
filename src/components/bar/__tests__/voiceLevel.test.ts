import { describe, expect, it } from "vitest";
import { clampLevel, voiceScale } from "../voiceLevel";

describe("voice level", () => {
  it("is safe against anything the backend might send", () => {
    expect(clampLevel(0.4)).toBe(0.4);
    expect(clampLevel(-1)).toBe(0);
    expect(clampLevel(3)).toBe(1);
    expect(clampLevel(Number.NaN)).toBe(0);
    expect(clampLevel(undefined)).toBe(0);
  });

  it("rests at 1 when silent and grows with the voice, quiet speech visibly", () => {
    expect(voiceScale(0)).toBe(1);
    expect(voiceScale(1)).toBeCloseTo(1.9);
    expect(voiceScale(1, 1.1)).toBeCloseTo(2.1);
    // A quarter of full level already shows half the swell.
    expect(voiceScale(0.25)).toBeCloseTo(1.45);
    expect(voiceScale(0.6)).toBeGreaterThan(voiceScale(0.3));
  });
});
