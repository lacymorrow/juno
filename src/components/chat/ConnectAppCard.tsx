import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { COMMANDS } from "@/lib/constants.generated";

/**
 * The just-in-time connect card (LAC-4210).
 *
 * This is Juno's whole integrations UI for the common case: no settings
 * screen, no catalog. When a request needs an app that is not connected, the
 * reply is this card with one button. The button runs the app's own consent
 * flow in the person's browser (the backend drives it; the agent never runs
 * OAuth), and when consent lands, the original request is asked again.
 *
 * Designed states, per the acceptance list: connect (idle), connecting
 * (browser open), connected (request retrying), and failed (one sentence,
 * button becomes Try again). "Composio" appears exactly once in the product,
 * as the disclosure line on a first-ever connect card.
 */

type Phase = "idle" | "connecting" | "retrying" | "done" | "failed";

interface ConnectAppCardProps {
  toolkitSlug: string;
  appName: string;
  /** The request to ask again once the app connects. */
  retryQuery?: string;
}

export const ConnectAppCard = ({
  toolkitSlug,
  appName,
  retryQuery,
}: ConnectAppCardProps) => {
  const [phase, setPhase] = useState<Phase>("idle");
  const [failure, setFailure] = useState<string | null>(null);
  // Named once, on the first connect ever; silent after anything is connected.
  const [showDisclosure, setShowDisclosure] = useState(false);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    invoke<{ connected_apps: unknown[] }>(
      COMMANDS.INTEGRATIONS_GET_INTEGRATIONS_STATUS
    )
      .then((status) => {
        if (mounted.current && status.connected_apps.length === 0) {
          setShowDisclosure(true);
        }
      })
      .catch(() => {});
    return () => {
      mounted.current = false;
    };
  }, []);

  const connect = async () => {
    setPhase("connecting");
    setFailure(null);
    try {
      await invoke(COMMANDS.INTEGRATIONS_CONNECT_INTEGRATION_APP, {
        toolkitSlug,
      });
      if (!mounted.current) return;
      if (retryQuery) {
        setPhase("retrying");
        await invoke(COMMANDS.AGENT_DISPATCH_QUERY, {
          query: retryQuery,
          images: null,
        });
      }
      if (mounted.current) setPhase("done");
    } catch (error) {
      if (!mounted.current) return;
      setFailure(
        typeof error === "string" ? error : "The connection did not finish."
      );
      setPhase("failed");
    }
  };

  const line = (() => {
    switch (phase) {
      case "idle":
        return `Juno needs your ${appName}.`;
      case "connecting":
        return `Approve ${appName} in your browser, then come back.`;
      case "retrying":
        return `${appName} connected. Asking again…`;
      case "done":
        return retryQuery
          ? `${appName} connected.`
          : `${appName} connected. Ask me again.`;
      case "failed":
        return failure ?? "The connection did not finish.";
    }
  })();

  return (
    <div className="flex justify-start w-full">
      <div className="max-w-sm space-y-3 rounded-[10px] border border-border bg-card p-4 shadow-sm">
        <p className="text-sm leading-snug">{line}</p>
        {(phase === "idle" || phase === "connecting" || phase === "failed") && (
          <Button size="sm" onClick={connect} disabled={phase === "connecting"}>
            {phase === "connecting"
              ? "Waiting for your browser…"
              : phase === "failed"
                ? "Try again"
                : `Connect ${appName}`}
          </Button>
        )}
        {showDisclosure && phase === "idle" && (
          <p className="text-xs leading-snug text-muted-foreground">
            Connections go through Composio, which holds the sign-in for this
            app.
          </p>
        )}
      </div>
    </div>
  );
};
