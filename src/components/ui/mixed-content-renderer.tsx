import * as React from "react";
import { MessageResponse as Response } from "@/components/ai-elements/message";
import {
  ComponentFallback,
  ComponentSkeleton,
} from "@/components/ui/component-skeleton";
import {
  JsxMessageRenderer,
  availableComponents,
} from "@/components/ui/jsx-message-renderer";
import { WhyBlock } from "@/components/ui/why-block";
import {
  findOpenTagEnd,
  stripJsxMarkup,
  trimPartialTrailingTag,
  trimUnresolvedTagTail,
} from "@/lib/jsx-utils";
import { cn } from "@/lib/utils";
import {
  JSX_OPEN_TAG_PATTERN,
  RATIONALE_ANYWHERE_PATTERN,
  RATIONALE_LEAD_PATTERN,
  hasMixedContent,
} from "@/lib/mixedContent";

/**
 * Content segment — either markdown text or a JSX component block.
 */
type ContentSegment =
  | { type: "text"; content: string }
  /**
   * A component block. `partial` means it is still streaming: its opening tag
   * has arrived, its closing tag has not, and it renders progressively.
   */
  | { type: "jsx"; content: string; partial?: boolean; name?: string }
  /** A component whose opening tag is still arriving: shown as a skeleton. `content` is the markup so far, never displayed. */
  | { type: "pending"; name: string; content: string }
  /** A component that ended malformed: its words, as plain text. */
  | { type: "fallback"; content: string }
  /** Method rationale, rendered collapsed (see `WhyBlock`). */
  | { type: "why"; content: string };


/**
 * `<Why>…</Why>` is the agent's method rationale. It is prose, not a
 * component: it is lifted out here (never parsed as JSX, so braces and `>`
 * in it are harmless) and rendered collapsed by `WhyBlock`.
 */
const WHY_TAG = "Why";
const WHY_CLOSE = "</Why>";


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
 * While streaming, a component never shows as raw markup:
 * - its opening tag still arriving (`<WeatherCard temp={5`) -> `pending`,
 *   a skeleton sized for that component;
 * - its opening tag done but the block still open -> `jsx` with `partial`,
 *   rendered progressively with any half-arrived inner tag trimmed off.
 *
 * Once streaming ends, an unclosed block of a known component becomes a
 * `fallback` segment (its words as plain text). An unclosed tag that is not a
 * known component (`Vec<String>`) is prose and stays text.
 */
export function splitMixedContent(content: string, isStreaming = false): ContentSegment[] {
  const segments: ContentSegment[] = [];
  const openFenceCount = (content.match(/```/g) || []).length;
  // A tag name still arriving at the very end (`<Weath`) is not text yet.
  let remaining =
    isStreaming && openFenceCount % 2 === 0 ? trimUnresolvedTagTail(content) : content;

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
    let jsxEnd = findJsxBlockEnd(remaining, jsxStart, componentName);
    const openTagEnd = findOpenTagEnd(remaining, jsxStart);

    // A self-closing tag with a `>` inside an attribute value slips past the
    // regex in findJsxBlockEnd; the quote-aware scan still sees it close.
    if (jsxEnd === -1 && openTagEnd !== -1 && remaining[openTagEnd - 2] === "/") {
      jsxEnd = openTagEnd;
    }

    if (jsxEnd === -1) {
      const rest = remaining.slice(jsxStart);
      if (isStreaming) {
        if (openTagEnd === -1) {
          // Attributes still arriving: nothing renderable yet.
          segments.push({ type: "pending", name: componentName, content: rest });
        } else {
          // Shell is open: render what has arrived, auto-closed by JsxRenderer.
          segments.push({
            type: "jsx",
            content: trimPartialTrailingTag(rest),
            partial: true,
            name: componentName,
          });
        }
      } else if (Object.prototype.hasOwnProperty.call(availableComponents, componentName)) {
        // A known component that never closed is malformed: keep its words.
        segments.push({ type: "fallback", content: stripJsxMarkup(rest) });
      } else {
        // Not a component (`Vec<String>`): it was prose all along.
        pushText(segments, rest);
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

// Re-exported: the check lives in a module with no rendering in it, so the
// bar can ask it without loading the renderer (see `lib/mixedContent.ts`).
export { hasMixedContent };

function renderSegment(seg: ContentSegment, className?: string): React.ReactNode {
  switch (seg.type) {
    case "text":
      return <Response className={className}>{seg.content}</Response>;
    case "why":
      return <WhyBlock className={className}>{seg.content}</WhyBlock>;
    case "pending":
      return <ComponentSkeleton name={seg.name} className={className} />;
    case "fallback":
      return <ComponentFallback text={seg.content} className={className} />;
    case "jsx":
      return (
        <JsxMessageRenderer
          jsx={seg.content}
          partial={seg.partial}
          name={seg.name}
          className={className}
        />
      );
  }
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
      return renderSegment(segments[0], className);
    }

    // A skeleton turning into its card keeps the same key and wrapper, so the
    // card replaces it in place without replaying the entry animation.
    return (
      <div className={cn("space-y-3", className)}>
        {segments.map((seg, i) => (
          <div key={i} className="jsx-segment-enter">
            {renderSegment(seg)}
          </div>
        ))}
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
