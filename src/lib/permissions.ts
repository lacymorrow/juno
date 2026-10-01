/**
 * Permissions: the contract between Rust's approval policy and the UI.
 *
 * Rust decides. `agent/tools/permission_policy.rs` holds the one function that
 * answers "does this need asking", and these values are the stored setting it
 * reads. Everything here is the names and the sentences, which is the part a
 * person deals with.
 *
 * The copy is the feature. Lacy's complaint was not only the number of prompts,
 * it was that "our permissions settings" were "scary or opaque" and that the
 * setting meant to turn the prompts off did nothing. So each mode says what it
 * permits, in a sentence you could predict the behaviour from, and none of them
 * is a number on a slider: nobody can reason about what 60 percent permissive
 * allows, including the person deciding whether to trust it.
 */

/** How much Juno interrupts to ask permission. */
export type PermissionMode = "ask_first" | "ask_when_risky" | "dont_ask";

/** The mode Juno ships with. Matches `defaults::PERMISSION_MODE` in Rust. */
export const DEFAULT_PERMISSION_MODE: PermissionMode = "ask_when_risky";

export interface PermissionModeOption {
  value: PermissionMode;
  /** The name on the row. */
  name: string;
  /** What this mode permits, in one sentence. */
  consequence: string;
}

/**
 * The three modes, in order from most to least interruption.
 *
 * Deliberately not four: there is no full-autonomy mode. Every mode keeps the
 * same floor, which the footer states, because a mode that gave it up would
 * have to be an informed choice with its own screen, and nobody asked for one.
 */
export const PERMISSION_MODES: readonly PermissionModeOption[] = [
  {
    value: "ask_first",
    name: "Ask me first",
    consequence:
      "Juno checks with you before it changes anything: running a command, saving a file, typing into a page. You will answer a lot of questions.",
  },
  {
    value: "ask_when_risky",
    name: "Ask about risky things",
    consequence:
      "Juno gets on with ordinary work and stops to ask before anything that could delete your files, install software, or spend money.",
  },
  {
    value: "dont_ask",
    name: "Don't ask",
    consequence:
      "Juno works without stopping. Use it when you are watching, because you are the only thing between it and a mistake.",
  },
];

/**
 * The floor, said out loud. True in all three modes.
 *
 * Keep this honest. If the floor ever changes, this sentence changes with it,
 * and widening what counts as irreversible to make a mode read as safer than it
 * is would be the same defect as the setting that did nothing.
 */
export const PERMISSION_FLOOR =
  "Whichever you pick, Juno always asks before something it cannot undo: using sudo, deleting a folder, writing into system files, or typing a password or a card number into a page.";

/** What Juno never asks about, so the list is not a mystery. */
export const PERMISSION_NEVER_ASKS =
  "Juno never asks about commands that only look around or wait, like ls, pwd, which, date and sleep.";

/** Read a stored value. Anything unrecognised becomes the default. */
export function parsePermissionMode(value: unknown): PermissionMode {
  return PERMISSION_MODES.some((mode) => mode.value === value)
    ? (value as PermissionMode)
    : DEFAULT_PERMISSION_MODE;
}

/** The option row for a mode. */
export function permissionModeOption(mode: PermissionMode): PermissionModeOption {
  return (
    PERMISSION_MODES.find((option) => option.value === mode) ?? PERMISSION_MODES[1]
  );
}
