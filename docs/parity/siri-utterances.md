# Siri parity scorecard

One row per thing a person says. Plan: `docs/plans/siri-parity.md`. A row is green only when it has a real-run recording in `docs/changelog/media/<PR>/` and a first-audio number from the `[TurnTiming]` log. Target for a single-intent tier 1 request: first audio within 1500 ms.

Tiers: 0 is a local intent (no model), 1 is a typed tool the model calls, 2 is computer use.

## Walking the list

`scripts/juno-build.sh` installs the branch build. Hold Fn+Control, say the row, and grant whatever macOS asks for in the moment: the ask is part of the row.

- First audio: `grep TurnTiming ~/Library/Logs/Juno/juno-$(date +%F).log | tail -1`, the `first_audio=` field, milliseconds from key release.
- A row that misses 1500 ms: keep the whole line, not just the number. The gaps between the fields say which lever the row needs, and they have different owners:

  | gap | what it is | lever |
  | --- | --- | --- |
  | release → `llm_request_sent` | speech-to-text and dispatch | STT |
  | `llm_request_sent` → `llm_first_token` | the model round trip | plan lever 1, the small-turn router |
  | `llm_first_token` → `tts_open` | the model wrote something before it opened `<TTS>` | the prompt |
  | `tts_open` → `first_tts_text` | the parser waiting for a sentence it may release (`MIN_SPOKEN_CHUNK_CHARS`) | the chunking rule |
  | `first_tts_text` → `first_audio` | the speech engine | the TTS engine, and which engine is set decides whether this gap is 1% or all of it (see below) |

  A row that misses only because of the last two gaps does not need the speed slice at all, so recording them is what keeps the plan's "only if the number misses" from over-firing.
- Recording: one clip per row in `docs/changelog/media/<PR>/`, named after the row.
- The Messages read rows need Full Disk Access. Without it the row should say so in one sentence, not come back empty; that is itself a row worth recording.

What the Mac is needed for is the permission dialogs, the timings and the recordings. Everything checkable without it was checked in #814: the Music search areas and every Mail, Notes and Messages term against the installed dictionaries, the `attributedBody` decoder against archives macOS wrote, and the Juno Focus shortcut's signature and bundling.

## Where the time goes today

Measured from `~/Library/Logs/Juno/juno-*.log`, 12 to 18 October 2026: 61 `[TurnTiming]` turns, 49 reached first audio, 38 of those by voice. Every one ran sonnet through `claude_cli/persistent-*` with system TTS.

| | voice (n=38) |
| --- | --- |
| turns under the 1500 ms target | 1 |
| median first audio | 3447 ms |
| best turn logged | 1492 ms |

Median of each gap, voice turns (gaps are measured per turn, then taken as a median, so they do not sum to the median total):

| gap | median | share of a 3447 ms turn |
| --- | --- | --- |
| to `llm_first_token` | 2219 ms | 64% |
| `llm_first_token` → `first_tts_text` | 695 ms | 20% |
| `first_tts_text` → `first_audio` | 47 ms | 1% |

Two things this does and does not say. The model round trip is most of the budget and has never gone under 1236 ms by voice in any logged turn, which is already 82% of the target on its own. But these are organic turns with real conversation context, not the short single-intent utterances below, so they are an upper bound and not a measurement of these rows. That is what the walkthrough is for.

The 695 ms middle gap is the reason `tts_open` exists: until a row is recorded with it, that fifth of the budget cannot be attributed to either the prompt or the chunking rule.

### The 47 ms above is system TTS only. Read the engine before you read the row.

Kokoro became the default in #783, and the table above predates it. On the same app in the same week, with the engine as the only thing that changed:

| engine | n | min | p10 | median | p90 | max |
| --- | --- | --- | --- | --- | --- | --- |
| system | 41 | 2 | 37 | **58** | 85 | 175 |
| kokoro | 50 | 1271 | 1611 | **2238** | 4664 | 7260 |

**47 of those 50 Kokoro turns spend the entire 1500 ms budget on synthesis alone**, before the model, the router or tier 0 contribute anything. The cause is that `tts/kokoro.rs` generates a whole chunk to a complete WAV before a sample plays, at a real-time factor near 1.0, so **the wait before the first word is roughly the spoken length of the first sentence.** A row whose answer opens with a full sentence cannot pass on Kokoro however fast the model is.

So, when a row misses:

- Note the engine. `grep 'Kokoro] Synthesized' <log>` gives the device, the model's own sample rate, the elapsed time and the RTF per synthesis; `Model loaded in Nms on Metal(0)/F32` gives the device the preference resolved to. Both were added for this.
- A Kokoro row that misses on the last gap alone is LAC-5009, not a plan lever. Pulling the small-turn router or tier 0 for it optimises a term that is not the binding one.
- The levers and their real costs are on LAC-5009. Only the first chunk is exposed: `drain_speech_queue` already renders chunk N+1 while chunk N plays, so the cost is paid once per turn rather than once per sentence.
- To measure the rows against the plan's own target rather than against the engine, record the row on system TTS as well. Two numbers on one row is cheaper than a second walkthrough.

## Reminders, Calendar, Contacts (slice 1, LAC-4233)

| utterance | Siri on Mac | Juno tier | status | first-audio ms | recording |
| --- | --- | --- | --- | --- | --- |
| remind me to call Katie tomorrow at nine | yes | 1 (`reminders_create`) | built, unrecorded | - | - |
| remind me to call Katie (Reminders not yet allowed) | yes | 1 (`reminders_create` behind the permission preflight: "I need Reminders for that." + Continue, then the system dialog) | built, unrecorded | - | - |
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
| what did Katie text me (Full Disk Access off) | yes | 1 (`messages_recent` blocked by the permission preflight: one sentence + the button that opens the Full Disk Access pane) | built, unrecorded | - | - |
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
