import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import {
  summonDemo,
  type TriggerHints,
} from "@/components/onboarding/Onboarding";

const hint = (shortcut: string, gesture: string, sentence: string) => ({
  shortcut,
  gesture,
  sentence,
});

const hints = (
  agent: TriggerHints["agent"],
  dictation: TriggerHints["dictation"] = null,
): TriggerHints => ({ agent, dictation });

const glyphs = (caps: { glyph: string }[]) => caps.map((c) => c.glyph);

describe("what the last onboarding screen teaches", () => {
  it("draws the key that is actually bound", () => {
    // The factory default: hold the globe key to talk to Juno.
    const demo = summonDemo(
      hints(hint("Fn", "Hold", "Hold to talk to Juno")),
    );
    expect(demo).not.toBeNull();
    expect(glyphs(demo!.caps)).toEqual(["🌐"]);
    expect(demo!.sentence).toBe("Hold to talk to Juno");
  });

  it("changes when the trigger changes", () => {
    // The reported defect. Onboarding taught a hardcoded Option+D, so changing
    // the trigger moved the lie rather than fixing it. Nothing in this screen
    // knows a shortcut of its own any more.
    const before = summonDemo(hints(hint("Fn", "Hold", "Hold to talk to Juno")));
    const after = summonDemo(
      hints(hint("Control+J", "Tap", "Tap to talk to Juno")),
    );

    expect(glyphs(before!.caps)).toEqual(["🌐"]);
    expect(glyphs(after!.caps)).toEqual(["⌃", "J"]);
    expect(after!.sentence).toBe("Tap to talk to Juno");
    expect(after).not.toEqual(before);
  });

  it("names the gesture the row actually uses", () => {
    const demo = summonDemo(
      hints(
        hint("Fn", "Double-tap and hold", "Double-tap and hold to talk to Juno"),
      ),
    );
    expect(demo!.sentence).toBe("Double-tap and hold to talk to Juno");
  });

  it("falls back to the dictation key when nothing talks to Juno", () => {
    const demo = summonDemo(
      hints(null, hint("Option+Space", "Hold", "Hold to dictate")),
    );
    expect(glyphs(demo!.caps)).toEqual(["⌥", "Space"]);
    expect(demo!.sentence).toBe("Hold to dictate");
  });

  it("teaches nothing when no key is bound", () => {
    // Saying nothing is honest. Naming a key that cannot fire is not, which is
    // exactly what the hardcoded default did.
    expect(summonDemo(hints(null, null))).toBeNull();
    expect(summonDemo(null)).toBeNull();
  });

  it("teaches nothing when the bound key is blank", () => {
    expect(summonDemo(hints(hint("   ", "Hold", "Hold to talk to Juno")))).toBeNull();
  });
});
