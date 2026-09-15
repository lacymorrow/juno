/**
 * Input control: the contract between the Rust takeover logic and the UI.
 *
 * Juno normally acts on other apps in the background, without touching the
 * physical cursor. A few things cannot be done that way. When one comes up the
 * backend asks, waits for an answer, and says when it has the mouse and when it
 * has given it back. Everything here is pure so the rules can be tested without
 * a running app.
 */

/** What the user can answer when Juno asks for the physical cursor. */
export type InputControlDecision = "once" | "always" | "deny";

/** `input-control-request` payload. */
export interface InputControlRequest {
  request_id: string;
  tool: string;
  reason: string;
  target_app?: string | null;
}

/** `input-control-state` payload. */
export interface InputControlStatePayload {
  active: boolean;
  tool?: string | null;
  target_app?: string | null;
}

/** `app-reopen-attempt` payload. */
export interface ReopenAttemptPayload {
  count: number;
}

/** How Juno treats a request for the physical cursor. */
export type MouseControlMode = "ask" | "always";

/**
 * Reopen attempts inside the backend's one minute window before the quiet
 * "we are in the menu bar" hint turns into an offer to undo menu-bar mode.
 * Three tries in a minute is what scrambling looks like.
 */
export const REOPEN_ESCALATION_THRESHOLD = 3;

/** What the Juno pane shows after someone tries to reopen an already-running Juno. */
export type ReopenNotice = "none" | "hint" | "escalated";

/**
 * Which reopen notice a given attempt count earns.
 *
 * One or two tries: a calm line saying where Juno lives. Three or more in the
 * same minute: the two things they actually want, a Dock icon back or a quit.
 */
export function reopenNoticeFor(count: number): ReopenNotice {
  if (!Number.isFinite(count) || count < 1) return "none";
  return count >= REOPEN_ESCALATION_THRESHOLD ? "escalated" : "hint";
}

/**
 * Whether the one-time "stop asking" offer may be shown.
 *
 * It is worth asking only while Juno is still asking every time, only if the
 * user has not already said to stop offering, and at most once per session.
 * Anything else is nagging.
 */
export function shouldShowMouseControlOffer(input: {
  mode: MouseControlMode;
  dismissed: boolean;
  offeredThisSession: boolean;
}): boolean {
  return (
    input.mode === "ask" && !input.dismissed && !input.offeredThisSession
  );
}

/** One line naming what Juno is about to do, with the app it will do it in. */
export function describeRequest(request: InputControlRequest): string {
  const reason = request.reason?.trim();
  const app = request.target_app?.trim();
  const what = reason && reason.length > 0 ? reason : "finish what you asked for";
  return app ? `${what} in ${app}` : what;
}

/** The floating bar's label while Juno holds the physical cursor. */
export function drivingLabel(state: InputControlStatePayload): string {
  const app = state.target_app?.trim();
  return app ? `using the mouse in ${app}` : "using the mouse";
}
