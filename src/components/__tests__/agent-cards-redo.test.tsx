import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MixedContentRenderer } from "@/components/ui/mixed-content-renderer";
import { filterAgentClassName } from "@/lib/agent-classnames";
import { describeFallback } from "@/lib/jsx-utils";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), convertFileSrc: (p: string) => p }));
vi.mock("@tauri-apps/api/path", () => ({ homeDir: async () => "/Users/me/" }));

const show = (jsx: string) => render(<MixedContentRenderer content={jsx} />);

describe("the colour-class filter", () => {
  it("drops colour, gradients, uppercase and tracking, and keeps layout", () => {
    expect(
      filterAgentClassName(
        "flex gap-2 text-green-700 dark:bg-yellow-950/40 bg-gradient-to-br from-amber-50 uppercase tracking-wide text-[13px] mt-2 text-muted-foreground border-t border-red-500/40 text-white",
      ),
    ).toBe("flex gap-2 text-[13px] mt-2 text-muted-foreground border-t");
  });

  it("is applied to the agent's html and to icons", () => {
    const { container } = show('<div className="p-2 text-red-600 bg-green-50">Hi <CheckCircle className="text-green-500 size-6" /></div>');
    const div = container.querySelector("div.p-2");
    expect(div).not.toBeNull();
    expect(div?.className).not.toMatch(/red|green/);
    const icon = container.querySelector("svg");
    expect(icon?.getAttribute("class")).toMatch(/size-6/);
    expect(icon?.getAttribute("class")).not.toMatch(/green/);
  });
});

describe("describeFallback", () => {
  it("names the card and keeps its labels", () => {
    expect(
      describeFallback('<TaskSummaryCard title="Organized Downloads" tasks={[{label: "23 images moved", done: true}, {label: "45 documents sorted"}'),
    ).toBe("Organized Downloads: 23 images moved, 45 documents sorted");
  });
  it("is empty when there are no words", () => {
    expect(describeFallback("<WeatherCard temp")).toBe("");
  });
});

describe("the redone cards", () => {
  it("WeatherCard draws only what it was given", () => {
    show('<WeatherCard location="Boone" condition="snow" />');
    expect(screen.getByText("Boone")).toBeInTheDocument();
    expect(screen.getByText("Snow")).toBeInTheDocument();
    expect(screen.queryByText(/humidity|wind/i)).toBeNull();
  });

  it("WeatherCard says so when it has nothing", () => {
    show("<WeatherCard />");
    expect(screen.getByText("No weather to show.")).toBeInTheDocument();
  });

  it("FileListCard shows eight rows and a Show all button, and an empty line", () => {
    const files = Array.from({ length: 12 }, (_, i) => `{name: "f${i}.txt", size: "1 KB"}`).join(",");
    const { unmount } = show(`<FileListCard title="Desktop" path="~/Desktop" totalCount={12} files={[${files}]} />`);
    expect(screen.getAllByRole("listitem")).toHaveLength(8);
    expect(screen.getByRole("button", { name: "Show all 12" })).toBeInTheDocument();
    unmount();
    show('<FileListCard title="Downloads" files={[]} totalCount={0} />');
    expect(screen.getByText("Empty.")).toBeInTheDocument();
  });

  it("MetricList colours a row only when the agent says warning, and the alias still works", () => {
    const { container } = show(
      '<SystemStatusCard hostname="MacBook Pro" uptime="3d 12h" metrics={[{label: "CPU", value: 95}, {label: "Disk", value: 94, tone: "warning"}]} />',
    );
    expect(screen.getByText("MacBook Pro")).toBeInTheDocument();
    expect(screen.getByText("Up 3d 12h")).toBeInTheDocument();
    const fills = container.querySelectorAll('[data-slot="progress-indicator"]');
    expect(fills).toHaveLength(2);
    expect(fills[0].className).toMatch(/bg-primary/);
    expect(fills[1].className).toMatch(/system-orange/);
  });

  it("ComparisonCard keeps its title when options are not an array", () => {
    show('<ComparisonCard title="Oops" options="not an array" />');
    expect(screen.getByText("Oops")).toBeInTheDocument();
    expect(screen.getByText("Nothing to compare.")).toBeInTheDocument();
  });

  it("ComparisonCard marks the pick with a word and the rating for assistive tech", () => {
    show('<ComparisonCard title="A vs B" options={[{name: "A", rating: 4, recommended: true, pros: ["Fast"]}, {name: "B", cons: ["Slow"]}]} />');
    expect(screen.getByText("Recommended")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Rated 4 of 5" })).toBeInTheDocument();
  });

  it("LinkCard shows the domain once, with an Open button", () => {
    show('<LinkCard url="https://www.junebug.ai/" />');
    expect(screen.getAllByText("junebug.ai")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Open" })).toBeInTheDocument();
  });

  it("LinkCard with nothing renders nothing", () => {
    const { container } = show("<LinkCard />");
    expect(container.querySelector('[data-slot="card"]')).toBeNull();
  });

  it("MiniChart is a labelled image, and says so when empty", () => {
    const { unmount } = show('<MiniChart data={[3, 5]} labels={["a", "b"]} />');
    expect(screen.getByRole("img", { name: "Bar chart: a 3, b 5" })).toBeInTheDocument();
    unmount();
    show("<MiniChart data={[]} />");
    expect(screen.getByText("No data.")).toBeInTheDocument();
  });

  it("the skeleton is a card, not a slab", () => {
    render(<MixedContentRenderer content={'<TaskSummaryCard title="Organized Downloads" tasks={[{label: "23 imag'} isStreaming />);
    expect(document.querySelector('[data-testid="component-skeleton"] [data-slot="card"]')).not.toBeNull();
  });

  it("a card that fails to close keeps its name and words", () => {
    show('<TaskSummaryCard title="Organized Downloads" tasks={[{label: "23 images moved", done: true}');
    expect(screen.getByTestId("component-fallback")).toHaveTextContent("Organized Downloads: 23 images moved");
  });
});
