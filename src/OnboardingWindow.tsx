import OnboardingFlow from "@/components/onboarding/Onboarding";
import { invoke } from "@tauri-apps/api/core";
import { Window } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { WINDOW_LABELS } from "@/lib/constants.generated";

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

  // Check permissions and onboarding info on mount
  useEffect(() => {
    const checkInitialData = async () => {
      try {
        console.log("OnboardingWindow: Starting permission check...");

        // Check permissions using native APIs - eliminates all password prompts
        const permissionsResult = await invoke<{
          accessibility: { granted: boolean; required: boolean };
          screen_recording: { granted: boolean; required: boolean };
          microphone: { granted: boolean; required: boolean };
          input_monitoring: { granted: boolean; required: boolean };
          all_granted: boolean;
          app_name: string;
        }>("check_permissions_status_native");

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
      // Re-check permissions at completion
      const permissionsResult = await invoke<{
        accessibility: { granted: boolean; required: boolean };
        screen_recording: { granted: boolean; required: boolean };
        microphone: { granted: boolean; required: boolean };
        input_monitoring: { granted: boolean; required: boolean };
        all_granted: boolean;
        app_name: string;
      }>("check_permissions_status_native");

      if (permissionsResult.all_granted) {
        // If backend restart policy requires it, it will handle internally
        try {
          const restartNeeded = await invoke<boolean>(
            "check_restart_needed_after_permissions"
          );
          if (restartNeeded) {
            await invoke("restart_app_after_permissions");
            return;
          }
        } catch (_e) {
          // Fallback: proceed without restart if command unavailable or returns error
        }

        // Mark onboarding as complete and open main window
        await invoke("complete_onboarding");
        await invoke("open_main_window");
        await invoke("close_onboarding_window");

        // Optionally notify main window
        try {
          const mainWindow = await Window.getByLabel(WINDOW_LABELS.MAIN);
          if (mainWindow) {
            await mainWindow.emit("onboarding-complete", {});
          }
        } catch (_err) {
          // ignore
        }
      } else {
        // Not all required granted; keep onboarding open
        // Optionally guide user; nothing to do here
      }
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
