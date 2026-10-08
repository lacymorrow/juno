import { describe, expect, it } from "vitest";
import { FRAG, LOOK, progressAt, uniformsFor, type IntroPlan } from "../introModel";

const topCentre: IntroPlan = {
  duration_ms: 2600,
  bar_at_ms: 1000,
  pill_x: 280,
  pill_y: 40,
  pill_w: 56,
  pill_h: 16,
  pill_radius: 8,
  inward_x: 0,
  inward_y: 1,
};
const viewport = { width: 560, height: 360 };

describe("uniformsFor", () => {
  it("flips the plan from top-left y-down points into bottom-left y-up device pixels", () => {
    const u = uniformsFor(topCentre, viewport, 2, true, false);
    expect(u.uRes).toEqual([1120, 720]);
    expect(u.uPill).toEqual([560, (360 - 40) * 2]);
    // "Down" in the plan is "-y" in the shader.
    expect(u.uInward).toEqual([0, -1]);
  });

  it("draws the pill at the size the backend measured, so the smoke hugs the real shape", () => {
    const u = uniformsFor(topCentre, viewport, 2, true, false);
    expect(u.uHalf).toEqual([56, 16]);
    expect(u.uRadius).toBe(16);
  });

  it("picks white smoke over a dark desktop and charcoal over a light one", () => {
    expect(uniformsFor(topCentre, viewport, 1, true, false).uColor).toEqual([...LOOK.colorOnDark]);
    expect(uniformsFor(topCentre, viewport, 1, false, false).uColor).toEqual([...LOOK.colorOnLight]);
  });

  it("keeps the rim and drops the smoke under Reduce Motion", () => {
    const u = uniformsFor(topCentre, viewport, 1, true, true);
    expect(u.uSmoke).toBe(0);
    expect(u.uRim).toBe(LOOK.rim);
    expect(uniformsFor(topCentre, viewport, 1, true, false).uSmoke).toBe(1);
  });

  it("leaves a mid-screen pill with no direction, so the cloud is round", () => {
    const centred = { ...topCentre, inward_x: 0, inward_y: 0 };
    expect(uniformsFor(centred, viewport, 1, true, false).uInward).toEqual([0, -0]);
  });

  it("starts at the beginning of the sequence", () => {
    const u = uniformsFor(topCentre, viewport, 1, true, false);
    expect(u.uT).toBe(0);
    expect(u.uTime).toBe(0);
  });
});

describe("progressAt", () => {
  it("runs 0..1 over the backend's duration and clamps at both ends", () => {
    expect(progressAt(1000, 1000, 2600)).toBe(0);
    expect(progressAt(2300, 1000, 2600)).toBeCloseTo(0.5);
    expect(progressAt(9000, 1000, 2600)).toBe(1);
    expect(progressAt(500, 1000, 2600)).toBe(0);
  });

  it("treats a zero-length sequence as already over", () => {
    expect(progressAt(1000, 1000, 0)).toBe(1);
  });
});

describe("the shader", () => {
  it("reads every uniform the model produces, so a renamed tweakable cannot go quietly unused", () => {
    const u = uniformsFor(topCentre, viewport, 1, true, false);
    for (const name of Object.keys(u)) {
      expect(FRAG, name).toContain(`uniform`);
      expect(FRAG).toMatch(new RegExp(`uniform\\s+\\w+\\s+${name};`));
    }
  });
});
