import { render } from "@testing-library/react";
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { UI } from "@/lib/constants.generated";
import { REST_DRIFT_MS, orbLook, orbTargets, type OrbLook } from "../shaderOrbModel";

/**
 * The canvas's own loop, with ogl standing in for a GPU. jsdom has no WebGL,
 * so the stub records the uniforms the loop writes and counts the frames it
 * draws; everything else here is the real component.
 *
 * The test that matters most is the sleep one. The loop stopping was once
 * gated on the `frameloop` prop, which the bar only sets to "demand" in
 * response to `onSettled` from the loop itself: a condition that could never
 * become true, so an idle desk would have burned a frame every 16ms forever.
 */

const { uniforms, renders } = vi.hoisted(() => ({
  uniforms: {} as Record<string, { value: unknown }>,
  renders: { count: 0 },
}));

vi.mock("ogl", () => {
  class Renderer {
    gl = {
      canvas: Object.assign(document.createElement("canvas"), { width: 320, height: 320 }),
      clearColor: () => {},
      getExtension: () => null,
    };
    setSize() {}
    render() {
      renders.count += 1;
    }
  }
  class Program {
    uniforms: Record<string, { value: unknown }>;
    constructor(_gl: unknown, opts: { uniforms: Record<string, { value: unknown }> }) {
      this.uniforms = opts.uniforms;
      for (const key of Object.keys(opts.uniforms)) uniforms[key] = opts.uniforms[key];
    }
  }
  class Mesh {}
  class Triangle {}
  return { Renderer, Program, Mesh, Triangle };
});

import { OrbShaderCanvas, type OrbDrive } from "../OrbShaderCanvas";

function driveFor(look: OrbLook, audioLevel = 0): { current: OrbDrive } {
  return { current: { look, targets: orbTargets(look, audioLevel), impulse: 0 } };
}

const run = async (ms: number) => {
  await act(async () => {
    vi.advanceTimersByTime(ms);
  });
};

beforeEach(() => {
  renders.count = 0;
  for (const key of Object.keys(uniforms)) delete uniforms[key];
  vi.useFakeTimers({
    toFake: ["requestAnimationFrame", "cancelAnimationFrame", "performance", "setTimeout", "clearTimeout"],
  });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("OrbShaderCanvas", () => {
  it("stops drawing once the resting look has settled and drifted to a stop", async () => {
    const onSettled = vi.fn();
    const drive = driveFor(orbLook({ state: UI.BAR_STATES_DEFAULT }));
    render(<OrbShaderCanvas drive={drive} frameloop="always" onSettled={onSettled} size={160} />);

    // It keeps drawing through the drift window, so the inside comes to rest
    // rather than cutting to a still frame.
    await run(REST_DRIFT_MS - 400);
    expect(onSettled).not.toHaveBeenCalled();
    expect(renders.count).toBeGreaterThan(10);

    await run(800);
    expect(onSettled).toHaveBeenCalledTimes(1);

    const drawn = renders.count;
    await run(2000);
    expect(renders.count).toBe(drawn);
  });

  it("never stops in a look that is meant to be moving", async () => {
    const onSettled = vi.fn();
    const drive = driveFor(orbLook({ state: UI.BAR_STATES_LOADING }));
    render(<OrbShaderCanvas drive={drive} frameloop="always" onSettled={onSettled} size={160} />);
    await run(REST_DRIFT_MS * 2);
    expect(onSettled).not.toHaveBeenCalled();
    expect(renders.count).toBeGreaterThan(50);
  });

  it("stops for an approval, because a frozen orb is a resting one", async () => {
    const onSettled = vi.fn();
    const drive = driveFor(orbLook({ state: UI.BAR_STATES_LOADING, approvalPending: true }));
    render(<OrbShaderCanvas drive={drive} frameloop="always" onSettled={onSettled} size={160} />);
    await run(REST_DRIFT_MS * 2);
    expect(onSettled).toHaveBeenCalledTimes(1);
    // Frozen means frozen: the inside's clock did not advance.
    expect(uniforms.uTime.value).toBe(0);
    expect(uniforms.uRot.value).toBe(0);
  });

  it("wakes again when the bar sets the loop back to always", async () => {
    const onSettled = vi.fn();
    const drive = driveFor(orbLook({ state: UI.BAR_STATES_DEFAULT }));
    const { rerender } = render(
      <OrbShaderCanvas drive={drive} frameloop="always" onSettled={onSettled} size={160} />,
    );
    await run(REST_DRIFT_MS + 500);
    expect(onSettled).toHaveBeenCalledTimes(1);

    // The bar records the sleep, then Rust says something.
    rerender(<OrbShaderCanvas drive={drive} frameloop="demand" onSettled={onSettled} size={160} />);
    const asleep = renders.count;
    await run(500);
    expect(renders.count).toBe(asleep);

    drive.current = { ...driveFor(orbLook({ state: UI.BAR_STATES_LISTENING })).current, impulse: 0 };
    rerender(<OrbShaderCanvas drive={drive} frameloop="always" onSettled={onSettled} size={160} />);
    await run(500);
    expect(renders.count).toBeGreaterThan(asleep);
  });

  it("eases the state onto its uniforms, and takes the short way round the hue", async () => {
    const onSettled = vi.fn();
    const drive = driveFor(orbLook({ state: UI.BAR_STATES_DEFAULT }));
    render(<OrbShaderCanvas drive={drive} frameloop="always" onSettled={onSettled} size={160} />);
    await run(100);
    expect(uniforms.uHue.value).toBe(0);
    expect(uniforms.uMono.value).toBe(0);

    // The microphone opens: the hue goes 0 to 350 the short way, downward.
    drive.current = driveFor(orbLook({ state: UI.BAR_STATES_LISTENING }), 1).current;
    await run(100);
    expect(uniforms.uHue.value as number).toBeLessThan(0);
    await run(1500);
    expect(uniforms.uHue.value as number).toBeCloseTo(-10, 0);
    expect(uniforms.uScale.value as number).toBeGreaterThan(0.9);

    // A failure collapses the colours onto one red object.
    drive.current = driveFor(orbLook({ state: UI.BAR_STATES_ERROR })).current;
    await run(2000);
    expect(uniforms.uMono.value as number).toBeCloseTo(1, 1);
  });

  it("leaves the page standing when a WebGL context cannot be created", async () => {
    const ogl = await import("ogl");
    const spy = vi.spyOn(ogl, "Renderer").mockImplementation(() => {
      throw new Error("no GPU");
    });
    const onSettled = vi.fn();
    const drive = driveFor(orbLook({ state: UI.BAR_STATES_DEFAULT }));
    expect(() =>
      render(<OrbShaderCanvas drive={drive} frameloop="always" onSettled={onSettled} size={160} />),
    ).not.toThrow();
    await run(500);
    spy.mockRestore();
  });
});
