import { act, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

// The drag flag, as `useBarSnapWells` reports it.
let dragging = false;
const listeners = new Set<() => void>();
const setDragging = (next: boolean) =>
  act(() => {
    dragging = next;
    listeners.forEach((fn) => fn());
  });

vi.mock("@/hooks/useBarSnapWells", async () => {
  const { useSyncExternalStore } = await import("react");
  return {
    useBarDragging: () =>
      useSyncExternalStore(
        (fn: () => void) => {
          listeners.add(fn);
          return () => listeners.delete(fn);
        },
        () => dragging,
      ),
  };
});

import { PillTooltip, PILL_TOOLTIP_DELAY_MS } from "../PillTooltip";
import { TooltipProvider } from "@/components/ui/tooltip";

function Dot({ forcedOpen }: { forcedOpen: boolean }) {
  return (
    <TooltipProvider delayDuration={PILL_TOOLTIP_DELAY_MS}>
      <PillTooltip label="Network unavailable" forcedOpen={forcedOpen} side="bottom">
        <div data-testid="dot" />
      </PillTooltip>
    </TooltipProvider>
  );
}

const waitDelay = () =>
  act(async () => {
    await new Promise((r) => setTimeout(r, PILL_TOOLTIP_DELAY_MS + 20));
  });

describe("PillTooltip while the bar is dragged", () => {
  it("closes the moment a drag starts and stays closed until the cursor leaves", async () => {
    dragging = false;
    const { rerender } = render(<Dot forcedOpen />);
    await waitDelay();
    expect(screen.getByTestId("pill-tooltip")).toHaveTextContent("Network unavailable");

    setDragging(true);
    expect(screen.queryByTestId("pill-tooltip")).toBeNull();

    // The snap settles with the cursor still resting on the control: nothing
    // pops up under it.
    setDragging(false);
    await waitDelay();
    expect(screen.queryByTestId("pill-tooltip")).toBeNull();

    // Leaving and coming back is a fresh hover.
    rerender(<Dot forcedOpen={false} />);
    rerender(<Dot forcedOpen />);
    await waitDelay();
    expect(screen.getByTestId("pill-tooltip")).toHaveTextContent("Network unavailable");
  });

  it("never opens during a drag", async () => {
    dragging = true;
    render(<Dot forcedOpen />);
    await waitDelay();
    expect(screen.queryByTestId("pill-tooltip")).toBeNull();
    setDragging(false);
  });
});
