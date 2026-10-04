import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import { globePressTiming, type TriggerHints } from "../Onboarding";

const hint = (shortcut: string) => ({ shortcut, gesture: "Hold", sentence: "Hold to talk" });

describe("globe key press on the last onboarding screen", () => {
  it("lights the key on the first press when the globe key is already the one drawn", () => {
    const hints: TriggerHints = { agent: hint("Fn"), dictation: null };
    expect(globePressTiming(hints)).toBe("now");
  });

  it("lights the key once the globe key is adopted when another key is drawn", () => {
    const hints: TriggerHints = { agent: hint("Alt+D"), dictation: null };
    expect(globePressTiming(hints)).toBe("after-adopt");
    expect(globePressTiming(null)).toBe("after-adopt");
  });
});
