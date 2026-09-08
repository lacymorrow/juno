# Triggers: unified activation

Juno is summoned through **triggers**. A trigger pairs a **method** (how it fires)
with a **target** (what it does). This replaced the older setup where activation
was spread across three settings screens (key combos, agent tap/hold, dictation
tap/hold plus wake words).

Shipped in PR #520.

## The model

A trigger is unique by `(method, target)`, so the set is bounded to six:

|              | Agent                 | Dictation                   |
| ------------ | --------------------- | --------------------------- |
| Push-to-talk | key / mouse, hold     | key / mouse, hold           |
| Toggle       | key / mouse, tap      | key / mouse, tap            |
| Voice        | phrase (e.g. "juno")  | phrase (e.g. "transcribe")  |

- **Push-to-talk**: activate while held, stop on release.
- **Toggle**: press once to start, again to stop.
- **Voice**: speak a wake phrase. Optional "hey" prefix ("juno" or "hey juno";
  require the prefixed form with the toggle).
- **Target** decides what the activation drives: the agent, or dictation
  (speech typed at the cursor). Voice routing is per phrase, so "juno" can wake
  the agent while "transcribe" starts dictation.

A binding is either a **key combo** or a **mouse button**. The Triggers settings
screen starts with one default trigger and a `+` that offers whichever combos are
not yet used.

## Where it lives

Rust owns all activation logic; the frontend is display-only.

| Concern | Location |
| ------- | -------- |
| Data model, migration, validation | `src-tauri/src/triggers/mod.rs` |
| Read/write commands | `src-tauri/src/commands/triggers.rs` (`get_triggers`, `set_triggers`) |
| Persistence + in-memory cache | `settings/` (a `triggers` store key) and `AppState` |
| Keyboard registration + dispatch | `src-tauri/src/commands/shortcuts.rs`, `src-tauri/src/events/shortcuts.rs` |
| Mouse-button observer | `src-tauri/src/platform/mouse_button_monitor.rs` |
| Voice engine + routing | `tauri-plugin-voice-transcription/src/always_listening.rs`, `src-tauri/src/integration.rs` |
| Settings UI | `src/components/settings/sections/TriggersSettings.tsx` |

`triggers` is the source of truth. The legacy fields (`KeyboardShortcuts`,
`AgentSettings.trigger_mode`, the always-listening wake words) are **derived**
from it, so existing runtime consumers keep working without a rewrite.

## How an activation flows

1. **Key** bindings register through the tauri global-shortcut plugin. The
   dispatcher matches the fired combo against the enabled triggers and routes
   each by its own method and target.
2. **Mouse** bindings cannot use that plugin, so a passive `NSEvent` monitor
   (modeled on `stop_key_monitor`, non-consuming) watches the bound buttons and
   routes matches the same way. It needs Accessibility permission to see events
   in other apps, the same permission computer use already requires.
3. **Voice**: the always-listening engine reports which wake phrase matched.
   `integration.rs` looks up that phrase's target and either submits the
   follow-up speech to the agent or types it as dictation.

Keyboard and mouse both go through one routing core,
`events::shortcuts::fire_trigger_edge`, which owns the onboarding and
bar-voice guards so the two input sources behave identically.

## Migration

On first load with no stored triggers, the list is synthesized from the legacy
shortcut, tap/hold, and wake-word settings, so an upgrading user keeps their
setup. A fresh install is seeded with one default trigger, so the app is never
left with no way to be summoned.

## Adding a method or target later

The matrix is intentionally closed. To extend it, add a variant to
`TriggerMethod` or `TriggerTarget` in `triggers/mod.rs`, handle it in
`fire_trigger_edge` (the compiler will point at the non-exhaustive match), and
surface it in `TriggersSettings.tsx`.
