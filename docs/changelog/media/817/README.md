# Starters that grow with you (#817)

Empty state after #817: three starters per new conversation, easier first.

| File | State |
|------|-------|
| `starters-empty-state-first-run.png` | 0-4 conversations. Two First and one Regular. |
| `starters-empty-state-second-conversation.png` | The next new conversation, same level, different mix. |
| `starters-empty-state-seasoned.png` | 20+ conversations. Multi-app errands only. |
| `starters-empty-state-bar-360.png` | Seasoned row in the 360px bar pane. |

## How these were captured

The real `ExamplePrompts` component inside the real `ConversationEmptyState`,
rendered at the shipped source for `v0.8.218`, with the Tauri bridge stubbed so
`get_starters` returns the exact rows the shipped Rust `pick()` produced for
that level. Chip text, order and styling are the product's; only the IPC call
is stood in for. Capturing from the installed app instead would mean launching
Juno on the owner's Mac, which speaks a greeting aloud and takes a global
hotkey, so it stays the owner's action to take.
