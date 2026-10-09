# Siri parity scorecard

One row per thing a person says. Plan: `docs/plans/siri-parity.md`. A row is green only when it has a real-run recording in `docs/changelog/media/<PR>/` and a first-audio number from the `[TurnTiming]` log. Target for a single-intent tier 1 request: first audio within 1500 ms.

Tiers: 0 is a local intent (no model), 1 is a typed tool the model calls, 2 is computer use.

## Reminders, Calendar, Contacts (slice 1, LAC-4233)

| utterance | Siri on Mac | Juno tier | status | first-audio ms | recording |
| --- | --- | --- | --- | --- | --- |
| remind me to call Katie tomorrow at nine | yes | 1 (`reminders_create`) | built, unrecorded | - | - |
| remind me to pick up the drawings Friday | yes | 1 (`reminders_create`, date only) | built, unrecorded | - | - |
| what are my reminders | yes | 1 (`reminders_list`) | built, unrecorded | - | - |
| what's due today | yes | 1 (`reminders_list`, due window) | built, unrecorded | - | - |
| what's on my grocery list | yes | 1 (`reminders_list`, one list) | built, unrecorded | - | - |
| mark call Katie done | yes | 1 (`reminders_complete`) | built, unrecorded | - | - |
| what's on my calendar this afternoon | yes | 1 (`calendar_events`) | built, unrecorded | - | - |
| what's my next meeting | yes | 1 (`calendar_events`) | built, unrecorded | - | - |
| am I free Thursday morning | yes | 1 (`calendar_events`) | built, unrecorded | - | - |
| add dentist Thursday at three to my calendar | yes | 1 (`calendar_create_event`) | built, unrecorded | - | - |
| add this to my calendar (invite on screen) | yes | 1 (`calendar_create_event` after reading the screen) | built, unrecorded | - | - |
| move my three o'clock to four | yes | 1 (`calendar_events`, `calendar_move_event`) | built, unrecorded | - | - |
| cancel my four o'clock | yes | 1 (`calendar_delete_event`, always asks) | built, unrecorded | - | - |
| what's Doug's number | yes | 1 (`contacts_find`) | built, unrecorded | - | - |
| what's my wife's email | yes | 1 (`contacts_find`, relationship) | built, unrecorded | - | - |

## Later slices

Messages, FaceTime, Mail, Notes (slice 2), Music, Maps, Shortcuts, Focus (slice 3), and the timing pass (slice 4) add their rows when they ship. System commands that tier 0 already answers (volume, dark mode, timers, open an app) are added in slice 4 with their numbers.
