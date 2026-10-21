/**
 * Inline markdown for a one-line answer preview.
 *
 * The small chat preview shows the answer's prose on a single truncated row
 * above the component. The answer is markdown, so that row printed its syntax:
 * `**Jenny Song**, mobile: 867-5309` (juno#858). The full renderer is not the
 * fix. `MixedContentRenderer` emits paragraphs, lists and headings, which a
 * `truncate` row cannot hold, and the module note in `lib/mixedContent.ts`
 * explains what pulling markdown rendering onto a bar path costs at first
 * paint.
 *
 * So this parses the marks a sentence can carry and nothing else: strong,
 * emphasis, strike, inline code, and a link's words. There is deliberately no
 * block syntax in the node type at all, which is what keeps the row one line:
 * the renderer has no paragraph to emit, rather than a paragraph it has to
 * suppress. Block markers that open the text (`## `, `- `, `> `) are dropped
 * as leading noise.
 *
 * Pure, so the hard parts (what is a delimiter, what is `snake_case`) are
 * tested without a DOM. Rendering lives in `components/ui/inline-markdown.tsx`.
 */

export type InlineNode =
  | { type: "text"; value: string }
  | { type: "code"; value: string }
  | { type: "strong"; children: InlineNode[] }
  | { type: "em"; children: InlineNode[] }
  | { type: "strike"; children: InlineNode[] }
  /** `href` is parsed but a preview does not have to render it as a link. */
  | { type: "link"; href: string; children: InlineNode[] };

/** Characters a backslash can escape, so `\*not bold\*` keeps its asterisks. */
const ESCAPABLE = new Set(["\\", "`", "*", "_", "~", "[", "]", "(", ")", "#", "-", "+", ">", "!", "."]);

/** A letter or a digit in any script: `_` between two of these is not a mark. */
const WORD_CHAR = /[\p{L}\p{N}]/u;

/**
 * A file extension right after a closing `_` run, as in `__init__.py`.
 *
 * This is the one place the parser is deliberately stricter than markdown,
 * which bolds the `init` there. Juno's answers name files, and a filename
 * losing its underscores is the same bug as `snake_case_name` losing them.
 * Prose is untouched: emphasis ending a sentence is followed by `.` and a
 * space, not by `.` and a word.
 */
const FILE_EXTENSION = /^\.[\p{L}\p{N}]/u;

/**
 * Block markers that open a line. After the prose is collapsed to one line
 * these are the only ones left, and they are noise in a preview rather than
 * structure it can show.
 */
const LEADING_BLOCK_MARK = /^\s*(?:#{1,6}\s+|>\s*|[-*+]\s+|\d+[.)]\s+)/;

/** How long the delimiter run at `start` is, counting the same character. */
function runLength(src: string, start: number): number {
  const ch = src[start];
  let n = 0;
  while (src[start + n] === ch) n++;
  return n;
}

/**
 * A code span opening at `start`: a run of N backticks closed by a run of
 * exactly N. Returns null for a lone backtick, which is then just a backtick.
 */
function matchCodeSpan(src: string, start: number): { value: string; end: number } | null {
  const fence = runLength(src, start);
  const open = start + fence;
  let i = open;
  while (i < src.length) {
    if (src[i] !== "`") {
      i++;
      continue;
    }
    const close = runLength(src, i);
    if (close === fence) {
      let value = src.slice(open, i);
      // Markdown lets one space on each side hold a backtick inside: `` ` ``.
      if (value.length > 2 && value.startsWith(" ") && value.endsWith(" ") && value.trim()) {
        value = value.slice(1, -1);
      }
      return { value, end: i + fence };
    }
    i += close;
  }
  return null;
}

/** Scan to the character after the bracket opened at `start`, or -1. */
function matchBracket(src: string, start: number, open: string, close: string): number {
  let depth = 0;
  for (let i = start; i < src.length; i++) {
    const ch = src[i];
    if (ch === "\\") {
      i++;
      continue;
    }
    if (ch === open) depth++;
    else if (ch === close) {
      depth--;
      if (depth === 0) return i;
    }
  }
  return -1;
}

/** `[label](href)` at `start`. Nested brackets in either half are counted. */
function matchLink(
  src: string,
  start: number,
): { label: string; href: string; end: number } | null {
  const labelEnd = matchBracket(src, start, "[", "]");
  if (labelEnd === -1 || src[labelEnd + 1] !== "(") return null;
  const hrefEnd = matchBracket(src, labelEnd + 1, "(", ")");
  if (hrefEnd === -1) return null;
  return {
    label: src.slice(start + 1, labelEnd),
    href: src.slice(labelEnd + 2, hrefEnd).trim(),
    end: hrefEnd + 1,
  };
}

type EmphasisType = "strong" | "em" | "strike";

/**
 * An emphasis run opening at `start`.
 *
 * Two rules keep prose out of it, both from the way markdown itself decides:
 * a run only opens when a non-space follows it and only closes when a
 * non-space precedes it (so `5 * 3 * 2` is arithmetic), and `_` does not open
 * or close inside a word (so `snake_case_name` is a name). The closing run
 * must be the same length as the opening one, which is what lets a `**bold**`
 * sit inside an `*italic*` without either one claiming the other's asterisks.
 */
function matchEmphasis(
  src: string,
  start: number,
): { type: EmphasisType; inner: string; end: number } | null {
  const ch = src[start];
  const length = Math.min(runLength(src, start), 3);
  // `~` is a mark only doubled; a single one is a tilde.
  if (ch === "~" && length < 2) return null;
  const width = ch === "~" ? 2 : length;

  const after = src[start + width];
  if (after === undefined || /\s/.test(after)) return null;
  if (ch === "_" && WORD_CHAR.test(src[start - 1] ?? "")) return null;

  let i = start + width;
  while (i < src.length) {
    if (src[i] === "\\") {
      i += 2;
      continue;
    }
    // A code span inside the run keeps its own delimiters: `a * b` is code.
    if (src[i] === "`") {
      const code = matchCodeSpan(src, i);
      if (code) {
        i = code.end;
        continue;
      }
    }
    if (src[i] !== ch) {
      i++;
      continue;
    }
    const close = runLength(src, i);
    // A longer run belongs to a stronger mark; step over it whole.
    if (close !== width || /\s/.test(src[i - 1] ?? " ")) {
      i += close;
      continue;
    }
    if (ch === "_" && (WORD_CHAR.test(src[i + width] ?? "") || FILE_EXTENSION.test(src.slice(i + width)))) {
      i += close;
      continue;
    }
    const type: EmphasisType = ch === "~" ? "strike" : width === 1 ? "em" : "strong";
    return { type, inner: src.slice(start + width, i), end: i + width };
  }
  return null;
}

function parseNodes(src: string): InlineNode[] {
  const out: InlineNode[] = [];
  let text = "";
  const flush = () => {
    if (text) {
      out.push({ type: "text", value: text });
      text = "";
    }
  };

  let i = 0;
  while (i < src.length) {
    const ch = src[i];

    if (ch === "\\" && ESCAPABLE.has(src[i + 1] ?? "")) {
      text += src[i + 1];
      i += 2;
      continue;
    }

    if (ch === "`") {
      const code = matchCodeSpan(src, i);
      if (code) {
        // An empty span (``` `` ```) has nothing to show; drop the backticks.
        if (code.value) {
          flush();
          out.push({ type: "code", value: code.value });
        }
        i = code.end;
        continue;
      }
    }

    // An image is its alt text here. A preview row has no room for the image,
    // and `![Chart](…)` would otherwise read as a stray exclamation mark.
    if (ch === "!" && src[i + 1] === "[") {
      const link = matchLink(src, i + 1);
      if (link) {
        flush();
        out.push(...parseNodes(link.label));
        i = link.end;
        continue;
      }
    }

    if (ch === "[") {
      const link = matchLink(src, i);
      if (link) {
        flush();
        out.push({ type: "link", href: link.href, children: parseNodes(link.label) });
        i = link.end;
        continue;
      }
    }

    if (ch === "*" || ch === "_" || ch === "~") {
      const mark = matchEmphasis(src, i);
      if (mark) {
        flush();
        const children = parseNodes(mark.inner);
        // `***both***` is emphasis inside strong, the way markdown nests it.
        out.push(
          runLength(src, i) >= 3 && mark.type === "strong"
            ? { type: "strong", children: [{ type: "em", children }] }
            : { type: mark.type, children },
        );
        i = mark.end;
        continue;
      }
      // A run that cannot open a mark is literal, and all of it is. Stepping
      // one character on would re-enter the same run, letting its tail open a
      // mark against its own head: in `__init__.py and it was _fine_.` the
      // `__` is refused (its only closer is a filename's), then the second
      // `_` opens against the first, because `_` is punctuation rather than a
      // word character, and pairs with the `_` after `fine` (QA, LAC-5166).
      const run = runLength(src, i);
      text += src.slice(i, i + run);
      i += run;
      continue;
    }

    text += ch;
    i++;
  }

  flush();
  return out;
}

/** Parse one line of prose into inline nodes. Never returns block content. */
export function parseInlineMarkdown(text: string): InlineNode[] {
  return parseNodes(text.replace(LEADING_BLOCK_MARK, ""));
}

/** The same words with every mark removed: for a `title` or an aria label. */
export function inlineMarkdownToText(text: string): string {
  const walk = (nodes: InlineNode[]): string =>
    nodes
      .map((node) =>
        node.type === "text" ? node.value : node.type === "code" ? node.value : walk(node.children),
      )
      .join("");
  return walk(parseInlineMarkdown(text));
}
