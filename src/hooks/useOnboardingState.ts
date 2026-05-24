/**
 * Hook that tracks the current onboarding phase from the backend.
 *
 * Subscribes to "onboarding-state-changed" events and provides a boolean
 * `isOnboardingActive` flag the main window uses to gate the text input.
 * Initial state is fetched via get_onboarding_state on mount.
 */
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useEventListener } from "@/hooks/useEventListener";
import { EVENTS } from "@/lib/constants.generated";

export type OnboardingPhase =
  | "greeting"
  | "screen_recording"
  | "accessibility"
  | "optional_permissions"
  | "provider"
  | "ready"
  | "complete";

export interface OnboardingStateInfo {
  phase: OnboardingPhase;
  can_advance: boolean;
  can_skip: boolean;
}

const COMPLETE_STATE: OnboardingStateInfo = {
  phase: "complete",
  can_advance: false,
  can_skip: false,
};

export function useOnboardingState() {
  const [state, setState] = useState<OnboardingStateInfo | null>(null);

  // Fetch current state on mount
  useEffect(() => {
    invoke<OnboardingStateInfo>("get_onboarding_state")
      .then(setState)
      .catch(() => {});
  }, []);

  // Live updates from the backend state machine
  useEventListener<OnboardingStateInfo>(EVENTS.ONBOARDING_STATE_CHANGED, (payload) => {
    setState(payload);
  });

  // Also listen for completion/skip events from the onboarding window
  useEventListener(EVENTS.ONBOARDING_COMPLETE, () => {
    setState(COMPLETE_STATE);
  });

  useEventListener(EVENTS.ONBOARDING_SKIPPED, () => {
    setState(COMPLETE_STATE);
  });

  const isOnboardingActive =
    state !== null && state.phase !== "complete";

  return { state, isOnboardingActive };
}
