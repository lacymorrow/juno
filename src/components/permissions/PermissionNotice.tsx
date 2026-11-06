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
 * One card, three states, one button each:
 *
 * 1. the ask, whose button is whatever the backend said it is ("Open Settings"
 *    for a switch in System Settings, "Allow" for Automation, where macOS does
 *    the asking and there is no pane to open until it has),
 * 2. the restart, for the permissions macOS will not hand a running process,
 *    which the backend decides per capability so nobody is told to restart for
 *    a permission that does not need it, and
 * 3. the receipt, once Juno can act.
 *
 * Every one of those decisions is made in Rust. This file renders them.
 *
 * Two ways in, because one was never enough. The `permission-needed` event is
 * the nudge, and it only reaches a surface that is already mounted. Every
 * surface that draws this card mounts later than that: the Pill opens its pane
 * *on* the event, and the Island mounts the conversation only on its chat
 * face. So on mount the card also asks Rust for the ask already standing
 * (`standing_permission_ask`), which is what makes a late surface ask the
 * question at all rather than showing an empty pane (juno#946).
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
  /** What the one button does. Decided in Rust, not inferred here. */
  primary_action?: "open_settings" | "allow_automation";
  /** The words on that button. */
  primary_label?: string;
}

/** The backend's answer about one capability, right now. */
interface PermissionMoment {
  permission: string;
  granted: boolean;
  restart_unblocks: boolean;
  restart_detail: string;
}

/** "screen_recording" reads as "Screen Recording" in the confirmation line. */
function humanName(permission: string): string {
  return permission
    .replace(/^automation_/, "")
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

/** The receipt line. Automation is a yes about an app, not a switch being on. */
function receipt(permission: string): string {
  const name = humanName(permission);
  return permission.startsWith("automation_")
    ? `Juno can control ${name} now. Ask again and Juno will pick up where it left off.`
    : `${name} is on. Ask again and Juno will pick up where it left off.`;
}

export function PermissionNotice({ className }: { className?: string }) {
  const [needed, setNeeded] = useState<PermissionNeeded | null>(null);
  const [granted, setGranted] = useState(false);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** The restart sentence, once the backend says a restart is the missing step. */
  const [restart, setRestart] = useState<string | null>(null);
  const [restarting, setRestarting] = useState(false);
  /** True once the person has done their part, which is when a restart is news. */
  const [answered, setAnswered] = useState(false);

  // Polling only runs while a card is up and only for a bounded stretch.
  const pollStartedAt = useRef<number>(0);

  /**
   * True once the event has been heard here. The mount read below resolves a
   * tick later, and the ask in front of the person wins over the one Rust is
   * holding (they are the same ask in every case but a second need arriving in
   * that tick).
   */
  const heardEventRef = useRef(false);

  useEventListener<PermissionNeeded>(EVENTS.PERMISSIONS_NEEDED, (payload) => {
    if (!payload?.permission) return;
    heardEventRef.current = true;
    setNeeded((current) => {
      // A second ask for the same thing should not restart the card underneath
      // someone who is reading it.
      if (current?.permission === payload.permission) return current;
      return payload;
    });
    setGranted(false);
    setError(null);
    setRestart(null);
    setAnswered(false);
    pollStartedAt.current = Date.now();
  });

  // The ask Juno made before this surface existed. Rust drops one that has
  // gone stale or has since been granted, so anything that comes back here is
  // a question still worth asking.
  useEffect(() => {
    let mounted = true;
    void (async () => {
      try {
        const standing = await invoke<PermissionNeeded | null>(COMMANDS.PERMISSIONS_STANDING_ASK);
        if (!mounted || heardEventRef.current || !standing?.permission) return;
        pollStartedAt.current = Date.now();
        setNeeded(standing);
      } catch {
        // Nothing standing is the ordinary answer, and a failed read says the
        // same thing: say nothing.
      }
    })();
    return () => {
      mounted = false;
    };
  }, []);

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
          const moment = await invoke<PermissionMoment>(COMMANDS.PERMISSIONS_PERMISSION_MOMENT, {
            permission: needed.permission,
          });
          if (moment.granted) {
            setGranted(true);
            return;
          }
          // Only after the person has acted. Before that, a restart is not the
          // missing step, it is a step they have not reached.
          if (answered && moment.restart_unblocks && moment.restart_detail) {
            setRestart(moment.restart_detail);
          }
        } catch {
          // A failed poll is not worth saying anything about; the next one may
          // well succeed, and the person can still use the buttons.
        }
      })();
    }, POLL_MS);

    return () => window.clearInterval(timer);
  }, [needed, granted, answered]);

  // Once it is on, say so briefly and get out of the way.
  useEffect(() => {
    if (!granted) return;
    const timer = window.setTimeout(() => setNeeded(null), 6000);
    return () => window.clearTimeout(timer);
  }, [granted]);

  /** The card's one button, before a restart is on offer. */
  const primary = useCallback(async () => {
    if (!needed) return;
    setOpening(true);
    setError(null);
    try {
      if (needed.primary_action === "allow_automation") {
        const allowed = await invoke<boolean>(COMMANDS.PERMISSIONS_ALLOW_AUTOMATION, {
          target: needed.permission.replace(/^automation_/, ""),
        });
        if (allowed) setGranted(true);
        else setError("macOS did not allow it. You can change this later in System Settings.");
      } else {
        await invoke(COMMANDS.PERMISSIONS_OPEN_SYSTEM_SETTINGS, {
          permissionType: needed.permission,
        });
      }
      // Their part is done. A restart offer is now the next step rather than a
      // demand made before they had a chance.
      setAnswered(true);
    } catch {
      setError(
        needed.primary_action === "allow_automation"
          ? "macOS did not answer. Try again, or change this later in System Settings."
          : "System Settings did not open. You can find this under Privacy and Security."
      );
    } finally {
      setOpening(false);
    }
  }, [needed]);

  const restartJuno = useCallback(async () => {
    setRestarting(true);
    setError(null);
    try {
      await invoke(COMMANDS.PERMISSIONS_RESTART_AFTER_PERMISSIONS);
    } catch {
      setRestarting(false);
      setError("Juno could not restart itself. Quit and open it again.");
    }
  }, []);

  const dismiss = useCallback(() => {
    setNeeded(null);
    setError(null);
    setRestart(null);
    // Not now is a real answer, so Rust forgets the ask. Without this the next
    // surface to open would read it back and ask again.
    void invoke(COMMANDS.PERMISSIONS_DISMISS_ASK).catch(() => {});
  }, []);

  if (!needed) return null;

  const offeringRestart = Boolean(restart) && !granted;

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
            <p className={TITLE}>
              {offeringRestart
                ? `Juno has to start again to pick up ${humanName(needed.permission)}`
                : needed.title}
            </p>
            <p className={DETAIL}>{offeringRestart ? restart : needed.detail}</p>
            <ConfirmationActions className="flex-wrap">
              {offeringRestart ? (
                <ConfirmationAction
                  onClick={() => void restartJuno()}
                  disabled={restarting}
                  data-testid="permission-restart"
                >
                  {restarting ? "Restarting Juno" : "Restart Juno"}
                </ConfirmationAction>
              ) : (
                <ConfirmationAction
                  onClick={() => void primary()}
                  disabled={opening}
                  data-testid="permission-primary"
                >
                  {needed.primary_label ?? "Open Settings"}
                </ConfirmationAction>
              )}
              <ConfirmationAction variant="ghost" onClick={dismiss}>
                Not now
              </ConfirmationAction>
            </ConfirmationActions>
            {error && (
              <p
                className={cn(DETAIL, "text-destructive")}
                role="alert"
                data-testid="permission-error"
              >
                {error}
              </p>
            )}
          </div>
        </ConfirmationRequest>
        <ConfirmationAccepted className="col-start-2">
          {receipt(needed.permission)}
        </ConfirmationAccepted>
      </Confirmation>
    </div>
  );
}
