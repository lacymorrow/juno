import { describe, expect, it } from "vitest";

import { isSendKey } from "@/hooks/useAutoGrowTextarea";
import { floatingBarWindowSize, BAR_COMPOSER_LINE_PX } from "@/components/FloatingBar";

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

describe("floatingBarWindowSize with a grown composer", () => {
  const base = { layout: "full" as const, paneOpen: false, rosterVisible: false };

  it("is unchanged while the composer is one line tall", () => {
    expect(floatingBarWindowSize({ ...base, composerGrowth: 0 })).toEqual(
      floatingBarWindowSize(base)
    );
  });

  it("grows the window by exactly what the text needs", () => {
    const one = floatingBarWindowSize(base);
    const three = floatingBarWindowSize({
      ...base,
      composerGrowth: BAR_COMPOSER_LINE_PX * 2,
    });
    // The window is sized to its contents, so a pill that grew without this
    // would just be clipped by its own window.
    expect(three.height).toBe(one.height + BAR_COMPOSER_LINE_PX * 2);
    expect(three.width).toBe(one.width);
  });

  it("keeps the pill centred on the band as it grows", () => {
    const one = floatingBarWindowSize(base);
    const grown = floatingBarWindowSize({ ...base, composerGrowth: 36 });
    // anchorY is the pill's centre, which useWindowSize pins to the same spot
    // on screen. Growing by 36 moves that centre down by half of it, so the
    // pill opens evenly around itself instead of lurching.
    expect(grown.anchorY).toBe(one.anchorY + 18);
  });

  it("still accounts for the pane underneath", () => {
    const withPane = floatingBarWindowSize({
      ...base,
      paneOpen: true,
      composerGrowth: 18,
    });
    const withoutPane = floatingBarWindowSize({ ...base, composerGrowth: 18 });
    expect(withPane.height).toBeGreaterThan(withoutPane.height);
  });
});
