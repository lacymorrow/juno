import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";
import {
  ESCAPE_COALESCE_MS,
  ESCAPE_REARM_MS,
  LADDER_CHAIN_MS,
  PRESENTED_MS,
  barIsExpanded,
  climbTarget,
  escapeHandled,
  escapeHidesBar,
  escapeOutcome,
  outsidePressStep,
  type BarEscapeState,
  type ClimbTarget,
  type LadderStep,
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
  /**
   * One rung up the ladder, asked for by the Dock or tray icon (or by Escape
   * in the big chat, which steps down to the floating chat). `composer`: open
   * this look's text input, focused and ready to type. `floating`: open this
   * look's chat (the Pill's pane, a card, an answer), or the text input when
   * there is nothing to show yet.
   */
  climb: (to: ClimbTarget) => void;
  /**
   * The top rung, for a look that holds the big chat itself (the Island). A
   * look without one gets the full-size chat window instead.
   */
  openFullChat?: () => void;
  /**
   * Something is typed or pasted in this look's box. A press outside the bar
   * then leaves the chat open once before putting it away (`outsidePressStep`).
   */
  draftPresent?: boolean;
  /**
   * Straight to the idle bar, draft kept, for a press outside. Defaults to
   * `collapse`; a look whose `collapse` steps down one rung at a time (the
   * Island) passes its put-away.
   */
  putAway?: () => void;
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
 *
 * It is also the bar's half of the visibility ladder. A press the bar has
 * nothing else for, on the idle bar, takes the bar off the screen, but only
 * when it was aimed at Juno (see `escapeHidesBar`). And `bar-ladder` from Rust
 * climbs the other way through the look's `climb`.
 */
export function useEscapeToIdle(options: EscapeToIdleOptions): void {
  const { barState, working, overlayOpen, composerOpen, popupOpen } = options;

  // Read through a ref so the listeners never need re-attaching and a press
  // always sees the current state, not the state when the effect last ran.
  const latest = useRef(options);
  latest.current = options;
  const lastHandledRef = useRef<{ at: number; route: EscapeRoute }>({ at: 0, route: "dom" });

  const expanded = barIsExpanded({ barState, working, overlayOpen, composerOpen, popupOpen });

  // Just after a press did something, the next one is the same walk down the
  // ladder, and the stop key stays claimed so it can be seen from any app.
  const [chainOpen, setChainOpen] = useState(false);
  const chainTimer = useRef<number | undefined>(undefined);
  const openChain = useCallback(() => {
    setChainOpen(true);
    window.clearTimeout(chainTimer.current);
    chainTimer.current = window.setTimeout(() => setChainOpen(false), LADDER_CHAIN_MS);
  }, []);
  useEffect(() => () => window.clearTimeout(chainTimer.current), []);

  // Just after Juno came onto the screen by herself (launch, setup ending),
  // she is what an Escape is aimed at, though the bar was never focused: the
  // stop key is claimed so the press can be seen at all, and the press puts
  // her away. A press elsewhere, or any press she handled, ends it sooner.
  const [presented, setPresented] = useState(false);
  const presentedRef = useRef(false);
  presentedRef.current = presented;
  const presentedTimer = useRef<number | undefined>(undefined);
  const endPresented = useCallback(() => {
    window.clearTimeout(presentedTimer.current);
    setPresented(false);
  }, []);
  useEventListener(EVENTS.BAR_PRESENTED, () => {
    setPresented(true);
    window.clearTimeout(presentedTimer.current);
    presentedTimer.current = window.setTimeout(() => setPresented(false), PRESENTED_MS);
  });
  useEffect(() => () => window.clearTimeout(presentedTimer.current), []);
  const claimed = expanded || chainOpen || presented;

  /**
   * Act on one press. `route` is which of the two paths it arrived on: a press
   * echoed back by the other route moments later is the same keystroke, not a
   * second one, and must not collapse another layer. Two presses on the same
   * route are two presses, however fast they come.
   */
  const press = useCallback(
    (route: EscapeRoute, aimedAtJuno = true) => {
      const now = Date.now();
      const last = lastHandledRef.current;
      if (route !== last.route && now - last.at < ESCAPE_COALESCE_MS) return;
      const { barState, working, overlayOpen, composerOpen, popupOpen, collapse, report } =
        latest.current;
      const state = { barState, working, overlayOpen, composerOpen, popupOpen };
      const outcome = escapeOutcome(state);
      const wasPresented = aimedAtJuno && presentedRef.current;
      if (presentedRef.current) endPresented();
      if (escapeHandled(outcome)) {
        lastHandledRef.current = { at: now, route };
        openChain();
        if (outcome.closeLocally) collapse();
        if (outcome.report) report();
        return;
      }
      const hides = escapeHidesBar(state, {
        focused: route === "dom",
        chained: aimedAtJuno && now - last.at < LADDER_CHAIN_MS,
        presented: wasPresented,
      });
      if (!hides) return;
      lastHandledRef.current = { at: now, route };
      void invoke(COMMANDS.WINDOWS_HIDE_FLOATING_BAR).catch(() => {});
    },
    [openChain, endPresented],
  );

  // The same event also says "the big chat took over" (`reason: main_window`).
  // That collapses the bar like a press, but it was not typed, so it never
  // continues a walk down the ladder into hiding the bar.
  const pressFromBackend = useCallback(
    (payload: { reason?: string } | null | undefined) =>
      press("backend", payload?.reason !== "main_window"),
    [press],
  );

  // Claim the stop key while the bar is showing more than the idle bar, and
  // let go of it when it settles. The Rust ledger is idempotent and keyed by
  // name, so re-asserting is free.
  useEffect(() => {
    void invoke(COMMANDS.BAR_SET_BAR_PANE_OPEN, { open: claimed }).catch(() => {});
  }, [claimed]);

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

  // ── A press outside the bar ──
  //
  // Rust reports a fresh mouse press anywhere the bar is not drawing (never
  // while the agent drives the mouse). Each one walks the ladder a rung:
  // `outsidePressStep` decides, and a click never stops Juno's work.
  //
  // The same click often also takes focus from the bar, and the looks fold an
  // empty composer on focus loss. A press arriving just after the bar's layers
  // closed is that same click, not a second one, so it does not hide the bar.
  const layersOpen = overlayOpen || composerOpen;
  const sparedRef = useRef(false);
  const layersClosedAtRef = useRef(0);
  const layersOpenRef = useRef(layersOpen);
  useEffect(() => {
    if (layersOpenRef.current && !layersOpen) {
      layersClosedAtRef.current = Date.now();
      sparedRef.current = false;
    }
    layersOpenRef.current = layersOpen;
  }, [layersOpen]);
  // Back in the bar (typing, clicking in it): the next click away is a first one.
  useEffect(() => {
    const engaged = () => {
      sparedRef.current = false;
    };
    document.addEventListener("keydown", engaged);
    document.addEventListener("pointerdown", engaged);
    return () => {
      document.removeEventListener("keydown", engaged);
      document.removeEventListener("pointerdown", engaged);
    };
  }, []);
  useEventListener(EVENTS.SYSTEM_MOUSE_PRESSED_OUTSIDE, () => {
    // A press elsewhere also ends the window just after Juno appeared.
    endPresented();
    if (Date.now() - layersClosedAtRef.current < ESCAPE_COALESCE_MS) return;
    const { barState, working, overlayOpen, composerOpen, popupOpen } = latest.current;
    const { collapse, putAway, report, draftPresent = false } = latest.current;
    const state = { barState, working, overlayOpen, composerOpen, popupOpen };
    const step = outsidePressStep(state, { draftPresent, spared: sparedRef.current });
    if (step === "spare") {
      sparedRef.current = true;
    } else if (step === "collapse") {
      (putAway ?? collapse)();
      // Rust holds a composer's input state; settle it, but only when nothing
      // is running, since reporting is what stops work.
      if (composerOpen && !working) report();
    }
  });

  // One rung up: the Dock or tray icon, or Escape in the big chat.
  useEventListener<{ to: LadderStep }>(EVENTS.BAR_LADDER, (payload) => {
    const { barState, working, overlayOpen, composerOpen, popupOpen, climb } = latest.current;
    const target = climbTarget(payload.to, {
      barState,
      working,
      overlayOpen,
      composerOpen,
      popupOpen,
    });
    if (target === "full") {
      const { openFullChat } = latest.current;
      if (openFullChat) openFullChat();
      else void invoke(COMMANDS.WINDOWS_OPEN_MAIN_WINDOW).catch(() => {});
    } else {
      climb(target);
    }
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") press("dom");
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [press]);
}
export type { ClimbTarget } from "@/lib/barEscape";

/**
 * Open a look's text input the way its own type button does: make the bar the
 * key window so typing lands in it, then click the idle bar, which Rust takes
 * to its input state. For the looks whose composer is Rust's input state.
 */
export function openComposerThroughRust(sendClick: () => void): void {
  getCurrentWindow()
    .setFocus()
    .catch((error) => console.debug("Bar: window activation failed:", error));
  sendClick();
}
