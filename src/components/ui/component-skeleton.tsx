import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

/**
 * Stand-in for an agent component whose markup is still streaming in.
 *
 * It is a card: the same hairline, radius and header row as the real thing, a
 * title line, a meta line and three body lines on a 1.2s opacity pulse, held
 * to roughly the height of the component it will become so the swap does not
 * shove the conversation around. Small inline pieces (badges, stats, buttons)
 * get a pill; anything unknown gets a medium card.
 */

/** Approximate rendered height of each block-level component, in px. */
const CARD_HEIGHTS: Record<string, number> = {
  WeatherCard: 150,
  NowPlayingCard: 108,
  AgendaCard: 96,
  MessageCard: 128,
  PermissionAskCard: 56,
  ImageCard: 260,
  FileListCard: 190,
  MetricList: 150,
  SystemStatusCard: 150,
  ComparisonCard: 200,
  LinkCard: 56,
  TaskSummaryCard: 160,
  AnimatedCard: 120,
  Card: 120,
  Form: 200,
  Alert: 56,
  AnimatedProgress: 40,
  Meter: 40,
  Table: 128,
  MiniChart: 96,
  Tabs: 128,
};

const DEFAULT_HEIGHT = 96;

/** Cards this short get one line, not a header and three. */
const COMPACT_BELOW = 64;

/** Components that render inline and small. */
const INLINE_COMPONENTS = new Set([
  "Badge",
  "GlowBadge",
  "Stat",
  "Stats",
  "AnimatedNumber",
  "ShimmerText",
  "Confetti",
  "PulseRing",
  "Button",
  "OpenButton",
  "QueryButton",
  "CopyButton",
  "Actions",
]);

interface ComponentSkeletonProps {
  /** Tag name of the component that is arriving, if known. */
  name?: string;
  className?: string;
}

const Line = ({ width, className }: { width: string; className?: string }) => (
  <Skeleton className={cn("h-2.5 rounded-full", className)} style={{ width }} />
);

export const ComponentSkeleton = ({ name, className }: ComponentSkeletonProps) => {
  const inline = name ? INLINE_COMPONENTS.has(name) : false;
  const height = (name && CARD_HEIGHTS[name]) || DEFAULT_HEIGHT;

  return (
    // `agent-content` carries the tokens: the skeleton is drawn before any card is.
    <div
      role="status"
      aria-busy="true"
      aria-label="Loading card"
      data-testid="component-skeleton"
      data-component={name}
      className={cn("agent-content", className)}
    >
      {inline ? (
        <Skeleton className="h-6 w-32 rounded-full" />
      ) : height < COMPACT_BELOW ? (
        <Card style={{ minHeight: height }}>
          <CardContent className="flex items-center py-3">
            <Line width="60%" />
          </CardContent>
        </Card>
      ) : (
        <Card style={{ minHeight: height }}>
          <CardHeader>
            <div className="flex items-center justify-between gap-6">
              <Line width="38%" />
              <Line width="12%" />
            </div>
          </CardHeader>
          <CardContent className="space-y-2.5 pt-2">
            <Line width="72%" />
            <Line width="58%" />
            <Line width="64%" />
          </CardContent>
        </Card>
      )}
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
        className={cn("text-[11px] text-muted-foreground", className)}
      >
        This card couldn't be shown.
      </p>
    );
  }
  return (
    <p
      data-testid="component-fallback"
      className={cn("whitespace-pre-wrap text-[13px]", className)}
    >
      {text}
    </p>
  );
};
