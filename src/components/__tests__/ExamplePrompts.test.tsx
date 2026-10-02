import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ExamplePrompts } from "../ExamplePrompts";
import { isDevelopment } from "@/lib";

const STARTERS = vi.hoisted(() => [
  { id: "screen", title: "What's on my screen?", prompt: "What's on my screen? Look at it and tell me what I'm working on." },
  { id: "chess", title: "Play chess with me", prompt: "Play chess with me. Open Chess and make the first move as white." },
]);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => STARTERS),
}));

// The dev-command drawer asks the backend whether this is a debug build.
vi.mock("@/lib", () => ({ isDevelopment: vi.fn(async () => false) }));

const SCREENSHOT_PROMPT = STARTERS[0].prompt;
const MOUSE_SQUARE_PROMPT =
  "Move your mouse in a perfect square pattern on the screen, then return to center";

describe("ExamplePrompts", () => {
  it("waits, disabled, behind one Connecting line until the backend is up, then sends on click", async () => {
    const onPromptSelect = vi.fn();
    const { rerender } = render(
      <ExamplePrompts onPromptSelect={onPromptSelect} backendStatus="connecting" />,
    );

    const screenshot = () => screen.getByRole("button", { name: "What's on my screen?" });
    await screen.findByRole("button", { name: "What's on my screen?" });
    expect(screenshot()).toBeDisabled();
    expect(screen.getByTestId("example-prompts-connecting")).toHaveTextContent("Connecting…");

    fireEvent.click(screenshot());
    expect(onPromptSelect).not.toHaveBeenCalled();

    rerender(<ExamplePrompts onPromptSelect={onPromptSelect} backendStatus="connected" />);

    expect(screenshot()).toBeEnabled();
    expect(screen.queryByTestId("example-prompts-connecting")).not.toBeInTheDocument();

    fireEvent.click(screenshot());
    expect(onPromptSelect).toHaveBeenCalledWith(SCREENSHOT_PROMPT);
  });

  it("gates the development test commands the same way as the production prompts", async () => {
    vi.mocked(isDevelopment).mockResolvedValueOnce(true);
    const onPromptSelect = vi.fn();
    const { rerender } = render(
      <ExamplePrompts onPromptSelect={onPromptSelect} backendStatus="connecting" />,
    );

    fireEvent.click(await screen.findByText("Development test commands"));
    const mouseSquare = () => screen.getByRole("button", { name: "Mouse Square" });
    expect(mouseSquare()).toBeDisabled();
    fireEvent.click(mouseSquare());
    expect(onPromptSelect).not.toHaveBeenCalled();

    rerender(<ExamplePrompts onPromptSelect={onPromptSelect} backendStatus="connected" />);

    expect(mouseSquare()).toBeEnabled();
    fireEvent.click(mouseSquare());
    expect(onPromptSelect).toHaveBeenCalledWith(MOUSE_SQUARE_PROMPT);
  });

  it("keeps the buttons live when the backend is in error and says so in one line", async () => {
    const onPromptSelect = vi.fn();
    render(<ExamplePrompts onPromptSelect={onPromptSelect} backendStatus="error" />);

    // The click still lands the words in the input; the caller decides not to send.
    fireEvent.click(await screen.findByRole("button", { name: "What's on my screen?" }));
    expect(onPromptSelect).toHaveBeenCalledWith(SCREENSHOT_PROMPT);
    expect(screen.getByTestId("example-prompts-error")).toBeInTheDocument();
    expect(screen.queryByTestId("example-prompts-connecting")).not.toBeInTheDocument();
  });
});
