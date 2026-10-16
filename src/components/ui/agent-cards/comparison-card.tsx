/**
 * ComparisonCard: two or three options with pros, cons, a rating and a pick.
 *
 * A shadcn Card. Options sit in one column on a narrow card (the Pill, the
 * Island) and side by side from 480px, by container width, not window width.
 * The pick is the word "Recommended" in system blue beside the name; rating
 * is five dots; pros and cons are marked with "+" and a minus sign in the
 * gutter, so the words do the work colour used to.
 */

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";

interface ComparisonOption {
  name: string;
  pros?: string[];
  cons?: string[];
  /** 0-5 */
  rating?: number;
  recommended?: boolean;
}

export interface ComparisonCardProps {
  title?: string;
  options?: ComparisonOption[];
  className?: string;
}

const strings = (v: unknown): string[] => (Array.isArray(v) ? v.filter((s): s is string => typeof s === "string" && !!s) : []);

function Points({ items, mark }: { items: string[]; mark: string }) {
  if (items.length === 0) return null;
  return (
    <ul className="space-y-1">
      {items.map((text, i) => (
        <li key={`${text}-${i}`} className="grid grid-cols-[10px_1fr] gap-1.5 text-[12.5px] leading-snug">
          <span aria-hidden="true" className="text-muted-foreground">
            {mark}
          </span>
          <span className="min-w-0 break-words">
            <span className="sr-only">{mark === "+" ? "Pro: " : "Con: "}</span>
            {text}
          </span>
        </li>
      ))}
    </ul>
  );
}

export function ComparisonCard({ title = "Comparison", options, className }: ComparisonCardProps) {
  const list = (Array.isArray(options) ? options : []).filter((o) => o && typeof o.name === "string" && o.name).slice(0, 3);
  const three = list.length === 3;

  return (
    <Card className={className} data-testid="comparison-card" data-state={list.length === 0 ? "empty" : "ready"}>
      <CardHeader>
        <CardTitle className="min-w-0 truncate">{title}</CardTitle>
      </CardHeader>
      <CardContent className="@container">
        {list.length === 0 ? (
          <div className="py-2 text-[13px] text-muted-foreground">Nothing to compare.</div>
        ) : (
          <div className={cn("grid gap-x-5 pt-1", three ? "@[30rem]:grid-cols-3" : "@[30rem]:grid-cols-2")}>
            {list.map((opt, i) => {
              const rating = typeof opt.rating === "number" ? Math.min(5, Math.max(0, Math.round(opt.rating))) : undefined;
              return (
                <section
                  key={`${opt.name}-${i}`}
                  aria-label={opt.name}
                  className={cn(
                    "min-w-0 space-y-2 border-t border-[var(--sep,var(--border))] py-3 first:border-t-0 first:pt-1",
                    "@[30rem]:border-t-0 @[30rem]:pt-1 @[30rem]:pb-2",
                    three
                      ? "@[30rem]:[&:not(:first-child)]:border-l @[30rem]:[&:not(:first-child)]:border-[var(--sep,var(--border))] @[30rem]:[&:not(:first-child)]:pl-5"
                      : "@[30rem]:[&:nth-child(even)]:border-l @[30rem]:[&:nth-child(even)]:border-[var(--sep,var(--border))] @[30rem]:[&:nth-child(even)]:pl-5",
                  )}
                >
                  <div className="flex flex-wrap items-baseline gap-x-2">
                    <span className="min-w-0 break-words text-[13px] font-semibold">{opt.name}</span>
                    {opt.recommended && <span className="text-[11px] font-medium text-primary">Recommended</span>}
                  </div>
                  {rating !== undefined && (
                    <div role="img" aria-label={`Rated ${rating} of 5`} className="flex gap-[3px]">
                      {Array.from({ length: 5 }).map((_, j) => (
                        <span
                          key={j}
                          className={cn("size-1.5 rounded-full", j < rating ? "bg-primary" : "bg-[var(--fill-2,var(--muted))]")}
                        />
                      ))}
                    </div>
                  )}
                  <Points items={strings(opt.pros)} mark="+" />
                  <Points items={strings(opt.cons)} mark={"−"} />
                </section>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
