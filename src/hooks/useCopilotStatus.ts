import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";
import type { CopilotStatus } from "@/lib/copilot-status";

/**
 * GitHub Copilot's sign-in, as the backend found it. Asked on open and again
 * whenever the window comes back, so a `gh auth login` done in Terminal shows
 * up without reopening anything.
 */
export function useCopilotStatus(enabled = true) {
  const [status, setStatus] = useState<CopilotStatus | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    if (!enabled) return;
    let mounted = true;
    let latest = 0;
    const check = () => {
      const request = ++latest;
      invoke<CopilotStatus>(COMMANDS.PROVIDERS_GET_COPILOT_STATUS)
        .then((next) => {
          if (mounted && request === latest) setStatus(next);
        })
        .catch((error) => console.debug("Copilot status unavailable:", error));
    };
    check();
    window.addEventListener("focus", check);
    return () => {
      mounted = false;
      window.removeEventListener("focus", check);
    };
  }, [enabled, refreshKey]);

  return { status, setStatus, refresh: () => setRefreshKey((k) => k + 1) };
}
