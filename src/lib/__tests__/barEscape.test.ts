import { describe, expect, it } from "vitest";
import { UI } from "@/lib/constants.generated";
import {
  RESTING_STATES,
  barIsExpanded,
  escapeHandled,
  escapeOutcome,
  isRestingState,
  type BarEscapeState,
} from "@/lib/barEscape";

/**
 * Every bar state Rust can report, read off the generated constants rather
 * than listed here, so a state added to Rust tomorrow is in this table
 * tomorrow. That is the point of the file: the bug it replaces was seven
 * hand-written state lists, each missing something different.
 */
const ALL_BAR_STATES: string[] = Object.entries(UI)
  .filter(([key]) => key.startsWith("BAR_STATES_"))
  .map(([, value]) => value as string);

/** The states in which the bar is showing something other than the idle bar. */
const EXPANDED_STATES = ALL_BAR_STATES.filter((s) => !isRestingState(s));

const at = (over: Partial<BarEscapeState> = {}): BarEscapeState => ({
  barState: UI.BAR_STATES_DEFAULT,
  working: false,
  overlayOpen: false,
  composerOpen: false,
  popupOpen: false,
  ...over,
});

describe("barEscape: the state table", () => {
  it("reads every bar state the backend has", () => {
    // A guard on the guard: if the constants ever stop being generated under
    // this prefix, the table above would silently be empty and every
    // assertion below would pass for nothing.
    expect(ALL_BAR_STATES.length).toBeGreaterThanOrEqual(17);
    expect(ALL_BAR_STATES).toContain(UI.BAR_STATES_SPEAKING);
    expect(ALL_BAR_STATES).toContain(UI.BAR_STATES_ERROR);
  });

  it("counts only the four resting states as resting", () => {
    expect([...RESTING_STATES].sort()).toEqual(
      [
        UI.BAR_STATES_DEFAULT,
        UI.BAR_STATES_DICTATION_READY,
        UI.BAR_STATES_SHRINKING,
        UI.BAR_STATES_ALWAYS_LISTENING,
      ].sort(),
    );
  });

  it.each(EXPANDED_STATES)("escape does something from %s, with nothing else open", (barState) => {
    // This is the defect, stated as a test. Each appearance used to decide for
    // itself which states were worth answering, so speaking, error, success,
    // listening and dictating fell through every ladder and Escape did
    // nothing. One press now always reaches Rust.
    const outcome = escapeOutcome(at({ barState }));
    expect(outcome.report).toBe(true);
    expect(escapeHandled(outcome)).toBe(true);
  });

  it.each(EXPANDED_STATES)("%s counts as expanded, so the stop key is armed", (barState) => {
    expect(barIsExpanded(at({ barState }))).toBe(true);
  });

  it.each(RESTING_STATES)("does nothing from %s with no layer of its own open", (barState) => {
    const outcome = escapeOutcome(at({ barState }));
    expect(escapeHandled(outcome)).toBe(false);
    expect(barIsExpanded(at({ barState }))).toBe(false);
  });

  it.each(RESTING_STATES)("still collapses an overlay left open in %s", (barState) => {
    // The common shape of the bug in daily use: the answer card lingers with
    // the bar state already back at rest. Nothing in Rust knows the card is
    // up, so Escape has to close it here and must not also fire a stop.
    const outcome = escapeOutcome(at({ barState, overlayOpen: true }));
    expect(outcome).toEqual({ closeLocally: true, report: false });
    expect(barIsExpanded(at({ barState, overlayOpen: true }))).toBe(true);
  });

  it("reports a turn already in flight before the bar state has caught up", () => {
    const outcome = escapeOutcome(at({ barState: UI.BAR_STATES_DEFAULT, working: true }));
    expect(outcome.report).toBe(true);
  });

  it("closes an overlay and reports in one press when both apply", () => {
    const outcome = escapeOutcome(
      at({ barState: UI.BAR_STATES_AGENT_RESPONDING, overlayOpen: true }),
    );
    expect(outcome).toEqual({ closeLocally: true, report: true });
  });

  it("reports an open composer even once the bar state has settled", () => {
    const outcome = escapeOutcome(at({ barState: UI.BAR_STATES_DEFAULT, composerOpen: true }));
    expect(outcome).toEqual({ closeLocally: true, report: true });
  });

  it.each(ALL_BAR_STATES)("leaves %s alone while a popup owns the key", (barState) => {
    // The skill autocomplete dismisses itself first. The bar must not collapse
    // out from under it.
    const outcome = escapeOutcome(
      at({ barState, working: true, overlayOpen: true, composerOpen: true, popupOpen: true }),
    );
    expect(outcome).toEqual({ closeLocally: false, report: false });
  });

  it("arms the stop key in exactly the cases where a press would do something", () => {
    // The two have to agree, or a state exists where Escape would be answered
    // but is never observed: a press outside Juno's own windows would be lost,
    // which is the second half of the defect. The popup case is the one
    // deliberate exception, since the bar is up but the key is not the bar's.
    for (const barState of ALL_BAR_STATES) {
      for (const working of [false, true]) {
        for (const overlayOpen of [false, true]) {
          for (const composerOpen of [false, true]) {
            const state = at({ barState, working, overlayOpen, composerOpen });
            expect(barIsExpanded(state)).toBe(escapeHandled(escapeOutcome(state)));
          }
        }
      }
    }
  });
});
