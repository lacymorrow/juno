import * as React from "react";
import { ChevronDownIcon, LightbulbIcon } from "lucide-react";
import {
  Reasoning,
  ReasoningContent,
  ReasoningTrigger,
  useReasoning,
} from "@/components/ai-elements/reasoning";
import { cn } from "@/lib/utils";

export interface WhyBlockProps {
  /** Markdown explaining how/why the agent did something. */
  children?: React.ReactNode;
  /** Trigger label. */
  title?: string;
  className?: string;
}

const DEFAULT_TITLE = "Why I did it this way";

function toText(children: React.ReactNode): string {
  return React.Children.toArray(children)
    .map((child) =>
      typeof child === "string" || typeof child === "number" ? String(child) : "",
    )
    .join("");
}

function WhyTriggerLabel({ title }: { title: string }) {
  const { isOpen } = useReasoning();
  return (
    <>
      <LightbulbIcon className="size-4" />
      <span>{title}</span>
      <ChevronDownIcon
        className={cn(
          "size-4 transition-transform",
          isOpen ? "rotate-180" : "rotate-0",
        )}
      />
    </>
  );
}

/**
 * The agent's method rationale ("AppleScript instead of clicking because…"),
 * collapsed behind a dropdown so the visible reply stays about the outcome.
 *
 * Rendered for `<Why>…</Why>` blocks in agent responses, and for the legacy
 * `**Why X instead of Y:**` paragraphs the splitter recognises. Never opens on
 * its own — the user asks to see it.
 */
export function WhyBlock({
  children,
  title = DEFAULT_TITLE,
  className,
}: WhyBlockProps) {
  const text = toText(children).trim();
  if (!text) return null;

  return (
    <Reasoning
      defaultOpen={false}
      className={cn("mb-0 mt-1", className)}
      data-testid="why-block"
    >
      <ReasoningTrigger>
        <WhyTriggerLabel title={title} />
      </ReasoningTrigger>
      <ReasoningContent className="mt-2">{text}</ReasoningContent>
    </Reasoning>
  );
}
