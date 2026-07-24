// Export all modular settings components
export { default as GeneralSettings } from './sections/GeneralSettings';
export { default as VoiceSettings } from './sections/VoiceSettings';
export { default as AIProviderSettings } from './sections/AIProviderSettings';
export { default as SecuritySettings } from './sections/SecuritySettings';
export { default as AdvancedSettings } from './sections/AdvancedSettings';
export { default as NotificationSettings } from './sections/NotificationSettings';
export { default as NetworkSettings } from './sections/NetworkSettings';
export { default as ShortcutsSettings } from './sections/ShortcutsSettings';
export { default as ToolsSettings } from './sections/ToolsSettings';
export { default as AutomationsSettings } from './sections/AutomationsSettings';

// Export shared components
export { default as ShortcutInput } from './ShortcutInput';

// Export types
export * from './types';

// Export modular settings window
export { default as ModularSettingsWindow } from './ModularSettingsWindow';
