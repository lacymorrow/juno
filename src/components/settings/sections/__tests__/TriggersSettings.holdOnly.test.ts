import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * # Which keys cannot be tapped, pinned to the source
 *
 * The gesture menu greys Tap on a key that can only be held, and it has to
 * decide that before anything is saved: a menu cannot wait on an `invoke` per
 * item without every option flashing available first. So the rule is mirrored
 * in TypeScript (`HOLD_ONLY_MODIFIERS`), and a mirror is only safe while
 * something fails when it stops being one.
 *
 * Rust owns the rule. `ModifierKey::hold_only` decides it and
 * `triggers::issues` refuses the save on it. If a key is added there and not
 * here, the menu offers Tap, the save refuses, and the person is told no after
 * choosing instead of before. That is the bug this file exists to catch.
 *
 * It reads source text rather than running either side, because the two sides
 * are a Rust `const` and a TypeScript array literal and no runtime sees both.
 */
// The body runs to the function's own closing brace (indented four spaces),
// not the first `}`: an inner block such as `|| { false }` would otherwise end
// the capture early and hide every constant after it (same class as LAC-2987).
const read = (relative: string) =>
  readFileSync(resolve(process.cwd(), relative), "utf8");

/**
 * Every `ModifierKey` constant named in Rust's `hold_only`, and the label
 * `ModifierKey::label` gives it, which is the word the TypeScript list holds.
 */
const LABEL_FOR_RUST_CONST: Record<string, string> = {
  CONTROL: "Control",
  RIGHT_OPTION: "Right Option",
};

describe("hold-only keys", () => {
  const rust = read("src-tauri/src/triggers/mod.rs");
  const tsx = read("src/components/settings/sections/TriggersSettings.tsx");

  it("names the same keys in Rust and in the gesture menu", () => {
    const body = rust.match(/pub fn hold_only\(self\) -> bool \{([\s\S]*?)\n    \}/);
    expect(body, "ModifierKey::hold_only not found in triggers/mod.rs").not.toBeNull();

    const rustConsts = [...(body?.[1] ?? "").matchAll(/Self::([A-Z_]+)/g)].map(
      (m) => m[1],
    );
    expect(rustConsts.length).toBeGreaterThan(0);

    // A constant Rust tests for that this file has never heard of: add it to
    // the map above and to HOLD_ONLY_MODIFIERS, in that order.
    const unmapped = rustConsts.filter((c) => !(c in LABEL_FOR_RUST_CONST));
    expect(unmapped, "unmapped ModifierKey constant in hold_only").toEqual([]);

    const expected = rustConsts.map((c) => LABEL_FOR_RUST_CONST[c]).sort();

    const list = tsx.match(/const HOLD_ONLY_MODIFIERS = \[([^\]]*)\]/);
    expect(list, "HOLD_ONLY_MODIFIERS not found").not.toBeNull();
    const actual = [...(list?.[1] ?? "").matchAll(/"([^"]+)"/g)]
      .map((m) => m[1])
      .sort();

    expect(actual).toEqual(expected);
  });

  it("greys Tap with the sentence the save would refuse with", () => {
    // Same wording on both sides, so the greyed-out option and the refusal it
    // prevents cannot read as two different rules.
    expect(rust).toContain('can only be held, not tapped.');
    expect(tsx).toContain('can only be held, not tapped.');
  });

  it("ends each gesture with the sentence Rust gives it", () => {
    const fn = rust.match(/pub fn ending\(self\) -> &'static str \{([\s\S]*?)\n    \}/);
    expect(fn, "Gesture::ending not found").not.toBeNull();
    const fromRust = Object.fromEntries(
      [...(fn?.[1] ?? "").matchAll(/Self::(\w+)\s*=>\s*"([^"]+)"/g)].map(
        (m) => [m[1].toLowerCase(), m[2]],
      ),
    );
    const map = tsx.match(/const GESTURE_ENDING[^=]*= \{([\s\S]*?)\n\};/);
    expect(map, "GESTURE_ENDING not found").not.toBeNull();
    const fromTs = Object.fromEntries(
      [...(map?.[1] ?? "").matchAll(/(\w+):\s*"([^"]+)"/g)].map((m) => [m[1], m[2]]),
    );
    expect(Object.keys(fromRust).length).toBeGreaterThan(0);
    expect(fromTs).toEqual(fromRust);
  });

  it("resets to the factory bindings Rust ships", () => {
    const shortcut = (name: string) => {
      const m = rust.match(
        new RegExp(`pub const ${name}: &str = "([^"]+)";`),
      );
      expect(m, `${name} not found`).not.toBeNull();
      return m?.[1];
    };
    const fn = rust.match(/pub fn default_triggers\(\) -> Vec<Trigger> \{([\s\S]*?)\n\}/);
    expect(fn, "default_triggers not found").not.toBeNull();
    const fromRust: Record<string, string | undefined> = {};
    for (const m of (fn?.[1] ?? "").matchAll(
      /Gesture::(\w+),\s*TriggerTarget::(\w+),\s*Some\(Binding::Keyboard \{\s*shortcut: (\w+)\.to_string\(\)/g,
    )) {
      fromRust[`${m[1].toLowerCase()}:${m[2].toLowerCase()}`] = shortcut(m[3]);
    }
    const map = tsx.match(/const DEFAULT_BINDINGS[^=]*= \{([\s\S]*?)\n\};/);
    expect(map, "DEFAULT_BINDINGS not found").not.toBeNull();
    const fromTs = Object.fromEntries(
      [...(map?.[1] ?? "").matchAll(/"([^"]+)":\s*"([^"]+)"/g)].map((m) => [m[1], m[2]]),
    );
    expect(Object.keys(fromRust).length).toBeGreaterThan(0);
    expect(fromTs).toEqual(fromRust);
  });
});
