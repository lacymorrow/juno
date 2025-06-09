import { PermissionsManager } from "./PermissionsManager";

interface PermissionsFlowProps {
  onComplete?: () => void;
  onSkip?: () => void;
  showSkipOption?: boolean;
  className?: string;
  autoRedirectEnabled?: boolean;
}

export function PermissionsFlow({
  onComplete,
  onSkip,
  showSkipOption = false,
  className = "",
  autoRedirectEnabled = true,
}: PermissionsFlowProps) {
  return (
    <PermissionsManager
      variant="splash"
      showHeader={true}
      showSkipOption={showSkipOption}
      autoRedirectEnabled={autoRedirectEnabled}
      className={className}
      onComplete={onComplete}
      onSkip={onSkip}
    />
  );
}
