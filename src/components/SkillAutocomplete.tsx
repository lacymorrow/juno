/**
 * Slash-command autocomplete UI (LAC-3031).
 *
 * Two pieces, shared by every input surface:
 *   - SkillSuggestionList — the dropdown of fuzzy-matched skills
 *   - SkillGhostText      — inline grey completion overlaid on the input
 *
 * Pure display components: all matching/selection state comes from
 * `useSkillAutocomplete`. Two visual variants: "bar" (dark translucent, for
 * the floating dynamic island) and "chat" (shadcn popover styling, for the
 * main chat window).
 */

import { Fragment } from "react";
import { cn } from "@/lib/utils";
import type { SkillSuggestion } from "@/hooks/useSkillAutocomplete";

type Variant = "bar" | "chat";

/** Bold the characters of `name` that matched the typed query. */
const HighlightedName = ({
  name,
  positions,
  variant,
}: {
  name: string;
  positions: number[];
  variant: Variant;
}) => {
  const matched = new Set(positions);
  return (
    <span className="truncate">
      {Array.from(name).map((ch, i) => (
        <Fragment key={`${i}-${ch}`}>
          {matched.has(i) ? (
            <span
              className={cn(
                "font-medium",
                variant === "bar" ? "text-white" : "text-foreground"
              )}
            >
              {ch}
            </span>
          ) : (
            ch
          )}
        </Fragment>
      ))}
    </span>
  );
};

export interface SkillSuggestionListProps {
  suggestions: SkillSuggestion[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onHighlight: (index: number) => void;
  variant: Variant;
  className?: string;
}

export const SkillSuggestionList = ({
  suggestions,
  selectedIndex,
  onSelect,
  onHighlight,
  variant,
  className,
}: SkillSuggestionListProps) => {
  if (suggestions.length === 0) return null;

  const isBar = variant === "bar";

  return (
    <div
      role="listbox"
      aria-label="Skill suggestions"
      className={cn(
        "overflow-hidden py-1",
        isBar
          ? "rounded-xl border border-white/[0.08] bg-[#141416]/95 backdrop-blur-xl shadow-[0_8px_32px_rgba(0,0,0,0.45)]"
          : "rounded-md border bg-popover text-popover-foreground shadow-md",
        className
      )}
    >
      {suggestions.map((s, i) => {
        const isSelected = i === selectedIndex;
        return (
          <button
            key={s.item.name}
            type="button"
            role="option"
            aria-selected={isSelected}
            // preventDefault keeps focus in the input while clicking.
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => onSelect(i)}
            onMouseEnter={() => onHighlight(i)}
            className={cn(
              "flex w-full items-baseline gap-2 px-3 py-1.5 text-left outline-none transition-colors duration-75",
              isBar
                ? cn(
                    "text-[12px] tracking-[-0.01em] text-white/60",
                    isSelected && "bg-white/[0.08] text-white/90"
                  )
                : cn(
                    "text-sm text-muted-foreground",
                    isSelected && "bg-accent text-accent-foreground"
                  )
            )}
          >
            <span className="flex min-w-0 shrink-0 items-baseline">
              <span className={isBar ? "text-white/30" : "text-muted-foreground/60"}>
                /
              </span>
              <HighlightedName
                name={s.item.name}
                positions={s.match.positions}
                variant={variant}
              />
            </span>
            {s.item.description && (
              <span
                className={cn(
                  "min-w-0 flex-1 truncate",
                  isBar
                    ? "text-[11px] text-white/25"
                    : "text-xs text-muted-foreground/70"
                )}
              >
                {s.item.description}
              </span>
            )}
            {isSelected && (
              <span
                className={cn(
                  "ml-auto shrink-0 select-none text-[10px] tracking-[0.04em]",
                  isBar ? "text-white/25" : "text-muted-foreground/50"
                )}
              >
                tab
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
};

export interface SkillGhostTextProps {
  /** Exact text currently in the input. */
  value: string;
  /** Grey remainder to render after it. */
  ghostText: string;
  /**
   * Classes replicating the input's font, padding, and tracking so the
   * overlay aligns glyph-for-glyph. Caller owns the metrics.
   */
  className?: string;
  ghostClassName?: string;
}

/**
 * Inline completion overlay. Render inside a `relative` wrapper around the
 * input; the typed portion is invisible (it only reserves width) and the
 * remainder shows dimmed, exactly where the user's caret is.
 */
export const SkillGhostText = ({
  value,
  ghostText,
  className,
  ghostClassName,
}: SkillGhostTextProps) => {
  if (!ghostText) return null;
  return (
    <div
      aria-hidden="true"
      className={cn(
        "pointer-events-none absolute inset-0 overflow-hidden whitespace-pre",
        className
      )}
    >
      <span className="invisible">{value}</span>
      <span className={ghostClassName}>{ghostText}</span>
    </div>
  );
};
