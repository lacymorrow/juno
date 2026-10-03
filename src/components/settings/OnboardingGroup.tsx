import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";
import { RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { COMMANDS } from "@/lib/constants.generated";
import { SettingsGroup, SettingsRow } from "./ui";

/** Go through the welcome guide again. */
export function OnboardingGroup() {
  const [restartOnboardingLoading, setRestartOnboardingLoading] =
    useState(false);
  const [onboardingInfo, setOnboardingInfo] = useState<any>(null);

  useEffect(() => {
    invoke(COMMANDS.ONBOARDING_GET_ONBOARDING_INFO)
      .then(setOnboardingInfo)
      .catch((error) => console.error("Failed to load onboarding info:", error));
  }, []);

  const handleRestartOnboarding = async () => {
    if (restartOnboardingLoading) return;

    setRestartOnboardingLoading(true);

    try {
      await invoke(COMMANDS.ONBOARDING_RESTART_ONBOARDING);
      toast.success("Onboarding restarted successfully", {
        description: "The onboarding window has been opened",
      });

      // Refresh onboarding info
      const info = await invoke(COMMANDS.ONBOARDING_GET_ONBOARDING_INFO);
      setOnboardingInfo(info);
    } catch (error) {
      console.error("Failed to restart onboarding:", error);
      toast.error("Failed to restart onboarding", {
        description: error as string,
      });
    } finally {
      setRestartOnboardingLoading(false);
    }
  };

  return (
    <SettingsGroup
      title="Onboarding"
      footer={
        onboardingInfo?.completed_at
          ? `Last completed ${new Date(onboardingInfo.completed_at).toLocaleDateString()}.`
          : undefined
      }
    >
      <SettingsRow
        id="restart-onboarding"
        label="Restart onboarding"
        description={
          onboardingInfo?.is_development_mode
            ? "Go through the welcome guide again. Development mode: onboarding always shows on restart."
            : "Go through the welcome guide and setup process again"
        }
      >
        <Button
          onClick={handleRestartOnboarding}
          disabled={restartOnboardingLoading}
          variant="outline"
          size="sm"
        >
          {restartOnboardingLoading ? (
            <>
              <RotateCcw className="mr-2 h-4 w-4 animate-spin" />
              Restarting…
            </>
          ) : (
            <>
              <RotateCcw className="mr-2 h-4 w-4" />
              Restart
            </>
          )}
        </Button>
      </SettingsRow>
    </SettingsGroup>
  );
}
