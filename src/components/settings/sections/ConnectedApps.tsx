import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";

import { SettingsGroup, SettingsRow } from "../ui";

/**
 * Connected apps (LAC-4210). Deliberately not a section of its own: most
 * people never use integrations, so there is no catalog and no curated list.
 * Connecting happens just-in-time from a reply card in the conversation; what
 * lives here is only management (a row per connected app, Disconnect) and the
 * one Advanced toggle that turns the internal beta on.
 *
 * The "Connected apps" group renders only once something is connected. Empty
 * means not shown, not an empty state.
 */

interface ConnectedApp {
  toolkit_slug: string;
  app_name: string;
  account?: string | null;
  connected_at: number;
}

interface IntegrationsStatus {
  enabled: boolean;
  authorized: boolean;
  connected_apps: ConnectedApp[];
}

export function ConnectedApps() {
  const [status, setStatus] = useState<IntegrationsStatus | null>(null);
  const [pending, setPending] = useState(false);

  const load = useCallback(() => {
    invoke<IntegrationsStatus>(COMMANDS.INTEGRATIONS_GET_INTEGRATIONS_STATUS)
      .then(setStatus)
      .catch((error) =>
        console.error("Failed to load integrations status:", error)
      );
  }, []);

  useEffect(() => {
    load();
  }, [load]);
  useEventListener(EVENTS.INTEGRATIONS_CONNECTIONS_CHANGED, load);

  const handleToggle = async (enabled: boolean) => {
    setPending(true);
    try {
      const updated = await invoke<IntegrationsStatus>(
        COMMANDS.INTEGRATIONS_SET_BYO_COMPOSIO_ENABLED,
        { enabled }
      );
      setStatus(updated);
      toast.success(
        enabled
          ? "Connected apps are on. Ask for your mail or calendar to connect an app."
          : "Connected apps are off."
      );
    } catch (error) {
      console.error("Failed to toggle connected apps:", error);
      toast.error(
        typeof error === "string" ? error : "Could not change the setting"
      );
      load();
    } finally {
      setPending(false);
    }
  };

  const handleDisconnect = async (app: ConnectedApp) => {
    try {
      await invoke(COMMANDS.INTEGRATIONS_DISCONNECT_INTEGRATION_APP, {
        toolkitSlug: app.toolkit_slug,
      });
      toast.success(`${app.app_name} disconnected`);
      load();
    } catch (error) {
      console.error("Failed to disconnect app:", error);
      toast.error(
        typeof error === "string" ? error : `Could not disconnect ${app.app_name}`
      );
    }
  };

  const apps = status?.connected_apps ?? [];

  return (
    <>
      {apps.length > 0 && (
        <SettingsGroup
          title="Connected apps"
          footer="Juno asks before an app sends, deletes, or shares anything."
        >
          {apps.map((app) => (
            <SettingsRow
              key={app.toolkit_slug}
              label={app.app_name}
              description={app.account ?? undefined}
            >
              <Button
                size="sm"
                variant="outline"
                onClick={() => handleDisconnect(app)}
              >
                Disconnect
              </Button>
            </SettingsRow>
          ))}
        </SettingsGroup>
      )}

      <SettingsGroup
        advanced
        title="Connected apps (internal beta)"
        footer="Signs this Mac into your own Composio account in your browser. Apps then connect from the conversation when a request needs one."
      >
        <SettingsRow
          htmlFor="byo-composio"
          label="Use my own Composio account"
          description={
            status?.enabled && !status.authorized
              ? "Turned on, but not signed in yet. Turn it off and on to retry."
              : undefined
          }
        >
          <Switch
            id="byo-composio"
            checked={status?.enabled ?? false}
            disabled={pending || status === null}
            onCheckedChange={handleToggle}
          />
        </SettingsRow>
      </SettingsGroup>
    </>
  );
}
