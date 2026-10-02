import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { APPEARANCE_CATALOG } from "@/components/bar/appearanceCatalog";

/**
 * # The Escape contract, pinned to the source
 *
 * Escape has one behaviour (`src/lib/barEscape.ts`) reached through one hook
 * (`useEscapeToIdle`). The table test next to that file proves the behaviour
 * is right for every bar state; this file proves every appearance actually
 * uses it, which is the half that rotted last time.
 *
 * Eight looks each grew their own Escape ladder off the same three rungs, each
 * naming a different set of local flags, so a look sitting in a state its own
 * ladder forgot swallowed the key. That is this codebase's recurring defect:
 * a control that names a behaviour it is not wired to. The fix is only worth
 * anything if the ninth appearance cannot repeat it, so:
 *
 * - a look that hand-rolls `e.key === "Escape"` fails here;
 * - a look that forgets the hook fails here;
 * - an appearance added to the catalog and not to this map fails here;
 * - a look that passes a constant where its own state belongs fails here.
 *
 * The six fields themselves are enforced by the compiler: `BarEscapeState` has
 * no optional members, so a look that omits one does not build.
 */
const COMPONENT_FOR_APPEARANCE: Record<string, string> = {
  floating: "src/components/FloatingBar.tsx",
  app: "src/components/bar/app-bar.tsx",
  voice_ai: "src/components/bar/voice-ai-bar.tsx",
  dynamic: "src/components/bar/island/IslandBar.tsx",
  shader_orb: "src/components/bar/shader-orb-bar.tsx",
  orb: "src/components/bar/elevenlabs-orb-bar.tsx",
  react_orb: "src/components/bar/halo/HaloBar.tsx",
  persona: "src/components/bar/persona-bar.tsx",
};

/** Every field one appearance has to hand the hook. */
const REQUIRED_FIELDS = [
  "barState",
  "working",
  "overlayOpen",
  "composerOpen",
  "popupOpen",
  "collapse",
  "report",
] as const;

/**
 * Fields that must carry this look's own state. `popupOpen: false` is honest
 * for a look with no autocomplete, so it is not on the list; `working: false`
 * or `overlayOpen: false` would be the exact lie that broke Escape before.
 */
const MUST_NOT_BE_CONSTANT = ["barState", "working", "overlayOpen", "composerOpen"] as const;

const read = (relative: string) => readFileSync(resolve(process.cwd(), relative), "utf8");

/** The `useEscapeToIdle({ ... })` call in one appearance, as written. */
function escapeCall(source: string): string | null {
  const start = source.indexOf("useEscapeToIdle({");
  if (start === -1) return null;
  const end = source.indexOf("});", start);
  return end === -1 ? null : source.slice(start, end);
}

describe("every bar appearance shares one Escape", () => {
  it("maps every appearance in the catalog to a component", () => {
    const missing = APPEARANCE_CATALOG.filter(
      (entry) => !COMPONENT_FOR_APPEARANCE[entry.value],
    ).map((entry) => entry.name);
    expect(missing).toEqual([]);
    expect(Object.keys(COMPONENT_FOR_APPEARANCE)).toHaveLength(APPEARANCE_CATALOG.length);
  });

  it.each(APPEARANCE_CATALOG.map((entry) => [entry.name, entry.value] as const))(
    "%s calls the shared hook instead of hand-rolling Escape",
    (_name, value) => {
      const source = read(COMPONENT_FOR_APPEARANCE[value]);

      expect(source).toContain('from "@/hooks/useEscapeToIdle"');
      const call = escapeCall(source);
      expect(call, "the appearance must call useEscapeToIdle({ ... })").not.toBeNull();

      for (const field of REQUIRED_FIELDS) {
        expect(call).toContain(`${field}:`);
      }

      // No ladder of its own. The key may still be named in a comment, so the
      // check is on the comparison, not the word.
      expect(source).not.toMatch(/key\s*===\s*["']Escape["']/);
      expect(source).not.toMatch(/key\s*!==\s*["']Escape["']/);
    },
  );

  it.each(APPEARANCE_CATALOG.map((entry) => [entry.name, entry.value] as const))(
    "%s tells the truth about its own state",
    (_name, value) => {
      const call = escapeCall(read(COMPONENT_FOR_APPEARANCE[value]));
      for (const field of MUST_NOT_BE_CONSTANT) {
        expect(call).not.toMatch(new RegExp(`${field}:\\s*(false|true|""|'')\\s*,`));
      }
    },
  );

  it("leaves the stop-key arming and the backend route to the hook alone", () => {
    // Two things the Pill used to own privately, and the reason a global
    // Escape worked for it and for no other look. If an appearance starts
    // arming the monitor or listening for the dismissal itself, the two paths
    // can disagree again.
    for (const relative of Object.values(COMPONENT_FOR_APPEARANCE)) {
      const source = read(relative);
      expect(source, `${relative} arms the stop key itself`).not.toContain(
        "BAR_SET_BAR_PANE_OPEN",
      );
      expect(source, `${relative} listens for the dismissal itself`).not.toContain(
        "BAR_DISMISS_PANE",
      );
    }
  });
});
