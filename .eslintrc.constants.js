/**
 * ESLint rule to prevent hardcoded event and command strings
 * This ensures all event names and command names use the generated constants
 */

module.exports = {
  rules: {
    'no-hardcoded-tauri-strings': {
      meta: {
        type: 'problem',
        docs: {
          description: 'Disallow hardcoded Tauri event and command strings',
          category: 'Best Practices',
          recommended: true,
        },
        fixable: 'code',
        schema: [],
      },
      create(context) {
        // Load known events and commands (this would ideally be dynamic)
        const knownEvents = [
          'agent-event',
          'agent-processing-complete',
          'agent-processing-error',
          'agent-state-changed',
          'agent-tool-call',
          'agent-thought-process',
          'agent-status-update',
          'agent-stop',
          'agent-active',
          'agent-stream-start',
          'agent-stream-end',
          'agent-text-stream',
          'backend-response',
          'tts-audio-ready',
          'tts-stop-requested',
          'dictation-started',
          'dictation-finished',
          'dictation-partial-result',
          'dictation-state-changed',
          'ui-state-update',
          'bar-state-changed',
          'voice-transcription:final-result',
          'voice-transcription:dictation-stopped',
          'voice-transcription:error',
          'voice-transcription:dictation-started',
          'voice-transcription:partial-result',
        ];

        const knownCommands = [
          'get_all_settings',
          'get_settings',
          'update_settings',
          'reset_settings',
          'submit_query',
          'stop_agent',
          'handle_tool_approval',
          'get_available_providers',
          'set_provider',
          'start_dictation',
          'stop_dictation',
          'check_permissions',
          'request_permissions',
          'capture_screenshot',
          'save_screenshot',
          'start_always_listening',
          'stop_always_listening',
          'toggle_always_listening',
          'start_tts',
          'stop_tts',
          'pause_tts',
          'resume_tts',
        ];

        return {
          CallExpression(node) {
            // Check listen() calls
            if (
              node.callee.name === 'listen' &&
              node.arguments[0] &&
              node.arguments[0].type === 'Literal' &&
              typeof node.arguments[0].value === 'string'
            ) {
              const eventName = node.arguments[0].value;
              if (knownEvents.includes(eventName)) {
                context.report({
                  node: node.arguments[0],
                  message: `Use EVENTS constant instead of hardcoded event name "${eventName}"`,
                  fix(fixer) {
                    const constantName = eventName
                      .toUpperCase()
                      .replace(/-/g, '_')
                      .replace(/:/g, '_');
                    return fixer.replaceText(node.arguments[0], `EVENTS.${constantName}`);
                  },
                });
              }
            }

            // Check invoke() calls
            if (
              node.callee.name === 'invoke' &&
              node.arguments[0] &&
              node.arguments[0].type === 'Literal' &&
              typeof node.arguments[0].value === 'string'
            ) {
              const commandName = node.arguments[0].value;
              if (knownCommands.includes(commandName)) {
                context.report({
                  node: node.arguments[0],
                  message: `Use COMMANDS constant instead of hardcoded command name "${commandName}"`,
                  fix(fixer) {
                    const constantName = commandName.toUpperCase();
                    return fixer.replaceText(node.arguments[0], `COMMANDS.${constantName}`);
                  },
                });
              }
            }
          },

          // Check switch case statements
          SwitchCase(node) {
            if (
              node.test &&
              node.test.type === 'Literal' &&
              typeof node.test.value === 'string'
            ) {
              const value = node.test.value;
              if (knownEvents.includes(value)) {
                context.report({
                  node: node.test,
                  message: `Use EVENTS constant instead of hardcoded event name "${value}"`,
                  fix(fixer) {
                    const constantName = value
                      .toUpperCase()
                      .replace(/-/g, '_')
                      .replace(/:/g, '_');
                    return fixer.replaceText(node.test, `EVENTS.${constantName}`);
                  },
                });
              }
            }
          },
        };
      },
    },
  },
};

// Alternative: Plugin format for @typescript-eslint
export const tauriConstantsPlugin = {
  rules: {
    'prefer-constants': {
      meta: {
        type: 'suggestion',
        docs: {
          description: 'Prefer using generated constants over hardcoded strings for Tauri events and commands',
          recommended: true,
        },
        fixable: 'code',
        schema: [],
        messages: {
          useEventConstant: 'Use EVENTS.{{constant}} instead of hardcoded event "{{value}}"',
          useCommandConstant: 'Use COMMANDS.{{constant}} instead of hardcoded command "{{value}}"',
        },
      },
      create(context) {
        return {
          CallExpression(node) {
            const callee = node.callee;
            const firstArg = node.arguments[0];

            if (!firstArg || firstArg.type !== 'Literal' || typeof firstArg.value !== 'string') {
              return;
            }

            // Check listen() calls
            if (callee.type === 'Identifier' && callee.name === 'listen') {
              const eventName = firstArg.value;
              const constantName = eventName.toUpperCase().replace(/-/g, '_').replace(/:/g, '_');
              
              context.report({
                node: firstArg,
                messageId: 'useEventConstant',
                data: {
                  constant: constantName,
                  value: eventName,
                },
                fix(fixer) {
                  return fixer.replaceText(firstArg, `EVENTS.${constantName}`);
                },
              });
            }

            // Check invoke() calls
            if (callee.type === 'Identifier' && callee.name === 'invoke') {
              const commandName = firstArg.value;
              const constantName = commandName.toUpperCase();
              
              context.report({
                node: firstArg,
                messageId: 'useCommandConstant',
                data: {
                  constant: constantName,
                  value: commandName,
                },
                fix(fixer) {
                  return fixer.replaceText(firstArg, `COMMANDS.${constantName}`);
                },
              });
            }
          },
        };
      },
    },
  },
};