import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  Confirmation,
  ConfirmationAction,
  ConfirmationAccepted,
  ConfirmationActions,
  ConfirmationRequest,
} from "@/components/ai-elements/confirmation";
import { useEventListener } from "@/hooks/useEventListener";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { cn } from "@/lib/utils";

/**
 * The permission ask, at the moment Juno actually needs the permission.
 *
 * Onboarding no longer demands Accessibility and Screen Recording up front, so
 * this card is where the question gets asked instead: after the person has told
 * Juno to do something, with the reason attached, and with "Not now" as an
 * answer that costs them nothing.
 *
 * It never takes focus and renders nothing when there is nothing to ask.
 */

const CARD = "mt-0 mb-2 border-solid border-border bg-card dark:bg-card shadow-sm";
const BODY = "col-start-2 flex w-full flex-col gap-2";
const TITLE = "text-[13px] font-medium leading-tight text-foreground";
const DETAIL = "text-[12px] leading-snug text-muted-foreground";

/** How often to look for the switch being flipped while the card is up. */
const POLL_MS = 1500;

/** Stop polling after this long, so a card left open forever is not a timer leak. */
const POLL_FOR_MS = 120000;

interface PermissionNeeded {
  permission: string;
  action: string;
  title: string;
  detail: string;
}

interface PermissionStatusShape {
  granted: boolean;
}

interface PermissionsStateShape {
  accessibility: PermissionStatusShape;
  screen_recording: PermissionStatusShape;
}

/** Reads one permission out of the state blob by the key the backend sent. */
function isGranted(state: PermissionsStateShape, permission: string): boolean {
  if (permission === "accessibility") return Boolean(state.accessibility?.granted);
  if (permission === "screen_recording") return Boolean(state.screen_recording?.granted);
  return false;
}

/** "screen_recording" reads as "Screen Recording" in the confirmation line. */
function humanName(permission: string): string {
  return permission
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

export function PermissionNotice({ className }: { className?: string }) {
  const [needed, setNeeded] = useState<PermissionNeeded | null>(null);
  const [granted, setGranted] = useState(false);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Polling only runs while a card is up and only for a bounded stretch.
  const pollStartedAt = useRef<number>(0);

  useEventListener<PermissionNeeded>(EVENTS.PERMISSIONS_NEEDED, (payload) => {
    if (!payload?.permission) return;
    setNeeded((current) => {
      // A second ask for the same thing should not restart the card underneath
      // someone who is reading it.
      if (current?.permission === payload.permission) return current;
      return payload;
    });
    setGranted(false);
    setError(null);
    pollStartedAt.current = Date.now();
  });

  // Watch for the switch being flipped in System Settings. The card turns into
  // a confirmation rather than vanishing, so the person sees it worked.
  useEffect(() => {
    if (!needed || granted) return;

    const timer = window.setInterval(() => {
      if (Date.now() - pollStartedAt.current > POLL_FOR_MS) {
        window.clearInterval(timer);
        return;
      }
      void (async () => {
        try {
          const state = await invoke<PermissionsStateShape>(
            COMMANDS.PERMISSIONS_CHECK_PERMISSIONS_STATUS
          );
          if (isGranted(state, needed.permission)) setGranted(true);
        } catch {
          // A failed poll is not worth saying anything about; the next one may
          // well succeed, and the person can still use the buttons.
        }
      })();
    }, POLL_MS);

    return () => window.clearInterval(timer);
  }, [needed, granted]);

  // Once it is on, say so briefly and get out of the way.
  useEffect(() => {
    if (!granted) return;
    const timer = window.setTimeout(() => setNeeded(null), 6000);
    return () => window.clearTimeout(timer);
  }, [granted]);

  const openSettings = useCallback(async () => {
    if (!needed) return;
    setOpening(true);
    setError(null);
    try {
      await invoke(COMMANDS.PERMISSIONS_OPEN_SYSTEM_SETTINGS, {
        permissionType: needed.permission,
      });
    } catch {
      setError(
        "System Settings did not open. You can find this under Privacy and Security."
      );
    } finally {
      setOpening(false);
    }
  }, [needed]);

  const dismiss = useCallback(() => {
    setNeeded(null);
    setError(null);
  }, []);

  if (!needed) return null;

  return (
    <div
      data-testid="permission-notice"
      role="status"
      aria-live="polite"
      className={cn("shrink-0 px-4 pt-2", className)}
    >
      <Confirmation
        state={granted ? "approval-responded" : "approval-requested"}
        className={CARD}
        data-testid="permission-prompt"
      >
        {/* The request slot hides itself once the state moves on, and the
            accepted slot takes over, so the card turns into its own receipt
            rather than blinking out of existence. */}
        <ConfirmationRequest>
          <div className={BODY}>
            <p className={TITLE}>{needed.title}</p>
            <p className={DETAIL}>{needed.detail}</p>
            <ConfirmationActions className="flex-wrap">
              <ConfirmationAction onClick={() => void openSettings()} disabled={opening}>
                Open Settings
              </ConfirmationAction>
              <ConfirmationAction variant="ghost" onClick={dismiss}>
                Not now
              </ConfirmationAction>
            </ConfirmationActions>
            {error && (
              <p className={cn(DETAIL, "text-destructive")} role="alert">
                {error}
              </p>
            )}
          </div>
        </ConfirmationRequest>
        <ConfirmationAccepted className="col-start-2">
          {humanName(needed.permission)} is on. Ask again and Juno will pick up
          where it left off.
        </ConfirmationAccepted>
      </Confirmation>
    </div>
  );
}
