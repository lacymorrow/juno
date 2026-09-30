import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useEventListener } from "@/hooks/useEventListener";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";

/**
 * Where the update flow is. Mirrors `UpdateStage` in `src-tauri/src/updater.rs`
 * — the backend owns every transition between these, and the UI only renders
 * whichever one it was last handed.
 */
export type UpdateStage =
  | "idle"
  | "checking"
  | "upToDate"
  | "downloading"
  | "readyToRestart"
  | "failed";

export type UpdateChannel = "stable" | "prerelease";

/** Mirrors `UpdateStatus` in `src-tauri/src/updater.rs`. */
export interface UpdateStatus {
  stage: UpdateStage;
  currentVersion: string;
  availableVersion: string | null;
  notes: string | null;
  error: string | null;
  channel: UpdateChannel;
  downloadedBytes: number;
  totalBytes: number | null;
}

export interface UpdateSettings {
  auto_check_enabled: boolean;
  channel: string;
}

/**
 * The update flow, as the UI sees it.
 *
 * There is deliberately no local state machine here. Checking, downloading,
 * installing and the schedule all live in Rust, which is what lets the same
 * behaviour hold with no window open. This hook reads the status once, then
 * follows the `update-status` event.
 */
export function useUpdater() {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [settings, setSettings] = useState<UpdateSettings | null>(null);

  useEffect(() => {
    let mounted = true;
    void (async () => {
      try {
        const [initialStatus, initialSettings] = await Promise.all([
          invoke<UpdateStatus>(COMMANDS.UPDATES_GET_STATUS),
          invoke<UpdateSettings>(COMMANDS.UPDATES_GET_SETTINGS),
        ]);
        if (!mounted) return;
        setStatus(initialStatus);
        setSettings(initialSettings);
      } catch (error) {
        console.error("Failed to read update status:", error);
      }
    })();
    return () => {
      mounted = false;
    };
  }, []);

  useEventListener<UpdateStatus>(EVENTS.UPDATES_STATUS, (payload) => {
    setStatus(payload);
  });

  const checkNow = useCallback(async () => {
    // The command returns as soon as the check starts; everything after that
    // arrives on the event, including the failure.
    await invoke(COMMANDS.UPDATES_CHECK_NOW);
  }, []);

  const restart = useCallback(async () => {
    await invoke(COMMANDS.UPDATES_RESTART_TO_UPDATE);
  }, []);

  const saveSettings = useCallback(
    async (next: { autoCheckEnabled: boolean; channel: UpdateChannel }) => {
      const saved = await invoke<UpdateSettings>(COMMANDS.UPDATES_SET_SETTINGS, {
        autoCheckEnabled: next.autoCheckEnabled,
        channel: next.channel,
      });
      setSettings(saved);
      return saved;
    },
    [],
  );

  return { status, settings, checkNow, restart, saveSettings };
}
