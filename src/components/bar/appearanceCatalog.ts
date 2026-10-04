import { UI } from "@/lib/constants.generated";
import type { BarAppearance } from "./barAppearance";

/**
 * The bar appearances as a person meets them: a name and one line, in the
 * order the picker walks through them. Values are the Rust enum; names and
 * descriptors are the only thing the settings window shows. Keep the words
 * honest against the preview: they describe what the look does, not how it is
 * built.
 */
export interface AppearanceEntry {
  value: BarAppearance;
  name: string;
  descriptor: string;
}

const ALL_APPEARANCES: readonly AppearanceEntry[] = [
  {
    value: UI.BAR_APPEARANCES_FLOATING,
    name: "Pill",
    descriptor: "A small pill that opens when you speak. The default.",
  },
  {
    value: UI.BAR_APPEARANCES_PERSONA,
    name: "Avatar",
    descriptor: "A character you talk to. It leans in, thinks, and talks back with its mouth moving.",
  },
  {
    value: UI.BAR_APPEARANCES_DYNAMIC,
    name: "Island",
    descriptor: "One small shape that grows to hold the answer, then settles back.",
  },
  {
    value: UI.BAR_APPEARANCES_APP,
    name: "Bar",
    descriptor: "A wide strip that tells each turn step by step, left to right.",
  },
  {
    value: UI.BAR_APPEARANCES_VOICE_AI,
    name: "Studio",
    descriptor: "A light recording studio: a real waveform, a teleprompter, and the answer as a script.",
  },
  {
    value: UI.BAR_APPEARANCES_SHADER_ORB,
    name: "Orb",
    descriptor: "One orb of light. Colour and motion say the whole turn; words only when Juno needs you.",
  },
  {
    value: UI.BAR_APPEARANCES_ORB,
    name: "Presence",
    descriptor: "A presence that tells you everything by how it moves, with subtitles beneath.",
  },
  {
    value: UI.BAR_APPEARANCES_REACT_ORB,
    name: "Halo",
    descriptor: "A ring that fills as you speak and measures what Juno does.",
  },
];

/**
 * What the picker and onboarding offer. Hidden looks (the Rust list
 * `bar_appearances::HIDDEN`, the one place to bring one back) keep their code
 * and entries above but are not shown.
 */
export const APPEARANCE_CATALOG: readonly AppearanceEntry[] = ALL_APPEARANCES.filter(
  (entry) => !(UI.BAR_APPEARANCES_HIDDEN as readonly string[]).includes(entry.value),
);

/** The catalog entry for a stored value, falling back to the default look. */
export function appearanceEntry(value: string | null | undefined): AppearanceEntry {
  return (
    ALL_APPEARANCES.find((entry) => entry.value === value) ??
    ALL_APPEARANCES.find((entry) => entry.value === UI.BAR_APPEARANCES_DEFAULT)!
  );
}

/** Route the settings window frames to preview one appearance. */
export const APPEARANCE_PREVIEW_PATH = "/__bar-preview";

export function appearancePreviewUrl(
  value: BarAppearance,
  options: { reducedMotion?: boolean } = {},
): string {
  const params = new URLSearchParams({ appearance: value });
  if (options.reducedMotion) params.set("motion", "reduced");
  return `${APPEARANCE_PREVIEW_PATH}?${params.toString()}`;
}

/** Every look, hidden ones included, for the dev harness and tests. */
export const ALL_APPEARANCE_ENTRIES = ALL_APPEARANCES;
