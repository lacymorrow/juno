import { invoke } from "@tauri-apps/api/core";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { toast } from "sonner";

/**
 * Advanced-settings visibility.
 *
 * The settings window shows a trimmed "basic" set by default. Sections and
 * fields opt into the advanced tier with a single `advanced` marker
 * (`SettingsCategory.advanced`, `<SettingsSection advanced>`,
 * `<SettingsField advanced>`, or the `<AdvancedOnly>` wrapper); this context
 * is the one place that decides whether they render.
 *
 * The flag is a real backend setting (`advanced_settings_enabled` in the
 * central Tauri store) so it survives restarts. Never mirror it to
 * localStorage.
 */

export const GET_ADVANCED_SETTINGS_ENABLED = "get_advanced_settings_enabled";
export const SET_ADVANCED_SETTINGS_ENABLED = "set_advanced_settings_enabled";

export interface AdvancedSettingsContextValue {
  /** True when every setting should be shown. */
  advanced: boolean;
  /** True until the persisted value has been read from the backend. */
  loading: boolean;
  /** Persist a new value; the UI updates optimistically and reverts on error. */
  setAdvanced: (enabled: boolean) => Promise<void>;
}

const AdvancedSettingsContext =
  createContext<AdvancedSettingsContextValue | null>(null);

export function AdvancedSettingsProvider({
  children,
}: {
  children: ReactNode;
}) {
  const [advanced, setAdvancedState] = useState(false);
  const [loading, setLoading] = useState(true);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    const load = async () => {
      try {
        const enabled = await invoke<boolean>(GET_ADVANCED_SETTINGS_ENABLED);
        if (mountedRef.current) setAdvancedState(Boolean(enabled));
      } catch (error) {
        console.error("Failed to load advanced settings flag:", error);
      } finally {
        if (mountedRef.current) setLoading(false);
      }
    };
    load();
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const setAdvanced = useCallback(async (enabled: boolean) => {
    // Optimistic update; reverted below if the backend rejects it.
    setAdvancedState(enabled);
    try {
      await invoke(SET_ADVANCED_SETTINGS_ENABLED, { enabled });
    } catch (error) {
      console.error("Failed to persist advanced settings flag:", error);
      if (mountedRef.current) setAdvancedState(!enabled);
      toast.error("Failed to update advanced settings");
    }
  }, []);

  const value = useMemo(
    () => ({ advanced, loading, setAdvanced }),
    [advanced, loading, setAdvanced]
  );

  return (
    <AdvancedSettingsContext.Provider value={value}>
      {children}
    </AdvancedSettingsContext.Provider>
  );
}

const OUTSIDE_PROVIDER: AdvancedSettingsContextValue = {
  // Outside the settings window nothing is gated, so a section rendered
  // elsewhere (onboarding, dev tools) never silently loses fields.
  advanced: true,
  loading: false,
  setAdvanced: async () => {},
};

export function useAdvancedSettings(): AdvancedSettingsContextValue {
  return useContext(AdvancedSettingsContext) ?? OUTSIDE_PROVIDER;
}

/** Renders its children only while advanced settings are enabled. */
export function AdvancedOnly({ children }: { children: ReactNode }) {
  const { advanced } = useAdvancedSettings();
  if (!advanced) return null;
  return <>{children}</>;
}
