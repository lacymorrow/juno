import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { QueryButton, ActionButton } from "@/components/ui/agent-actions";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

describe("QueryButton", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("routes the query through the unified submission pipeline", async () => {
    mockedInvoke.mockResolvedValue(undefined);

    render(<QueryButton query="Play my liked songs on Spotify" label="Play Liked Songs" />);
    fireEvent.click(screen.getByRole("button", { name: /play liked songs/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("dispatch_query", {
        query: "Play my liked songs on Spotify",
      });
    });
    expect(mockedInvoke).not.toHaveBeenCalledWith("submit_query", expect.anything());
  });

  it("leaves the loading state and re-enables after a successful submit", async () => {
    mockedInvoke.mockResolvedValue(undefined);

    render(<QueryButton query="Resume playback on Spotify" label="Resume" />);
    const button = screen.getByRole("button", { name: /resume/i });

    fireEvent.click(button);
    // Disabled while the submit is in flight
    expect(button).toBeDisabled();

    // Re-enabled as soon as the backend accepts the query
    await waitFor(() => expect(button).not.toBeDisabled());

    // Transient "sent" confirmation returns to idle
    await act(async () => {
      vi.advanceTimersByTime(1500);
    });
    expect(button).not.toBeDisabled();
    expect(button).toHaveTextContent("Resume");
  });

  it("recovers to idle after a failed submit", async () => {
    mockedInvoke.mockRejectedValue(new Error("UI Manager not initialized"));
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(<QueryButton query="Skip to next track" />);
    const button = screen.getByRole("button", { name: /skip to next track/i });

    fireEvent.click(button);
    await waitFor(() => expect(button).not.toBeDisabled());
    expect(errorSpy).toHaveBeenCalled();

    await act(async () => {
      vi.advanceTimersByTime(3000);
    });
    expect(button).not.toBeDisabled();

    errorSpy.mockRestore();
  });
});

describe("ActionButton", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
  });

  it("invokes whitelisted commands directly", async () => {
    mockedInvoke.mockResolvedValue(undefined);

    render(<ActionButton command="open_url" args={{ url: "https://example.com" }} label="Open" />);
    fireEvent.click(screen.getByRole("button", { name: /open/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("open_url", { url: "https://example.com" });
    });
  });

  it("turns non-whitelisted commands into a component query", async () => {
    mockedInvoke.mockResolvedValue(undefined);

    render(<ActionButton command="restart_spotify" label="Restart" />);
    fireEvent.click(screen.getByRole("button", { name: /restart/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("dispatch_query", {
        query: "restart spotify",
      });
    });
    expect(mockedInvoke).not.toHaveBeenCalledWith("restart_spotify", expect.anything());
  });
});
