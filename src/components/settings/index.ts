// Export all settings components from a central location
export { default as SettingsWindow } from './SettingsWindow';
export { default as GeneralSettings } from './sections/GeneralSettings';
export { default as VoiceSettings } from './sections/VoiceSettings';
export { default as AIProviderSettings } from './sections/AIProviderSettings';
export { default as NetworkSettings } from './sections/NetworkSettings';
export { default as SecuritySettings } from './sections/SecuritySettings';
export { default as ShortcutsSettings } from './sections/ShortcutsSettings';
export { default as ToolsSettings } from './sections/ToolsSettings';
export { default as AdvancedSettings } from './sections/AdvancedSettings';

export type { SettingsCategory } from './types';