import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { APPEARANCE_CATALOG } from "@/components/bar/appearanceCatalog";
import { AppearancePicker } from "../AppearancePicker";

function frames() {
  return Array.from(document.querySelectorAll("iframe")).map((f) => ({
    appearance: new URL(f.getAttribute("src") ?? "", "http://x").searchParams.get("appearance"),
    current: f.getAttribute("data-current") === "true",
    hidden: f.className.includes("opacity-0"),
  }));
}

function ready(appearance: string) {
  act(() => {
    window.dispatchEvent(
      new MessageEvent("message", {
        origin: window.location.origin,
        data: { type: "juno-bar-preview", status: "ready", appearance },
      }),
    );
  });
}

describe("AppearancePicker", () => {
  it("keeps the previous and next looks mounted, hidden, so stepping is instant", () => {
    const onChange = vi.fn();
    render(<AppearancePicker value={APPEARANCE_CATALOG[1].value} onChange={onChange} />);
    ready(APPEARANCE_CATALOG[1].value);
    const mounted = frames();
    expect(mounted.map((f) => f.appearance)).toEqual([
      APPEARANCE_CATALOG[0].value,
      APPEARANCE_CATALOG[1].value,
      APPEARANCE_CATALOG[2].value,
    ]);
    expect(mounted.filter((f) => f.current)).toHaveLength(1);
    expect(mounted.filter((f) => f.hidden).map((f) => f.appearance)).toEqual([
      APPEARANCE_CATALOG[0].value,
      APPEARANCE_CATALOG[2].value,
    ]);
  });

  it("shows the current frame once it says it has painted, and remembers a warm neighbour", () => {
    const { rerender } = render(
      <AppearancePicker value={APPEARANCE_CATALOG[1].value} onChange={() => {}} />,
    );
    const current = () => document.querySelector('iframe[data-current="true"]') as HTMLIFrameElement;
    expect(current().className).toContain("opacity-0");
    ready(APPEARANCE_CATALOG[1].value);
    expect(current().className).toContain("opacity-100");
    // The hidden neighbour paints while it waits in the wings.
    ready(APPEARANCE_CATALOG[2].value);
    rerender(<AppearancePicker value={APPEARANCE_CATALOG[2].value} onChange={() => {}} />);
    expect(current().getAttribute("src")).toContain(`appearance=${APPEARANCE_CATALOG[2].value}`);
    expect(current().className).toContain("opacity-100");
  });

  it("steps with the arrows and the arrow keys", () => {
    const onChange = vi.fn();
    render(<AppearancePicker value={APPEARANCE_CATALOG[0].value} onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: "Next appearance" }));
    expect(onChange).toHaveBeenLastCalledWith(APPEARANCE_CATALOG[1].value);
    fireEvent.keyDown(screen.getByRole("group", { name: "Bar appearance" }), { key: "ArrowLeft" });
    expect(onChange).toHaveBeenLastCalledWith(APPEARANCE_CATALOG[APPEARANCE_CATALOG.length - 1].value);
  });
});

describe("AppearancePicker loading state", () => {
  it("fades the outgoing look out instead of hiding it at once", () => {
    const { rerender } = render(
      <AppearancePicker value={APPEARANCE_CATALOG[1].value} onChange={() => {}} />,
    );
    ready(APPEARANCE_CATALOG[1].value);
    rerender(<AppearancePicker value={APPEARANCE_CATALOG[2].value} onChange={() => {}} />);
    const outgoing = document.querySelector(
      `iframe[src*="appearance=${APPEARANCE_CATALOG[1].value}"]`,
    ) as HTMLIFrameElement;
    expect(outgoing.className).toContain("opacity-0");
    expect(outgoing.className).toContain("transition-opacity");
    expect(outgoing.className).not.toContain("invisible");
  });

  it("renders no appearance name or error text while a look is loading or failed", () => {
    vi.useFakeTimers();
    const entry = APPEARANCE_CATALOG[0];
    const { container } = render(<AppearancePicker value={entry.value} onChange={() => {}} />);
    const stage = () => container.querySelector("div.relative.h-\\[150px\\]") as HTMLElement;
    // No shimmer or placeholder: the stage is its plain ground until the look fades in.
    expect(stage().querySelector('[data-testid="appearance-preview-placeholder"]')).toBeNull();
    expect(stage().textContent).toBe("");
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(stage().textContent).toBe("");
    expect(stage().textContent).not.toContain("Preview unavailable");
    vi.useRealTimers();
  });
});
