/**
 * ListRow: one row of a list inside a card's content.
 *
 * A thin composition, not a primitive of its own: a 16px glyph in the label
 * colour, a primary line, an optional second line, and a trailing value in
 * tabular figures. A hairline separates rows; the first has none.
 */

import type * as React from "react";
import { cn } from "@/lib/utils";

export interface ListRowProps {
  icon?: React.ReactNode;
  primary: React.ReactNode;
  secondary?: React.ReactNode;
  trailing?: React.ReactNode;
  className?: string;
}

export function ListRow({ icon, primary, secondary, trailing, className }: ListRowProps) {
  return (
    <li
      className={cn(
        "flex items-center gap-2.5 border-t border-[var(--sep,var(--border))] py-[7px] text-[13px] first:border-t-0",
        className,
      )}
    >
      {icon && <span className="shrink-0 text-muted-foreground [&_svg]:size-4">{icon}</span>}
      <div className="min-w-0 flex-1">
        <div className="truncate">{primary}</div>
        {secondary && <div className="truncate text-[11px] text-muted-foreground">{secondary}</div>}
      </div>
      {trailing && <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">{trailing}</span>}
    </li>
  );
}
