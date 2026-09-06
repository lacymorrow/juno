import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { NowPlayingCard, type MediaState } from "@/components/ui/agent-cards";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

const playing: MediaState = {
  app: "Spotify",
  running: true,
  state: "playing",
  track: "Houdini",
  artist: "Foster The People",
  album: "Supermodel",
  position_secs: 42,
  duration_secs: 203,
  artwork_url: "https://i.scdn.co/image/abc",
};

const paused: MediaState = { ...playing, state: "paused" };

const notRunning: MediaState = {
  app: "Spotify",
  running: false,
  state: "not_running",
  track: null,
  artist: null,
  album: null,
  position_secs: null,
  duration_secs: null,
  artwork_url: null,
};

function mockBackend(initial: MediaState) {
  let current = initial;
  mockedInvoke.mockImplementation(async (cmd, args) => {
    if (cmd === "media_get_state") return current;
    if (cmd === "media_control") {
      const action = (args as { action: string }).action;
      if (action === "pause") current = { ...current, state: "paused" };
      if (action === "play") current = { ...current, state: "playing" };
      if (action === "next") current = { ...current, track: "Next Song", position_secs: 0 };
      return current;
    }
    if (cmd === "open_application") return undefined;
    throw new Error(`unexpected command ${String(cmd)}`);
  });
  return { get current() { return current; } };
}

describe("NowPlayingCard", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("reads the real player state on mount and shows the track", async () => {
    mockBackend(playing);
    render(<NowPlayingCard app="Spotify" />);

    await waitFor(() => expect(screen.getByText("Houdini")).toBeInTheDocument());
    expect(mockedInvoke).toHaveBeenCalledWith("media_get_state", { app: "Spotify" });
    expect(screen.getByText("Playing")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /pause/i })).toBeInTheDocument();
    expect(screen.getByText("0:42")).toBeInTheDocument();
    expect(screen.getByText("3:23")).toBeInTheDocument();
  });

  it("pauses through media_control and flips the button to Play from the returned state", async () => {
    mockBackend(playing);
    render(<NowPlayingCard app="Spotify" />);
    const pauseButton = await screen.findByRole("button", { name: /pause/i });

    fireEvent.click(pauseButton);

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("media_control", {
        app: "Spotify",
        action: "pause",
      });
    });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /^play$/i })).not.toBeDisabled();
    });
    expect(screen.getByText("Paused")).toBeInTheDocument();
    expect(mockedInvoke).not.toHaveBeenCalledWith("dispatch_query", expect.anything());
  });

  it("re-polls and reflects a change made outside the card", async () => {
    const backend = mockBackend(paused);
    render(<NowPlayingCard app="Spotify" pollMs={500} />);
    await screen.findByText("Paused");

    // Someone presses play in Spotify itself.
    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "media_get_state") return playing;
      return backend.current;
    });

    await act(async () => {
      vi.advanceTimersByTime(600);
    });
    await waitFor(() => expect(screen.getByText("Playing")).toBeInTheDocument());
  });

  it("shows a not-running state with an open button and disables transport", async () => {
    mockBackend(notRunning);
    render(<NowPlayingCard app="Spotify" />);

    await screen.findByText(/isn't running/i);
    expect(screen.getByText("Not running")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /pause/i })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /open spotify/i }));
    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("open_application", { appName: "Spotify" });
    });
  });

  it("surfaces a backend error instead of hanging in a loading state", async () => {
    mockBackend(playing);
    render(<NowPlayingCard app="Spotify" />);
    const nextButton = await screen.findByRole("button", { name: /next track/i });

    mockedInvoke.mockImplementation(async (cmd) => {
      if (cmd === "media_control") throw new Error("Spotify is not running");
      return playing;
    });
    fireEvent.click(nextButton);

    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent(/not running/i));
    await waitFor(() => expect(nextButton).not.toBeDisabled());
  });

  it("falls back to the music icon when the artwork fails to load", async () => {
    mockBackend(playing);
    const { container } = render(<NowPlayingCard app="Spotify" />);
    const img = await waitFor(() => {
      const el = container.querySelector("img");
      if (!el) throw new Error("no img yet");
      return el;
    });
    fireEvent.error(img);
    await waitFor(() => expect(container.querySelector("img")).toBeNull());
    expect(screen.getByText("Houdini")).toBeInTheDocument();
  });

  it("names Apple Music in the UI but addresses the app as Music", async () => {
    mockBackend({ ...notRunning, app: "Music" });
    render(<NowPlayingCard app="Music" />);

    await screen.findByText("Apple Music isn't running.");
    expect(screen.getByText(/now playing · apple music/i)).toBeInTheDocument();
    expect(mockedInvoke).toHaveBeenCalledWith("media_get_state", { app: "Music" });

    fireEvent.click(screen.getByRole("button", { name: /open apple music/i }));
    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("open_application", { appName: "Music" });
    });
  });

  it("renders exported Apple Music artwork from the asset protocol URL", async () => {
    const assetUrl =
      "asset://localhost/%2FUsers%2Fme%2FLibrary%2FCaches%2Fai.junebug%2Fmedia-artwork%2F0123abcd.jpg";
    mockBackend({
      ...playing,
      app: "Music",
      track: "All I Really Want",
      artist: "Alanis Morissette",
      album: "Jagged Little Pill",
      artwork_url: assetUrl,
    });
    const { container } = render(<NowPlayingCard app="Music" />);

    await screen.findByText("All I Really Want");
    const img = container.querySelector("img");
    expect(img).not.toBeNull();
    expect(img?.getAttribute("src")).toBe(assetUrl);
    expect(screen.getByText(/now playing · apple music/i)).toBeInTheDocument();
  });

  it("stops polling on unmount", async () => {
    mockBackend(playing);
    const { unmount } = render(<NowPlayingCard app="Spotify" pollMs={200} />);
    await screen.findByText("Houdini");
    unmount();
    const callsAfterUnmount = mockedInvoke.mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(1000);
    });
    expect(mockedInvoke.mock.calls.length).toBe(callsAfterUnmount);
  });
});
