import * as React from "react";
import { MessageResponse as Response } from "@/components/ai-elements/message";
import { JsxMessageRenderer } from "@/components/ui/jsx-message-renderer";
import { cn } from "@/lib/utils";

/**
 * Content segment — either markdown text or a JSX component block.
 */
type ContentSegment =
  | { type: "text"; content: string }
  | { type: "jsx"; content: string };

/**
 * Top-level JSX component names that the agent can use.
 * Used to detect JSX boundaries in mixed content.
 * Must match the keys in `availableComponents` from jsx-message-renderer.tsx.
 */
const JSX_COMPONENT_NAMES = [
  "Card",
  "Alert",
  "Badge",
  "Button",
  "StatusCard",
  "ProgressBar",
  "ColorShowcase",
  "VisualDemo",
  "Circle",
  "Rectangle",
  "Triangle",
  "Dialog",
  "Tabs",
  "Separator",
  "Skeleton",
  // Domain-specific agent response cards
  "WeatherCard",
  "FileListCard",
  "SystemStatusCard",
  "ComparisonCard",
  "TimerCard",
  "LinkCard",
  "TaskSummaryCard",
  // Interactive action components
  "ActionButton",
  "QueryButton",
  "OpenButton",
  "CopyButton",
  // Onboarding inline components
  "OnboardingActionButton",
  "OnboardingActions",
  "PermissionStatusCard",
  "PermissionStatusGrid",
  "ProviderSelector",
  // Animated components
  "AnimatedCard",
  "AnimatedList",
  "AnimatedProgress",
  "GlowBadge",
  "ShimmerText",
  "Confetti",
  "PulseRing",
  "AnimatedDivider",
  "Stat",
  "MiniChart",
  "AnimatedNumber",
] as const;

/**
 * Build a regex that matches the opening tag of any known JSX component.
 * Matches: `<Card>`, `<Card className="...">`, `<StatusCard status="success" message="Done" />`
 */
const JSX_OPEN_TAG_PATTERN = new RegExp(
  `<(${JSX_COMPONENT_NAMES.join("|")})(\\s|>|\\/)`,
);

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
      if (remaining.trim()) {
        segments.push({ type: "text", content: remaining });
      }
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
        segments.push({ type: "text", content: remaining.slice(0, textEnd) });
        remaining = remaining.slice(textEnd);
        continue;
      }
      // No closing fence found — treat rest as text
      segments.push({ type: "text", content: remaining });
      break;
    }

    // Text before the JSX block
    if (match.index > 0) {
      const textBefore = remaining.slice(0, match.index);
      if (textBefore.trim()) {
        segments.push({ type: "text", content: textBefore });
      }
    }

    const componentName = match[1];
    const jsxStart = match.index;

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
        segments.push({ type: "text", content: remaining.slice(jsxStart) });
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
 * Check if content has any JSX components (quick check before expensive splitting).
 */
export function hasMixedContent(content: string): boolean {
  return JSX_OPEN_TAG_PATTERN.test(content);
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
      return <JsxMessageRenderer jsx={seg.content} className={className} />;
    }

    return (
      <div className={cn("space-y-3", className)}>
        {segments.map((seg, i) =>
          seg.type === "text" ? (
            <div key={i} className="jsx-segment-enter">
              <Response>{seg.content}</Response>
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
