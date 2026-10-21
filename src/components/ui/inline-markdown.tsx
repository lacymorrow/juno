import { Fragment, type ReactNode, memo } from "react";
import { type InlineNode, parseInlineMarkdown } from "@/lib/inlineMarkdown";
import { cn } from "@/lib/utils";

/**
 * The marks a sentence carries, on whatever line it is already on.
 *
 * This renders a fragment, never a wrapper: it is dropped inside the row that
 * already exists (the small chat preview's `truncate` caption) and inherits
 * that row's size, colour and clipping untouched. Every node is an inline
 * element, so there is no block box to grow the row — see
 * `lib/inlineMarkdown.ts` for why that is the parser's job and not CSS's.
 *
 * For a full answer, use `MixedContentRenderer`. This is the preview's
 * renderer, and it knows no headings, lists, tables or code blocks.
 */

/**
 * Inline code is monospace and nothing else. The chip the full renderer draws
 * carries padding, which is the one way a mark here could change the row's
 * height, and on a single line the change of face already reads as code.
 */
const CODE_CLASS = "font-mono text-[0.92em]";

/**
 * A link is its words, underlined, not an anchor. The preview row is itself a
 * control on some surfaces (the caption opens the full text), and a second
 * click target inside it would be both invalid markup and an ambiguous press.
 * The answer below the preview carries the real link.
 */
const LINK_CLASS = "underline decoration-1 underline-offset-2";

function render(nodes: InlineNode[], prefix: string): ReactNode[] {
  return nodes.map((node, i) => {
    const key = `${prefix}.${i}`;
    switch (node.type) {
      case "text":
        return <Fragment key={key}>{node.value}</Fragment>;
      case "code":
        return (
          <code key={key} className={CODE_CLASS}>
            {node.value}
          </code>
        );
      case "strong":
        return (
          <strong key={key} className="font-semibold">
            {render(node.children, key)}
          </strong>
        );
      case "em":
        return (
          <em key={key} className="italic">
            {render(node.children, key)}
          </em>
        );
      case "strike":
        return (
          <s key={key} className="line-through">
            {render(node.children, key)}
          </s>
        );
      case "link":
        return (
          <span key={key} className={LINK_CLASS}>
            {render(node.children, key)}
          </span>
        );
    }
  });
}

/**
 * `className` wraps the words in a span. Leave it off (the usual case) and
 * nothing is wrapped at all, so the caller's own row is the only box.
 */
export const InlineMarkdown = memo(function InlineMarkdown({
  text,
  className,
}: {
  text: string;
  className?: string;
}) {
  const nodes = parseInlineMarkdown(text);
  const words = render(nodes, "m");
  if (!className) return <>{words}</>;
  return <span className={cn(className)}>{words}</span>;
});
