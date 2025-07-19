/**
 * Hook to sync frontend state with Rust backend state
 * Ensures type safety between Rust and TypeScript
 */

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { safeUnlistenAll } from '@/lib/tauri-event-utils';
import type { 
  FrontendAppState, 
  PermissionsState,
  ToolApprovalRequest,
  AgentTriggerMode,
  DictationTriggerMode,
  KeyboardShortcuts
} from '../types/state';
import { 
  createDefaultState, 
  validateFrontendAppState,
  mergeState,
  type DeepPartial
} from '../types/state-validation';

/**
 * Commands that interact with Rust state
 */
const StateCommands = {
  // Audio settings
  getTtsProvider: 'get_tts_provider',
  setTtsProvider: 'set_tts_provider',
  getDictationActive: 'get_dictation_active',
  setDictationActive: 'set_dictation_active',
  getDictationClipboardEnabled: 'get_dictation_clipboard_enabled',
  setDictationClipboardEnabled: 'set_dictation_clipboard_enabled',
  getSoundEnabled: 'get_sound_enabled',
  setSoundEnabled: 'set_sound_enabled',
  getAlwaysListeningActive: 'get_always_listening_active',
  setAlwaysListeningActive: 'set_always_listening_active',
  getAlwaysListeningSensitivity: 'get_always_listening_sensitivity',
  setAlwaysListeningSensitivity: 'set_always_listening_sensitivity',
  getAlwaysListeningWakeWords: 'get_always_listening_wake_words',
  setAlwaysListeningWakeWords: 'set_always_listening_wake_words',
  getNotificationSoundEnabled: 'get_notification_sound_enabled',
  setNotificationSoundEnabled: 'set_notification_sound_enabled',
  
  // UI settings
  getBarUiState: 'get_bar_ui_state',
  setBarUiState: 'set_bar_ui_state',
  getPerformanceMonitoringEnabled: 'get_performance_monitoring_enabled',
  setPerformanceMonitoringEnabled: 'set_performance_monitoring_enabled',
  getDebugMode: 'get_debug_mode',
  setDebugMode: 'set_debug_mode',
  getNotificationType: 'get_notification_type',
  setNotificationType: 'set_notification_type',
  getNotificationDuration: 'get_notification_duration',
  setNotificationDuration: 'set_notification_duration',
  getNotificationPosition: 'get_notification_position',
  setNotificationPosition: 'set_notification_position',
  getNotificationShowIcons: 'get_notification_show_icons',
  setNotificationShowIcons: 'set_notification_show_icons',
  getNotificationPersistImportant: 'get_notification_persist_important',
  setNotificationPersistImportant: 'set_notification_persist_important',
  getSmoothMouseMovement: 'get_smooth_mouse_movement',
  setSmoothMouseMovement: 'set_smooth_mouse_movement',
  
  // Input settings
  getKeyboardShortcuts: 'get_keyboard_shortcuts',
  setKeyboardShortcuts: 'set_keyboard_shortcuts',
  getAgentTriggerMode: 'get_agent_trigger_mode',
  setAgentTriggerMode: 'set_agent_trigger_mode',
  getDictationTriggerMode: 'get_dictation_trigger_mode',
  setDictationTriggerMode: 'set_dictation_trigger_mode',
  
  // Agent execution
  isAgentExecuting: 'is_agent_executing',
  getCurrentAgentExecutionId: 'get_current_agent_execution_id',
  getAgentCurrentStep: 'get_agent_current_step',
  getAgentMaxSteps: 'get_agent_max_steps',
  isToolApprovalRequired: 'is_tool_approval_required',
  setToolApprovalRequired: 'set_tool_approval_required',
  
  // Permissions
  getPermissionsState: 'get_permissions_state',
  arePermissionsChecked: 'are_permissions_checked',
  
  // Cloud
  isCloudEnabled: 'is_cloud_enabled',
  
  // Tool approvals
  getPendingToolApprovals: 'get_pending_tool_approvals',
  approveToolUse: 'approve_tool_use',
  denyToolUse: 'deny_tool_use',
} as const;

/**
 * Events emitted by the backend
 */
const StateEvents = {
  stateChanged: 'state:changed',
  agentExecutionStarted: 'agent:execution_started',
  agentExecutionFinished: 'agent:execution_finished',
  agentStepUpdate: 'agent:step_update',
  dictationStateChanged: 'dictation:state_changed',
  permissionsUpdated: 'permissions:updated',
  toolApprovalRequested: 'tool:approval_requested',
  toolApprovalResolved: 'tool:approval_resolved',
} as const;

export function useAppStateSync() {
  const [state, setState] = useState<FrontendAppState>(createDefaultState());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  /**
   * Load initial state from backend
   */
  const loadInitialState = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      // Load all state values in parallel
      const [
        ttsProvider,
        dictationActive,
        dictationClipboardEnabled,
        soundEnabled,
        alwaysListeningActive,
        alwaysListeningSensitivity,
        alwaysListeningWakeWords,
        notificationSoundEnabled,
        barUiState,
        performanceMonitoringEnabled,
        debugMode,
        notificationType,
        notificationDuration,
        notificationPosition,
        notificationShowIcons,
        notificationPersistImportant,
        smoothMouseMovement,
        keyboardShortcuts,
        agentTriggerMode,
        dictationTriggerMode,
        executionActive,
        executionId,
        currentStep,
        maxSteps,
        toolApprovalRequired,
        permissionsState,
        permissionsChecked,
        cloudEnabled,
        pendingToolApprovals,
      ] = await Promise.all([
        invoke<string>(StateCommands.getTtsProvider),
        invoke<boolean>(StateCommands.getDictationActive),
        invoke<boolean>(StateCommands.getDictationClipboardEnabled),
        invoke<boolean>(StateCommands.getSoundEnabled),
        invoke<boolean>(StateCommands.getAlwaysListeningActive),
        invoke<number>(StateCommands.getAlwaysListeningSensitivity),
        invoke<string[]>(StateCommands.getAlwaysListeningWakeWords),
        invoke<boolean>(StateCommands.getNotificationSoundEnabled),
        invoke<string>(StateCommands.getBarUiState),
        invoke<boolean>(StateCommands.getPerformanceMonitoringEnabled),
        invoke<boolean>(StateCommands.getDebugMode),
        invoke<string>(StateCommands.getNotificationType),
        invoke<number>(StateCommands.getNotificationDuration),
        invoke<string>(StateCommands.getNotificationPosition),
        invoke<boolean>(StateCommands.getNotificationShowIcons),
        invoke<boolean>(StateCommands.getNotificationPersistImportant),
        invoke<boolean>(StateCommands.getSmoothMouseMovement),
        invoke<KeyboardShortcuts>(StateCommands.getKeyboardShortcuts),
        invoke<AgentTriggerMode>(StateCommands.getAgentTriggerMode),
        invoke<DictationTriggerMode>(StateCommands.getDictationTriggerMode),
        invoke<boolean>(StateCommands.isAgentExecuting),
        invoke<string | null>(StateCommands.getCurrentAgentExecutionId),
        invoke<number | null>(StateCommands.getAgentCurrentStep),
        invoke<number | null>(StateCommands.getAgentMaxSteps),
        invoke<boolean>(StateCommands.isToolApprovalRequired),
        invoke<PermissionsState | null>(StateCommands.getPermissionsState),
        invoke<boolean>(StateCommands.arePermissionsChecked),
        invoke<boolean>(StateCommands.isCloudEnabled),
        invoke<ToolApprovalRequest[]>(StateCommands.getPendingToolApprovals),
      ]);

      const loadedState: FrontendAppState = {
        audioSettings: {
          ttsProvider,
          dictationActive,
          dictationClipboardEnabled,
          soundEnabled,
          alwaysListeningActive,
          alwaysListeningSensitivity,
          alwaysListeningWakeWords,
          notificationSoundEnabled,
        },
        agentExecution: {
          executionActive,
          executionId,
          currentStep,
          maxSteps,
          toolApprovalRequired,
        },
        uiSettings: {
          barUiState,
          performanceMonitoringEnabled,
          debugMode,
          notificationType,
          notificationDuration,
          notificationPosition,
          notificationShowIcons,
          notificationPersistImportant,
          smoothMouseMovement,
        },
        inputSettings: {
          keyboardShortcuts,
          agentTriggerMode,
          dictationTriggerMode,
        },
        permissionsState,
        permissionsChecked,
        cloudEnabled,
        pendingToolApprovals,
      };

      // Validate the loaded state
      if (!validateFrontendAppState(loadedState)) {
        throw new Error('Invalid state loaded from backend');
      }

      setState(loadedState);
    } catch (err) {
      console.error('Failed to load initial state:', err);
      setError(err instanceof Error ? err.message : 'Failed to load state');
    } finally {
      setLoading(false);
    }
  }, []);

  /**
   * Update state in both frontend and backend
   */
  const updateState = useCallback(async (updates: DeepPartial<FrontendAppState>) => {
    // Save current state for rollback
    const previousState = state;
    
    try {
      // Update frontend state optimistically
      setState(prev => mergeState(prev, updates));

      // Update backend state
      const updatePromises: Promise<void>[] = [];

      // Audio settings updates
      if (updates.audioSettings) {
        const audio = updates.audioSettings;
        if (audio.ttsProvider !== undefined) {
          updatePromises.push(invoke(StateCommands.setTtsProvider, { provider: audio.ttsProvider }));
        }
        if (audio.dictationActive !== undefined) {
          updatePromises.push(invoke(StateCommands.setDictationActive, { active: audio.dictationActive }));
        }
        if (audio.dictationClipboardEnabled !== undefined) {
          updatePromises.push(invoke(StateCommands.setDictationClipboardEnabled, { enabled: audio.dictationClipboardEnabled }));
        }
        if (audio.soundEnabled !== undefined) {
          updatePromises.push(invoke(StateCommands.setSoundEnabled, { enabled: audio.soundEnabled }));
        }
        if (audio.alwaysListeningActive !== undefined) {
          updatePromises.push(invoke(StateCommands.setAlwaysListeningActive, { active: audio.alwaysListeningActive }));
        }
        if (audio.alwaysListeningSensitivity !== undefined) {
          updatePromises.push(invoke(StateCommands.setAlwaysListeningSensitivity, { sensitivity: audio.alwaysListeningSensitivity }));
        }
        if (audio.alwaysListeningWakeWords !== undefined) {
          updatePromises.push(invoke(StateCommands.setAlwaysListeningWakeWords, { wakeWords: audio.alwaysListeningWakeWords }));
        }
        if (audio.notificationSoundEnabled !== undefined) {
          updatePromises.push(invoke(StateCommands.setNotificationSoundEnabled, { enabled: audio.notificationSoundEnabled }));
        }
      }

      // UI settings updates
      if (updates.uiSettings) {
        const ui = updates.uiSettings;
        if (ui.barUiState !== undefined) {
          updatePromises.push(invoke(StateCommands.setBarUiState, { state: ui.barUiState }));
        }
        if (ui.performanceMonitoringEnabled !== undefined) {
          updatePromises.push(invoke(StateCommands.setPerformanceMonitoringEnabled, { enabled: ui.performanceMonitoringEnabled }));
        }
        if (ui.debugMode !== undefined) {
          updatePromises.push(invoke(StateCommands.setDebugMode, { enabled: ui.debugMode }));
        }
        if (ui.notificationType !== undefined) {
          updatePromises.push(invoke(StateCommands.setNotificationType, { notificationType: ui.notificationType }));
        }
        if (ui.notificationDuration !== undefined) {
          updatePromises.push(invoke(StateCommands.setNotificationDuration, { duration: ui.notificationDuration }));
        }
        if (ui.notificationPosition !== undefined) {
          updatePromises.push(invoke(StateCommands.setNotificationPosition, { position: ui.notificationPosition }));
        }
        if (ui.notificationShowIcons !== undefined) {
          updatePromises.push(invoke(StateCommands.setNotificationShowIcons, { showIcons: ui.notificationShowIcons }));
        }
        if (ui.notificationPersistImportant !== undefined) {
          updatePromises.push(invoke(StateCommands.setNotificationPersistImportant, { persist: ui.notificationPersistImportant }));
        }
        if (ui.smoothMouseMovement !== undefined) {
          updatePromises.push(invoke(StateCommands.setSmoothMouseMovement, { enabled: ui.smoothMouseMovement }));
        }
      }

      // Input settings updates
      if (updates.inputSettings) {
        const input = updates.inputSettings;
        if (input.keyboardShortcuts !== undefined) {
          updatePromises.push(invoke(StateCommands.setKeyboardShortcuts, { shortcuts: input.keyboardShortcuts }));
        }
        if (input.agentTriggerMode !== undefined) {
          updatePromises.push(invoke(StateCommands.setAgentTriggerMode, { mode: input.agentTriggerMode }));
        }
        if (input.dictationTriggerMode !== undefined) {
          updatePromises.push(invoke(StateCommands.setDictationTriggerMode, { mode: input.dictationTriggerMode }));
        }
      }

      // Agent execution updates
      if (updates.agentExecution?.toolApprovalRequired !== undefined) {
        updatePromises.push(invoke(StateCommands.setToolApprovalRequired, { required: updates.agentExecution.toolApprovalRequired }));
      }

      await Promise.all(updatePromises);
    } catch (err) {
      console.error('Failed to update state:', err);
      setError(err instanceof Error ? err.message : 'Failed to update state');
      
      // Rollback optimistic update
      setState(previousState);
      
      // Reload state to ensure consistency
      await loadInitialState();
    }
  }, [loadInitialState, state]);

  /**
   * Approve a tool use request
   */
  const approveToolUse = useCallback(async (toolId: string) => {
    try {
      await invoke(StateCommands.approveToolUse, { toolId });
      // Reload pending approvals
      const pendingToolApprovals = await invoke<ToolApprovalRequest[]>(StateCommands.getPendingToolApprovals);
      setState(prev => ({ ...prev, pendingToolApprovals }));
    } catch (err) {
      console.error('Failed to approve tool use:', err);
      setError(err instanceof Error ? err.message : 'Failed to approve tool use');
    }
  }, []);

  /**
   * Deny a tool use request
   */
  const denyToolUse = useCallback(async (toolId: string) => {
    try {
      await invoke(StateCommands.denyToolUse, { toolId });
      // Reload pending approvals
      const pendingToolApprovals = await invoke<ToolApprovalRequest[]>(StateCommands.getPendingToolApprovals);
      setState(prev => ({ ...prev, pendingToolApprovals }));
    } catch (err) {
      console.error('Failed to deny tool use:', err);
      setError(err instanceof Error ? err.message : 'Failed to deny tool use');
    }
  }, []);

  // Set up event listeners
  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    // Listen for state changes
    const setupListeners = async () => {
      unlisteners.push(
        await listen(StateEvents.agentExecutionStarted, (event) => {
          const { session_id, max_iterations } = event.payload as any;
          setState(prev => mergeState(prev, {
            agentExecution: {
              executionActive: true,
              executionId: session_id,
              currentStep: 1,
              maxSteps: max_iterations,
            }
          }));
        })
      );

      unlisteners.push(
        await listen(StateEvents.agentExecutionFinished, () => {
          setState(prev => mergeState(prev, {
            agentExecution: {
              executionActive: false,
              executionId: null,
              currentStep: null,
              maxSteps: null,
            }
          }));
        })
      );

      unlisteners.push(
        await listen(StateEvents.agentStepUpdate, (event) => {
          const { step } = event.payload as any;
          setState(prev => mergeState(prev, {
            agentExecution: {
              currentStep: step,
            }
          }));
        })
      );

      unlisteners.push(
        await listen(StateEvents.dictationStateChanged, (event) => {
          const { active } = event.payload as any;
          setState(prev => mergeState(prev, {
            audioSettings: {
              dictationActive: active,
            }
          }));
        })
      );

      unlisteners.push(
        await listen(StateEvents.permissionsUpdated, (event) => {
          const permissionsState = event.payload as PermissionsState;
          setState(prev => ({ ...prev, permissionsState }));
        })
      );

      unlisteners.push(
        await listen(StateEvents.toolApprovalRequested, (event) => {
          const request = event.payload as ToolApprovalRequest;
          setState(prev => ({
            ...prev,
            pendingToolApprovals: [...prev.pendingToolApprovals, request],
          }));
        })
      );

      unlisteners.push(
        await listen(StateEvents.toolApprovalResolved, (event) => {
          const { tool_id } = event.payload as any;
          setState(prev => ({
            ...prev,
            pendingToolApprovals: prev.pendingToolApprovals.filter(r => r.tool_id !== tool_id),
          }));
        })
      );
    };

    setupListeners();

    return () => {
      safeUnlistenAll(unlisteners);
    };
  }, []);

  // Load initial state on mount
  useEffect(() => {
    loadInitialState();
  }, [loadInitialState]);

  return {
    state,
    loading,
    error,
    updateState,
    approveToolUse,
    denyToolUse,
    reload: loadInitialState,
  };
}