import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ExamplePrompts } from "../ExamplePrompts";

// The dev-command drawer asks the backend whether this is a debug build.
vi.mock("@/lib", () => ({ isDevelopment: vi.fn(async () => false) }));

const SCREENSHOT_PROMPT = "Take a screenshot and open System Preferences";

describe("ExamplePrompts", () => {
  it("waits, disabled, behind one Connecting line until the backend is up, then sends on click", () => {
    const onPromptSelect = vi.fn();
    const { rerender } = render(
      <ExamplePrompts onPromptSelect={onPromptSelect} backendStatus="connecting" />,
    );

    const screenshot = () => screen.getByRole("button", { name: "Screenshot" });
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

  it("keeps the buttons live when the backend is in error and says so in one line", () => {
    const onPromptSelect = vi.fn();
    render(<ExamplePrompts onPromptSelect={onPromptSelect} backendStatus="error" />);

    // The click still lands the words in the input; the caller decides not to send.
    fireEvent.click(screen.getByRole("button", { name: "Screenshot" }));
    expect(onPromptSelect).toHaveBeenCalledWith(SCREENSHOT_PROMPT);
    expect(screen.getByTestId("example-prompts-error")).toBeInTheDocument();
    expect(screen.queryByTestId("example-prompts-connecting")).not.toBeInTheDocument();
  });
});
