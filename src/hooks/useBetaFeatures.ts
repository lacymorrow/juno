import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { useEventListener } from "@/hooks/useEventListener";
import { COMMANDS, SETTINGS } from "@/lib/constants.generated";

export interface BetaFeatures {
  /** True when features that are still being finished are shown. */
  enabled: boolean;
  /** Persist a new value. Updates at once; goes back if the save fails. */
  setEnabled: (enabled: boolean) => Promise<void>;
}

/**
 * Whether beta features are on. Rust owns the answer (`beta_features_active`,
 * persisted in the settings store); this only displays it and follows the
 * `beta-features-changed` event, so every open window agrees. Off until the
 * backend says otherwise.
 *
 * Gates the speaking speed row. The agent's `settings` tool hides the same
 * settings on the same answer (`SettingSpec::beta_only`).
 */
export function useBetaFeatures(): BetaFeatures {
  const [enabled, setEnabledState] = useState(false);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    invoke<boolean>(COMMANDS.SETTINGS_GET_BETA_FEATURES)
      .then((on) => mounted.current && setEnabledState(Boolean(on)))
      .catch(() => {});
    return () => {
      mounted.current = false;
    };
  }, []);

  useEventListener<boolean>(SETTINGS.EVENTS_BETA_FEATURES_CHANGED, (on) => {
    if (mounted.current) setEnabledState(Boolean(on));
  });

  const setEnabled = useCallback(async (next: boolean) => {
    setEnabledState(next);
    try {
      await invoke(COMMANDS.SETTINGS_SET_BETA_FEATURES, { enabled: next });
    } catch (error) {
      console.error("Failed to save beta features:", error);
      if (mounted.current) setEnabledState(!next);
      toast.error("Failed to update beta features");
    }
  }, []);

  return { enabled, setEnabled };
}
