// Notification utility functions

// Helper function to determine notification duration based on level and estimated duration
export const getNotificationDuration = (
  notificationLevel: string,
  estimatedDuration?: string
): number => {
  // Base duration by notification level
  const baseDurations = {
    minimal: 1500,
    standard: 3000,
    detailed: 5000,
  };

  const baseDuration =
    baseDurations[notificationLevel as keyof typeof baseDurations] || 3000;

  // Adjust based on estimated duration
  if (estimatedDuration) {
    const durationMultipliers = {
      instant: 0.5,
      short: 0.8,
      medium: 1.0,
      long: 1.5,
    };
    const multiplier =
      durationMultipliers[
        estimatedDuration as keyof typeof durationMultipliers
      ] || 1.0;
    return Math.round(baseDuration * multiplier);
  }

  return baseDuration;
};

// Helper function to get notification styling based on tool category
export const getNotificationClassName = (
  toolCategory?: string,
  eventType?: string,
  success?: boolean
): string => {
  let className = "tool-notification";

  // Add category-specific styling
  if (toolCategory) {
    className += ` ${toolCategory.toLowerCase()}-category`;
  }

  // Add event type styling
  if (eventType) {
    className += ` ${eventType}-event`;
  }

  // Add success/failure styling for results
  if (eventType === "result" && success !== undefined) {
    className += success ? " success-result" : " failure-result";
  }

  return className;
};