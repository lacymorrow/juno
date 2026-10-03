import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { KeyCaps, shortcutCaps, shortcutSpoken } from "../KeyCaps";

describe("shortcutCaps", () => {
  it("draws modifiers as glyphs in Apple's order, whatever order they were stored in", () => {
    expect(shortcutCaps("Shift+Cmd+Option+Ctrl+K").map((c) => c.glyph)).toEqual([
      "⌃",
      "⌥",
      "⇧",
      "⌘",
      "K",
    ]);
  });

  it("accepts the backend's spellings and the platform ones", () => {
    expect(shortcutCaps("Alt+Space").map((c) => c.glyph)).toEqual(["⌥", "Space"]);
    expect(shortcutCaps("Meta+Return").map((c) => c.glyph)).toEqual(["⌘", "↩"]);
    expect(shortcutCaps("Fn").map((c) => c.glyph)).toEqual(["🌐"]);
  });

  it("draws the globe key and Control as two caps, Control first", () => {
    expect(shortcutCaps("Fn+Control").map((c) => c.glyph)).toEqual(["⌃", "🌐"]);
    expect(shortcutCaps("Control+Fn").map((c) => c.glyph)).toEqual(["⌃", "🌐"]);
  });

  it("prints a single letter as its cap", () => {
    expect(shortcutCaps("Option+d").map((c) => c.glyph)).toEqual(["⌥", "D"]);
  });

  it("speaks the keys by name for assistive tech", () => {
    expect(shortcutSpoken("Option+Space")).toBe("Option Space");
  });
});

describe("KeyCaps", () => {
  it("renders one cap per key with a spoken label", () => {
    render(<KeyCaps shortcut="Option+Space" />);
    const group = screen.getByRole("img", { name: "Option Space" });
    expect(group.querySelectorAll("kbd")).toHaveLength(2);
  });

  it("renders nothing for an empty binding", () => {
    const { container } = render(<KeyCaps shortcut="" />);
    expect(container).toBeEmptyDOMElement();
  });
});
