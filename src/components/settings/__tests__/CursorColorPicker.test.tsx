import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { UI } from "@/lib/constants.generated";
import { CursorColorPicker, cursorColorHex } from "../CursorColorPicker";

describe("CursorColorPicker", () => {
  it("offers the five colors Rust defines, blue first and checked by default", () => {
    render(<CursorColorPicker value={UI.AGENT_CURSOR_COLORS_DEFAULT} onChange={vi.fn()} />);
    const radios = screen.getAllByRole("radio");
    expect(radios.map((r) => r.getAttribute("aria-label"))).toEqual([
      "Blue",
      "Pink",
      "Green",
      "Orange",
      "Purple",
    ]);
    expect(radios[0]).toHaveAttribute("aria-checked", "true");
    expect(cursorColorHex(UI.AGENT_CURSOR_COLORS_DEFAULT)).toBe("#0A84FF");
  });

  it("reads an unknown saved value as blue", () => {
    render(<CursorColorPicker value="chartreuse" onChange={vi.fn()} />);
    expect(screen.getByRole("radio", { name: "Blue" })).toHaveAttribute("aria-checked", "true");
  });

  it("chooses by click and by arrow key", () => {
    const onChange = vi.fn();
    render(<CursorColorPicker value="blue" onChange={onChange} />);
    fireEvent.click(screen.getByRole("radio", { name: "Orange" }));
    expect(onChange).toHaveBeenLastCalledWith("orange");
    fireEvent.keyDown(screen.getByRole("radio", { name: "Blue" }), { key: "ArrowLeft" });
    expect(onChange).toHaveBeenLastCalledWith("purple");
  });
});
