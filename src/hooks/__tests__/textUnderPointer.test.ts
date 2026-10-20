import { afterEach, describe, expect, it } from "vitest";
import { textUnderPointer } from "../useDragWindow";

afterEach(() => {
  document.body.innerHTML = "";
  window.getSelection()?.removeAllRanges();
});

describe("textUnderPointer: what the bar's right-click menu leads with", () => {
  it("a text box is editable, with a selection only when one is made", () => {
    const box = document.createElement("textarea");
    box.value = "hello world";
    document.body.append(box);
    expect(textUnderPointer(box)).toEqual({ editable: true, hasSelection: false });
    box.setSelectionRange(0, 5);
    expect(textUnderPointer(box)).toEqual({ editable: true, hasSelection: true });
  });

  it("selected read-only text offers Copy; plain chrome offers nothing", () => {
    const p = document.createElement("p");
    p.textContent = "Sunny, 72.";
    document.body.append(p);
    expect(textUnderPointer(p)).toEqual({ editable: false, hasSelection: false });
    const range = document.createRange();
    range.selectNodeContents(p);
    window.getSelection()?.addRange(range);
    expect(textUnderPointer(p)).toEqual({ editable: false, hasSelection: true });
    expect(textUnderPointer(null)).toEqual({ editable: false, hasSelection: true });
  });
});
