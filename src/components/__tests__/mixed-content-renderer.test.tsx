import { describe, it, expect, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { splitMixedContent, hasMixedContent } from "@/components/ui/mixed-content-renderer";

describe("mixed-content JSX detection (no allowlist)", () => {
  it("segments the exact NowPlayingCard response the agent emits", () => {
    const content = `<TTS>Paused.</TTS>

<NowPlayingCard app="Spotify" />

**Why AppleScript here:** Spotify exposes a scripting dictionary.

The card above is live-bound to the player.`;

    expect(hasMixedContent(content)).toBe(true);
    const jsx = splitMixedContent(content).filter((s) => s.type === "jsx");
    expect(jsx).toHaveLength(1);
    expect(jsx[0].content.trim()).toBe('<NowPlayingCard app="Spotify" />');
  });

  it("hands a well-formed tag it has never heard of to the renderer instead of stripping it", () => {
    const content = "Here you go:\n<BrandNewThing tone=\"warm\">hello</BrandNewThing>\nDone.";
    const segments = splitMixedContent(content);
    expect(segments.map((s) => s.type)).toEqual(["text", "jsx", "text"]);
    expect(segments[1].content).toBe('<BrandNewThing tone="warm">hello</BrandNewThing>');
  });

  it("never treats the spoken channel as a component", () => {
    expect(hasMixedContent("<TTS>Paused.</TTS>")).toBe(false);
    expect(splitMixedContent("<TTS>Paused.</TTS> ok").every((s) => s.type === "text")).toBe(true);
  });

  it("leaves prose that only looks like a tag as text once streaming is over", () => {
    const content = "Use Vec<String> or Option<T> here.";
    expect(splitMixedContent(content, false).every((s) => s.type === "text")).toBe(true);
  });

  it("ignores lowercase html and tags inside code fences", () => {
    expect(hasMixedContent("<div>plain html</div>")).toBe(false);
    const fenced = "```jsx\n<Card>inside a fence</Card>\n```\nafter";
    expect(splitMixedContent(fenced).every((s) => s.type === "text")).toBe(true);
  });
});

describe("method rationale segments", () => {
  it("lifts <Why>…</Why> out as a collapsed rationale segment, never JSX", () => {
    const content = `<TTS>Playing.</TTS>

<NowPlayingCard app="Spotify" />

<Why>
Spotify was running, so \`osascript -e 'tell application "Spotify" to play'\` hit it directly — {no braces trouble} and a > sign are fine here.
</Why>`;
    const segments = splitMixedContent(content);
    expect(segments.map((s) => s.type)).toEqual(["text", "jsx", "why"]);
    expect(segments[2].content).toContain("{no braces trouble}");
    expect(segments[2].content).not.toContain("<Why>");
  });

  it("keeps a half-arrived <Why> collapsed while streaming and after the stream ends", () => {
    const partial = "Done.\n\n<Why>\nBecause the app was already";
    expect(splitMixedContent(partial, true).map((s) => s.type)).toEqual(["text", "why"]);
    expect(splitMixedContent(partial, false).map((s) => s.type)).toEqual(["text", "why"]);
  });

  it("recognises the bold 'Why … instead of …:' paragraph as rationale", () => {
    const content = `<NowPlayingCard app="Spotify" />

**Why AppleScript instead of clicking:** Spotify was already running.

The card above is live-bound.`;
    const segments = splitMixedContent(content);
    expect(segments.map((s) => s.type)).toEqual(["jsx", "why", "text"]);
    expect(segments[1].content.startsWith("**Why AppleScript")).toBe(true);
    expect(hasMixedContent("**Why AppleScript here:** it is fastest.")).toBe(true);
  });

  it("leaves an answer to the user's own 'why' question visible", () => {
    const content = "**Why it failed:** the folder is read-only.\n\nFix the permissions first.";
    expect(splitMixedContent(content).map((s) => s.type)).toEqual(["text"]);
    expect(hasMixedContent(content)).toBe(false);
  });
});
