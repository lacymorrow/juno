import React from "react";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import { ChevronRight, HelpCircle } from "lucide-react";
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
  /**
   * Fold the card away under its title, closed until clicked. For groups
   * most people never touch. The rows stay mounted (just hidden), so search
   * and the agent can still find one and open the group to show it.
   */
  collapsible?: boolean;
  /** Start a collapsible group open, e.g. when something in it needs attention. */
  defaultOpen?: boolean;
  /** One short line beside a closed group's title saying what's inside. */
  summary?: React.ReactNode;
}

/**
 * Fired on a settings row (bubbling) to ask the folded group around it to
 * open. `revealRow` sends it before scrolling to the row.
 */
export const SETTINGS_REVEAL_EVENT = "settings:reveal";

export function SettingsGroup({
  title,
  footer,
  children,
  className,
  advanced = false,
  collapsible = false,
  defaultOpen = false,
  summary,
}: SettingsGroupProps) {
  const { advanced: showAdvanced } = useAdvancedSettings();
  const [open, setOpen] = React.useState(defaultOpen);
  const sectionRef = React.useRef<HTMLElement>(null);
  const contentId = React.useId();

  // Follow `defaultOpen` as it changes (data loads, a microphone is unplugged)
  // until the person has used the group; after that it only ever opens, so it
  // never folds away under their hands.
  const touched = React.useRef(false);
  React.useEffect(() => {
    if (defaultOpen) setOpen(true);
    else if (!touched.current) setOpen(false);
  }, [defaultOpen]);

  React.useEffect(() => {
    const el = sectionRef.current;
    if (!collapsible || !el) return;
    const reveal = () => {
      touched.current = true;
      setOpen(true);
    };
    el.addEventListener(SETTINGS_REVEAL_EVENT, reveal);
    return () => el.removeEventListener(SETTINGS_REVEAL_EVENT, reveal);
  }, [collapsible]);

  if (advanced && !showAdvanced) return null;

  const shown = !collapsible || open;

  return (
    <section ref={sectionRef} className={cn("space-y-1.5", className)}>
      {title &&
        (collapsible ? (
          <h3 className="text-[13px] font-semibold text-muted-foreground">
            <button
              type="button"
              aria-expanded={open}
              aria-controls={contentId}
              onClick={() => {
                touched.current = true;
                setOpen((o) => !o);
              }}
              className="flex w-full items-center gap-1 rounded-[4px] px-1 text-left transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <ChevronRight
                aria-hidden="true"
                className={cn(
                  "h-3.5 w-3.5 shrink-0 transition-transform duration-150 motion-reduce:transition-none",
                  open && "rotate-90",
                )}
              />
              <span>{title}</span>
              {!open && summary && (
                <span className="ml-auto min-w-0 truncate pl-3 text-[12px] font-normal">
                  {summary}
                </span>
              )}
            </button>
          </h3>
        ) : (
          <h3 className="px-1 text-[13px] font-semibold text-muted-foreground">
            {title}
          </h3>
        ))}
      <div
        id={contentId}
        hidden={!shown}
        className="space-y-1.5"
        onFocusCapture={() => {
          touched.current = true;
        }}
      >
        <div className="overflow-hidden rounded-[10px] border border-border bg-card shadow-sm">
          <div className="divide-y divide-border">{children}</div>
        </div>
        {footer && (
          <p className="px-1 text-[12px] leading-snug text-muted-foreground">
            {footer}
          </p>
        )}
      </div>
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
  /**
   * Unlocked by the Beta features switch: marks the label with a "Beta" badge
   * so the person can see what the switch turned on. The caller still gates
   * the row on `useBetaFeatures`.
   */
  beta?: boolean;
}

/** The small flat tag beside a beta row's label. */
export function BetaBadge() {
  return (
    <Badge
      variant="outline"
      className="rounded-[4px] px-1 py-0 text-[10px] font-medium leading-[14px] text-muted-foreground"
    >
      Beta
    </Badge>
  );
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
  beta = false,
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
          "scroll-mt-16 outline outline-2 -outline-offset-2 outline-transparent transition-[background-color,outline-color] duration-500 motion-reduce:transition-none",
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
                  {beta && <BetaBadge />}
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
