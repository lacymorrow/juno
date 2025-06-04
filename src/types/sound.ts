export interface SoundPlayResult {
    success: boolean;
    message: string;
    file_path?: string;
}

export enum SoundCategory {
    HeroSounds = "HeroSounds",
    AlertsAndNotifications = "AlertsAndNotifications",
    PrimarySystemSounds = "PrimarySystemSounds",
    SecondarySystemSounds = "SecondarySystemSounds",
}

export enum SoundType {
    // Hero sounds for celebrations and major achievements
    HeroSimpleCelebration01 = "HeroSimpleCelebration01",
    HeroSimpleCelebration02 = "HeroSimpleCelebration02",
    HeroSimpleCelebration03 = "HeroSimpleCelebration03",
    HeroDecorativeCelebration01 = "HeroDecorativeCelebration01",
    HeroDecorativeCelebration02 = "HeroDecorativeCelebration02",
    HeroDecorativeCelebration03 = "HeroDecorativeCelebration03",

    // Alert and notification sounds
    AlertSimple = "AlertSimple",
    AlertHighIntensity = "AlertHighIntensity",
    NotificationSimple01 = "NotificationSimple01",
    NotificationSimple02 = "NotificationSimple02",
    NotificationAmbient = "NotificationAmbient",
    NotificationDecorative01 = "NotificationDecorative01",
    NotificationDecorative02 = "NotificationDecorative02",
    NotificationHighIntensity = "NotificationHighIntensity",
    RingtoneMinimal = "RingtoneMinimal",
    AlarmGentle = "AlarmGentle",
}

// Sound system interface for use in React components
export interface SoundSystem {
    // Primary sound playing functions
    playSound: (soundType: SoundType) => Promise<SoundPlayResult>;
    playSoundFile: (filePath: string) => Promise<SoundPlayResult>;

    // Convenience functions for common use cases
    playNotification: () => Promise<SoundPlayResult>;
    playSuccess: () => Promise<SoundPlayResult>;
    playError: () => Promise<SoundPlayResult>;
    playAlert: () => Promise<SoundPlayResult>;

    // Utility functions
    getAvailableSounds: () => Promise<SoundType[]>;
}

// Helper function to get the sound file path for a given sound type
// Note: The actual file extension used by the backend is platform-specific:
// - macOS: .caf (Core Audio Format) for native support
// - Other platforms: .ogg for cross-platform compatibility
export function getSoundFilePath(soundType: SoundType): string {
    const soundPaths: Record<SoundType, string> = {
        [SoundType.HeroSimpleCelebration01]: "01 Hero Sounds/hero_simple-celebration-01",
        [SoundType.HeroSimpleCelebration02]: "01 Hero Sounds/hero_simple-celebration-02",
        [SoundType.HeroSimpleCelebration03]: "01 Hero Sounds/hero_simple-celebration-03",
        [SoundType.HeroDecorativeCelebration01]: "01 Hero Sounds/hero_decorative-celebration-01",
        [SoundType.HeroDecorativeCelebration02]: "01 Hero Sounds/hero_decorative-celebration-02",
        [SoundType.HeroDecorativeCelebration03]: "01 Hero Sounds/hero_decorative-celebration-03",

        [SoundType.AlertSimple]: "02 Alerts and Notifications/alert_simple",
        [SoundType.AlertHighIntensity]: "02 Alerts and Notifications/alert_high-intensity",
        [SoundType.NotificationSimple01]: "02 Alerts and Notifications/notification_simple-01",
        [SoundType.NotificationSimple02]: "02 Alerts and Notifications/notification_simple-02",
        [SoundType.NotificationAmbient]: "02 Alerts and Notifications/notification_ambient",
        [SoundType.NotificationDecorative01]: "02 Alerts and Notifications/notification_decorative-01",
        [SoundType.NotificationDecorative02]: "02 Alerts and Notifications/notification_decorative-02",
        [SoundType.NotificationHighIntensity]: "02 Alerts and Notifications/notification_high-intensity",
        [SoundType.RingtoneMinimal]: "02 Alerts and Notifications/ringtone_minimal",
        [SoundType.AlarmGentle]: "02 Alerts and Notifications/alarm_gentle",
    };

    return soundPaths[soundType];
}

// Helper function to get the category for a sound type
export function getSoundCategory(soundType: SoundType): SoundCategory {
    const heroSounds = [
        SoundType.HeroSimpleCelebration01,
        SoundType.HeroSimpleCelebration02,
        SoundType.HeroSimpleCelebration03,
        SoundType.HeroDecorativeCelebration01,
        SoundType.HeroDecorativeCelebration02,
        SoundType.HeroDecorativeCelebration03,
    ];

    if (heroSounds.includes(soundType)) {
        return SoundCategory.HeroSounds;
    }

    return SoundCategory.AlertsAndNotifications;
}
