import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { availableComponents } from "@/components/ui/jsx-message-renderer";

/**
 * The agent prompt and the renderer's registry have to agree: a tag the prompt
 * teaches that the registry lacks renders nothing, and a tag the registry
 * dropped must not still be taught. The prompt is Rust, so it is read as text.
 */
const prompt = readFileSync(
  resolve(process.cwd(), "src-tauri/src/agent/prompts/templates.rs"),
  "utf8",
);

/** Not components: the spoken channel and the collapsed rationale (lifted out before parsing). */
const NOT_COMPONENTS = new Set(["TTS", "Why"]);

/** Taught but never registered. Prompt change 6 in the audit (remove or register); tracked there. */
const KNOWN_GAPS = new Set(["SwatchPicker"]);

/** Cut from the registry: the demo kit and ActionButton. */
const CUT = ["Circle", "Rectangle", "Triangle", "ColorShowcase", "VisualDemo", "StatusCard", "ProgressBar", "ActionButton"];

function taughtTags(): Set<string> {
  const tags = new Set<string>();
  for (const m of prompt.matchAll(/<([A-Z][A-Za-z0-9]*)[\s/>]/g)) tags.add(m[1]);
  // The "AVAILABLE JSX COMPONENTS" lists name tags without angle brackets.
  for (const line of prompt.split("\n")) {
    const list = /^\*\*(Layout|Data Display|Actions|Icons)\*\*: (.+)$/.exec(line);
    if (!list) continue;
    for (const part of list[2].split(/,(?![^(]*\))/)) {
      const name = /^\s*([A-Z][A-Za-z0-9]*)/.exec(part)?.[1];
      if (name) tags.add(name);
    }
  }
  return tags;
}

describe("agent prompt and component registry", () => {
  it("registers every tag the prompt teaches", () => {
    const missing = [...taughtTags()].filter(
      (tag) => !NOT_COMPONENTS.has(tag) && !KNOWN_GAPS.has(tag) && !(tag in availableComponents),
    );
    expect(missing).toEqual([]);
  });

  it("no longer teaches, or registers, the cut components", () => {
    const taught = taughtTags();
    for (const tag of CUT) {
      expect(taught.has(tag), `${tag} is still in the prompt`).toBe(false);
      expect(tag in availableComponents, `${tag} is still registered`).toBe(false);
    }
  });

  it("keeps the shadcn parts and the effects registered", () => {
    for (const tag of [
      "Card", "Alert", "Badge", "Separator", "Tabs", "Tooltip", "Skeleton", "Button",
      "Table", "Accordion", "Progress", "Avatar",
      "GlowBadge", "ShimmerText", "PulseRing", "Confetti", "AnimatedDivider",
      "AnimatedCard", "AnimatedList", "AnimatedNumber", "AnimatedProgress",
      "Actions", "Stats", "Meter",
    ]) {
      expect(tag in availableComponents, `${tag} is missing`).toBe(true);
    }
  });
});
