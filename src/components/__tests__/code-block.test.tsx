import { render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { CodeBlock } from "@/components/ai-elements/code-block";

// A shiki token span carries the theme colour inline; the raw pre-highlight
// tokens use `color: inherit`.
const colouredSpans = (root: HTMLElement) =>
  Array.from(root.querySelectorAll("code span")).filter((span) => {
    const colour = (span as HTMLElement).style.color;
    return colour !== "" && colour !== "inherit";
  });

describe("CodeBlock", () => {
  it("highlights a curated language with shiki token spans", async () => {
    const { container } = render(
      <CodeBlock code={'const greeting = "hello";'} language="javascript" />
    );

    await waitFor(() => {
      expect(colouredSpans(container).length).toBeGreaterThan(1);
    });
    expect(container.querySelector("code")?.textContent).toBe(
      'const greeting = "hello";'
    );
  });

  it("renders an unknown language as plain text without throwing", async () => {
    const source = "IDENTIFICATION DIVISION.\n\nPROGRAM-ID. HELLO.";
    const { container } = render(
      <CodeBlock code={source} language="cobol" />
    );
    const lineTexts = () =>
      Array.from(container.querySelectorAll("code > span")).map(
        (line) => line.textContent
      );

    // Text is on screen immediately (raw tokens) and stays, line for line,
    // after the highlighter resolves to plain text.
    expect(lineTexts()).toEqual(["IDENTIFICATION DIVISION.", "\n", "PROGRAM-ID. HELLO."]);
    // Wait for the async highlight to settle (raw tokens carry an explicit
    // `color: inherit`; shiki's plain tokens carry no colour at all).
    await waitFor(() => {
      const firstToken = container.querySelector("code > span > span") as HTMLElement;
      expect(firstToken.style.color).toBe("");
    });
    expect(lineTexts()).toEqual(["IDENTIFICATION DIVISION.", "\n", "PROGRAM-ID. HELLO."]);
    expect(colouredSpans(container)).toHaveLength(0);
  });

  it("resolves fence aliases such as sh", async () => {
    const { container } = render(
      <CodeBlock code="echo $HOME" language="sh" />
    );
    await waitFor(() => {
      expect(colouredSpans(container).length).toBeGreaterThan(1);
    });
  });
});
