import { fireEvent, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useEscapeStepsDown } from "@/hooks/useEscapeStepsDown";

const press = (init: KeyboardEventInit = {}) =>
  fireEvent.keyDown(window, { key: "Escape", ...init });

afterEach(() => {
  document.body.innerHTML = "";
});

describe("useEscapeStepsDown", () => {
  it("puts the window away on a bare Escape", () => {
    const onEscape = vi.fn();
    renderHook(() => useEscapeStepsDown(onEscape));
    press();
    expect(onEscape).toHaveBeenCalledTimes(1);
  });

  it("leaves a press something inside already handled", () => {
    const onEscape = vi.fn();
    renderHook(() => useEscapeStepsDown(onEscape));
    const recorder = document.createElement("div");
    document.body.appendChild(recorder);
    recorder.addEventListener("keydown", (e) => e.preventDefault());
    fireEvent.keyDown(recorder, { key: "Escape" });
    expect(onEscape).not.toHaveBeenCalled();
  });

  it("lets an open dialog or menu close first", () => {
    const onEscape = vi.fn();
    renderHook(() => useEscapeStepsDown(onEscape));
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    document.body.appendChild(dialog);
    press();
    expect(onEscape).not.toHaveBeenCalled();
  });

  it("lets an open popover close first", () => {
    const onEscape = vi.fn();
    renderHook(() => useEscapeStepsDown(onEscape));
    const popper = document.createElement("div");
    popper.setAttribute("data-radix-popper-content-wrapper", "");
    popper.innerHTML = '<div data-slot="popover-content">Pick one</div>';
    document.body.appendChild(popper);
    press();
    expect(onEscape).not.toHaveBeenCalled();
  });

  it("closes the window while a hover tooltip is showing", () => {
    // Settings' info icons open a tooltip on hover with no delay; the pointer
    // resting on one must not swallow Escape.
    const onEscape = vi.fn();
    renderHook(() => useEscapeStepsDown(onEscape));
    const popper = document.createElement("div");
    popper.setAttribute("data-radix-popper-content-wrapper", "");
    popper.innerHTML =
      '<div data-slot="tooltip-content">Info<span role="tooltip">Info</span></div>';
    document.body.appendChild(popper);
    press();
    expect(onEscape).toHaveBeenCalledTimes(1);
  });

  it("ignores chords and key repeat", () => {
    const onEscape = vi.fn();
    renderHook(() => useEscapeStepsDown(onEscape));
    press({ metaKey: true });
    press({ repeat: true });
    expect(onEscape).not.toHaveBeenCalled();
  });
});
