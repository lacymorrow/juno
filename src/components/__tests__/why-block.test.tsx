import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { WhyBlock } from "@/components/ui/why-block";
import { MixedContentRenderer } from "@/components/ui/mixed-content-renderer";

describe("WhyBlock", () => {
  it("is collapsed by default and only shows the rationale when opened", () => {
    render(<WhyBlock>Spotify was already running, so `osascript` hit it directly.</WhyBlock>);

    const trigger = screen.getByRole("button", { name: /why i did it this way/i });
    expect(screen.queryByText(/already running/)).toBeNull();

    fireEvent.click(trigger);
    expect(screen.getByText(/already running/)).toBeTruthy();
  });

  it("renders nothing when there is no rationale", () => {
    const { container } = render(<WhyBlock>{"   "}</WhyBlock>);
    expect(container.innerHTML).toBe("");
  });
});

describe("MixedContentRenderer with rationale", () => {
  it("hides a <Why> block behind the dropdown and keeps the outcome visible", () => {
    const content = `Playing.

<Why>
No screenshot, no coordinate guessing, no stolen focus.
</Why>`;
    render(<MixedContentRenderer content={content} />);

    expect(screen.getByText("Playing.")).toBeTruthy();
    expect(screen.queryByText(/no coordinate guessing/i)).toBeNull();
    expect(screen.getByRole("button", { name: /why i did it this way/i })).toBeTruthy();
  });

  it("collapses the legacy bold 'Why X instead of Y:' paragraph the same way", () => {
    const content = `The card above is live-bound to the player.

**Why AppleScript instead of clicking:** Spotify was already running, so \`osascript\` hits the app directly.`;
    render(<MixedContentRenderer content={content} />);

    expect(screen.getByText(/live-bound to the player/)).toBeTruthy();
    expect(screen.queryByText(/already running/)).toBeNull();
    expect(screen.getByRole("button", { name: /why i did it this way/i })).toBeTruthy();
  });
});
