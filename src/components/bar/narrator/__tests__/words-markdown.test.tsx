import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Words } from "../NarratorStrip";
import type { Segment } from "../narratorModel";

const seg = (kind: Segment["kind"], text: string): Segment => ({ kind, text, tone: "plain", bead: false }) as Segment;

describe("Words inline markdown", () => {
  it("styles the answer and keeps the hover title plain", () => {
    render(<Words segment={seg("answer", "It is **done** and `ls` ran.")} theme="light" />);
    const el = screen.getByTestId("bar-answer");
    expect(el.querySelector("strong")?.textContent).toBe("done");
    expect(el.querySelector("code")?.textContent).toBe("ls");
    expect(el.textContent).toBe("It is done and ls ran.");
    expect(el.getAttribute("title")).toBe("It is done and ls ran.");
  });

  it("styles the answer inside the tail ticker", () => {
    render(<Words segment={seg("answer", "a **b** c")} theme="dark" tail />);
    expect(screen.getByTestId("bar-answer").querySelector("bdi strong")?.textContent).toBe("b");
  });

  it.each(["question", "step", "error", "note"] as const)("leaves %s literal", (kind) => {
    const text = "open *my_file_name* and **x**";
    render(<Words segment={seg(kind, text)} theme="light" />);
    const el = screen.getByTestId(`bar-${kind}`);
    expect(el.textContent).toBe(text);
    expect(el.querySelector("strong, em, code")).toBeNull();
    expect(el.getAttribute("title")).toBe(text);
  });
});
