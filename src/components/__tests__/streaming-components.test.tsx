import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import {
  MixedContentRenderer,
  splitMixedContent,
} from "@/components/ui/mixed-content-renderer";
import {
  findOpenTagEnd,
  stripJsxMarkup,
  trimPartialTrailingTag,
  trimUnresolvedTagTail,
} from "@/lib/jsx-utils";

/** No raw component markup may ever reach the screen. */
function expectNoRawMarkup(container: HTMLElement) {
  expect(container.textContent ?? "").not.toMatch(/<\/?[A-Z]/);
}

describe("streaming segmentation", () => {
  it("shows nothing for a component name that is still arriving", () => {
    const segments = splitMixedContent("Here you go.\n<Weath", true);
    expect(segments).toEqual([{ type: "text", content: "Here you go.\n" }]);
  });

  it("marks a component whose attributes are still arriving as pending", () => {
    const segments = splitMixedContent('Here.\n<WeatherCard location="SF" temperature={5', true);
    expect(segments.map((s) => s.type)).toEqual(["text", "pending"]);
    expect(segments[1]).toMatchObject({ type: "pending", name: "WeatherCard" });
  });

  it("renders an open container progressively, minus the half-arrived inner tag", () => {
    const segments = splitMixedContent(
      '<AnimatedCard animation="fade-up"><div>Sunny</div><Stat value={7',
      true,
    );
    expect(segments).toHaveLength(1);
    expect(segments[0]).toMatchObject({ type: "jsx", partial: true, name: "AnimatedCard" });
    expect(segments[0].type === "jsx" && segments[0].content).toBe(
      '<AnimatedCard animation="fade-up"><div>Sunny</div>',
    );
  });

  it("treats a finished component mid-stream as complete, not partial", () => {
    const segments = splitMixedContent(
      'Saved.\n<StatusCard status="success" message="Saved" />\nMore on the',
      true,
    );
    expect(segments.map((s) => s.type)).toEqual(["text", "jsx", "text"]);
    expect(segments[1]).not.toHaveProperty("partial", true);
  });

  it("sees a self-closing tag close even with > inside an attribute", () => {
    const segments = splitMixedContent('<StatusCard status="info" message="a > b" />', false);
    expect(segments).toEqual([
      { type: "jsx", content: '<StatusCard status="info" message="a > b" />' },
    ]);
  });

  it("turns a known component that never closed into its words once streaming ends", () => {
    const segments = splitMixedContent(
      'Result:\n<AnimatedCard animation="fade-up"><div>Sunny, 72</div>',
      false,
    );
    expect(segments.map((s) => s.type)).toEqual(["text", "fallback"]);
    expect(segments[1]).toMatchObject({ type: "fallback", content: "Sunny, 72" });
  });

  it("still leaves prose that only looks like a tag as text", () => {
    expect(
      splitMixedContent("Use Vec<String> here.", false).every((s) => s.type === "text"),
    ).toBe(true);
  });
});

describe("streaming renderer", () => {
  it("shows a card-sized skeleton while a component's tag is still open", () => {
    const { container } = render(
      <MixedContentRenderer
        content={'Checking.\n<WeatherCard location="SF" condition="ra'}
        isStreaming
      />,
    );
    const skeleton = screen.getByTestId("component-skeleton");
    expect(skeleton).toHaveAttribute("data-component", "WeatherCard");
    expect(skeleton).toHaveAttribute("aria-busy", "true");
    expectNoRawMarkup(container);
  });

  it("renders a complete component immediately, while the reply is still streaming", () => {
    const { container } = render(
      <MixedContentRenderer
        content={'<StatusCard status="success" message="Spreadsheet saved" />\nAnd the'}
        isStreaming
      />,
    );
    expect(screen.getByText("Spreadsheet saved")).toBeInTheDocument();
    expect(screen.queryByTestId("component-skeleton")).toBeNull();
    expectNoRawMarkup(container);
  });

  it("swaps the skeleton for the card in place when the tag closes", () => {
    const { rerender, container } = render(
      <MixedContentRenderer content={'Done.\n<StatusCard status="success" mess'} isStreaming />,
    );
    expect(screen.getByTestId("component-skeleton")).toBeInTheDocument();
    rerender(
      <MixedContentRenderer
        content={'Done.\n<StatusCard status="success" message="All set" />'}
        isStreaming
      />,
    );
    expect(screen.queryByTestId("component-skeleton")).toBeNull();
    expect(screen.getByText("All set")).toBeInTheDocument();
    expectNoRawMarkup(container);
  });

  it("falls back to plain text for a component left unclosed at the end", () => {
    const { container } = render(
      <MixedContentRenderer
        content={'<AnimatedCard animation="fade-up"><div>Sunny, 72</div>'}
        isStreaming={false}
      />,
    );
    expect(screen.getByTestId("component-fallback")).toHaveTextContent("Sunny, 72");
    expectNoRawMarkup(container);
  });

  it("says the card couldn't be shown when a broken tag carried no words", () => {
    const { container } = render(
      <MixedContentRenderer content={'Here.\n<WeatherCard location="SF" temp'} />,
    );
    expect(screen.getByText("This card couldn't be shown.")).toBeInTheDocument();
    expectNoRawMarkup(container);
  });

  it("falls back instead of crashing when complete markup does not parse", () => {
    const { container } = render(
      <MixedContentRenderer content={"<Card><div>Kept words</div>{broken</Card>"} />,
    );
    expect(screen.getByTestId("component-fallback")).toHaveTextContent("Kept words");
    expectNoRawMarkup(container);
  });
});

describe("jsx-utils streaming helpers", () => {
  it("finds the end of an opening tag past quotes and braces", () => {
    const code = '<Stat label="a > b" data={[{v: 1}]} />rest';
    expect(code.slice(0, findOpenTagEnd(code, 0))).toBe('<Stat label="a > b" data={[{v: 1}]} />');
    expect(findOpenTagEnd('<Stat value={7', 0)).toBe(-1);
  });

  it("trims only a trailing tag that has not finished", () => {
    expect(trimPartialTrailingTag("<Card><Stat value={7")).toBe("<Card>");
    expect(trimPartialTrailingTag("<Card><Stat />")).toBe("<Card><Stat />");
  });

  it("trims a trailing tag name that is still arriving", () => {
    expect(trimUnresolvedTagTail("see <Weath")).toBe("see ");
    expect(trimUnresolvedTagTail("see </")).toBe("see ");
    expect(trimUnresolvedTagTail("a <b")).toBe("a <b");
  });

  it("strips markup down to the words", () => {
    expect(
      stripJsxMarkup('<Card>\n  <span className="b">Crust:</span> rock {1 + 1}.\n</Card>'),
    ).toBe("Crust: rock .");
  });
});
