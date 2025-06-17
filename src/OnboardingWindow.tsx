import OnboardingFlow from "@/components/onboarding/Onboarding";
import { invoke } from "@tauri-apps/api/core";
import { Window } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

export default function OnboardingWindow() {
  const [permissionsGranted, setPermissionsGranted] = useState(false);
  const [permissionsChecked, setPermissionsChecked] = useState(false);

  // Check permissions on mount
  useEffect(() => {
    const checkPermissions = async () => {
      try {
        const permissionsResult = await invoke<{
          accessibility: { granted: boolean; required: boolean };
          screenRecording: { granted: boolean; required: boolean };
          microphone: { granted: boolean; required: boolean };
          allGranted: boolean;
        }>("check_permissions_status");

        setPermissionsGranted(permissionsResult.allGranted);
        setPermissionsChecked(true);
      } catch (error) {
        console.error("Error checking permissions:", error);
        setPermissionsChecked(true);
      }
    };

    checkPermissions();
  }, []);

  const handleOnboardingComplete = async () => {
    try {
      // Use backend command to mark onboarding as completed
      await invoke("complete_onboarding");
      console.log("Onboarding completed via backend");

      // Notify main window of completion
      const mainWindow = await Window.getByLabel("main");
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
      const mainWindow = await Window.getByLabel("main");
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

  if (!permissionsChecked) {
    return (
      <div className="min-h-screen bg-background flex items-center justify-center">
        <div className="text-center">
          <div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
          <p className="text-muted-foreground">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <OnboardingFlow
      onComplete={handleOnboardingComplete}
      onSkip={handleOnboardingSkip}
      permissionsAlreadyGranted={permissionsGranted}
    />
  );
}
