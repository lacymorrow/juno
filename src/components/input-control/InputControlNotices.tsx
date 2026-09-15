import { useCallback, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { exit } from "@tauri-apps/plugin-process";

import {
  Confirmation,
  ConfirmationAction,
  ConfirmationActions,
  ConfirmationRequest,
} from "@/components/ai-elements/confirmation";
import { useEventListener } from "@/hooks/useEventListener";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import {
  describeRequest,
  reopenNoticeFor,
  shouldShowMouseControlOffer,
  type InputControlDecision,
  type InputControlRequest,
  type InputControlStatePayload,
  type MouseControlMode,
  type ReopenAttemptPayload,
  type ReopenNotice,
} from "@/lib/inputControl";
import { cn } from "@/lib/utils";

/**
 * The three things Juno has to say about running in the background, pinned
 * under the conversation in both the main window and the floating bar's pane.
 *
 * 1. It needs the physical mouse for one step and is asking.
 * 2. It just finished a takeover and can stop asking, if you like.
 * 3. You tried to open Juno while it was already running in the menu bar.
 *
 * Nothing here ever takes focus. Every card is a plain stack of buttons in tab
 * order, and the whole thing renders nothing at all when there is nothing to
 * say.
 */

/** Neutral card chrome: the state colours in `Confirmation` are for tools. */
const CARD =
  "mt-0 mb-2 border-solid border-border bg-card dark:bg-card shadow-sm";

/** Alert lays out a two-column grid; content belongs in the second column. */
const BODY = "col-start-2 flex w-full flex-col gap-2";

const TITLE = "text-[13px] font-medium leading-tight text-foreground";
const DETAIL = "text-[12px] leading-snug text-muted-foreground";

interface AgentSettingsShape {
  mouse_control?: string;
  mouse_control_prompt_dismissed?: boolean;
}

export function InputControlNotices({ className }: { className?: string }) {
  const [request, setRequest] = useState<InputControlRequest | null>(null);
  const [sending, setSending] = useState(false);
  const [requestError, setRequestError] = useState<string | null>(null);

  const [offerOpen, setOfferOpen] = useState(false);
  // Once per session, never twice. A ref, because the guard has to hold across
  // two offers that arrive in the same tick.
  const offeredThisSession = useRef(false);

  const [reopen, setReopen] = useState<ReopenNotice>("none");

  // --- Juno is asking for the physical cursor ---

  useEventListener<InputControlRequest>(EVENTS.INPUT_CONTROL_REQUEST, (payload) => {
    if (!payload?.request_id) return;
    setRequestError(null);
    setSending(false);
    setRequest(payload);
  });

  // A takeover that started (or ended) settles the question either way.
  useEventListener<InputControlStatePayload>(EVENTS.INPUT_CONTROL_STATE, (payload) => {
    if (payload?.active) setRequest(null);
  });

  const respond = useCallback(
    async (decision: InputControlDecision) => {
      const pending = request;
      if (!pending || sending) return;
      setSending(true);
      setRequestError(null);
      try {
        await invoke(COMMANDS.INPUT_CONTROL_RESPOND_TO_REQUEST, {
          requestId: pending.request_id,
          decision,
        });
        setRequest(null);
      } catch (error) {
        console.error("Failed to answer the mouse control request:", error);
        setRequestError("Juno did not get that. Try again.");
      } finally {
        setSending(false);
      }
    },
    [request, sending],
  );

  // --- The one-time offer to stop asking ---

  useEventListener(EVENTS.INPUT_CONTROL_OFFER, () => {
    if (offeredThisSession.current) return;
    void (async () => {
      let mode: MouseControlMode = "ask";
      let dismissed = false;
      try {
        const settings = await invoke<AgentSettingsShape>(
          COMMANDS.SETTINGS_GET_AGENT_SETTINGS,
        );
        mode = settings?.mouse_control === "always" ? "always" : "ask";
        dismissed = Boolean(settings?.mouse_control_prompt_dismissed);
      } catch (error) {
        // Cannot read the setting, so cannot know it is welcome. Stay quiet.
        console.error("Failed to read mouse control settings:", error);
        return;
      }
      if (
        !shouldShowMouseControlOffer({
          mode,
          dismissed,
          offeredThisSession: offeredThisSession.current,
        })
      ) {
        return;
      }
      offeredThisSession.current = true;
      setOfferOpen(true);
    })();
  });

  const acceptOffer = useCallback(async () => {
    setOfferOpen(false);
    try {
      await invoke(COMMANDS.INPUT_CONTROL_SET_MOUSE_CONTROL, { mode: "always" });
    } catch (error) {
      console.error("Failed to set mouse control to always:", error);
    }
  }, []);

  const dismissOfferForever = useCallback(async () => {
    setOfferOpen(false);
    try {
      await invoke(COMMANDS.INPUT_CONTROL_DISMISS_PROMPT);
    } catch (error) {
      console.error("Failed to dismiss the mouse control prompt:", error);
    }
  }, []);

  // --- "I opened Juno and nothing happened" ---

  useEventListener<ReopenAttemptPayload>(
    EVENTS.APP_LIFECYCLE_REOPEN_ATTEMPT,
    (payload) => setReopen(reopenNoticeFor(payload?.count ?? 0)),
  );

  const restoreDockIcon = useCallback(async () => {
    setReopen("none");
    try {
      await invoke(COMMANDS.INPUT_CONTROL_SET_DOCK_ICON_VISIBLE, { visible: true });
    } catch (error) {
      console.error("Failed to show Juno in the Dock:", error);
    }
  }, []);

  const quitJuno = useCallback(async () => {
    try {
      await exit(0);
    } catch (error) {
      console.error("Failed to quit Juno:", error);
    }
  }, []);

  if (!request && !offerOpen && reopen === "none") return null;

  return (
    <div
      data-testid="input-control-notices"
      role="status"
      aria-live="polite"
      className={cn("shrink-0 px-4 pt-2", className)}
    >
      {request && (
        <Confirmation
          state="approval-requested"
          className={CARD}
          data-testid="input-control-prompt"
        >
          <ConfirmationRequest>
            <div className={BODY}>
              <p className={TITLE}>Juno needs your mouse</p>
              <p className={DETAIL}>
                Juno wants to {describeRequest(request)}. It cannot do this one in
                the background, so it has to move your pointer for a moment. Your
                cursor gets bigger while it works.
              </p>
              <ConfirmationActions className="flex-wrap">
                <ConfirmationAction
                  onClick={() => void respond("once")}
                  disabled={sending}
                >
                  Allow once
                </ConfirmationAction>
                <ConfirmationAction
                  variant="outline"
                  onClick={() => void respond("always")}
                  disabled={sending}
                >
                  Always allow
                </ConfirmationAction>
                <ConfirmationAction
                  variant="ghost"
                  onClick={() => void respond("deny")}
                  disabled={sending}
                >
                  Not now
                </ConfirmationAction>
              </ConfirmationActions>
              {requestError && (
                <p className={cn(DETAIL, "text-destructive")} role="alert">
                  {requestError}
                </p>
              )}
            </div>
          </ConfirmationRequest>
        </Confirmation>
      )}

      {offerOpen && (
        <Confirmation
          state="approval-requested"
          className={CARD}
          data-testid="mouse-control-offer"
        >
          <ConfirmationRequest>
            <div className={BODY}>
              <p className={TITLE}>Let Juno use the mouse without asking?</p>
              <p className={DETAIL}>
                You can change this any time under Settings, Advanced, Mouse
                control.
              </p>
              <ConfirmationActions className="flex-wrap">
                <ConfirmationAction onClick={() => void acceptOffer()}>
                  Yes
                </ConfirmationAction>
                <ConfirmationAction
                  variant="outline"
                  onClick={() => setOfferOpen(false)}
                >
                  No
                </ConfirmationAction>
                <ConfirmationAction
                  variant="ghost"
                  onClick={() => void dismissOfferForever()}
                >
                  Don't ask again
                </ConfirmationAction>
              </ConfirmationActions>
            </div>
          </ConfirmationRequest>
        </Confirmation>
      )}

      {reopen !== "none" && (
        <Confirmation
          state="approval-requested"
          className={CARD}
          data-testid="reopen-notice"
          data-notice={reopen}
        >
          <ConfirmationRequest>
            <div className={BODY}>
              <p className={TITLE}>Juno is already running</p>
              <p className={DETAIL}>
                {reopen === "hint"
                  ? "It lives in the menu bar at the top of your screen. Click the Juno icon there to open it."
                  : "It has no Dock icon right now, so opening it from Applications does nothing new. Put the Dock icon back, or quit Juno."}
              </p>
              <ConfirmationActions className="flex-wrap">
                {reopen === "escalated" ? (
                  <>
                    <ConfirmationAction onClick={() => void restoreDockIcon()}>
                      Show Juno in the Dock again
                    </ConfirmationAction>
                    <ConfirmationAction
                      variant="outline"
                      onClick={() => void quitJuno()}
                    >
                      Quit Juno
                    </ConfirmationAction>
                  </>
                ) : (
                  <ConfirmationAction
                    variant="outline"
                    onClick={() => setReopen("none")}
                  >
                    Got it
                  </ConfirmationAction>
                )}
              </ConfirmationActions>
            </div>
          </ConfirmationRequest>
        </Confirmation>
      )}
    </div>
  );
}
