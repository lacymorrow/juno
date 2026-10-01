import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { isDevelopment } from "@/lib";
import { ChatContainerV2 } from "../chat/ChatContainerV2";
import type { ChatMessage } from "@/types/chat";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-process", () => ({ exit: vi.fn() }));
vi.mock("@/hooks/useEventListener", () => ({ useEventListener: vi.fn() }));
vi.mock("@/lib", () => ({ isDevelopment: vi.fn(async () => false) }));

const CONNECTED = "Connected. Enter your query below.";

const user = (content: string, timestamp: number): ChatMessage => ({
  role: "user",
  content,
  timestamp,
});

const toolCall = (toolName: string, timestamp: number): ChatMessage => ({
  role: "tool_call_request",
  content: `Using tool: ${toolName}`,
  tool_name: toolName,
  tool_args: { action: "screenshot" },
  timestamp,
});

/** A capture that came back, as the tool row carries it. */
const capture = (base64: string, timestamp: number): ChatMessage => ({
  role: "tool_call_request",
  content: "Using tool: computer",
  tool_name: "computer",
  tool_args: { action: "screenshot" },
  screenshot_base64: base64,
  success: true,
  timestamp,
});

const pendingApproval = (timestamp: number): ChatMessage => ({
  role: "tool_call_request",
  content: "Run bash: rm -rf /tmp/scratch",
  tool_name: "bash",
  tool_args: { command: "rm -rf /tmp/scratch" },
  tool_id: "tool-7",
  approval_state: "pending",
  risk_level: "high",
  target_app: "Terminal",
  approval_timeout_seconds: 60,
  timestamp,
});

const system = (content: string, timestamp: number): ChatMessage => ({
  role: "system",
  content,
  timestamp,
});

/**
 * The rows the message list actually lays out.
 *
 * Load-bearing for the layer this fix picked: a row that renders `null` is
 * still a child of a `gap-6` column, so it keeps its 24px of space. Hiding has
 * to happen before the row is built, not inside it.
 */
const messageRows = (container: HTMLElement): Element[] => {
  const list = container.querySelector(".gap-6");
  if (!list) throw new Error("message list not found");
  return Array.from(list.children);
};

const renderChat = (conversation: ChatMessage[]) =>
  render(
    <ChatContainerV2
      conversation={conversation}
      copiedMessageId={null}
      onCopyResponse={vi.fn()}
      onShareResponse={vi.fn()}
      onExamplePromptSelect={vi.fn()}
    />,
  );

describe("ChatContainerV2 empty state", () => {
  it("keeps the example prompts up when only the app has spoken", () => {
    // The backend's "Connected" note used to count as a conversation and
    // replace the prompts the moment anything could actually be sent.
    renderChat([system(CONNECTED, 1_700_000_000_000)]);

    expect(screen.getByText("What can I help you with?")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Screenshot" })).toBeInTheDocument();
    expect(screen.queryByText(CONNECTED)).not.toBeInTheDocument();
  });

  it("shows the conversation, system notes included, once the person has sent something", () => {
    renderChat([
      system(CONNECTED, 1_700_000_000_000),
      { role: "user", content: "Open Safari", timestamp: 1_700_000_001_000 },
      system("Stopped by the person.", 1_700_000_002_000),
    ]);

    expect(screen.queryByText("What can I help you with?")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Screenshot" })).not.toBeInTheDocument();
    expect(screen.getByText("Open Safari")).toBeInTheDocument();
    expect(screen.getByText(CONNECTED)).toBeInTheDocument();
    expect(screen.getByText("Stopped by the person.")).toBeInTheDocument();
  });
});

describe("tool rows outside development", () => {
  beforeEach(() => {
    vi.mocked(isDevelopment).mockResolvedValue(false);
  });

  it("keeps the tool calls out of the transcript", async () => {
    const { container } = renderChat([
      user("Take a screenshot", 1_700_000_000_000),
      toolCall("computer", 1_700_000_001_000),
      {
        role: "assistant",
        content: "Here it is.",
        timestamp: 1_700_000_002_000,
      },
    ]);

    await waitFor(() => {
      expect(screen.queryByText(/Using tool: computer/)).not.toBeInTheDocument();
    });
    expect(screen.queryByText("computer")).not.toBeInTheDocument();
    expect(screen.getByText("Take a screenshot")).toBeInTheDocument();
    expect(screen.getByText("Here it is.")).toBeInTheDocument();
    // The turn still says what it spent, which is what replaces the rows.
    expect(screen.getByText(/1 action/)).toBeInTheDocument();
    // Two rows, not three with a hole in the middle.
    const rows = messageRows(container);
    expect(rows).toHaveLength(2);
    for (const row of rows) {
      expect(row.textContent?.trim()).not.toBe("");
    }
  });

  it("still asks before a risky action, in words, with both answers", async () => {
    // Hiding this row would leave the agent waiting forever on an answer the
    // person was never asked for.
    renderChat([
      user("Clean up the scratch folder", 1_700_000_000_000),
      pendingApproval(1_700_000_001_000),
    ]);

    await waitFor(() => {
      expect(screen.getByText("Juno needs your OK")).toBeInTheDocument();
    });
    expect(screen.getByText("Run bash: rm -rf /tmp/scratch")).toBeInTheDocument();
    // Both answers are on screen with nothing opened first: the question sits
    // at the top level, not inside the tool disclosure where it was buried.
    expect(screen.getByRole("button", { name: "Allow" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Don't allow" })).toBeInTheDocument();
    expect(screen.getByText("Terminal")).toBeInTheDocument();
    // The tool card around it is gone: no disclosure, no status badge, no
    // risk-level jargon, no JSON arguments.
    expect(screen.queryByText(/Awaiting Approval/)).not.toBeInTheDocument();
    expect(screen.queryByText(/High risk/)).not.toBeInTheDocument();
    expect(screen.queryByText(/"command"/)).not.toBeInTheDocument();
    expect(screen.queryByText("bash")).not.toBeInTheDocument();
  });

  it("says something is happening when the turn is nothing but hidden work", async () => {
    renderChat([
      user("Open Safari and read the page", 1_700_000_000_000),
      toolCall("computer", 1_700_000_001_000),
    ]);

    await waitFor(() => {
      expect(screen.getByText("Working...")).toBeInTheDocument();
    });
  });

  it("drops the activity line once the reply lands", async () => {
    renderChat([
      user("Open Safari and read the page", 1_700_000_000_000),
      toolCall("computer", 1_700_000_001_000),
      {
        role: "assistant",
        content: "The page is about otters.",
        timestamp: 1_700_000_002_000,
      },
    ]);

    await waitFor(() => {
      expect(screen.getByText("The page is about otters.")).toBeInTheDocument();
    });
    expect(screen.queryByText("Working...")).not.toBeInTheDocument();
  });
});

/**
 * The screenshots a turn took, in the transcript an ordinary person reads.
 *
 * This is the regression the feature exists for: the images only ever rode on
 * tool rows, every tool row is filtered out of the transcript outside
 * development, and the one renderer that sat on the reply bubble read a field
 * Rust never sent. Three renderers, nothing on screen. These tests fail if the
 * disclosure ever loses its way back to the reply.
 */
describe("screenshots outside development", () => {
  beforeEach(() => {
    vi.mocked(isDevelopment).mockResolvedValue(false);
  });

  it("offers what the agent saw on the reply, with the tool rows still hidden", async () => {
    renderChat([
      user("What is on my screen?", 1_700_000_000_000),
      capture("aGVsbG8=", 1_700_000_001_000),
      { role: "assistant", content: "Your inbox.", timestamp: 1_700_000_002_000 },
    ]);

    const trigger = await waitFor(() =>
      screen.getByRole("button", { name: "Saw the screen" }),
    );
    // The tool card it came from is still gone.
    expect(screen.queryByText("computer")).not.toBeInTheDocument();
    // Closed means no image in the DOM at all, so nothing is decoded until asked.
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(trigger).toHaveAttribute("aria-expanded", "false");

    fireEvent.click(trigger);

    expect(trigger).toHaveAttribute("aria-expanded", "true");
    const image = screen.getByRole("img", { name: "Screenshot from computer" });
    expect(image).toHaveAttribute("src", "data:image/png;base64,aGVsbG8=");

    fireEvent.click(trigger);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("counts several captures and stacks them in the order they happened", async () => {
    // The normal shape of a computer-use task. Rendering the first and dropping
    // the rest was the other way this could have gone wrong.
    renderChat([
      user("Reply to the top email", 1_700_000_000_000),
      capture("b25l", 1_700_000_001_000),
      capture("dHdv", 1_700_000_002_000),
      capture("dGhyZWU=", 1_700_000_003_000),
      { role: "assistant", content: "Sent.", timestamp: 1_700_000_004_000 },
    ]);

    const trigger = await waitFor(() =>
      screen.getByRole("button", { name: "Saw the screen 3 times" }),
    );
    fireEvent.click(trigger);

    const sources = screen.getAllByRole("img").map((img) => img.getAttribute("src"));
    expect(sources).toEqual([
      "data:image/png;base64,b25l",
      "data:image/png;base64,dHdv",
      "data:image/png;base64,dGhyZWU=",
    ]);
  });

  it("says a capture did not come back instead of offering an empty drawer", async () => {
    // A denied screen-recording permission looks exactly like this: the call
    // happened, no image arrived. There is nothing to open, so there is no
    // disclosure to open, and the label never claims Juno saw anything.
    renderChat([
      user("What is on my screen?", 1_700_000_000_000),
      {
        role: "tool_call_request",
        content: "Using tool: capture_screenshot",
        tool_name: "capture_screenshot",
        success: false,
        timestamp: 1_700_000_001_000,
      },
      { role: "assistant", content: "I could not see it.", timestamp: 1_700_000_002_000 },
    ]);

    await waitFor(() => {
      expect(
        screen.getByText("A screen capture did not come back."),
      ).toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: /Saw the screen/ })).not.toBeInTheDocument();
  });

  it("names the captures that went missing alongside the ones that did not", async () => {
    renderChat([
      user("Walk through setup", 1_700_000_000_000),
      capture("b25l", 1_700_000_001_000),
      {
        role: "tool_call_request",
        content: "Using tool: capture_screenshot",
        tool_name: "capture_screenshot",
        success: false,
        timestamp: 1_700_000_002_000,
      },
      { role: "assistant", content: "Done.", timestamp: 1_700_000_003_000 },
    ]);

    // The label counts what there is to look at, not what was attempted.
    const trigger = await waitFor(() =>
      screen.getByRole("button", { name: "Saw the screen" }),
    );
    fireEvent.click(trigger);

    expect(screen.getAllByRole("img")).toHaveLength(1);
    expect(
      screen.getByText("One more capture did not come back."),
    ).toBeInTheDocument();
  });

  it("falls back to words when the browser refuses the image", async () => {
    // A truncated or malformed data URI otherwise leaves the broken-image glyph
    // sitting in the transcript.
    renderChat([
      user("What is on my screen?", 1_700_000_000_000),
      capture("not-base64", 1_700_000_001_000),
      { role: "assistant", content: "Your inbox.", timestamp: 1_700_000_002_000 },
    ]);

    const trigger = await waitFor(() =>
      screen.getByRole("button", { name: "Saw the screen" }),
    );
    fireEvent.click(trigger);

    fireEvent.error(screen.getByRole("img"));

    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(
      screen.getByText("This capture could not be shown."),
    ).toBeInTheDocument();
  });

  it("stays out of a turn that took no captures", async () => {
    renderChat([
      user("What is the capital of Spain?", 1_700_000_000_000),
      { role: "assistant", content: "Madrid.", timestamp: 1_700_000_001_000 },
    ]);

    await waitFor(() => expect(screen.getByText("Madrid.")).toBeInTheDocument());
    expect(screen.queryByRole("button", { name: /Saw the screen/ })).not.toBeInTheDocument();
    expect(screen.queryByText(/did not come back/)).not.toBeInTheDocument();
  });

  it("keeps each turn's captures on its own reply", async () => {
    renderChat([
      user("First", 1_700_000_000_000),
      capture("Zmlyc3Q=", 1_700_000_001_000),
      { role: "assistant", content: "One.", timestamp: 1_700_000_002_000 },
      user("Second", 1_700_000_003_000),
      capture("c2Vjb25k", 1_700_000_004_000),
      capture("dGhpcmQ=", 1_700_000_005_000),
      { role: "assistant", content: "Two.", timestamp: 1_700_000_006_000 },
    ]);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Saw the screen" })).toBeInTheDocument();
    });
    expect(
      screen.getByRole("button", { name: "Saw the screen 2 times" }),
    ).toBeInTheDocument();
  });
});

describe("tool rows in development", () => {
  beforeEach(() => {
    vi.mocked(isDevelopment).mockResolvedValue(true);
  });

  it("shows the whole tool card, arguments and all", async () => {
    renderChat([
      user("Take a screenshot", 1_700_000_000_000),
      toolCall("computer", 1_700_000_001_000),
    ]);

    await waitFor(() => {
      expect(screen.getByText("computer")).toBeInTheDocument();
    });
    expect(screen.queryByText("Working...")).not.toBeInTheDocument();
  });

  it("keeps the tool card on an approval, with Approve and Deny", async () => {
    renderChat([
      user("Clean up the scratch folder", 1_700_000_000_000),
      pendingApproval(1_700_000_001_000),
    ]);

    // The dev card is a disclosure, so the buttons live one click in. That is
    // how it already was; production is the surface that had to be redrawn.
    const header = await waitFor(() =>
      screen.getByRole("button", { name: /Awaiting Approval/ }),
    );
    fireEvent.click(header);

    expect(screen.getByRole("button", { name: "Approve" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deny" })).toBeInTheDocument();
    expect(screen.queryByText("Juno needs your OK")).not.toBeInTheDocument();
  });
});
