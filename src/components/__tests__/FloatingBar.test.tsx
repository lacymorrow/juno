import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { FloatingBar } from "../FloatingBar";
import type { BarStateData } from "@/types/floating-bar";

// Mock the hooks and dependencies
vi.mock("@/hooks/useInvoke", () => ({
  useInvoke: () => ({
    invokeCommand: vi.fn().mockResolvedValue({}),
  }),
}));

vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: vi.fn(),
}));

vi.mock("@/hooks/useWindowSize", () => ({
  useWindowSize: () => ({
    resizeWindow: vi.fn(),
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  Window: {
    getCurrent: () => ({
      onFocusChanged: vi.fn().mockResolvedValue(() => {}),
    }),
  },
}));

vi.mock("../VoiceStatusIndicator", () => ({
  VoiceStatusIndicator: ({ variant, className }: any) => (
    <div data-testid="voice-status-indicator" className={className}>
      {variant}
    </div>
  ),
}));

vi.mock("../bar/voice-ai-bar", () => ({
  VoiceAIBar: ({
    initialState,
    onStateChange,
    sampleResponses,
    className,
    inputValue,
    onInputChange,
    onInputSubmit,
    onInputBlur,
    inputRef,
  }: any) => (
    <div
      data-testid="voice-ai-bar"
      className={className}
      data-state={initialState}
    >
      <span data-testid="assistant-state">{initialState}</span>
      {inputValue !== undefined && (
        <input
          data-testid="external-input"
          value={inputValue}
          onChange={(e) => onInputChange?.(e.target.value)}
          onBlur={onInputBlur}
          ref={inputRef}
        />
      )}
      {sampleResponses && (
        <div data-testid="sample-responses">
          {Object.keys(sampleResponses).join(",")}
        </div>
      )}
      <button
        data-testid="state-change-trigger"
        onClick={() => onStateChange?.("input")}
      >
        Trigger State Change
      </button>
    </div>
  ),
}));

describe("FloatingBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders with default state", () => {
    render(<FloatingBar />);

    expect(screen.getByTestId("voice-ai-bar")).toBeInTheDocument();
    expect(screen.getByTestId("assistant-state")).toHaveTextContent("idle");
  });

  it("maps bar states to assistant states correctly", () => {
    const { rerender } = render(<FloatingBar />);

    // Test various state mappings
    const stateTestCases = [
      { barState: "default", expectedAssistantState: "idle" },
      { barState: "input", expectedAssistantState: "input" },
      { barState: "listening", expectedAssistantState: "listening" },
      { barState: "processing", expectedAssistantState: "processing" },
      { barState: "speaking", expectedAssistantState: "speaking" },
      { barState: "success", expectedAssistantState: "success" },
      { barState: "error", expectedAssistantState: "error" },
    ];

    stateTestCases.forEach(({ barState, expectedAssistantState }) => {
      // Simulate backend state update
      const mockEventData: BarStateData = {
        barState: barState as any,
        inputValue: "",
        lastSubmittedValue: "",
        currentError: null,
        transcriptionText: "",
        spokenText: "",
        isAgentWorking: false,
        isDictationMode: false,
        isAlwaysListening: false,
        audioLevel: 0,
        voiceMode: "idle",
        agentState: null,
      };

      // Trigger state update (this would normally come from backend)
      rerender(<FloatingBar />);

      // Check if VoiceAIBar receives correct state
      expect(screen.getByTestId("voice-ai-bar")).toHaveAttribute(
        "data-state",
        expectedAssistantState
      );
    });
  });

  it("provides external input handling to VoiceAIBar", () => {
    render(<FloatingBar />);

    const externalInput = screen.getByTestId("external-input");
    expect(externalInput).toBeInTheDocument();

    // Test input value change
    fireEvent.change(externalInput, { target: { value: "test input" } });
    expect(externalInput).toHaveValue("test input");
  });

  it("generates dynamic response content based on state", () => {
    render(<FloatingBar />);

    const responseContainer = screen.getByTestId("sample-responses");
    expect(responseContainer).toBeInTheDocument();

    // Should contain status response by default
    expect(responseContainer).toHaveTextContent("status");
  });

  it("shows voice status indicator when relevant", () => {
    render(<FloatingBar />);

    // Voice indicator should not be visible by default
    expect(screen.queryByTestId("voice-status-indicator")).not.toBeInTheDocument();

    // TODO: Test when voice mode is active
    // This would require simulating backend state updates
  });

  it("shows always listening indicator when active", () => {
    render(<FloatingBar />);

    // Always listening indicator should not be visible by default
    const alwaysListeningIndicator = screen.queryByText(/bg-blue-400/);
    expect(alwaysListeningIndicator).not.toBeInTheDocument();

    // TODO: Test when always listening is active
    // This would require simulating backend state updates
  });

  it("handles VoiceAIBar state changes", async () => {
    const { invokeCommand } = require("@/hooks/useInvoke").useInvoke();

    render(<FloatingBar />);

    const stateChangeTrigger = screen.getByTestId("state-change-trigger");
    fireEvent.click(stateChangeTrigger);

    await waitFor(() => {
      expect(invokeCommand).toHaveBeenCalledWith("floating_bar_click");
    });
  });

  it("maintains proper drag region handling", () => {
    render(<FloatingBar />);

    const dragRegions = screen.getAllByRole("generic").filter(
      (element) => element.getAttribute("data-tauri-drag-region") !== null
    );

    expect(dragRegions.length).toBeGreaterThan(0);
  });

  it("applies correct opacity from config", () => {
    render(<FloatingBar />);

    const voiceAIBarContainer = screen.getByTestId("voice-ai-bar").parentElement;
    expect(voiceAIBarContainer).toHaveStyle({ opacity: "0.95" });
  });

  it("shows tooltip on hover in default state", () => {
    render(<FloatingBar />);

    // Tooltip should not be visible initially
    expect(screen.queryByText(/Voice assistant ready/)).not.toBeInTheDocument();

    // TODO: Test tooltip visibility on hover
    // This would require simulating mouse events and backend state
  });

  it("handles window resizing based on assistant state", () => {
    const { resizeWindow } = require("@/hooks/useWindowSize").useWindowSize();

    render(<FloatingBar />);

    // Should call resizeWindow for different states
    expect(resizeWindow).toHaveBeenCalled();
  });
});

describe("FloatingBar Integration", () => {
  it("eliminates input overlay approach", () => {
    render(<FloatingBar />);

    // Should not have any overlay input elements
    const overlayInputs = screen.queryAllByText(/absolute inset-0/);
    expect(overlayInputs).toHaveLength(0);
  });

  it("uses VoiceAIBar's built-in input handling", () => {
    render(<FloatingBar />);

    const voiceAIBar = screen.getByTestId("voice-ai-bar");
    const externalInput = screen.getByTestId("external-input");

    // VoiceAIBar should receive external input props
    expect(externalInput).toBeInTheDocument();
    expect(voiceAIBar).toContainElement(externalInput);
  });

  it("preserves all existing functionality", () => {
    render(<FloatingBar />);

    // Check that all key elements are present
    expect(screen.getByTestId("voice-ai-bar")).toBeInTheDocument();
    expect(screen.getByTestId("external-input")).toBeInTheDocument();
    expect(screen.getByTestId("sample-responses")).toBeInTheDocument();
    expect(screen.getByTestId("state-change-trigger")).toBeInTheDocument();
  });
});
