import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
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

const system = (content: string, timestamp: number): ChatMessage => ({
  role: "system",
  content,
  timestamp,
});

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
