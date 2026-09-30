/**
 * The one rule every appearance uses to turn the mic level Rust reports
 * (`audioLevel`, 0..1 on `bar-state-update`) into motion. Kept here so the
 * looks agree on what "louder" means and so a NaN or a stray value from the
 * backend can never blow a shape up.
 */

/** A level Rust reported, made safe: finite and within 0..1. */
export function clampLevel(level: number | null | undefined): number {
  if (typeof level !== "number" || !Number.isFinite(level)) return 0;
  return Math.min(1, Math.max(0, level));
}

/**
 * The scale a voice-driven mark takes at a level: 1 when silent, `1 + max` at
 * full voice. A square root lifts quiet speech so a normal speaking voice is
 * visible, not just shouting.
 */
export function voiceScale(level: number | null | undefined, max = 0.9): number {
  return 1 + max * Math.sqrt(clampLevel(level));
}

/** Follows the mic without lagging behind speech; short enough to feel live. */
export const VOICE_TRANSITION = "transform 90ms linear";
