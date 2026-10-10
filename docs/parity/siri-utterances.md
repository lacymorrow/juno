# Siri parity scorecard

One row per thing a person says. Plan: `docs/plans/siri-parity.md`. A row is green only when it has a real-run recording in `docs/changelog/media/<PR>/` and a first-audio number from the `[TurnTiming]` log. Target for a single-intent tier 1 request: first audio within 1500 ms.

Tiers: 0 is a local intent (no model), 1 is a typed tool the model calls, 2 is computer use.

## Reminders, Calendar, Contacts (slice 1, LAC-4233)

| utterance | Siri on Mac | Juno tier | status | first-audio ms | recording |
| --- | --- | --- | --- | --- | --- |
| remind me to call Katie tomorrow at nine | yes | 1 (`reminders_create`) | built, unrecorded | - | - |
| remind me to pick up the drawings Friday | yes | 1 (`reminders_create`, date only) | built, unrecorded | - | - |
| what are my reminders | yes | 0 (local intent `agenda`, once Reminders is allowed), else 1 (`reminders_list`) | built, unrecorded | - | - |
| what's due today | yes | 1 (`reminders_list`, due window) | built, unrecorded | - | - |
| what's on my grocery list | yes | 1 (`reminders_list`, one list) | built, unrecorded | - | - |
| mark call Katie done | yes | 1 (`reminders_complete`) | built, unrecorded | - | - |
| what's on my calendar this afternoon | yes | 1 (`calendar_events`) | built, unrecorded | - | - |
| what's my next meeting | yes | 0 (local intent `agenda`, once Calendar is allowed), else 1 (`calendar_events`) | built, unrecorded | - | - |
| am I free Thursday morning | yes | 1 (`calendar_events`) | built, unrecorded | - | - |
| add dentist Thursday at three to my calendar | yes | 1 (`calendar_create_event`) | built, unrecorded | - | - |
| add this to my calendar (invite on screen) | yes | 1 (`calendar_create_event` after reading the screen) | built, unrecorded | - | - |
| move my three o'clock to four | yes | 1 (`calendar_events`, `calendar_move_event`) | built, unrecorded | - | - |
| cancel my four o'clock | yes | 1 (`calendar_delete_event`, always asks) | built, unrecorded | - | - |
| what's Doug's number | yes | 1 (`contacts_find`) | built, unrecorded | - | - |
| what's my wife's email | yes | 1 (`contacts_find`, relationship) | built, unrecorded | - | - |

## Messages, FaceTime, Mail, Notes (slice 2, LAC-4236)

A send always stops on the message card and goes only on "send it" or the Send button. A bare "yes" never sends.

| utterance | Siri on Mac | Juno tier | status | first-audio ms | recording |
| --- | --- | --- | --- | --- | --- |
| text Doug I'm running late | yes | 1 (`messages_send`, card, then "send it") | built, unrecorded | - | - |
| send it | yes | approval phrase, no model | built, unrecorded | - | - |
| tell my wife I'm on my way | yes | 1 (`messages_send`, relationship) | built, unrecorded | - | - |
| text Doug, actually make it twenty minutes (correction on the card) | yes | 1 (`messages_send` redone) | built, unrecorded | - | - |
| text Sam (two Sams in Contacts) | yes | 1 (`messages_send` returns both, asks which) | built, unrecorded | - | - |
| what did Katie text me | yes | 1 (`messages_recent`, from) | built, unrecorded | - | - |
| read my new messages | yes | 1 (`messages_recent`) | built, unrecorded | - | - |
| what did Katie text me (Full Disk Access off) | yes | 1 (`messages_recent`, opens the setting) | built, unrecorded | - | - |
| FaceTime Mom | yes | 1 (`facetime_call`) | built, unrecorded | - | - |
| FaceTime audio Doug | yes | 1 (`facetime_call`, audio) | built, unrecorded | - | - |
| read me my unread mail | yes | 1 (`mail_unread`) | built, unrecorded | - | - |
| find the email from Katie about the invoice | yes | 1 (`mail_search`, from) | built, unrecorded | - | - |
| any mail from Doug since Monday | yes | 1 (`mail_search`, since) | built, unrecorded | - | - |
| email Doug that the store is live | yes | 1 (`mail_send`, card, then "send it") | built, unrecorded | - | - |
| draft an email to Katie about Thursday | yes | 1 (`mail_draft`, never sends) | built, unrecorded | - | - |
| make a note: pick up the drawings Thursday | yes | 1 (`notes_create`) | built, unrecorded | - | - |
| add milk to my groceries note | yes | 1 (`notes_append`) | built, unrecorded | - | - |
| find my note about the wifi password | yes | 1 (`notes_search`) | built, unrecorded | - | - |

## Music, Maps, Shortcuts, Focus (slice 3, LAC-4243)

| utterance | Siri on Mac | Juno tier | status | first-audio ms | recording |
| --- | --- | --- | --- | --- | --- |
| play Boards of Canada | yes | 1 (`music_play`, artist; the first call asks to control Music) | built, unrecorded | - | - |
| play Roygbiv | yes | 1 (`music_play`, song) | built, unrecorded | - | - |
| play the album Geogaddi | yes | 1 (`music_play`, album) | built, unrecorded | - | - |
| play my workout playlist | yes | 1 (`music_play`, playlist) | built, unrecorded | - | - |
| play Boards of Canada on Spotify | no | 1, answers that Spotify cannot be searched (cut: no scripting search) | built, unrecorded | - | - |
| directions to the airport | yes | 1 (`maps_directions`) | built, unrecorded | - | - |
| walking directions to Starbucks | yes | 1 (`maps_directions`, walking) | built, unrecorded | - | - |
| how do I get to 123 Main Street from the office | yes | 1 (`maps_directions`, with a start) | built, unrecorded | - | - |
| run my morning routine | yes | 1 (`shortcuts_run`, fuzzy name) | built, unrecorded | - | - |
| what shortcuts do I have | yes | 1 (`shortcuts_list`) | built, unrecorded | - | - |
| turn on the porch lights | yes | 1 (`shortcuts_run` on the person's own HomeKit shortcut) | built, unrecorded | - | - |
| turn off the porch lights | yes | 1 (`shortcuts_run`) | built, unrecorded | - | - |
| turn on do not disturb (first time) | yes | 1 (`focus_set`, adds the Juno Focus shortcut with one click) | built, unrecorded | - | - |
| turn on do not disturb | yes | 0 (local intent `agenda`, once the shortcut is added) | built, unrecorded | - | - |
| turn off do not disturb | yes | 0 (local intent `agenda`, once the shortcut is added) | built, unrecorded | - | - |
| what's on my calendar today | yes | 0 (local intent `agenda`, once Calendar is allowed) | built, unrecorded | - | - |
| what's on my calendar tomorrow | yes | 0 (local intent `agenda`) | built, unrecorded | - | - |
| turn the brightness up | yes | cut, tier 2 only (no public API; not promised) | cut | - | - |
| turn on Bluetooth | yes | cut, tier 2 only (no public API; not promised) | cut | - | - |

## Later slices

The timing pass (slice 4) adds its rows when they ship. System commands that tier 0 already answers (volume, dark mode, timers, open an app) are added in slice 4 with their numbers.
