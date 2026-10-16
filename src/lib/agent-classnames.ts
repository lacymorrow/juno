/**
 * The backstop for "never set colour classes".
 *
 * The prompt tells the agent to pick a card and let the tokens colour it. When
 * it writes `text-green-700`, `bg-yellow-50`, a gradient, an uppercase tracked
 * label anyway, the renderer drops those classes and keeps layout, spacing and
 * type size. Theme tokens (`text-muted-foreground`, `bg-muted`, `text-primary`)
 * pass: they are the palette, not a colour choice.
 */

const PALETTE =
  "slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose";

const UTILITIES =
  "text|bg|border(?:-[trblxyse])?|ring|ring-offset|outline|decoration|divide|fill|stroke|shadow|from|via|to|accent|caret|placeholder";

/** `text-green-700`, `bg-white`, `border-t-red-500/40`, `bg-[#fff]`, `text-[rgb(1,2,3)]`. */
const COLOUR = new RegExp(
  `^!?(?:${UTILITIES})-(?:(?:${PALETTE})(?:-\\d{2,3})?|white|black|\\[(?:#|rgb|hsl|oklch|color)[^\\]]*\\])(?:/\\d+|/\\[[^\\]]+\\])?$`,
);

/** `bg-gradient-to-br`, `bg-linear-to-r`, `bg-[linear-gradient(...)]`. */
const GRADIENT = /^!?bg-(?:gradient|linear|radial|conic)(?:-|$)|^!?bg-\[(?:linear|radial|conic)-?/;

/** Uppercase and tracked labels are not Mac. */
const LABEL_STYLE = /^!?(?:uppercase|tracking-(?:wide|wider|widest))$/;

/** The utility itself, without responsive, state or dark variants (`dark:hover:text-red-500`). */
function utilityOf(token: string): string {
  let depth = 0;
  let cut = -1;
  for (let i = 0; i < token.length; i++) {
    const ch = token[i];
    if (ch === "[") depth++;
    else if (ch === "]") depth--;
    else if (ch === ":" && depth === 0) cut = i;
  }
  return token.slice(cut + 1);
}

export function isColourClass(token: string): boolean {
  const utility = utilityOf(token);
  return COLOUR.test(utility) || GRADIENT.test(utility) || LABEL_STYLE.test(utility);
}

/** `className` without the colour, gradient and label-style classes. */
export function filterAgentClassName(className: unknown): string | undefined {
  if (typeof className !== "string") return undefined;
  const kept = className
    .split(/\s+/)
    .filter((token) => token && !isColourClass(token))
    .join(" ");
  return kept || undefined;
}

/** Does the class list already set an icon's size? */
export function setsSize(className: string | undefined): boolean {
  return !!className && /(?:^|\s)(?:size|h|w)-/.test(className);
}
