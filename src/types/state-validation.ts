/**
 * State validation utilities for ensuring type safety between Rust and TypeScript
 */

import {
  FrontendAppState,
  PermissionsState,
  PermissionStatus,
  KeyboardShortcuts,
  AgentTriggerMode,
  DictationTriggerMode,
  ToolApprovalRequest,
  AudioSettings,
  AgentExecutionState,
  UISettings,
  InputSettings
} from './state';

/**
 * Validate that a value is a valid PermissionStatus
 */
export function validatePermissionStatus(value: unknown): value is PermissionStatus {
  if (!value || typeof value !== 'object') return false;
  const status = value as any;
  
  return (
    typeof status.permission_type === 'string' &&
    typeof status.granted === 'boolean' &&
    typeof status.required === 'boolean' &&
    typeof status.description === 'string' &&
    typeof status.instructions === 'string'
  );
}

/**
 * Validate that a value is a valid PermissionsState
 */
export function validatePermissionsState(value: unknown): value is PermissionsState {
  if (!value || typeof value !== 'object') return false;
  const state = value as any;
  
  return (
    validatePermissionStatus(state.accessibility) &&
    validatePermissionStatus(state.screen_recording) &&
    validatePermissionStatus(state.microphone) &&
    validatePermissionStatus(state.input_monitoring) &&
    typeof state.all_granted === 'boolean' &&
    typeof state.app_name === 'string'
  );
}

/**
 * Validate that a value is a valid KeyboardShortcuts
 */
export function validateKeyboardShortcuts(value: unknown): value is KeyboardShortcuts {
  if (!value || typeof value !== 'object') return false;
  const shortcuts = value as any;
  
  return (
    typeof shortcuts.agent_mode_toggle === 'string' &&
    typeof shortcuts.dictation_input === 'string' &&
    typeof shortcuts.stop_current_task === 'string' &&
    typeof shortcuts.open_settings === 'string'
  );
}

/**
 * Validate that a value is a valid AgentTriggerMode
 */
export function validateAgentTriggerMode(value: unknown): value is AgentTriggerMode {
  return value === AgentTriggerMode.Tap || value === AgentTriggerMode.Hold;
}

/**
 * Validate that a value is a valid DictationTriggerMode
 */
export function validateDictationTriggerMode(value: unknown): value is DictationTriggerMode {
  return value === DictationTriggerMode.Tap || value === DictationTriggerMode.Hold;
}

/**
 * Validate that a value is a valid ToolApprovalRequest
 */
export function validateToolApprovalRequest(value: unknown): value is ToolApprovalRequest {
  if (!value || typeof value !== 'object') return false;
  const request = value as any;
  
  return (
    typeof request.tool_id === 'string' &&
    typeof request.tool_name === 'string' &&
    request.tool_input !== undefined &&
    typeof request.description === 'string' &&
    typeof request.timestamp === 'number' &&
    (request.approved === null || typeof request.approved === 'boolean')
  );
}

/**
 * Validate that a value is a valid AudioSettings
 */
export function validateAudioSettings(value: unknown): value is AudioSettings {
  if (!value || typeof value !== 'object') return false;
  const settings = value as any;
  
  return (
    typeof settings.tts_provider === 'string' &&
    typeof settings.dictation_active === 'boolean' &&
    typeof settings.dictation_clipboard_enabled === 'boolean' &&
    typeof settings.sound_enabled === 'boolean' &&
    typeof settings.always_listening_active === 'boolean' &&
    typeof settings.always_listening_sensitivity === 'number' &&
    Array.isArray(settings.always_listening_wake_words) &&
    settings.always_listening_wake_words.every((w: any) => typeof w === 'string') &&
    typeof settings.notification_sound_enabled === 'boolean'
  );
}

/**
 * Validate that a value is a valid AgentExecutionState
 */
export function validateAgentExecutionState(value: unknown): value is AgentExecutionState {
  if (!value || typeof value !== 'object') return false;
  const state = value as any;
  
  return (
    typeof state.execution_active === 'boolean' &&
    (state.execution_id === null || typeof state.execution_id === 'string') &&
    (state.current_step === null || typeof state.current_step === 'number') &&
    (state.max_steps === null || typeof state.max_steps === 'number') &&
    typeof state.tool_approval_required === 'boolean'
  );
}

/**
 * Validate that a value is a valid UISettings
 */
export function validateUISettings(value: unknown): value is UISettings {
  if (!value || typeof value !== 'object') return false;
  const settings = value as any;
  
  return (
    typeof settings.bar_ui_state === 'string' &&
    typeof settings.performance_monitoring_enabled === 'boolean' &&
    typeof settings.debug_mode === 'boolean' &&
    typeof settings.notification_type === 'string' &&
    typeof settings.notification_duration === 'number' &&
    typeof settings.notification_position === 'string' &&
    typeof settings.notification_show_icons === 'boolean' &&
    typeof settings.notification_persist_important === 'boolean' &&
    typeof settings.smooth_mouse_movement === 'boolean'
  );
}

/**
 * Validate that a value is a valid InputSettings
 */
export function validateInputSettings(value: unknown): value is InputSettings {
  if (!value || typeof value !== 'object') return false;
  const settings = value as any;
  
  return (
    validateKeyboardShortcuts(settings.keyboard_shortcuts) &&
    validateAgentTriggerMode(settings.agent_trigger_mode) &&
    validateDictationTriggerMode(settings.dictation_trigger_mode)
  );
}

/**
 * Validate that a value is a valid FrontendAppState
 */
export function validateFrontendAppState(value: unknown): value is FrontendAppState {
  if (!value || typeof value !== 'object') return false;
  const state = value as any;
  
  // Validate nested audio settings
  const audioSettingsValid = state.audioSettings && (
    typeof state.audioSettings.ttsProvider === 'string' &&
    typeof state.audioSettings.dictationActive === 'boolean' &&
    typeof state.audioSettings.dictationClipboardEnabled === 'boolean' &&
    typeof state.audioSettings.soundEnabled === 'boolean' &&
    typeof state.audioSettings.alwaysListeningActive === 'boolean' &&
    typeof state.audioSettings.alwaysListeningSensitivity === 'number' &&
    Array.isArray(state.audioSettings.alwaysListeningWakeWords) &&
    typeof state.audioSettings.notificationSoundEnabled === 'boolean'
  );
  
  // Validate nested agent execution
  const agentExecutionValid = state.agentExecution && (
    typeof state.agentExecution.executionActive === 'boolean' &&
    (state.agentExecution.executionId === null || typeof state.agentExecution.executionId === 'string') &&
    (state.agentExecution.currentStep === null || typeof state.agentExecution.currentStep === 'number') &&
    (state.agentExecution.maxSteps === null || typeof state.agentExecution.maxSteps === 'number') &&
    typeof state.agentExecution.toolApprovalRequired === 'boolean'
  );
  
  // Validate nested UI settings
  const uiSettingsValid = state.uiSettings && validateUISettings(state.uiSettings);
  
  // Validate nested input settings
  const inputSettingsValid = state.inputSettings && validateInputSettings(state.inputSettings);
  
  // Validate permissions state (can be null)
  const permissionsValid = state.permissionsState === null || validatePermissionsState(state.permissionsState);
  
  // Validate pending tool approvals
  const toolApprovalsValid = Array.isArray(state.pendingToolApprovals) &&
    state.pendingToolApprovals.every(validateToolApprovalRequest);
  
  return (
    audioSettingsValid &&
    agentExecutionValid &&
    uiSettingsValid &&
    inputSettingsValid &&
    permissionsValid &&
    typeof state.permissionsChecked === 'boolean' &&
    typeof state.cloudEnabled === 'boolean' &&
    toolApprovalsValid
  );
}

/**
 * Type-safe state update helper
 */
export function updateState<K extends keyof FrontendAppState>(
  state: FrontendAppState,
  key: K,
  value: FrontendAppState[K]
): FrontendAppState {
  return {
    ...state,
    [key]: value
  };
}

/**
 * Deep partial type for partial state updates
 */
export type DeepPartial<T> = {
  [P in keyof T]?: T[P] extends object ? DeepPartial<T[P]> : T[P];
};

/**
 * Type-safe partial state update helper
 */
export function mergeState(
  state: FrontendAppState,
  updates: DeepPartial<FrontendAppState>
): FrontendAppState {
  const result: FrontendAppState = {
    ...state,
    audioSettings: updates.audioSettings ? {
      ...state.audioSettings,
      ...updates.audioSettings,
      alwaysListeningWakeWords: updates.audioSettings.alwaysListeningWakeWords 
        ? updates.audioSettings.alwaysListeningWakeWords.filter((word): word is string => word !== undefined)
        : state.audioSettings.alwaysListeningWakeWords
    } : state.audioSettings,
    agentExecution: updates.agentExecution ? {
      ...state.agentExecution,
      ...updates.agentExecution
    } : state.agentExecution,
    uiSettings: updates.uiSettings ? {
      ...state.uiSettings,
      ...updates.uiSettings
    } : state.uiSettings,
    inputSettings: updates.inputSettings ? {
      ...state.inputSettings,
      ...updates.inputSettings,
      keyboardShortcuts: updates.inputSettings.keyboardShortcuts ? {
        ...state.inputSettings.keyboardShortcuts,
        ...updates.inputSettings.keyboardShortcuts
      } : state.inputSettings.keyboardShortcuts
    } : state.inputSettings,
  };
  
  return result;
}

/**
 * Create an empty default state
 */
export function createDefaultState(): FrontendAppState {
  return {
    audioSettings: {
      ttsProvider: 'system',
      dictationActive: false,
      dictationClipboardEnabled: true,
      soundEnabled: true,
      alwaysListeningActive: false,
      alwaysListeningSensitivity: 0.5,
      alwaysListeningWakeWords: ['Hey Juno', 'Okay Juno'] as string[],
      notificationSoundEnabled: true,
    },
    agentExecution: {
      executionActive: false,
      executionId: null,
      currentStep: null,
      maxSteps: null,
      toolApprovalRequired: false,
    },
    uiSettings: {
      barUiState: 'default',
      performanceMonitoringEnabled: true,
      debugMode: false,
      notificationType: 'system',
      notificationDuration: 5000,
      notificationPosition: 'bottom-right',
      notificationShowIcons: true,
      notificationPersistImportant: true,
      smoothMouseMovement: false,
    },
    inputSettings: {
      keyboardShortcuts: {
        agent_mode_toggle: process.platform === 'darwin' ? 'Option+D' : 'Alt+D',
        dictation_input: process.platform === 'darwin' ? 'Option+Space' : 'Alt+Space',
        stop_current_task: 'Escape',
        open_settings: process.platform === 'darwin' ? 'Cmd+,' : 'Ctrl+,',
      },
      agentTriggerMode: AgentTriggerMode.Tap,
      dictationTriggerMode: DictationTriggerMode.Hold,
    },
    permissionsState: null,
    permissionsChecked: false,
    cloudEnabled: false,
    pendingToolApprovals: [],
  };
}