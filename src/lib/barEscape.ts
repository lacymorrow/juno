import { UI } from "@/lib/constants.generated";

/**
 * # Escape, decided once
 *
 * Escape is the universal cancel key (`settings.rs`: fixed, not configurable).
 * Whatever the bar is showing, one press puts it back to the tiny idle bar.
 *
 * Every appearance used to hand-roll that for itself: a three-rung ladder of
 * `working` / `some overlay` / `composer`, with each look naming a different
 * set of locals on the middle rung and a different idea of what counts as
 * work. A look sitting in a state its own ladder forgot swallowed the key, and
 * Escape did nothing. Worse, the ladders only ran on a `document` keydown, so
 * they only ran at all while the bar window had keyboard focus, which a
 * floating panel usually does not.
 *
 * So the decision lives here, once, and it is deliberately small, because
 * Rust owns the logic:
 *
 * - React closes the layers React opened. A lingering answer card is local
 *   state; nothing in Rust knows it is up, so nothing in Rust can put it away.
 * - Everything else is reported to Rust and Rust decides what stopping means.
 *   `ui_handle_interaction(ESCAPE)` runs the coordinated stop, which drops the
 *   bar back to `Default`. React never re-derives "is Rust busy" from the bar
 *   state again; guessing at that is what the seven ladders were doing.
 *
 * The companion hook (`useEscapeToIdle`) is the only place either path is
 * wired, and it also arms Rust's passive Escape monitor while the bar is
 * expanded, so a press lands even when the bar is not the focused window.
 */

/**
 * Bar states in which the bar is already at rest: nothing for Escape to put
 * down unless the appearance has a layer of its own open.
 *
 * `always_listening` belongs here. It is a standing intent, not an activity:
 * Rust's `something_to_stop` does not count it, and the coordinated stop
 * re-arms the wake phrase on its way out, so "stopping" it would hand the bar
 * straight back the state it was in.
 */
export const RESTING_STATES: readonly string[] = [
  UI.BAR_STATES_DEFAULT,
  UI.BAR_STATES_DICTATION_READY,
  UI.BAR_STATES_SHRINKING,
  UI.BAR_STATES_ALWAYS_LISTENING,
];

/** Whether a bar state is one Rust has nothing to be cancelled out of. */
export function isRestingState(state: string): boolean {
  return RESTING_STATES.includes(state);
}

/**
 * What one appearance must say about itself for Escape to work.
 *
 * Every field is required, with no defaults: a look that forgets one does not
 * compile. The names are the contract, so a look maps its own vocabulary onto
 * them (`cardOpen`, `scriptOpen`, `unread`, `lingering`, `answerShowing` are
 * all `overlayOpen`) instead of inventing a rung.
 */
export interface BarEscapeState {
  /** The bar state Rust last reported. */
  barState: string;
  /**
   * A turn is in flight as this look knows it: its own working bar states plus
   * `chat.isProcessing`, which goes true the moment a query is accepted and
   * before Rust's bar state catches up.
   *
   * It can only ever add a reason to report the press, never remove one: the
   * rule below already reports for every state that is not a resting one.
   */
  working: boolean;
  /**
   * A layer this appearance opened on top of the bar and can close by itself:
   * the card, sheet, script, answer bubble, cursor notice, or an unread or
   * lingering answer still holding the bar open.
   */
  overlayOpen: boolean;
  /** A composer is up, whether or not anything has been typed into it. */
  composerOpen: boolean;
  /**
   * A popup inside the bar owns Escape right now (the skill autocomplete).
   * Its own dismissal comes first; the bar does not collapse underneath it.
   */
  popupOpen: boolean;
}

/** What one press of Escape does. Both halves can be true at once. */
export interface EscapeOutcome {
  /** Put this appearance's own layers away. */
  closeLocally: boolean;
  /** Report the press to Rust, which stops the work and returns the bar to rest. */
  report: boolean;
}

/** Escape had nothing to do. */
export const NOTHING_TO_DO: EscapeOutcome = { closeLocally: false, report: false };

/**
 * Whether the bar is showing more than the idle bar, and so whether Rust's
 * passive Escape monitor needs to be armed for this appearance.
 */
export function barIsExpanded(state: BarEscapeState): boolean {
  return (
    state.working ||
    !isRestingState(state.barState) ||
    state.overlayOpen ||
    state.composerOpen
  );
}

/** One press of Escape, for any appearance, in any state. */
export function escapeOutcome(state: BarEscapeState): EscapeOutcome {
  if (state.popupOpen) return NOTHING_TO_DO;
  return {
    // Layers Rust cannot see: only the appearance can put these away.
    closeLocally: state.overlayOpen || state.composerOpen,
    // Anything but a resting state is Rust's to end, and so is a turn that is
    // already in flight. A composer reports too, because Rust is holding the
    // typed value even when the state has settled.
    report: state.working || !isRestingState(state.barState) || state.composerOpen,
  };
}

/** Whether a press did anything at all. */
export function escapeHandled(outcome: EscapeOutcome): boolean {
  return outcome.closeLocally || outcome.report;
}

/**
 * How long after acting on Escape a second arrival is treated as the same
 * press. Both paths see one keystroke: the DOM keydown when the bar is
 * focused, and `bar-dismiss-pane` from Rust's monitor a moment later. One
 * press must collapse the bar once, not twice.
 */
export const ESCAPE_COALESCE_MS = 350;

/**
 * How often an expanded bar re-asserts its claim on the stop key. The Rust
 * ledger sweeps a registration older than five minutes while the app is idle,
 * which is exactly the shape of a bar left expanded with a lingering answer,
 * so the claim is refreshed well inside that window.
 */
export const ESCAPE_REARM_MS = 120_000;
