import { useCallback, useRef } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import type { UpdateInfo } from "@/components/ModalSystem";

export function useUpdater() {
  const pendingUpdate = useRef<Update | null>(null);

  const checkForUpdates = useCallback(async (): Promise<{
    available: boolean;
    info: UpdateInfo | null;
  }> => {
    const update = await check();
    if (!update) return { available: false, info: null };

    pendingUpdate.current = update;
    return {
      available: true,
      info: {
        available: true,
        version: update.version,
        date: update.date ?? undefined,
        notes: update.body ?? undefined,
      },
    };
  }, []);

  const installUpdate = useCallback(async () => {
    const update = pendingUpdate.current;
    if (!update) throw new Error("No pending update — call checkForUpdates first");
    await update.downloadAndInstall();
    await relaunch();
  }, []);

  return { checkForUpdates, installUpdate };
}
