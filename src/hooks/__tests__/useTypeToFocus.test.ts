import { describe, expect, it } from "vitest";
import { isEditableTarget, overlayIsOpen, wantsComposer, type TypeToFocusContext, type TypeToFocusKey } from "../useTypeToFocus";

const ctx: TypeToFocusContext = { targetEditable: false, overlayOpen: false, recording: false };
const key = (k: string, extra: Partial<TypeToFocusKey> = {}): TypeToFocusKey => ({
  key: k,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  ...extra,
});

describe("wantsComposer", () => {
  it("takes a printable key, shifted or not", () => {
    for (const k of ["a", "A", "?", "é", "😀"]) expect(wantsComposer(key(k), ctx)).toBe(true);
  });

  it("leaves Cmd, Ctrl and Alt chords alone", () => {
    expect(wantsComposer(key("a", { metaKey: true }), ctx)).toBe(false);
    expect(wantsComposer(key("a", { ctrlKey: true }), ctx)).toBe(false);
    expect(wantsComposer(key("a", { altKey: true }), ctx)).toBe(false);
  });

  it("leaves named keys and Space alone", () => {
    for (const k of ["Escape", "Tab", "Enter", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "F1", "F12", "Shift", "Meta", "Backspace", " "]) {
      expect(wantsComposer(key(k), ctx)).toBe(false);
    }
  });

  it("takes Cmd+V, with Shift, but not Ctrl+V or Alt+Cmd+V", () => {
    expect(wantsComposer(key("v", { metaKey: true }), ctx)).toBe(true);
    expect(wantsComposer(key("V", { metaKey: true }), ctx)).toBe(true);
    expect(wantsComposer(key("v", { ctrlKey: true }), ctx)).toBe(false);
    expect(wantsComposer(key("v", { metaKey: true, altKey: true }), ctx)).toBe(false);
  });

  it("stands down for an IME composition", () => {
    expect(wantsComposer(key("a", { isComposing: true }), ctx)).toBe(false);
    expect(wantsComposer(key("Process", { keyCode: 229 }), ctx)).toBe(false);
    expect(wantsComposer(key("a", { keyCode: 229 }), ctx)).toBe(false);
  });

  it("stands down when the target is editable, a menu is open, a mic is open, or the key was handled", () => {
    expect(wantsComposer(key("a"), { ...ctx, targetEditable: true })).toBe(false);
    expect(wantsComposer(key("a"), { ...ctx, overlayOpen: true })).toBe(false);
    expect(wantsComposer(key("a"), { ...ctx, recording: true })).toBe(false);
    expect(wantsComposer(key("a", { defaultPrevented: true }), ctx)).toBe(false);
    expect(wantsComposer(key("v", { metaKey: true }), { ...ctx, recording: true })).toBe(false);
  });
});

describe("isEditableTarget", () => {
  it("knows inputs, textareas, selects and contenteditable", () => {
    expect(isEditableTarget(document.createElement("input"))).toBe(true);
    expect(isEditableTarget(document.createElement("textarea"))).toBe(true);
    expect(isEditableTarget(document.createElement("select"))).toBe(true);
    const host = document.createElement("div");
    host.setAttribute("contenteditable", "true");
    const child = document.createElement("span");
    host.appendChild(child);
    expect(isEditableTarget(host)).toBe(true);
    expect(isEditableTarget(child)).toBe(true);
  });

  it("is false for the body, a button and nothing", () => {
    expect(isEditableTarget(document.body)).toBe(false);
    expect(isEditableTarget(document.createElement("button"))).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
    const off = document.createElement("div");
    off.setAttribute("contenteditable", "false");
    expect(isEditableTarget(off)).toBe(false);
  });
});

describe("overlayIsOpen", () => {
  it("sees a menu or dialog, not a tooltip", () => {
    expect(overlayIsOpen()).toBe(false);
    const tip = document.createElement("div");
    tip.setAttribute("role", "tooltip");
    document.body.appendChild(tip);
    expect(overlayIsOpen()).toBe(false);
    const menu = document.createElement("div");
    menu.setAttribute("role", "menu");
    document.body.appendChild(menu);
    expect(overlayIsOpen()).toBe(true);
    tip.remove();
    menu.remove();
  });
});
