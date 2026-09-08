import React from "react";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import { useAdvancedSettings } from "./AdvancedSettingsContext";

/**
 * macOS System Settings-style primitives.
 *
 * A screen is a stack of `SettingsGroup`s; each group is a rounded, inset
 * card whose `SettingsRow` children are separated by hairline dividers. A
 * row puts its label (and optional description) on the left and its control
 * on the right, matching the native two-column list.
 */

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

interface SettingsRowProps {
  label?: React.ReactNode;
  description?: React.ReactNode;
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
  htmlFor,
  children,
  below,
  className,
  advanced = false,
  destructive = false,
}: SettingsRowProps) {
  const { advanced: showAdvanced } = useAdvancedSettings();
  if (advanced && !showAdvanced) return null;

  return (
    <div className={cn("px-4 py-2.5", className)}>
      <div className="flex min-h-[28px] items-center justify-between gap-4">
        {(label || description) && (
          <div className="min-w-0 flex-1 space-y-0.5">
            {label && (
              <Label
                htmlFor={htmlFor}
                className={cn(
                  "block text-[13px] font-medium leading-tight",
                  destructive && "text-destructive",
                )}
              >
                {label}
              </Label>
            )}
            {description && (
              <p className="text-[12px] leading-snug text-muted-foreground">
                {description}
              </p>
            )}
          </div>
        )}
        {children && <div className="shrink-0">{children}</div>}
      </div>
      {below && <div className="mt-3">{below}</div>}
    </div>
  );
}
