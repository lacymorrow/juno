import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { useEventListener } from "@/hooks/useEventListener";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";

/** `readiness::Snapshot` in Rust. */
export interface StartupReadiness {
  loading: boolean;
  pending: string[];
}

/**
 * Is Juno still loading what the first request needs (the speech engine), as
 * Rust reports it? False until the first answer: a dot that pulses on a guess
 * would be wrong on every launch after the engine was already warm.
 */
export function useStartupReadiness(): boolean {
  const [loading, setLoading] = useState(false);
  // An event is newer than the initial read, whichever arrives first.
  const heardEventRef = useRef(false);

  useEffect(() => {
    let alive = true;
    invoke<StartupReadiness>(COMMANDS.APP_GET_STARTUP_READINESS)
      .then((snapshot) => {
        if (alive && snapshot && !heardEventRef.current) setLoading(snapshot.loading === true);
      })
      .catch((error) => console.debug("useStartupReadiness: initial read failed:", error));
    return () => {
      alive = false;
    };
  }, []);

  useEventListener<StartupReadiness>(EVENTS.SYSTEM_STARTUP_READINESS_CHANGED, (snapshot) => {
    if (!snapshot) return;
    heardEventRef.current = true;
    setLoading(snapshot.loading === true);
  });

  return loading;
}
