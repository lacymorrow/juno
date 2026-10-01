import { useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";
import {
  ESCAPE_COALESCE_MS,
  ESCAPE_REARM_MS,
  barIsExpanded,
  escapeHandled,
  escapeOutcome,
  type BarEscapeState,
} from "@/lib/barEscape";

/**
 * Which path a press arrived on: the bar's own `document` keydown, or Rust's
 * passive stop-key monitor asking the bar to dismiss.
 */
type EscapeRoute = "dom" | "backend";

/**
 * Everything one bar appearance has to say for Escape to work. The state half
 * is the shared contract (`BarEscapeState`); the two callbacks are the only
 * parts that are the look's own.
 */
export interface EscapeToIdleOptions extends BarEscapeState {
  /** Put this look's own layers away: its card, sheet, bubble, notice, linger. */
  collapse: () => void;
  /**
   * Tell Rust the person pressed Escape, normally
   * `sendInteraction(UI.INTERACTION_TYPES_ESCAPE)`. What stopping means is
   * Rust's decision; this only reports the press.
   */
  report: () => void;
}

/**
 * # Escape returns the bar to the idle bar
 *
 * The one Escape handler, shared by every appearance. It wires both paths a
 * press can arrive on and makes them agree:
 *
 * 1. **Rust's passive Escape monitor.** `set_bar_pane_open` ref-counts a claim
 *    on the stop key; while any claim stands, an `NSEvent` monitor watches
 *    Escape without consuming it, wherever the person is typing. On a press
 *    Rust decides: something running gets the coordinated stop, nothing
 *    running gets `bar-dismiss-pane` back to the bar. Only the Pill ever armed
 *    this, which is why the other seven looks ignored Escape outright whenever
 *    the bar was not the focused window. Now every look arms it while it is
 *    expanded.
 * 2. **A `document` keydown.** Covers the focused bar even if the monitor is
 *    not up (Accessibility not granted yet, or an install that failed).
 *
 * Both routes land on one decision (`escapeOutcome`) and are coalesced, so one
 * keystroke collapses the bar exactly once.
 */
export function useEscapeToIdle(options: EscapeToIdleOptions): void {
  const { barState, working, overlayOpen, composerOpen, popupOpen } = options;

  // Read through a ref so the listeners never need re-attaching and a press
  // always sees the current state, not the state when the effect last ran.
  const latest = useRef(options);
  latest.current = options;
  const lastHandledRef = useRef<{ at: number; route: EscapeRoute }>({ at: 0, route: "dom" });

  const expanded = barIsExpanded({ barState, working, overlayOpen, composerOpen, popupOpen });

  /**
   * Act on one press. `route` is which of the two paths it arrived on: a press
   * echoed back by the other route moments later is the same keystroke, not a
   * second one, and must not collapse another layer. Two presses on the same
   * route are two presses, however fast they come.
   */
  const press = useCallback((route: EscapeRoute) => {
    const now = Date.now();
    const last = lastHandledRef.current;
    if (route !== last.route && now - last.at < ESCAPE_COALESCE_MS) return;
    const { barState, working, overlayOpen, composerOpen, popupOpen, collapse, report } =
      latest.current;
    const outcome = escapeOutcome({ barState, working, overlayOpen, composerOpen, popupOpen });
    if (!escapeHandled(outcome)) return;
    lastHandledRef.current = { at: now, route };
    if (outcome.closeLocally) collapse();
    if (outcome.report) report();
  }, []);

  const pressFromBackend = useCallback(() => press("backend"), [press]);

  // Claim the stop key while the bar is showing more than the idle bar, and
  // let go of it when it settles. The Rust ledger is idempotent and keyed by
  // name, so re-asserting is free.
  useEffect(() => {
    void invoke(COMMANDS.BAR_SET_BAR_PANE_OPEN, { open: expanded }).catch(() => {});
  }, [expanded]);

  useEffect(() => {
    if (!expanded) return;
    const id = window.setInterval(() => {
      void invoke(COMMANDS.BAR_SET_BAR_PANE_OPEN, { open: true }).catch(() => {});
    }, ESCAPE_REARM_MS);
    return () => window.clearInterval(id);
  }, [expanded]);

  // A look swapped out mid-expansion (the appearance picker) must not leave a
  // claim standing behind it.
  useEffect(
    () => () => {
      void invoke(COMMANDS.BAR_SET_BAR_PANE_OPEN, { open: false }).catch(() => {});
    },
    [],
  );

  useEventListener(EVENTS.BAR_DISMISS_PANE, pressFromBackend);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") press("dom");
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [press]);
}
