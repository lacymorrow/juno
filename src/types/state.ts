/**
 * Auto-generated TypeScript types from Rust state structures
 * Generated on: 2025-07-17T23:05:56.851Z
 * 
 * DO NOT EDIT MANUALLY - This file is auto-generated
 * Run: npm run generate-types
 */

export enum AgentTriggerMode {
  Tap = "Tap",
  Hold = "Hold",
}

export enum DictationTriggerMode {
  Tap = "Tap",
  Hold = "Hold",
}

export interface KeyboardShortcuts {
  agent_mode_toggle: string;
  dictation_input: string;
  stop_current_task: string;
  open_settings: string;
}

export interface TimestampTracker {
  last_timestamp_shown: number | null;
  events_since_last_timestamp: number;
}

export interface ToolApprovalRequest {
  tool_id: string;
  tool_name: string;
  tool_input: any;
  description: string;
  timestamp: number;
  approved: boolean | null;
}

export interface AudioSettings {
  tts_provider: string;
  dictation_active: boolean;
  dictation_clipboard_enabled: boolean;
  sound_enabled: boolean;
  always_listening_active: boolean;
  always_listening_sensitivity: number;
  always_listening_wake_words: string[];
  notification_sound_enabled: boolean;
}

export interface AgentExecutionState {
  execution_active: boolean;
  execution_id: string | null;
  current_step: number | null;
  max_steps: number | null;
  tool_approval_required: boolean;
}

export interface UISettings {
  bar_ui_state: string;
  performance_monitoring_enabled: boolean;
  debug_mode: boolean;
  notification_type: string;
  notification_duration: number;
  notification_position: string;
  notification_show_icons: boolean;
  notification_persist_important: boolean;
  smooth_mouse_movement: boolean;
}

export interface InputSettings {
  keyboard_shortcuts: KeyboardShortcuts;
  agent_trigger_mode: AgentTriggerMode;
  dictation_trigger_mode: DictationTriggerMode;
}

export interface PermissionStatus {
  permission_type: string;
  granted: boolean;
  required: boolean;
  description: string;
  instructions: string;
}

export interface PermissionsState {
  accessibility: PermissionStatus;
  screen_recording: PermissionStatus;
  microphone: PermissionStatus;
  input_monitoring: PermissionStatus;
  all_granted: boolean;
  app_name: string;
}

/**
 * Simplified AppState interface for frontend use
 * This excludes internal implementation details and async locks
 */
export interface FrontendAppState {
  // Audio Settings
  audioSettings: {
    ttsProvider: string;
    dictationActive: boolean;
    dictationClipboardEnabled: boolean;
    soundEnabled: boolean;
    alwaysListeningActive: boolean;
    alwaysListeningSensitivity: number;
    alwaysListeningWakeWords: string[];
    notificationSoundEnabled: boolean;
  };
  
  // Agent Execution State
  agentExecution: {
    executionActive: boolean;
    executionId: string | null;
    currentStep: number | null;
    maxSteps: number | null;
    toolApprovalRequired: boolean;
  };
  
  // UI Settings
  uiSettings: {
    barUiState: string;
    performanceMonitoringEnabled: boolean;
    debugMode: boolean;
    notificationType: string;
    notificationDuration: number;
    notificationPosition: string;
    notificationShowIcons: boolean;
    notificationPersistImportant: boolean;
    smoothMouseMovement: boolean;
  };
  
  // Input Settings
  inputSettings: {
    keyboardShortcuts: KeyboardShortcuts;
    agentTriggerMode: AgentTriggerMode;
    dictationTriggerMode: DictationTriggerMode;
  };
  
  // Permissions State
  permissionsState: PermissionsState | null;
  permissionsChecked: boolean;
  
  // Cloud State
  cloudEnabled: boolean;
  
  // Pending Tool Approvals
  pendingToolApprovals: ToolApprovalRequest[];
}

// Type Guards
export function isPermissionsState(value: any): value is PermissionsState {
  return value &&
    typeof value === 'object' &&
    'accessibility' in value &&
    'screen_recording' in value &&
    'microphone' in value &&
    'input_monitoring' in value &&
    'all_granted' in value;
}

export function isToolApprovalRequest(value: any): value is ToolApprovalRequest {
  return value &&
    typeof value === 'object' &&
    typeof value.tool_id === 'string' &&
    typeof value.tool_name === 'string' &&
    typeof value.description === 'string' &&
    typeof value.timestamp === 'number';
}

export function isKeyboardShortcuts(value: any): value is KeyboardShortcuts {
  return value &&
    typeof value === 'object' &&
    typeof value.agent_mode_toggle === 'string' &&
    typeof value.dictation_input === 'string' &&
    typeof value.stop_current_task === 'string' &&
    typeof value.open_settings === 'string';
}
