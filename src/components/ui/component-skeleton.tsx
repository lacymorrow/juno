import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

/**
 * Stand-in for an agent component whose markup is still streaming in.
 *
 * Sized to roughly match the component it will become, so the card swapping in
 * does not shove the conversation around. Small inline pieces (badges, stats,
 * buttons) get a pill; anything unknown gets a medium card.
 */

/** Approximate rendered height of each block-level component. */
const CARD_HEIGHTS: Record<string, string> = {
  WeatherCard: "h-36",
  NowPlayingCard: "h-24",
  AgendaCard: "h-24",
  FileListCard: "h-44",
  SystemStatusCard: "h-40",
  ComparisonCard: "h-48",
  TimerCard: "h-24",
  LinkCard: "h-20",
  TaskSummaryCard: "h-40",
  AnimatedCard: "h-32",
  Card: "h-32",
  Alert: "h-16",
  StatusCard: "h-14",
  ProgressBar: "h-12",
  AnimatedProgress: "h-10",
  MiniChart: "h-28",
  Tabs: "h-32",
};

/** Components that render inline and small. */
const INLINE_COMPONENTS = new Set([
  "Badge",
  "GlowBadge",
  "Stat",
  "AnimatedNumber",
  "ShimmerText",
  "Confetti",
  "PulseRing",
  "Button",
  "OpenButton",
  "QueryButton",
  "CopyButton",
  "ActionButton",
]);

interface ComponentSkeletonProps {
  /** Tag name of the component that is arriving, if known. */
  name?: string;
  className?: string;
}

export const ComponentSkeleton = ({ name, className }: ComponentSkeletonProps) => {
  const inline = name ? INLINE_COMPONENTS.has(name) : false;
  const size = inline
    ? "h-8 w-32 rounded-full"
    : cn("w-full rounded-xl", (name && CARD_HEIGHTS[name]) || "h-24");

  return (
    <div
      role="status"
      aria-busy="true"
      aria-label="Loading card"
      data-testid="component-skeleton"
      data-component={name}
      className={className}
    >
      <Skeleton className={size} />
    </div>
  );
};

interface ComponentFallbackProps {
  /** Plain text recovered from the component markup. */
  text: string;
  className?: string;
}

/**
 * What a component that could not render becomes: the words it contained, as
 * plain text. With no words to recover, one quiet line says so instead of
 * leaving a hole or showing raw markup.
 */
export const ComponentFallback = ({ text, className }: ComponentFallbackProps) => {
  if (!text.trim()) {
    return (
      <p
        data-testid="component-fallback"
        className={cn("text-xs text-muted-foreground", className)}
      >
        This card couldn't be shown.
      </p>
    );
  }
  return (
    <p
      data-testid="component-fallback"
      className={cn("whitespace-pre-wrap text-sm", className)}
    >
      {text}
    </p>
  );
};
