/**
 * MetricList: a few named values, each with a meter.
 *
 * What SystemStatusCard became. The meter is the shadcn Progress through
 * `Meter`; every fill is system blue. A row turns orange only when the agent
 * says `tone: "warning"` because the number is a problem for this person,
 * never because the component thinks it is high. `SystemStatusCard` stays as
 * an alias for one prompt cycle and takes the old `hostname` and `uptime`.
 */

import { Meter } from "@/components/ui/animated-components";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export interface Metric {
  label: string;
  /** 0-100 */
  value: number;
  /** Replaces the percentage: "13.1 of 16 GB". */
  detail?: string;
  tone?: "warning";
}

export interface MetricListProps {
  title?: string;
  /** Right side of the header: "Up 3d 12h". */
  meta?: string;
  metrics?: Metric[];
  /** Earlier names for `title` and `meta`. */
  hostname?: string;
  uptime?: string;
  className?: string;
}

export function MetricList({ title, meta, metrics, hostname, uptime, className }: MetricListProps) {
  const rows = (Array.isArray(metrics) ? metrics : []).filter((m) => m && typeof m.label === "string" && m.label);
  const heading = title || hostname || "System status";
  const side = meta || (uptime ? `Up ${uptime.replace(/^up:?\s*/i, "")}` : undefined);

  return (
    <Card className={className} data-testid="metric-list" data-state={rows.length === 0 ? "empty" : "ready"}>
      <CardHeader>
        <CardTitle className="min-w-0 truncate">{heading}</CardTitle>
        {side && <CardAction className="text-[11px] tabular-nums text-muted-foreground">{side}</CardAction>}
      </CardHeader>
      <CardContent>
        {rows.length === 0 ? (
          <div className="py-2 text-[13px] text-muted-foreground">Nothing to show.</div>
        ) : (
          <div className="space-y-3 pt-1">
            {rows.map((m, i) => (
              <Meter key={`${m.label}-${i}`} label={m.label} value={m.value} detail={m.detail} tone={m.tone} />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

/** The earlier tag for MetricList. */
export const SystemStatusCard = MetricList;
