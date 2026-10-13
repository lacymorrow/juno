import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { COMMANDS } from "@/lib/constants.generated";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { SettingsRow } from "../ui";

/** What Rust says about the cleanup pass (`dictation_cleanup::CleanupStatus`). */
interface CleanupStatus {
  enabled: boolean;
  availability: string;
  reason: string | null;
  can_open_settings: boolean;
}

const COPY = "Fixes grammar and punctuation on your Mac.";

/**
 * "Clean up dictation": one switch. Whether Apple's on-device model can run is
 * Rust's call; this only draws the answer. When it cannot, the switch is off
 * and disabled and the row says why, with a way into System Settings if that
 * is the fix.
 */
export function DictationCleanupRow() {
  const [status, setStatus] = useState<CleanupStatus | null>(null);

  useEffect(() => {
    let mounted = true;
    invoke<CleanupStatus>(COMMANDS.DICTATION_GET_DICTATION_CLEANUP_STATUS)
      .then((s) => {
        if (mounted && s && typeof s === "object" && "availability" in s) {
          setStatus(s);
        }
      })
      .catch(() => {
        // No status means no row: better than a switch that may do nothing.
      });
    return () => {
      mounted = false;
    };
  }, []);

  if (!status) return null;

  const usable = status.reason === null;

  const change = async (enabled: boolean) => {
    setStatus({ ...status, enabled });
    try {
      await invoke(COMMANDS.DICTATION_SET_DICTATION_CLEANUP_ENABLED, {
        enabled,
      });
    } catch {
      setStatus((current) => (current ? { ...current, enabled: !enabled } : current));
    }
  };

  return (
    <SettingsRow
      htmlFor="dictation-cleanup"
      label="Clean up dictation"
      beta
      description={
        usable ? (
          COPY
        ) : (
          <>
            {status.reason}
            {status.can_open_settings && (
              <>
                {" "}
                <Button
                  variant="link"
                  className="h-auto p-0 text-[12px]"
                  onClick={() =>
                    void invoke(
                      COMMANDS.DICTATION_OPEN_APPLE_INTELLIGENCE_SETTINGS,
                    ).catch(() => {})
                  }
                >
                  Open System Settings
                </Button>
              </>
            )}
          </>
        )
      }
    >
      <Switch
        id="dictation-cleanup"
        checked={usable && status.enabled}
        disabled={!usable}
        onCheckedChange={change}
      />
    </SettingsRow>
  );
}
