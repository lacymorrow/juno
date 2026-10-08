import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useUpdater, type UpdateStatus } from "@/hooks/useUpdater";
import { useState } from "react";
import { toast } from "sonner";
import { SettingsGroup, SettingsRow } from "./ui";

/**
 * The one place updates live.
 *
 * Every transition is decided in `src-tauri/src/updater.rs` and arrives as an
 * `UpdateStatus`. This renders that status and offers one action, so the row
 * cannot disagree with what the backend is actually doing.
 */

/** The sentence under "Juno x.y.z", for each stage. */
function describe(status: UpdateStatus): string {
  switch (status.stage) {
    case "checking":
      return "Looking for a newer version.";
    case "downloading": {
      const { downloadedBytes, totalBytes, availableVersion } = status;
      const version = availableVersion ?? "a new version";
      if (totalBytes && totalBytes > 0) {
        const percent = Math.min(
          100,
          Math.round((downloadedBytes / totalBytes) * 100),
        );
        return `Downloading ${version}, ${percent}%.`;
      }
      return `Downloading ${version}.`;
    }
    case "readyToRestart":
      return `Juno ${status.availableVersion ?? ""} is installed. It runs the next time Juno starts.`.replace(
        "  ",
        " ",
      );
    case "upToDate":
      return "This is the newest version.";
    case "failed":
      return status.error ?? "The last check did not finish.";
    case "idle":
    default:
      return "Juno checks on its own, shortly after launch and every few hours.";
  }
}

export function UpdatesGroup() {
  const { status, settings, checkNow, restart, saveSettings } = useUpdater();
  const [saving, setSaving] = useState(false);

  // The status is read from the backend on mount, which is a local call. The
  // row is hidden for that instant rather than flashing a wrong version.
  if (!status) return null;

  const busy = status.stage === "checking" || status.stage === "downloading";
  const ready = status.stage === "readyToRestart";

  const onPrimary = async () => {
    try {
      if (ready) {
        await restart();
      } else {
        await checkNow();
      }
    } catch (error) {
      console.error("Update action failed:", error);
      toast.error(
        ready
          ? "Couldn't restart. Quit Juno and open it again."
          : "Couldn't check for updates right now. Juno will try again later.",
      );
    }
  };

  const persist = async (next: {
    autoCheckEnabled: boolean;
    channel: "stable" | "prerelease";
  }) => {
    setSaving(true);
    try {
      await saveSettings(next);
    } catch (error) {
      toast.error(`Could not save: ${error}`);
    } finally {
      setSaving(false);
    }
  };

  const autoCheckEnabled = settings?.auto_check_enabled ?? true;
  const onPrereleases = (settings?.channel ?? "prerelease") === "prerelease";

  return (
    <SettingsGroup title="Updates">
      <SettingsRow
        id="app-updates"
        label={`Juno ${status.currentVersion}`}
        description={describe(status)}
      >
        <Button
          onClick={onPrimary}
          disabled={busy}
          variant={ready ? "default" : "outline"}
          size="sm"
        >
          {ready ? "Restart" : busy ? "Checking…" : "Check for Updates"}
        </Button>
      </SettingsRow>

      <SettingsRow
        advanced
        htmlFor="auto-update-check"
        label="Check automatically"
        description="Look for new versions in the background."
          info="Look for a new version shortly after launch, then every few hours. Off still leaves the button above."
      >
        <Switch
          id="auto-update-check"
          checked={autoCheckEnabled}
          disabled={saving}
          onCheckedChange={(checked) =>
            persist({
              autoCheckEnabled: checked,
              channel: onPrereleases ? "prerelease" : "stable",
            })
          }
        />
      </SettingsRow>

      <SettingsRow
        advanced
        htmlFor="update-prereleases"
        label="Get prereleases"
        description="Get every build as it is merged."
          info="Take every build as it is merged, not only the ones promoted for release. On while Juno's own team are the testers."
      >
        <Switch
          id="update-prereleases"
          checked={onPrereleases}
          disabled={saving}
          onCheckedChange={(checked) =>
            persist({
              autoCheckEnabled,
              channel: checked ? "prerelease" : "stable",
            })
          }
        />
      </SettingsRow>
    </SettingsGroup>
  );
}
