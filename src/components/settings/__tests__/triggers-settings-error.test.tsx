import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const toastError = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({
  toast: { error: (...a: unknown[]) => toastError(...a), success: vi.fn() },
}));

import { COMMANDS } from "@/lib/constants.generated";
import TriggersSettings from "@/components/settings/sections/TriggersSettings";
import type { SettingsSectionProps } from "@/components/settings/types";

/** The sentence `triggers::issues` builds when two rows fight over the key. */
const CONFLICT = '"Fn (globe)" already has Hold to talk to Juno.';

type Row = {
  id: string;
  gesture: string;
  target: string;
  binding: { kind: "keyboard"; shortcut: string } | null;
  phrase: string | null;
  require_hey_prefix: boolean;
  enabled: boolean;
};

const row = (
  id: string,
  gesture: string,
  target: string,
  shortcut: string | null,
  enabled = true,
): Row => ({
  id,
  gesture,
  target,
  binding: shortcut ? { kind: "keyboard", shortcut } : null,
  phrase: null,
  require_hey_prefix: false,
  enabled,
});

/* -------------------------------------------------------------------------- */
/* A stand-in for the backend                                                 */
/* -------------------------------------------------------------------------- */

/**
 * `triggers::can_share_key`, the two pairs that matter here: a Tap may not
 * share a key with a Hold, and the globe key may hold for one row and
 * double-tap-and-hold for another.
 */
const SHAREABLE = new Set([
  "hold|double_tap",
  "hold|double_tap_hold",
  "double_tap|double_tap_hold",
  "tap|double_tap_hold",
]);

const canShare = (a: string, b: string) =>
  a !== b && (SHAREABLE.has(`${a}|${b}`) || SHAREABLE.has(`${b}|${a}`));

const label = (r: Row) =>
  `${{ hold: "Hold", tap: "Tap", double_tap_hold: "Double-tap and hold" }[r.gesture]} ${
    r.target === "agent" ? "to talk to Juno" : "to dictate"
  }`;

const shown = (r: Row) =>
  r.binding?.shortcut === "Fn" ? "Fn (globe)" : (r.binding?.shortcut ?? "");

/**
 * `triggers::issues`: derived from the list it is handed, every time, with no
 * memory between calls. The whole contract under test is that the screen asks
 * this about the rows it is drawing, so it is modelled rather than scripted.
 */
function issuesFor(list: Row[]): Array<{ trigger_id: string; message: string }> {
  const out: Array<{ trigger_id: string; message: string }> = [];
  const claimed: Row[] = [];
  for (const r of list.filter((t) => t.enabled && t.binding)) {
    const other = claimed.find(
      (c) => c.binding!.shortcut === r.binding!.shortcut && !canShare(c.gesture, r.gesture),
    );
    if (other) {
      out.push({ trigger_id: r.id, message: `"${shown(r)}" already has ${label(other)}.` });
      continue;
    }
    claimed.push(r);
  }
  return out;
}

interface Backend {
  /** Refuse every save for this reason, whatever the list says. */
  failSavesWith?: string;
}

/** Wire `invoke` to the stand-in. Returns the accepted list, for assertions. */
function mountBackend(stored: Row[], opts: Backend = {}) {
  const state = { list: stored.map((r) => ({ ...r })) };

  invoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case COMMANDS.TRIGGERS_GET_TRIGGERS:
        return Promise.resolve(state.list);

      case COMMANDS.TRIGGERS_GET_TRIGGER_ISSUES:
        return Promise.resolve(issuesFor((args?.triggers as Row[]) ?? []));

      case COMMANDS.TRIGGERS_SET_TRIGGERS: {
        if (opts.failSavesWith) return Promise.reject(opts.failSavesWith);
        const next = args?.triggers as Row[];
        const bad = issuesFor(next);
        if (bad.length > 0) return Promise.reject(bad[0].message);
        state.list = next.map((r) => ({ ...r }));
        return Promise.resolve(state.list);
      }

      case COMMANDS.SHORTCUTS_VALIDATE_KEYBOARD_SHORTCUT: {
        const editing = String(args?.shortcutName ?? "").replace("trigger_", "");
        const asking = state.list.find((r) => r.id === editing);
        const clash = state.list.find(
          (r) =>
            r.id !== editing &&
            r.enabled &&
            r.binding?.shortcut === args?.shortcutValue &&
            !canShare(r.gesture, asking?.gesture ?? ""),
        );
        return clash
          ? Promise.reject(`"${String(args?.shortcutValue)}" already has ${label(clash)}.`)
          : Promise.resolve("Valid shortcut");
      }

      default:
        return Promise.resolve(null);
    }
  });

  return state;
}

const settings = { alwaysListeningActive: false } as unknown as SettingsSectionProps["settings"];

const renderScreen = () => render(<TriggersSettings settings={settings} />);

/** Hold on the globe key, plus a Tap on it that is switched off, so turning
    the Tap on is the one gesture that makes the two rows collide. */
const TWO_ROWS: Row[] = [
  row("row-hold", "hold", "agent", "Fn"),
  row("row-tap", "tap", "dictation", "Fn", false),
];

/** Flip the Tap on, which is the save the backend refuses. */
async function provokeConflict() {
  await screen.findByLabelText("Enable Hold to talk to Juno");
  screen.getByLabelText("Enable Tap to dictate").click();
  expect(await screen.findByText(CONFLICT)).toBeTruthy();
}

const gone = () =>
  waitFor(() => {
    expect(screen.queryByText(CONFLICT)).toBeNull();
  });

/* -------------------------------------------------------------------------- */

describe("a trigger conflict is a field state, not a remembered refusal", () => {
  beforeEach(() => {
    invoke.mockReset();
    toastError.mockReset();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("clears when the conflicting row is rebound to a free key", async () => {
    // The case the earlier fix missed, and the one the person hits: the
    // message was a string cached from a rejected save, so nothing but a
    // later *accepted* save took it down. Somebody who hit the conflict and
    // then simply rebound the row was fine; somebody who hit it and stopped
    // read it for the rest of the session. Derived, it goes the moment the
    // list stops producing it.
    mountBackend(TWO_ROWS);
    renderScreen();
    await provokeConflict();

    screen.getByLabelText("Edit binding for Tap to dictate").click();
    const recorder = await screen.findByLabelText("Shortcut");
    // By physical key: Option held, then the J key pressed and let go.
    fireEvent.keyDown(recorder, { code: "KeyJ", altKey: true });
    fireEvent.keyUp(recorder, { code: "KeyJ", altKey: true });

    await gone();
    expect(toastError).not.toHaveBeenCalled();
  });

  it("shows a conflict the stored list already has, with nothing attempted", async () => {
    // The sharpest line between the two designs. A remembered refusal can
    // only exist after a save is rejected, so a list that arrives already in
    // conflict used to draw no message at all. Derived, the screen reports
    // what is in front of the person whether or not they just did something.
    mountBackend([
      row("row-hold", "hold", "agent", "Fn"),
      row("row-tap", "tap", "dictation", "Fn"),
    ]);
    renderScreen();

    expect(await screen.findByText(CONFLICT)).toBeTruthy();
    expect(invoke).not.toHaveBeenCalledWith(COMMANDS.TRIGGERS_SET_TRIGGERS, expect.anything());
  });

  it("keeps the refused edit on screen, so the sentence is about a visible row", async () => {
    // Reverting is what made the message need a memory. The list snapped back
    // to the one the backend had accepted, which by definition had no
    // conflict in it, so the sentence described a state that was nowhere on
    // screen and could only be taken down by something else happening. The
    // edit stays, which is also what makes fixing the other row save the
    // binding they wanted rather than making them record it again.
    mountBackend(TWO_ROWS);
    renderScreen();
    await provokeConflict();

    expect(screen.getByLabelText("Enable Tap to dictate").getAttribute("aria-checked")).toBe(
      "true",
    );
  });

  it("clears when the binding is removed entirely", async () => {
    mountBackend(TWO_ROWS);
    renderScreen();
    await provokeConflict();

    screen.getByLabelText("Edit binding for Tap to dictate").click();
    (await screen.findByText("Remove binding")).click();
    await gone();
  });

  it("clears when the conflicting row is deleted", async () => {
    mountBackend(TWO_ROWS);
    renderScreen();
    await provokeConflict();

    screen.getByLabelText("Delete Tap to dictate").click();
    await gone();
  });

  it("clears when the other row is deleted, which resolves it from that side", async () => {
    // The case a naive fix misses: the sentence names the *other* row, so
    // anything that only watches the row the message sits beside keeps
    // showing it.
    mountBackend(TWO_ROWS);
    renderScreen();
    await provokeConflict();

    screen.getByLabelText("Delete Hold to talk to Juno").click();
    await gone();
  });

  it("clears when the other row is switched off, which also resolves it", async () => {
    mountBackend(TWO_ROWS);
    renderScreen();
    await provokeConflict();

    screen.getByLabelText("Enable Hold to talk to Juno").click();
    await gone();
  });

  it("clears when the pane is left and come back to", async () => {
    // Navigating away unmounts the section and the settings window reopening
    // remounts it. Either way the screen reloads the stored list, which the
    // backend accepted, so no refusal about it can still be true.
    mountBackend(TWO_ROWS);
    const { unmount } = renderScreen();
    await provokeConflict();
    unmount();

    renderScreen();
    await screen.findByLabelText("Enable Tap to dictate");
    await gone();
  });

  it("stays while it is still true, however long the person looks at it", async () => {
    // The trap in the obvious fix. A message hidden on a timer leaves a form
    // that refuses input and will not say why, which is worse than the stale
    // message it was meant to cure. Nothing here is on a clock.
    mountBackend(TWO_ROWS);
    renderScreen();
    await provokeConflict();

    vi.useFakeTimers();
    await act(async () => {
      vi.advanceTimersByTime(120_000);
    });

    expect(screen.getByText(CONFLICT)).toBeTruthy();
  });

  it("names the row the conflict belongs beside, not the row last touched", async () => {
    mountBackend(TWO_ROWS);
    renderScreen();
    await provokeConflict();

    // The Tap row is the one that cannot have the key, so the sentence sits
    // under it and names the Hold row that got there first.
    const message = screen.getByText(CONFLICT);
    const rowEl = screen.getByLabelText("Delete Tap to dictate").closest("div.px-4");
    expect(rowEl?.contains(message)).toBe(true);
  });

  it("sends a failure the derivation cannot explain to a toast, not to the field", async () => {
    // The other surface. A save that failed for its own reasons describes an
    // attempt rather than a field, so it is transient and the row says
    // nothing: the one thing it must not do is sit there forever.
    mountBackend([row("row-hold", "hold", "agent", "Fn")], {
      failSavesWith: "Could not write settings.",
    });
    renderScreen();
    await screen.findByLabelText("Enable Hold to talk to Juno");

    screen.getByLabelText("Enable Hold to talk to Juno").click();

    await waitFor(() => {
      expect(toastError).toHaveBeenCalledWith("Could not write settings.");
    });
    expect(screen.queryByText("Could not write settings.")).toBeNull();
  });
});

describe("a row says how its own gesture ends", () => {
  beforeEach(() => {
    invoke.mockReset();
    mountBackend([
      row("row-hold", "hold", "agent", "Fn"),
      row("row-dth", "double_tap_hold", "dictation", "Fn"),
    ]);
  });

  it("never describes a second gesture reaching the same target", async () => {
    // The copy the user quoted: "Hold the key while you speak, let go to
    // finish. Double-tap it to keep listening until you press it again." That
    // behaviour is deleted, so no row may still describe it.
    renderScreen();
    await screen.findByLabelText("Enable Hold to talk to Juno");

    expect(screen.getAllByText("Let go to finish.")).toHaveLength(2);
    expect(screen.queryByText(/Double-tap it to keep listening/i)).toBeNull();
  });

  it("reads Hold and Double-tap and hold as two rows on one key", async () => {
    renderScreen();
    expect(await screen.findByText("Hold")).toBeTruthy();
    expect(screen.getByText("Double-tap and hold")).toBeTruthy();
    expect(screen.getByText("to talk to Juno")).toBeTruthy();
    expect(screen.getByText("to dictate")).toBeTruthy();
  });
});
