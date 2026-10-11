// Export all modular settings components
export { default as GeneralSettings } from './sections/GeneralSettings';
export { default as VoiceSettings } from './sections/VoiceSettings';
export { default as AIProviderSettings } from './sections/AIProviderSettings';
export { default as ModelsSettings } from './sections/ModelsSettings';
export { default as SecuritySettings } from './sections/SecuritySettings';
export { default as AdvancedSettings } from './sections/AdvancedSettings';
export { NotificationsGroup } from './NotificationsGroup';
export { default as NetworkSettings } from './sections/NetworkSettings';
export { default as ToolsSettings } from './sections/ToolsSettings';
export { default as TriggersSettings } from './sections/TriggersSettings';
export { default as AutomationsSettings } from './sections/AutomationsSettings';

// Export shared components
export { ShortcutRecorder } from './ShortcutRecorder';
export { KeyCaps } from './KeyCaps';
export {
  AdvancedSettingsProvider,
  AdvancedOnly,
  useAdvancedSettings,
} from './AdvancedSettingsContext';

// Export types
export * from './types';

// Export modular settings window
export { default as ModularSettingsWindow } from './ModularSettingsWindow';
