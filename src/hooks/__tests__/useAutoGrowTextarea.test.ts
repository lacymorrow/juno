import { describe, expect, it } from "vitest";

import { isSendKey } from "@/hooks/useAutoGrowTextarea";
import {
  pillFootprint,
  BAR_COMPOSER_LINE_PX,
  BAR_COMPOSER_MAX_PX,
  PILL_STEADY_SPEC,
} from "@/components/FloatingBar";

describe("isSendKey", () => {
  it("sends on Enter", () => {
    expect(isSendKey({ key: "Enter", shiftKey: false })).toBe(true);
  });

  it("makes a new line on Shift+Enter", () => {
    // The pill used to be a single-line input, which cannot hold a newline at
    // all, so there was nothing for this to insert.
    expect(isSendKey({ key: "Enter", shiftKey: true })).toBe(false);
  });

  it("leaves a composing Enter to the input method", () => {
    expect(
      isSendKey({ key: "Enter", shiftKey: false, nativeEvent: { isComposing: true } })
    ).toBe(false);
  });

  it("ignores every other key", () => {
    expect(isSendKey({ key: "a", shiftKey: false })).toBe(false);
    expect(isSendKey({ key: "Escape", shiftKey: false })).toBe(false);
  });
});

describe("pillFootprint with a grown composer", () => {
  const base = { layout: "full" as const, paneOpen: false, rosterVisible: false };

  it("is unchanged while the composer is one line tall", () => {
    expect(pillFootprint({ ...base, composerGrowth: 0 })).toEqual(
      pillFootprint(base)
    );
  });

  it("grows the window by exactly what the text needs", () => {
    const one = pillFootprint(base);
    const three = pillFootprint({
      ...base,
      composerGrowth: BAR_COMPOSER_LINE_PX * 2,
    });
    // The window is sized to its contents, so a pill that grew without this
    // would just be clipped by its own window.
    expect(three.height).toBe(one.height + BAR_COMPOSER_LINE_PX * 2);
    expect(three.width).toBe(one.width);
  });

  it("never outgrows the steady window, even at its tallest", () => {
    // The window is sized once for the largest footprint; the text growing
    // to its limit with the roster and the pane up is that footprint.
    const tallest = pillFootprint({
      ...base,
      paneOpen: true,
      rosterVisible: true,
      composerGrowth: BAR_COMPOSER_MAX_PX,
    });
    expect(tallest.height).toBeLessThanOrEqual(PILL_STEADY_SPEC.max.height);
    expect(tallest.width).toBeLessThanOrEqual(PILL_STEADY_SPEC.max.width);
  });

  it("still accounts for the pane underneath", () => {
    const withPane = pillFootprint({
      ...base,
      paneOpen: true,
      composerGrowth: 18,
    });
    const withoutPane = pillFootprint({ ...base, composerGrowth: 18 });
    expect(withPane.height).toBeGreaterThan(withoutPane.height);
  });
});
