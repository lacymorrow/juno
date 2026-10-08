import React from "react";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import { HelpCircle } from "lucide-react";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useAdvancedSettings } from "./AdvancedSettingsContext";

/**
 * macOS System Settings-style primitives.
 *
 * A screen is a stack of `SettingsGroup`s; each group is a rounded, inset
 * card whose `SettingsRow` children are separated by hairline dividers. A
 * row puts its label (and optional description) on the left and its control
 * on the right, matching the native two-column list.
 */

/**
 * A small (?) that reveals longer explanation on hover or keyboard focus.
 * Keeps rows to a title plus one short line.
 */
export function InfoTip({
  children,
  label = "More info",
}: {
  children: React.ReactNode;
  label?: string;
}) {
  return (
    <TooltipProvider>
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          className="inline-flex h-4 w-4 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <HelpCircle className="h-3.5 w-3.5" aria-hidden="true" />
        </button>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-[260px] text-[12px] leading-snug">
        {children}
      </TooltipContent>
    </Tooltip>
    </TooltipProvider>
  );
}

interface SettingsGroupProps {
  /** Small heading shown above the card. */
  title?: string;
  /** Muted helper text shown beneath the card. */
  footer?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  /** Only render while the advanced-settings toggle is on. */
  advanced?: boolean;
}

export function SettingsGroup({
  title,
  footer,
  children,
  className,
  advanced = false,
}: SettingsGroupProps) {
  const { advanced: showAdvanced } = useAdvancedSettings();
  if (advanced && !showAdvanced) return null;

  return (
    <section className={cn("space-y-1.5", className)}>
      {title && (
        <h3 className="px-1 text-[13px] font-semibold text-muted-foreground">
          {title}
        </h3>
      )}
      <div className="overflow-hidden rounded-[10px] border border-border bg-card shadow-sm">
        <div className="divide-y divide-border">{children}</div>
      </div>
      {footer && (
        <p className="px-1 text-[12px] leading-snug text-muted-foreground">
          {footer}
        </p>
      )}
    </section>
  );
}

/** Prefix for the DOM anchor id set on a searchable row. */
export const SETTINGS_ROW_ID_PREFIX = "settings-row-";

interface SettingsRowProps {
  label?: React.ReactNode;
  description?: React.ReactNode;
  /** Longer explanation, shown in a (?) tooltip beside the label. */
  info?: React.ReactNode;
  /**
   * Stable anchor key so search deep-linking can scroll to and highlight this
   * row. Rendered as `settings-row-<id>`. Falls back to `htmlFor` when omitted,
   * so most control rows are addressable without extra wiring.
   */
  id?: string;
  /** Associates the label with a control for accessibility. */
  htmlFor?: string;
  /** The control shown at the right edge of the row. */
  children?: React.ReactNode;
  /** Full-width content rendered beneath the label/control line (sliders, etc.). */
  below?: React.ReactNode;
  className?: string;
  /** Only render while the advanced-settings toggle is on. */
  advanced?: boolean;
  /** Render the label in the destructive colour (danger actions). */
  destructive?: boolean;
}

export function SettingsRow({
  label,
  description,
  info,
  id,
  htmlFor,
  children,
  below,
  className,
  advanced = false,
  destructive = false,
}: SettingsRowProps) {
  const { advanced: showAdvanced } = useAdvancedSettings();
  if (advanced && !showAdvanced) return null;

  const hasTopLine = Boolean(label || description || children);
  const anchor = id ?? htmlFor;

  return (
    <div
      id={anchor ? `${SETTINGS_ROW_ID_PREFIX}${anchor}` : undefined}
      className={cn(
        "px-4 py-2.5",
        // Keep a deep-linked row clear of the drag band when scrolled into
        // view; the colour transition is the fade of `revealRow`'s tint.
        anchor &&
          "scroll-mt-16 transition-colors duration-300 motion-reduce:transition-none",
        className,
      )}
    >
      {hasTopLine && (
        <div className="flex min-h-[28px] flex-wrap items-center justify-between gap-x-4 gap-y-2">
          {(label || description) && (
            <div className="min-w-[14rem] flex-1 space-y-0.5">
              {label && (
                <div className="flex items-center gap-1.5">
                  <Label
                    htmlFor={htmlFor}
                    className={cn(
                      "block text-[13px] font-medium leading-tight",
                      destructive && "text-destructive",
                    )}
                  >
                    {label}
                  </Label>
                  {info && (
                    <InfoTip>{info}</InfoTip>
                  )}
                </div>
              )}
              {description && (
                <p className="text-[12px] leading-snug text-muted-foreground">
                  {description}
                </p>
              )}
            </div>
          )}
          {children && <div className="max-w-full shrink-0">{children}</div>}
        </div>
      )}
      {below && <div className={cn(hasTopLine && "mt-3")}>{below}</div>}
    </div>
  );
}
