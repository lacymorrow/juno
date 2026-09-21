/**
 * Tauri stand-in for the bar state harness (route `/__bar-harness`).
 *
 * The harness runs the REAL floating-bar components with no Rust backend, so
 * every Tauri call the bar makes has to land somewhere that will not throw.
 * Rather than fork the components or intercept each `invoke` import, we install
 * a fake `window.__TAURI_INTERNALS__` (via the same mock plumbing the unit
 * tests use) so the genuine `@tauri-apps/api` functions the bar imports keep
 * working: they route through this one global. Nothing here touches the real
 * components, and this module is only imported by the harness route, so a
 * normal build never installs it.
 *
 * The one piece of real state we model is the window frame. The bar sizes and
 * moves its own window through `set_bar_frame` / `setPosition`, and the harness
 * needs to show that happening against a container a developer also controls by
 * hand. So the frame lives in a tiny external store: the shim reads it when the
 * bar asks `outerPosition` / `outerSize`, writes it when the bar resizes or
 * snaps (unless the developer has frozen auto-resize to force a mismatch), and
 * the React panel subscribes to it for the simulated screen.
 */

import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

/** The label the bar's window carries in the shipping app. */
export const BAR_WINDOW_LABEL = "floating-bar";

/** A monitor, in PHYSICAL pixels, matching Tauri's monitor geometry. */
export interface HarnessMonitor {
  width: number;
  height: number;
  scaleFactor: number;
}

/** The simulated window frame, in PHYSICAL pixels (Tauri's units). */
export interface HarnessFrame {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface HarnessState {
  monitor: HarnessMonitor;
  /** The live frame the shim reports and the container renders. */
  frame: HarnessFrame;
  /** The last frame the bar ASKED for, shown even when auto-resize is frozen. */
  requested: HarnessFrame | null;
  /** Where the OS "says" the cursor is, in physical px; drives leave-verify. */
  cursor: { x: number; y: number };
  /**
   * When true the shim records `set_bar_frame` but does not apply it, so a
   * developer-set container size sticks and the bar visibly fights it. This is
   * the lever for reproducing clip / overflow / snap-back bugs.
   */
  freezeAutoResize: boolean;
}

// One monitor at the origin. Physical == logical at scaleFactor 1, which keeps
// the well math and the on-screen placement one-to-one and legible; the retina
// path has its own dedicated bugs and is not what this tool is for.
const DEFAULT_STATE: HarnessState = {
  monitor: { width: 1440, height: 900, scaleFactor: 1 },
  frame: { x: 1000, y: 40, width: 88, height: 66 },
  requested: null,
  cursor: { x: -1000, y: -1000 },
  freezeAutoResize: false,
};

let state: HarnessState = DEFAULT_STATE;
const listeners = new Set<() => void>();

function commit(next: HarnessState): void {
  state = next;
  listeners.forEach((l) => l());
}

/** External store the harness panel and simulated screen read from. */
export const harness = {
  subscribe(listener: () => void): () => void {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
  snapshot(): HarnessState {
    return state;
  },
  setFrame(patch: Partial<HarnessFrame>): void {
    commit({ ...state, frame: { ...state.frame, ...patch } });
  },
  setMonitor(patch: Partial<HarnessMonitor>): void {
    commit({ ...state, monitor: { ...state.monitor, ...patch } });
  },
  setCursor(x: number, y: number): void {
    commit({ ...state, cursor: { x, y } });
  },
  setFreezeAutoResize(freeze: boolean): void {
    commit({ ...state, freezeAutoResize: freeze });
  },
  reset(): void {
    commit(DEFAULT_STATE);
  },
};

/** A Tauri monitor object shaped the way `mapMonitor` expects to unwrap it. */
function monitorObject() {
  const { width, height, scaleFactor } = state.monitor;
  const position = { x: 0, y: 0 };
  const size = { width, height };
  return {
    name: "Harness Display",
    scaleFactor,
    position,
    size,
    workArea: { position, size },
  };
}

/**
 * Pull `{x,y}` / `{width,height}` out of a `set_position` / `set_size` value,
 * which arrives as a `Position` / `Size` instance (or a plain object) that
 * serializes to `{ Physical: {...} }` or `{ Logical: {...} }`.
 */
function unwrapValue(value: unknown): Record<string, number> | null {
  if (value == null || typeof value !== "object") return null;
  const raw = value as { toJSON?: () => unknown };
  const json = typeof raw.toJSON === "function" ? raw.toJSON() : value;
  const obj = json as Record<string, unknown>;
  const inner = (obj.Physical ?? obj.Logical ?? obj) as Record<string, number>;
  return inner;
}

/**
 * The IPC handler. Window-geometry reads come from the store; the bar's own
 * frame writes go back into it (unless frozen); every other command resolves
 * to a harmless default so nothing the bar fires can throw.
 */
function handleInvoke(cmd: string, args: Record<string, unknown> = {}): unknown {
  switch (cmd) {
    // --- window geometry the bar reads ---
    case "plugin:window|scale_factor":
      return state.monitor.scaleFactor;
    case "plugin:window|outer_position":
    case "plugin:window|inner_position":
      return { x: state.frame.x, y: state.frame.y };
    case "plugin:window|outer_size":
    case "plugin:window|inner_size":
      return { width: state.frame.width, height: state.frame.height };
    case "plugin:window|available_monitors":
      return [monitorObject()];
    case "plugin:window|current_monitor":
    case "plugin:window|primary_monitor":
      return monitorObject();
    case "plugin:window|cursor_position":
      return { x: state.cursor.x, y: state.cursor.y };
    case "plugin:window|get_all_windows":
      // useWindowSize resolves the window through getByLabel, which filters
      // this list; the label must be present or the resize silently no-ops.
      return [BAR_WINDOW_LABEL];

    // --- window moves the bar performs (snap animation, cursor-follow) ---
    case "plugin:window|set_position": {
      const p = unwrapValue(args.value);
      if (p && typeof p.x === "number" && typeof p.y === "number") {
        harness.setFrame({ x: Math.round(p.x), y: Math.round(p.y) });
      }
      return null;
    }
    case "plugin:window|set_size": {
      const s = unwrapValue(args.value);
      if (s && typeof s.width === "number" && typeof s.height === "number") {
        harness.setFrame({ width: Math.round(s.width), height: Math.round(s.height) });
      }
      return null;
    }

    // --- the bar's own frame command (resize + reposition in one shot) ---
    case "set_bar_frame": {
      const sf = state.monitor.scaleFactor;
      // width/height arrive logical; x/y arrive physical. Normalize to the
      // physical frame the store holds.
      const requested: HarnessFrame = {
        x: Math.round(Number(args.x)),
        y: Math.round(Number(args.y)),
        width: Math.round(Number(args.width) * sf),
        height: Math.round(Number(args.height) * sf),
      };
      if (state.freezeAutoResize) {
        // Record what the bar wanted without applying it, so the panel can show
        // the divergence between the bar's desired frame and the frozen one.
        commit({ ...state, requested });
      } else {
        commit({ ...state, frame: requested, requested });
      }
      return null;
    }

    // --- reads that must return a usable shape (callers map/some over these) ---
    case "get_bar_position":
      return null;
    case "get_triggers":
      return [];
    case "list_agent_sessions":
      return [];
    case "get_always_listening_status":
      return false;

    // Everything else the bar fires (interactions, dispatch, stop, focus,
    // dictation cancel, pane ledger, show-when-ready, webview focus, ...) has no
    // meaningful return in the harness; resolving undefined is enough.
    default:
      return null;
  }
}

let installed = false;

/** Install the fake internals once. Safe to import more than once. */
export function installBarHarnessTauri(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;
  // Sets metadata.currentWindow so getCurrentWindow().label works in render.
  mockWindows(BAR_WINDOW_LABEL);
  // shouldMockEvents wires listen/emit/unlisten through an in-memory registry,
  // so the harness can drive the bar by emitting the same events Rust would.
  mockIPC((cmd, payload) => handleInvoke(cmd, (payload ?? {}) as Record<string, unknown>), {
    shouldMockEvents: true,
  });
}

// Install on import. The harness route imports this module before the real bar
// components, so the internals exist before any bar effect calls `listen`.
installBarHarnessTauri();
