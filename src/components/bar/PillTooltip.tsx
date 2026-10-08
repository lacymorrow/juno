/**
 * A small, native-feeling tooltip for the Pill's controls.
 *
 * Two ways in, because the bar is looked at while another app is active:
 * - DOM hover, which Radix handles with the provider's delay. Works only while
 *   Juno is the active app.
 * - `forcedOpen`, the button under the cursor as the native tracking area
 *   reports it (`mouse-moved-window`). macOS does not route mouse-moved into
 *   an inactive window's web view, so without this a tooltip would never show
 *   in the case that matters most. It opens after the same delay.
 *
 * Placement: the Pill's window is its largest footprint (pane included), so
 * there is always room on the side the pill grows toward. The caller passes
 * that side; Radix keeps the tooltip inside the window if it would cross an
 * edge. The trigger keeps its `title` as a fallback, dropped only while this
 * tooltip is showing so the two never appear together.
 */

import { cloneElement, useEffect, useState, type ReactElement } from "react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

/** Matches macOS: long enough not to flash on a passing cursor. */
export const PILL_TOOLTIP_DELAY_MS = 500;

/** True once `flag` has held for `delayMs`; false the moment it drops. */
function useDelayedFlag(flag: boolean, delayMs: number): boolean {
  const [held, setHeld] = useState(false);
  useEffect(() => {
    if (!flag) {
      setHeld(false);
      return;
    }
    const timer = setTimeout(() => setHeld(true), delayMs);
    return () => clearTimeout(timer);
  }, [flag, delayMs]);
  return held;
}

export function PillTooltip({
  label,
  detail,
  forcedOpen = false,
  side,
  sideOffset = 6,
  children,
}: {
  label: string;
  /** Secondary text, e.g. the shortcut ("Hold 🌐"). */
  detail?: string | null;
  /** The native tracking area says the cursor is over this control. */
  forcedOpen?: boolean;
  side: "top" | "bottom";
  /** Gap between the trigger and the tooltip. */
  sideOffset?: number;
  children: ReactElement<{ title?: string }>;
}) {
  const [hoverOpen, setHoverOpen] = useState(false);
  const forced = useDelayedFlag(forcedOpen, PILL_TOOLTIP_DELAY_MS);
  // A press hides the tooltip the way a native one goes away on click, and it
  // stays away until the cursor leaves the control.
  const [pressed, setPressed] = useState(false);
  useEffect(() => {
    if (!forcedOpen) setPressed(false);
  }, [forcedOpen]);

  const open = !pressed && (hoverOpen || forced);
  const trigger = cloneElement(children, { title: open ? undefined : label });

  return (
    <Tooltip open={open} onOpenChange={setHoverOpen}>
      <TooltipTrigger
        asChild
        onPointerDown={() => setPressed(true)}
        onPointerLeave={() => setPressed(false)}
      >
        {trigger}
      </TooltipTrigger>
      <TooltipContent
        side={side}
        sideOffset={sideOffset}
        collisionPadding={6}
        arrow={false}
        data-testid="pill-tooltip"
        className="pointer-events-none flex items-center gap-1.5 rounded-md border border-white/10 bg-neutral-900/95 px-2 py-1 text-[11px] font-medium leading-4 text-white/90 shadow-sm"
      >
        <span>{label}</span>
        {detail ? <span className="text-white/45">{detail}</span> : null}
      </TooltipContent>
    </Tooltip>
  );
}
