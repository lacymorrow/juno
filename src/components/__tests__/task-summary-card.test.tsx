import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TaskSummaryCard } from "@/components/ui/agent-cards";
import { splitMixedContent } from "@/components/ui/mixed-content-renderer";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/path", () => ({ homeDir: vi.fn().mockResolvedValue("/Users/x") }));

const tasks = [
  { label: "23 images moved", done: true },
  { label: "45 documents sorted", done: true, detail: "PDF, DOCX" },
  { label: "15 videos categorized", done: false },
];

describe("TaskSummaryCard", () => {
  it("says 'N of M' and shows a meter while work is pending", () => {
    render(<TaskSummaryCard title="Organized Downloads" tasks={tasks} />);
    expect(screen.getByText("2 of 3")).toBeInTheDocument();
    expect(screen.getByRole("progressbar")).toBeInTheDocument();
  });

  it("says Done and drops the meter when everything is done", () => {
    render(<TaskSummaryCard title="Organized Downloads" tasks={tasks.map((t) => ({ ...t, done: true }))} />);
    expect(screen.getByText("Done")).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });

  it("never strikes through a finished row, and keeps its detail line", () => {
    const { container } = render(<TaskSummaryCard tasks={tasks} />);
    expect(container.innerHTML).not.toMatch(/line-through|purple|green/);
    expect(screen.getByText("PDF, DOCX")).toBeInTheDocument();
  });

  it("puts the actions in the footer, primary ones last, and keeps only one primary", () => {
    render(
      <TaskSummaryCard
        tasks={tasks}
        actions={[
          { open: "~/Downloads", label: "Open Downloads", primary: true },
          { query: "Clean duplicates", label: "Clean duplicates" },
          { copy: "x", label: "Copy path", primary: true },
        ]}
      />,
    );
    const buttons = screen.getAllByRole("button");
    expect(buttons.map((b) => b.textContent?.replace("Copied", ""))).toEqual(["Clean duplicates", "Open Downloads", "Copy path"]);
    expect(buttons.filter((b) => b.getAttribute("data-variant") === "default")).toHaveLength(1);
    expect(buttons[0].closest('[data-slot="card-footer"]')).not.toBeNull();
  });

  it("is one quiet line when there is nothing to do", () => {
    render(<TaskSummaryCard tasks={[]} />);
    expect(screen.getByText("Nothing to do.")).toBeInTheDocument();
  });
});

describe("splitMixedContent: lowercase wrapper around a component", () => {
  it("keeps a div that holds buttons as one block", () => {
    const md = 'Done.\n\n<div className="flex gap-2 mt-2">\n  <OpenButton path="~/x" label="Open" />\n  <QueryButton query="q" label="Q" />\n</div>\n\nBye.';
    const segs = splitMixedContent(md, false);
    expect(segs.map((s) => s.type)).toEqual(["text", "jsx", "text"]);
    expect(segs[1].type === "jsx" && segs[1].content.startsWith("<div")).toBe(true);
    expect(segs[1].type === "jsx" && segs[1].content.endsWith("</div>")).toBe(true);
  });

  it("leaves a div with no component in it as prose", () => {
    const segs = splitMixedContent('Use <div className="a"> sparingly.\n\n<OpenButton path="~/x" />', false);
    expect(segs.map((s) => s.type)).toEqual(["text", "jsx"]);
  });
});
