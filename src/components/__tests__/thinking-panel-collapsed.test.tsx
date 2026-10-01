import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ChatMessageComponent } from "@/components/ChatMessageV2";
import type { ChatMessage } from "@/types/chat";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

const THOUGHT = "First I will open Safari, then I will read the page.";

const thinking = (content: string, isStreaming: boolean): ChatMessage => ({
  role: "thinking",
  content,
  isStreaming,
  messageId: "think-1",
  timestamp: 1_700_000_000_000,
});

const renderThinking = (msg: ChatMessage) =>
  render(
    <ChatMessageComponent
      msg={msg}
      index={0}
      copiedMessageId={null}
      onCopyResponse={vi.fn()}
      onShareResponse={vi.fn()}
    />,
  );

describe("the thinking panel", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: false });
    vi.setSystemTime(new Date("2026-10-01T12:00:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("stays shut through a whole stream, and reports what it cost", () => {
    // The panel used to throw itself open on every thinking block and shut a
    // second later, moving the transcript twice per turn.
    const { rerender } = renderThinking(thinking("", true));

    const trigger = screen.getByRole("button");
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByText("Thinking...")).toBeInTheDocument();

    // Chunks arrive. Nothing opens.
    act(() => {
      vi.advanceTimersByTime(1500);
    });
    rerender(
      <ChatMessageComponent
        msg={thinking(THOUGHT, true)}
        index={0}
        copiedMessageId={null}
        onCopyResponse={vi.fn()}
        onShareResponse={vi.fn()}
      />,
    );
    expect(screen.getByRole("button")).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText(THOUGHT)).not.toBeInTheDocument();

    // Three seconds in, the stream ends.
    act(() => {
      vi.advanceTimersByTime(1500);
    });
    rerender(
      <ChatMessageComponent
        msg={thinking(THOUGHT, false)}
        index={0}
        copiedMessageId={null}
        onCopyResponse={vi.fn()}
        onShareResponse={vi.fn()}
      />,
    );
    // Past the auto-close delay, which has nothing left to close.
    act(() => {
      vi.advanceTimersByTime(2000);
    });

    expect(screen.getByRole("button")).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText(THOUGHT)).not.toBeInTheDocument();
    // The duration effect runs whether the panel is open or not, which is what
    // keeps the collapsed row from reading "Thought for a few seconds" forever.
    expect(screen.getByText(/Thought for 3 seconds/)).toBeInTheDocument();
  });

  it("opens when the person opens it", () => {
    renderThinking(thinking(THOUGHT, false));

    const trigger = screen.getByRole("button");
    act(() => {
      trigger.click();
    });

    expect(screen.getByRole("button")).toHaveAttribute("aria-expanded", "true");
  });
});
