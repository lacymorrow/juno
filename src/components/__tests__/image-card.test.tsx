/**
 * ImageCard (LAC-4204).
 *
 * The regression these guard: the card used to hand `~/Pictures/Juno/x.png`
 * straight to `convertFileSrc`, which percent-encodes the tilde instead of
 * expanding it, so the asset protocol looked for a directory named `~` and
 * every generated image rendered broken. The companion half of the fix is
 * `$PICTURE/Juno/**` in the asset scope in `tauri.conf.json`.
 */

import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { convertFileSrc } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { ImageCard } from "@/components/ui/agent-cards";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  // The real one percent-encodes the whole path; only the prefix matters here.
  convertFileSrc: vi.fn((p: string) => `asset://localhost/${encodeURIComponent(p)}`),
}));

vi.mock("@tauri-apps/api/path", () => ({
  homeDir: vi.fn(async () => "/Users/test/"),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  revealItemInDir: vi.fn(async () => undefined),
}));

const mockedConvert = vi.mocked(convertFileSrc);
const mockedReveal = vi.mocked(revealItemInDir);

beforeEach(() => {
  vi.clearAllMocks();
});

describe("ImageCard", () => {
  it("renders nothing without a path", () => {
    const { container } = render(<ImageCard caption="No path" />);
    expect(container.firstChild).toBeNull();
  });

  it("expands ~ to an absolute path before building the asset URL", async () => {
    render(<ImageCard path="~/Pictures/Juno/walls-in-sage.png" caption="Walls in Sage" />);

    await waitFor(() => {
      expect(mockedConvert).toHaveBeenCalledWith("/Users/test/Pictures/Juno/walls-in-sage.png");
    });
    const img = await screen.findByAltText("Walls in Sage");
    expect(img.getAttribute("src")).not.toContain("~");
  });

  it("leaves an already absolute path alone", async () => {
    render(<ImageCard path="/Users/test/Pictures/Juno/room.png" caption="Room" />);
    await waitFor(() => {
      expect(mockedConvert).toHaveBeenCalledWith("/Users/test/Pictures/Juno/room.png");
    });
  });

  it("expands ~ for the before pane too", async () => {
    render(
      <ImageCard
        path="~/Pictures/Juno/after.png"
        before="~/Pictures/Juno/before.png"
        caption="Before and after"
      />,
    );
    await waitFor(() => {
      expect(mockedConvert).toHaveBeenCalledWith("/Users/test/Pictures/Juno/before.png");
      expect(mockedConvert).toHaveBeenCalledWith("/Users/test/Pictures/Juno/after.png");
    });
    expect(await screen.findByAltText("Before")).toBeTruthy();
    expect(await screen.findByAltText("After: Before and after")).toBeTruthy();
  });

  it("reveals the expanded path in Finder", async () => {
    render(<ImageCard path="~/Pictures/Juno/walls-in-sage.png" caption="Walls in Sage" />);

    const button = await screen.findByText("Show in Finder");
    fireEvent.click(button);

    await waitFor(() => {
      expect(mockedReveal).toHaveBeenCalledWith("/Users/test/Pictures/Juno/walls-in-sage.png");
    });
  });

  it("leaves ~user alone rather than treating it as our home", async () => {
    render(<ImageCard path="~someone/Pictures/x.png" caption="Theirs" />);
    await waitFor(() => {
      expect(mockedConvert).toHaveBeenCalledWith("~someone/Pictures/x.png");
    });
    expect(mockedConvert).not.toHaveBeenCalledWith("/Users/testsomeone/Pictures/x.png");
  });

  it("drops to the single picture when the before pane cannot load", async () => {
    render(
      <ImageCard
        path="~/Pictures/Juno/after.png"
        before="~/Desktop/outside-the-scope.png"
        caption="Repainted"
      />,
    );

    const beforeImg = await screen.findByAltText("Before");
    fireEvent.error(beforeImg);

    await waitFor(() => {
      expect(screen.queryByAltText("Before")).toBeNull();
    });
    // The after image is still there, now as the whole picture.
    expect(screen.getByAltText("Repainted")).toBeTruthy();
  });

  it("says so when the file is gone instead of showing a broken image", async () => {
    render(<ImageCard path="~/Pictures/Juno/deleted.png" caption="Gone" />);

    const img = await screen.findByAltText("Gone");
    fireEvent.error(img);

    expect(await screen.findByText(/deleted\.png/)).toBeTruthy();
    // The actions survive, so there is still a way to go look.
    expect(screen.getByText("Show in Finder")).toBeTruthy();
    expect(screen.getByText("Open")).toBeTruthy();
  });
});
