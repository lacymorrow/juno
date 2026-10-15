/**
 * PermissionCoach — the card that sits beside System Settings and points at
 * the row Juno needs switched on (route `/permission-coach`, its own
 * transparent, click-through, always-on-top window declared in
 * tauri.conf.json).
 *
 * The moment it exists for: Juno has just sent somebody to Privacy and
 * Security, and they are looking at a list of applications with nothing on
 * screen saying which row is Juno's. So the card shows the Juno icon, because
 * the task is a visual search through a column of icons, and one line saying
 * what to do with the row once it is found.
 *
 * Everything that is a decision is Rust's (`src-tauri/src/permission_coach.rs`):
 * whether the card exists, where it sits, which way it points, and when it
 * leaves. This file renders a card. It has no timers, polls nothing, and holds
 * no opinion about permissions.
 *
 * It is click-through, so it can never cover, block or take focus from the
 * switch it is pointing at. That is also why there is nothing on it to press:
 * a button here could not be clicked, and a card that leaves on its own needs
 * no dismissing.
 *
 * Visual: one dark HUD card, the one macOS system blue as the only accent, and
 * a chevron that drifts toward the window it is pointing at. The drift stops
 * under prefers-reduced-motion.
 */

import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, ChevronRight } from "lucide-react";

import { useEventListener } from "@/hooks/useEventListener";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { cn } from "@/lib/utils";

/** The one accent: macOS system blue, as everywhere else in Juno. */
const ACCENT = "rgb(10, 132, 255)";

/** Which way the chevron faces, and which way it drifts. Rust picks the side. */
const ROTATION: Record<string, string> = {
  right: "rotate(0deg)",
  left: "rotate(180deg)",
  up: "rotate(-90deg)",
  down: "rotate(90deg)",
};

/** The card sits on the far side from the arrow, so the arrow touches the window. */
const LAYOUT: Record<string, string> = {
  right: "flex-row",
  left: "flex-row-reverse",
  up: "flex-col-reverse",
  down: "flex-col",
};

/** What Rust says the card should say. Mirrors `permission_coach::CoachState`. */
interface CoachState {
  permission: string;
  headline: string;
  arrow: string;
  granted: boolean;
  receipt: string;
}

export const PermissionCoach = () => {
  const [state, setState] = useState<CoachState | null>(null);
  /** True once an event has arrived, so a late first read cannot undo it. */
  const heardFromRust = useRef(false);

  useEventListener<CoachState>(EVENTS.PERMISSIONS_COACH, (payload) => {
    if (!payload?.permission) return;
    heardFromRust.current = true;
    setState(payload);
  });

  // The window is built before it is needed, so this page is usually mounted
  // and listening well before the first event. Usually is not always: a
  // permission asked for in the first seconds of a launch builds the window on
  // the spot, and then the event is emitted before anything is listening.
  useEffect(() => {
    let mounted = true;
    void (async () => {
      try {
        const current = await invoke<CoachState | null>(COMMANDS.PERMISSIONS_COACH_STATE);
        if (!mounted || heardFromRust.current || !current) return;
        setState(current);
      } catch (err) {
        console.debug("[PermissionCoach] could not read the current state:", err);
      }
    })();
    return () => {
      mounted = false;
    };
  }, []);

  if (!state) return null;

  const arrow = ROTATION[state.arrow] ? state.arrow : "right";
  const line = state.granted ? state.receipt : state.headline;

  return (
    <div
      data-testid="permission-coach"
      role="status"
      aria-live="polite"
      className={cn("flex h-screen w-screen items-center justify-center gap-0", LAYOUT[arrow])}
    >
      <style>{`
        @keyframes juno-coach-drift {
          0%, 100% { transform: translate(0, 0); }
          50% { transform: translate(var(--drift-x, 0), var(--drift-y, 0)); }
        }
        .juno-coach-arrow { animation: juno-coach-drift 1.6s ease-in-out infinite; }
        @media (prefers-reduced-motion: reduce) {
          .juno-coach-arrow { animation: none; }
        }
      `}</style>

      <div className="flex min-w-0 flex-1 items-center gap-3 rounded-2xl bg-black/80 px-3.5 py-3 ring-1 ring-inset ring-white/15 backdrop-blur-xl">
        {state.granted ? (
          <span
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full"
            style={{ backgroundColor: ACCENT }}
          >
            <Check className="h-5 w-5 text-white" strokeWidth={3} aria-hidden />
          </span>
        ) : (
          <img src="/juno.png" alt="" className="h-8 w-8 shrink-0 object-contain" />
        )}
        <p className="min-w-0 text-[13px] font-medium leading-snug text-white">{line}</p>
      </div>

      {/* Points back at the System Settings window, whichever side it is on.
          Hidden once the switch is on: there is nothing left to point at.
          The drift and the rotation are on separate elements on purpose: both
          are transforms, and one element cannot carry both. */}
      {!state.granted && (
        <span
          data-testid="permission-coach-arrow"
          aria-hidden
          className="juno-coach-arrow flex shrink-0 items-center justify-center"
          style={{
            ["--drift-x" as string]: arrow === "right" ? "4px" : arrow === "left" ? "-4px" : "0",
            ["--drift-y" as string]: arrow === "down" ? "4px" : arrow === "up" ? "-4px" : "0",
          }}
        >
          <ChevronRight
            className="h-5 w-5"
            strokeWidth={3}
            style={{ color: ACCENT, transform: ROTATION[arrow] }}
          />
        </span>
      )}
    </div>
  );
};

export default PermissionCoach;
