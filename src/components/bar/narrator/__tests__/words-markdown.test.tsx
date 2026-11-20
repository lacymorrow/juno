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

describe("the answer line, end to end from the model", () => {
  it("renders **Done.** bold and keeps file names and products intact", async () => {
    const { firstSentence } = await import("../narratorModel");
    const { unmount } = render(<Words segment={seg("answer", firstSentence("**Done.** Sent."))} theme="light" />);
    expect(screen.getByTestId("bar-answer").querySelector("strong")?.textContent).toBe("Done.");
    unmount();
    render(<Words segment={seg("answer", firstSentence("Wrote my_file_name.txt, 2 * 3 times. More."))} theme="light" />);
    expect(screen.getByTestId("bar-answer").textContent).toBe("Wrote my_file_name.txt, 2 * 3 times.");
  });
});
