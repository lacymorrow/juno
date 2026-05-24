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

export function useOnboardingState() {
  const [state, setState] = useState<OnboardingStateInfo | null>(null);

  // Fetch current state on mount
  useEffect(() => {
    invoke<OnboardingStateInfo>("get_onboarding_state")
      .then(setState)
      .catch(() => {});
  }, []);

  // Live updates from the backend state machine — single source of truth.
  // The backend emits this event on all transitions: complete, skip, restart, and init.
  useEventListener<OnboardingStateInfo>(EVENTS.ONBOARDING_STATE_CHANGED, (payload) => {
    setState(payload);
  });

  const isOnboardingActive =
    state !== null && state.phase !== "complete";

  return { state, isOnboardingActive };
}
