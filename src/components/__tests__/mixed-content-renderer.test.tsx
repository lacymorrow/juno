import { describe, it, expect, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import {
  JSX_COMPONENT_NAMES,
  splitMixedContent,
  hasMixedContent,
} from "@/components/ui/mixed-content-renderer";
import { availableComponents } from "@/components/ui/jsx-message-renderer";

describe("mixed-content JSX detection", () => {
  it("segments the exact NowPlayingCard response the agent emits", () => {
    const content = `<TTS>Paused.</TTS>

<NowPlayingCard app="Spotify" />

**Why AppleScript here:** Spotify exposes a scripting dictionary.

The card above is live-bound to the player.`;

    expect(hasMixedContent(content)).toBe(true);
    const segments = splitMixedContent(content);
    const jsx = segments.filter((s) => s.type === "jsx");
    expect(jsx).toHaveLength(1);
    expect(jsx[0].content.trim()).toBe('<NowPlayingCard app="Spotify" />');
  });

  it("only lists names the JSX renderer can actually render", () => {
    const renderable = new Set(Object.keys(availableComponents));
    for (const name of JSX_COMPONENT_NAMES) {
      expect(renderable.has(name), `${name} is detected but not renderable`).toBe(true);
    }
  });

  it("detects every renderable Card and Button component", () => {
    // A component that renders but is never detected as JSX is silently
    // dropped from the chat — exactly how NowPlayingCard went missing.
    const detected = new Set<string>(JSX_COMPONENT_NAMES);
    const missing = Object.keys(availableComponents).filter(
      (name) => /(Card|Button)$/.test(name) && !detected.has(name),
    );
    expect(missing).toEqual([]);
  });
});
