import OnboardingFlow from "@/components/onboarding/Onboarding";
import { invoke } from "@tauri-apps/api/core";
import { Window } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { WINDOW_LABELS } from "@/lib/constants.generated";
import { getPermissionsStatus } from "@/lib/permissions-service";

export default function OnboardingWindow() {
  console.log("OnboardingWindow: Component rendering/re-rendering");

  const [permissionsGranted, setPermissionsGranted] = useState(false);
  const [permissionsChecked, setPermissionsChecked] = useState(false);
  const [isDevelopmentMode, setIsDevelopmentMode] = useState(false);

  console.log(
    "OnboardingWindow: State - permissionsGranted:",
    permissionsGranted,
    "permissionsChecked:",
    permissionsChecked,
    "isDevelopmentMode:",
    isDevelopmentMode
  );

  // Detect system color scheme and apply dark class
  useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const applyTheme = (dark: boolean) => {
      document.documentElement.classList.toggle("dark", dark);
    };
    applyTheme(mediaQuery.matches);
    const handler = (e: MediaQueryListEvent) => applyTheme(e.matches);
    mediaQuery.addEventListener("change", handler);
    return () => mediaQuery.removeEventListener("change", handler);
  }, []);

  // Notify backend that onboarding is active (registers escape key, suppresses agent/dictation actions)
  useEffect(() => {
    invoke("set_onboarding_active", { active: true }).catch((e) =>
      console.warn("Failed to set onboarding active:", e)
    );

    return () => {
      invoke("set_onboarding_active", { active: false }).catch((e) =>
        console.warn("Failed to clear onboarding active:", e)
      );
    };
  }, []);

  // Check permissions and onboarding info on mount
  useEffect(() => {
    const checkInitialData = async () => {
      try {
        console.log("OnboardingWindow: Starting permission check...");

        // Check permissions using centralized service - prevents duplicate calls
        const permissionsResult = await getPermissionsStatus();

        console.log("OnboardingWindow: Permissions result:", permissionsResult);
        setPermissionsGranted(permissionsResult.all_granted);

        // Get onboarding info including development mode status
        const onboardingInfo = await invoke<any>("get_onboarding_info");
        console.log("OnboardingWindow: Onboarding info:", onboardingInfo);
        setIsDevelopmentMode(onboardingInfo?.is_development_mode || false);

        console.log("OnboardingWindow: Setting permissions checked to true");
        setPermissionsChecked(true);
      } catch (error) {
        console.error("OnboardingWindow: Error checking initial data:", error);
        console.error(
          "OnboardingWindow: Error details:",
          JSON.stringify(error, null, 2)
        );
        setPermissionsChecked(true);
      }
    };

    checkInitialData();
  }, []);

  const handleOnboardingComplete = async () => {
    try {
      // Use backend command to mark onboarding as completed
      await invoke("complete_onboarding");
      console.log("Onboarding completed via backend");

      // Notify main window of completion
      const mainWindow = await Window.getByLabel(WINDOW_LABELS.MAIN);
      if (mainWindow) {
        await mainWindow.emit("onboarding-complete", {});
      }

      // Close the onboarding window via backend
      await invoke("close_onboarding_window");
    } catch (error) {
      console.error("Error completing onboarding:", error);
      // Try to close anyway
      try {
        await invoke("close_onboarding_window");
      } catch (closeError) {
        console.error("Error closing onboarding window:", closeError);
      }
    }
  };

  const handleOnboardingSkip = async () => {
    try {
      // Use backend command to skip onboarding (still marks as completed)
      await invoke("skip_onboarding");
      console.log("Onboarding skipped via backend");

      // Notify main window that onboarding was skipped
      const mainWindow = await Window.getByLabel(WINDOW_LABELS.MAIN);
      if (mainWindow) {
        await mainWindow.emit("onboarding-skipped", {});
      }

      // Close the onboarding window via backend
      await invoke("close_onboarding_window");
    } catch (error) {
      console.error("Error skipping onboarding:", error);
      // Try to close anyway
      try {
        await invoke("close_onboarding_window");
      } catch (closeError) {
        console.error("Error closing onboarding window:", closeError);
      }
    }
  };

  console.log(
    "OnboardingWindow: About to render, permissionsChecked:",
    permissionsChecked
  );

  if (!permissionsChecked) {
    console.log("OnboardingWindow: Rendering loading state");
    return (
      <div className="min-h-screen bg-background flex items-center justify-center">
        <div className="text-center">
          <div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
          <p className="text-muted-foreground">Loading...</p>
        </div>
      </div>
    );
  }

  console.log("OnboardingWindow: Rendering OnboardingFlow component");
  return (
    <OnboardingFlow
      onComplete={handleOnboardingComplete}
      onSkip={handleOnboardingSkip}
      permissionsAlreadyGranted={permissionsGranted}
      isDevelopmentMode={isDevelopmentMode}
    />
  );
}
