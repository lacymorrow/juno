import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import {
  demoPressEvent,
  isDemoPress,
  summonDemo,
  usesGlobeKey,
  type TriggerHints,
} from "../Onboarding";
import { EVENTS } from "@/lib/constants.generated";

const hint = (shortcut: string, gesture = "Hold") => ({
  shortcut,
  gesture,
  sentence: `${gesture} to talk to Juno`,
});
const pressed = { state: "pressed" };

describe("the onboarding demo follows the person's own trigger", () => {
  it.each(["Fn", "RightOption", "Control+Option", "Fn+Control", "Alt+Space"])(
    "draws key caps and accepts a press for %s",
    (shortcut) => {
      const hints: TriggerHints = { agent: hint(shortcut), dictation: null };
      expect(summonDemo(hints)?.caps.length).toBeGreaterThan(0);
      expect(isDemoPress(hints, EVENTS.SHORTCUTS_AGENT_MODE, pressed)).toBe(true);
    }
  );

  it("accepts the Right Option fallback a keyboard without Fn settles on", () => {
    const hints: TriggerHints = { agent: hint("RightOption"), dictation: null, globe_key: false };
    expect(isDemoPress(hints, EVENTS.SHORTCUTS_AGENT_MODE, pressed)).toBe(true);
  });

  it("ignores a release, and a press of the other target's key", () => {
    const hints: TriggerHints = { agent: hint("Control+Option"), dictation: hint("Fn+Control") };
    expect(isDemoPress(hints, EVENTS.SHORTCUTS_AGENT_MODE, { state: "released" })).toBe(false);
    expect(isDemoPress(hints, EVENTS.SHORTCUTS_DICTATION_INPUT, pressed)).toBe(false);
  });

  it("listens for dictation when nothing is bound to the agent", () => {
    const hints: TriggerHints = { agent: null, dictation: hint("Control") };
    expect(demoPressEvent(hints)).toBe(EVENTS.SHORTCUTS_DICTATION_INPUT);
    expect(isDemoPress(hints, EVENTS.SHORTCUTS_DICTATION_INPUT, pressed)).toBe(true);
  });

  it("accepts nothing when no key is bound", () => {
    expect(demoPressEvent(null)).toBeNull();
    expect(isDemoPress(null, EVENTS.SHORTCUTS_AGENT_MODE, pressed)).toBe(false);
  });

  it("only mentions the globe key setting for a trigger that uses it", () => {
    expect(usesGlobeKey("Fn")).toBe(true);
    expect(usesGlobeKey("Fn+Control")).toBe(true);
    expect(usesGlobeKey("RightOption")).toBe(false);
    expect(usesGlobeKey(undefined)).toBe(false);
  });
});

describe("onboarding never rewrites triggers", () => {
  // Setup used to adopt the globe key as the talk trigger on a globe press,
  // and hold capture open so a custom trigger was swallowed. Finishing or
  // restarting setup therefore reset a person's trigger. The screen only
  // reads the registry now; this pins that it stays read-only.
  const source = readFileSync(resolve(__dirname, "../Onboarding.tsx"), "utf8");

  it("has no call that writes triggers or takes over the trigger keys", () => {
    expect(source).not.toContain("TRIGGERS_SET_TRIGGERS");
    expect(source).not.toContain("TRIGGERS_SET_TRIGGER_CAPTURE");
    expect(source).not.toContain("TRIGGERS_KEY_CAPTURED");
  });
});
