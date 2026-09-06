import * as React from "react";
import { MessageResponse as Response } from "@/components/ai-elements/message";
import { JsxMessageRenderer } from "@/components/ui/jsx-message-renderer";
import { WhyBlock } from "@/components/ui/why-block";
import { cn } from "@/lib/utils";

/**
 * Content segment — either markdown text or a JSX component block.
 */
type ContentSegment =
  | { type: "text"; content: string }
  | { type: "jsx"; content: string }
  /** Method rationale, rendered collapsed (see `WhyBlock`). */
  | { type: "why"; content: string };

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
const JSX_OPEN_TAG_PATTERN = /<(?!TTS\b)([A-Z][A-Za-z0-9]*)(\s|>|\/)/;

/**
 * `<Why>…</Why>` is the agent's method rationale. It is prose, not a
 * component: it is lifted out here (never parsed as JSX, so braces and `>`
 * in it are harmless) and rendered collapsed by `WhyBlock`.
 */
const WHY_TAG = "Why";
const WHY_CLOSE = "</Why>";

/**
 * Legacy form of the same thing: a paragraph opening with a bold
 * "**Why AppleScript instead of clicking:**" lead-in. Only method-choice
 * phrasings match — an answer to the user's own "why did it fail?" stays
 * visible.
 */
const RATIONALE_LEAD_PATTERN =
  /^\*\*Why\b[^*\n]*?(?:\bhere\b|\binstead of\b|\brather than\b|\bover\b|\bvs\.?|\bversus\b|\bthis (?:approach|way|route|method|path)\b|\bI (?:did|used|chose|went|picked|took)\b|\bapplescript\b|\bosascript\b|\bkeyboard\b|\bshortcut\b|\baccessibility\b|\bscreenshots?\b|\bclick(?:ing)?\b)[^*\n]*:?\*\*/i;
const RATIONALE_ANYWHERE_PATTERN = new RegExp(
  RATIONALE_LEAD_PATTERN.source.replace(/^\^/, "(?:^|\\n)"),
  "i",
);

/**
 * Split a run of text into text segments and collapsed rationale segments,
 * paragraph by paragraph.
 */
function splitRationaleParagraphs(text: string): ContentSegment[] {
  if (!RATIONALE_ANYWHERE_PATTERN.test(text)) {
    return [{ type: "text", content: text }];
  }
  const out: ContentSegment[] = [];
  let pending: string[] = [];
  const flush = () => {
    if (pending.length) {
      out.push({ type: "text", content: pending.join("\n\n") });
      pending = [];
    }
  };
  for (const paragraph of text.split(/\n{2,}/)) {
    if (!paragraph.trim()) continue;
    if (RATIONALE_LEAD_PATTERN.test(paragraph.trimStart())) {
      flush();
      out.push({ type: "why", content: paragraph.trim() });
    } else {
      pending.push(paragraph);
    }
  }
  flush();
  return out;
}

function pushText(segments: ContentSegment[], text: string) {
  if (!text.trim()) return;
  segments.push(...splitRationaleParagraphs(text));
}

/**
 * Split mixed content (markdown + JSX) into alternating segments.
 *
 * Strategy:
 * - Scan for top-level JSX component opening tags
 * - Find the matching closing tag (or self-closing)
 * - Everything before is text, the JSX block is a jsx segment, then continue
 *
 * This is intentionally simple — it handles the common case of JSX blocks
 * at the top level separated by text. It does NOT handle:
 * - JSX nested inside markdown code blocks (```<Card>...</Card>```)
 * - Malformed JSX (missing closing tags)
 * These edge cases are acceptable because the agent is instructed to produce
 * well-formed JSX at the top level.
 */
export function splitMixedContent(content: string, isStreaming = false): ContentSegment[] {
  const segments: ContentSegment[] = [];
  let remaining = content;

  while (remaining.length > 0) {
    const match = JSX_OPEN_TAG_PATTERN.exec(remaining);

    if (!match || match.index === undefined) {
      // No more JSX — rest is text
      pushText(segments, remaining);
      break;
    }

    // Check if this JSX tag is inside a markdown code block (``` ... ```)
    const beforeMatch = remaining.slice(0, match.index);
    const openFences = (beforeMatch.match(/```/g) || []).length;
    if (openFences % 2 !== 0) {
      // Inside a code fence — skip this match, treat everything up to the
      // closing fence as text, then continue scanning
      const closeFenceIdx = remaining.indexOf("```", match.index);
      if (closeFenceIdx !== -1) {
        const textEnd = closeFenceIdx + 3;
        pushText(segments, remaining.slice(0, textEnd));
        remaining = remaining.slice(textEnd);
        continue;
      }
      // No closing fence found — treat rest as text
      pushText(segments, remaining);
      break;
    }

    // Text before the JSX block
    if (match.index > 0) {
      pushText(segments, remaining.slice(0, match.index));
    }

    const componentName = match[1];
    const jsxStart = match.index;

    // `<Why>` is rationale prose, not a component: lift its body out verbatim
    if (componentName === WHY_TAG) {
      const openEnd = remaining.indexOf(">", jsxStart);
      if (openEnd === -1) {
        // Tag still arriving — nothing to show yet
        break;
      }
      if (remaining[openEnd - 1] === "/") {
        // `<Why />` carries nothing
        remaining = remaining.slice(openEnd + 1);
        continue;
      }
      const closeIdx = remaining.indexOf(WHY_CLOSE, openEnd + 1);
      const body =
        closeIdx === -1
          ? remaining.slice(openEnd + 1)
          : remaining.slice(openEnd + 1, closeIdx);
      if (body.trim()) {
        segments.push({ type: "why", content: body.trim() });
      }
      if (closeIdx === -1) break;
      remaining = remaining.slice(closeIdx + WHY_CLOSE.length);
      continue;
    }

    // Find the end of this JSX block
    const jsxEnd = findJsxBlockEnd(remaining, jsxStart, componentName);

    if (jsxEnd === -1) {
      if (isStreaming) {
        // During streaming, treat incomplete JSX as a jsx segment —
        // JsxRenderer with fixIncompleteJsx will auto-close tags,
        // rendering the component progressively as chunks arrive
        segments.push({ type: "jsx", content: remaining.slice(jsxStart) });
      } else {
        // After streaming, incomplete JSX is malformed — show as text
        pushText(segments, remaining.slice(jsxStart));
      }
      break;
    }

    const jsxContent = remaining.slice(jsxStart, jsxEnd);
    segments.push({ type: "jsx", content: jsxContent });
    remaining = remaining.slice(jsxEnd);
  }

  return segments;
}

/**
 * Find the end of a JSX block starting at `startIdx` for `componentName`.
 * Handles self-closing tags and nested same-name components.
 * Returns the index AFTER the closing tag, or -1 if not found.
 */
function findJsxBlockEnd(
  content: string,
  startIdx: number,
  componentName: string,
): number {
  // Check for self-closing tag first: <Component ... />
  const selfClosePattern = new RegExp(
    `<${componentName}[^>]*/>`,
  );
  const selfCloseMatch = selfClosePattern.exec(content.slice(startIdx));
  if (selfCloseMatch && selfCloseMatch.index === 0) {
    return startIdx + selfCloseMatch[0].length;
  }

  // Find matching closing tag, accounting for nesting
  const openPattern = new RegExp(`<${componentName}(\\s|>)`, "g");
  const closePattern = new RegExp(`</${componentName}>`, "g");

  let depth = 0;
  let pos = startIdx;

  // Count the opening tag we're starting from
  openPattern.lastIndex = pos;
  const firstOpen = openPattern.exec(content);
  if (firstOpen && firstOpen.index === pos) {
    depth = 1;
    pos = openPattern.lastIndex;
  } else {
    return -1;
  }

  while (depth > 0 && pos < content.length) {
    openPattern.lastIndex = pos;
    closePattern.lastIndex = pos;

    const nextOpen = openPattern.exec(content);
    const nextClose = closePattern.exec(content);

    if (!nextClose) {
      // No closing tag found — incomplete JSX
      return -1;
    }

    if (nextOpen && nextOpen.index < nextClose.index) {
      // Another opening tag before the closing tag — check if it's not self-closing
      const slice = content.slice(nextOpen.index);
      const selfClose = selfClosePattern.exec(slice);
      if (selfClose && selfClose.index === 0) {
        // Self-closing — skip it, don't increase depth
        pos = nextOpen.index + selfClose[0].length;
      } else {
        depth++;
        pos = openPattern.lastIndex;
      }
    } else {
      depth--;
      if (depth === 0) {
        return nextClose.index + nextClose[0].length;
      }
      pos = closePattern.lastIndex;
    }
  }

  return -1;
}

/**
 * Check if content needs the mixed renderer: any JSX component, or a
 * rationale paragraph that should be collapsed (quick check before splitting).
 */
export function hasMixedContent(content: string): boolean {
  return (
    JSX_OPEN_TAG_PATTERN.test(content) || RATIONALE_ANYWHERE_PATTERN.test(content)
  );
}

interface MixedContentRendererProps {
  content: string;
  isStreaming?: boolean;
  className?: string;
}

/**
 * Renders content that may contain interleaved markdown text and JSX components.
 *
 * - Pure text → Response (streamdown)
 * - Pure JSX → JsxMessageRenderer
 * - Mixed → alternating Response + JsxMessageRenderer segments
 */
export const MixedContentRenderer = React.memo(
  function MixedContentRenderer({
    content,
    isStreaming,
    className,
  }: MixedContentRendererProps) {
    const segments = React.useMemo(
      () => splitMixedContent(content, isStreaming),
      [content, isStreaming],
    );

    // Single segment optimization — no wrapper div needed
    if (segments.length === 1) {
      const seg = segments[0];
      if (seg.type === "text") {
        return <Response className={className}>{seg.content}</Response>;
      }
      if (seg.type === "why") {
        return <WhyBlock className={className}>{seg.content}</WhyBlock>;
      }
      return <JsxMessageRenderer jsx={seg.content} className={className} />;
    }

    return (
      <div className={cn("space-y-3", className)}>
        {segments.map((seg, i) =>
          seg.type === "text" ? (
            <div key={i} className="jsx-segment-enter">
              <Response>{seg.content}</Response>
            </div>
          ) : seg.type === "why" ? (
            <div key={i} className="jsx-segment-enter">
              <WhyBlock>{seg.content}</WhyBlock>
            </div>
          ) : (
            <div key={i} className="jsx-segment-enter">
              <JsxMessageRenderer jsx={seg.content} />
            </div>
          ),
        )}
        {isStreaming && (
          <span className="inline-block w-2 h-4 bg-current ml-1 animate-pulse">
            |
          </span>
        )}
      </div>
    );
  },
  (prev, next) =>
    prev.content === next.content && prev.isStreaming === next.isStreaming,
);
