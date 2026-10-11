/**
 * Does a message need the mixed renderer (a component block, or a rationale
 * paragraph to collapse)? The question, without the renderer.
 *
 * It lives apart from `components/ui/mixed-content-renderer.tsx` because the
 * floating bar asks it for every streamed message, and importing it from the
 * renderer pulled markdown, maths and diagram rendering (about 1 MB of script)
 * into the bar's first paint.
 */

/**
 * Opening tag of any component-style element: `<Name`, where Name starts with
 * a capital letter. There is deliberately NO allowlist here. The agent may use
 * any component the renderer knows about, and a tag the renderer does not know
 * is still handed to it rather than silently stripped as text, so new
 * components work the moment they are registered and nothing that could render
 * is thrown away up front.
 *
 * `<TTS>` is the one exclusion: it is the spoken channel, not a component.
 *
 * Prose that merely looks like a tag (`Vec<String>`, `<T>`) is protected by the
 * block finder: a tag with no matching close and no self-close falls back to
 * text once streaming has finished.
 */
export const JSX_OPEN_TAG_PATTERN = /<(?!TTS\b)([A-Z][A-Za-z0-9]*)(\s|>|\/)/;

/**
 * The legacy form of a `<Why>` rationale: a paragraph opening with a bold
 * "**Why AppleScript instead of clicking:**" lead-in. Only method-choice
 * phrasings match; an answer to the user's own "why did it fail?" stays
 * visible.
 */
export const RATIONALE_LEAD_PATTERN =
  /^\*\*Why\b[^*\n]*?(?:\bhere\b|\binstead of\b|\brather than\b|\bover\b|\bvs\.?|\bversus\b|\bthis (?:approach|way|route|method|path)\b|\bI (?:did|used|chose|went|picked|took)\b|\bapplescript\b|\bosascript\b|\bkeyboard\b|\bshortcut\b|\baccessibility\b|\bscreenshots?\b|\bclick(?:ing)?\b)[^*\n]*:?\*\*/i;
export const RATIONALE_ANYWHERE_PATTERN = new RegExp(
  RATIONALE_LEAD_PATTERN.source.replace(/^\^/, "(?:^|\\n)"),
  "i",
);

/**
 * Check if content needs the mixed renderer: any JSX component, or a
 * rationale paragraph that should be collapsed (quick check before splitting).
 */
export function hasMixedContent(content: string): boolean {
  return (
    JSX_OPEN_TAG_PATTERN.test(content) || RATIONALE_ANYWHERE_PATTERN.test(content)
  );
}
