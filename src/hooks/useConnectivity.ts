import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { useEventListener } from "@/hooks/useEventListener";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import type { Connectivity } from "@/lib/pillStatus";

/**
 * Whether Juno can reach the network and its provider, as Rust reports it.
 * `null` until the first answer arrives; the dot reads that as fine.
 */
export function useConnectivity(): Connectivity | null {
  const [connectivity, setConnectivity] = useState<Connectivity | null>(null);
  // An event is newer than the initial read, whichever arrives first.
  const heardEventRef = useRef(false);

  useEffect(() => {
    let alive = true;
    invoke<Connectivity>(COMMANDS.APP_GET_CONNECTIVITY)
      .then((snapshot) => {
        if (alive && snapshot && !heardEventRef.current) setConnectivity(snapshot);
      })
      .catch((error) => console.debug("useConnectivity: initial read failed:", error));
    return () => {
      alive = false;
    };
  }, []);

  useEventListener<Connectivity>(EVENTS.SYSTEM_CONNECTIVITY_CHANGED, (snapshot) => {
    if (!snapshot) return;
    heardEventRef.current = true;
    setConnectivity(snapshot);
  });

  return connectivity;
}
