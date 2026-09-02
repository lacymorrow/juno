import React, { createContext, useContext } from "react";
import { cn } from "@/lib/utils";

/**
 * Progressive-disclosure settings primitives (reUI-inspired).
 *
 * The design goal (LAC-3691): make Juno feel basic and user-friendly like
 * Claude or Wispr Flow. Essentials are always visible; anything tagged
 * `advanced` is hidden until the user opts into "Show advanced settings".
 *
 * Layout follows the reUI settings block pattern — a titled `SettingsGroup`
 * containing divided `SettingsRow`s (label + description on the left, the
 * control on the right). This replaces the ad-hoc `flex justify-between`
 * markup that was duplicated across every section.
 */

interface AdvancedContextValue {
  /** Whether advanced settings are currently revealed. */
  showAdvanced: boolean;
}

const AdvancedContext = createContext<AdvancedContextValue>({
  showAdvanced: false,
});

export function AdvancedSettingsProvider({
  showAdvanced,
  children,
}: {
  showAdvanced: boolean;
  children: React.ReactNode;
}) {
  return (
    <AdvancedContext.Provider value={{ showAdvanced }}>
      {children}
    </AdvancedContext.Provider>
  );
}

export function useAdvancedSettings(): boolean {
  return useContext(AdvancedContext).showAdvanced;
}

interface SettingsGroupProps {
  title?: string;
  description?: string;
  /** Hide the whole group unless advanced settings are revealed. */
  advanced?: boolean;
  children: React.ReactNode;
  className?: string;
}

/**
 * A titled group of settings rows. Renders as a single bordered surface with
 * dividers between rows (see `SettingsRow`). Uses semantic tokens so it adapts
 * to light/dark automatically.
 */
export function SettingsGroup({
  title,
  description,
  advanced = false,
  children,
  className,
}: SettingsGroupProps) {
  const showAdvanced = useAdvancedSettings();
  if (advanced && !showAdvanced) return null;

  // Drop rows that are advanced-only when advanced mode is off, so a group
  // never renders as an empty shell.
  const visibleChildren = React.Children.toArray(children).filter((child) => {
    if (!React.isValidElement(child)) return true;
    const props = child.props as { advanced?: boolean };
    return !props.advanced || showAdvanced;
  });

  if (visibleChildren.length === 0) return null;

  return (
    <section className={cn("space-y-3", className)}>
      {(title || description) && (
        <div className="space-y-0.5 px-1">
          {title && (
            <h3 className="text-sm font-semibold text-foreground">{title}</h3>
          )}
          {description && (
            <p className="text-xs text-muted-foreground">{description}</p>
          )}
        </div>
      )}
      <div className="divide-y divide-border rounded-xl border border-border bg-card">
        {visibleChildren}
      </div>
    </section>
  );
}

interface SettingsRowProps {
  label?: string;
  description?: React.ReactNode;
  htmlFor?: string;
  /** Hide this row unless advanced settings are revealed. */
  advanced?: boolean;
  /** Right-aligned control (switch, select, button…). */
  control?: React.ReactNode;
  /** Optional full-width content rendered below the label/control line. */
  children?: React.ReactNode;
  className?: string;
}

/**
 * A single settings row: label + description on the left, control on the right.
 * Stacks the control below on narrow widths. Pass `children` for controls that
 * need the full row width (sliders, nested panels).
 */
export function SettingsRow({
  label,
  description,
  htmlFor,
  advanced = false,
  control,
  children,
  className,
}: SettingsRowProps) {
  const showAdvanced = useAdvancedSettings();
  if (advanced && !showAdvanced) return null;

  return (
    <div className={cn("px-4 py-3.5", className)}>
      <div className="flex items-start justify-between gap-4">
        {(label || description) && (
          <div className="min-w-0 space-y-0.5">
            {label && (
              <label
                htmlFor={htmlFor}
                className="block text-sm font-medium text-foreground"
              >
                {label}
              </label>
            )}
            {description && (
              <p className="text-xs leading-relaxed text-muted-foreground">
                {description}
              </p>
            )}
          </div>
        )}
        {control && <div className="shrink-0">{control}</div>}
      </div>
      {children && <div className="mt-3">{children}</div>}
    </div>
  );
}
