# Siri parity on the Mac

Status: plan, 2026-10-08. DRI: Lacy (decisions), one agent per slice (build). Baseline machine: macOS 26.6.2 Tahoe. The bar is the macOS 27 Siri, not the Tahoe one.

**One sentence: give the agent typed, native tools for the seven apps the Mac already syncs (Reminders, Calendar, Contacts, Messages, Mail, Notes, Music), add "send" as a consequence class so a text or an email always stops and asks, and ship it in four slices, each one demoable by voice.**

Parity is the floor. Juno already beats Siri at everything Siri cannot touch: any app, any file, compound requests, no daily cap. What Juno cannot do today is the ordinary stuff a person expects from a Mac assistant on day one. This plan closes that gap and nothing else.

## The first ten seconds

Person holds Fn+Control and says "remind me to call Katie tomorrow at nine." Juno says "Tomorrow at 9. Call Katie." and shows the reminder as a card. Nothing was set up. The first time, macOS asked once for Reminders access and Juno's answer was the ask itself.

Second request: "text Doug I'm running ten minutes late." Juno shows the message with Doug's name on it and says "To Doug: I'm running ten minutes late. Say send it." Person says "send it." Juno says "Sent." That is the whole demo. Everything below exists to make those two exchanges true, fast, and true again on the fiftieth try.

## Where Siri on the Mac stands (checked 2026-10-08)

- Apple shipped the Siri overhaul with macOS 27 on 2026-09-14 as an English-only beta: M1 or later, daily server-side usage limits, no China. Personal context searches Messages, Mail, Photos and Notes. On-screen awareness shipped. Third-party in-app actions shipped through App Intents, and only for apps that opted in.
- Reviews (MacStories, October 2026) put it strongest in Calendar, Notes, Reminders, Mail, Messages, Maps and Photos. Third-party apps are inconsistent: Discord opens but cannot send. It cannot rename, move or duplicate files. It got "my latest download" wrong and miscounted a music library.
- Siri on the Mac cannot be extended by the person. No CLI, no AppleScript, no shortcuts beyond running the ones that exist.

So the parity list is short and concrete: the Apple apps, plus system commands, plus Shortcuts. Juno already has the rest.

## What Juno has today

Three layers exist and the plan keeps them:

| Layer | Where | What it covers now |
| --- | --- | --- |
| Tier 0, local intents (no model, under a second) | `~/repo/juno/src-tauri/src/agent/local_intents/` | play/pause/next/now playing, volume, dark mode, lock, sleep, battery, time, timers, open/quit app, open a site |
| Tier 1, typed tools the agent calls | `~/repo/juno/src-tauri/src/agent/tools/` | desktop, browser, Safari, shell, files, timers, schedules, MCP |
| Tier 2, computer use | `anthropic_computer_use.rs`, `accessibility_tools.rs` | AX-first control of any app (#748) |

Nothing typed exists for Reminders, Calendar, Contacts, Messages, Mail, Notes, Music search, Maps, Focus or Shortcuts. The system prompt tells the model to write AppleScript for these on the fly (`agent/prompts/templates.rs`, "TIER 0"), which is slow, wrong often enough to notice, and classed `High` risk as a blanket, so a harmless "what's on my calendar" and a "send this email" get the same treatment. Composio (LAC-4210) brings Gmail and Google Calendar through the cloud, for accounts that are not on the Mac. It does not touch the Apple apps and should not.

## Gap table

| Domain | Siri on Mac | Juno today | Juno path | Permission (asked at first use, never before) |
| --- | --- | --- | --- | --- |
| Reminders | create, list, complete | model-written AppleScript | EventKit via `objc2-event-kit` 0.3.2 | Reminders (TCC) |
| Calendar | create, show, move | same | EventKit | Calendars (TCC) |
| Contacts | resolve names, relationships | nothing | Contacts via `objc2-contacts` 0.3.2 | Contacts (TCC) |
| Messages send | send with confirm | model-written AppleScript, no send gate | constant AppleScript, `send` is in the Tahoe dictionary (verified) | Automation: Messages |
| Messages read | "what did Sam text me" | nothing | `~/Library/Messages/chat.db` read-only | Full Disk Access |
| FaceTime | call | nothing | `facetime://` and `tel:` URL | none |
| Mail | read unread, search, send | model-written AppleScript | constant AppleScript | Automation: Mail |
| Notes | create, append, find | same | constant AppleScript | Automation: Notes |
| Music | play by name | transport only | Music AppleScript search then play | Automation: Music |
| Maps | directions | nothing | `maps://` URL | none |
| Shortcuts | run by name | nothing | `shortcuts run` | none for the CLI |
| Focus / DND, alarms, HomeKit | yes | cut (no public API) | a signed Juno shortcut imported once, then `shortcuts run` | one click in Shortcuts |
| Wi-Fi | on/off | agent can via `networksetup` | keep in tier 1 | none |
| Brightness, Bluetooth | yes | cut | tier 2 drives System Settings; not promised | Accessibility |
| Timers | yes | tier 0, lost on quit | keep | none |
| Open app, files, dark mode, lock, sleep, volume | yes | tier 0 | keep | none |
| Knowledge, weather, web | yes, with daily caps | the model | keep; measure before building anything | none |
| On-screen awareness | "add this to my calendar" | screenshot + AX text exist | the prompt names the new tools as targets | Screen Recording |

Verified on this Mac: Clock.app has no scripting dictionary, so alarms are Shortcuts only. `shortcuts sign` exists, so a Juno-made shortcut can be shipped and imported with one click. Tahoe gotchas from the research: Music's "current track" errors on streaming catalog tracks; Mail message ids came back wrapped in angle brackets in 26.1 (fixed 26.2); JXA `run()` broke in a 26.2 beta, so everything stays AppleScript. MediaRemote is private API and stays out.

## Design

### Three tiers, one vocabulary

Tier 0 stays bounded. A local intent fires only when the whole utterance matches and every slot is a bounded type (integer, duration, installed app name). "Remind me to call Katie tomorrow at nine" has free text in it, so it is tier 1 by design. The tier-0 additions this plan allows are exactly these, because each has no free slot: "what's on my calendar today / tomorrow", "read my reminders", "what's my next meeting", "turn on do not disturb" once the shortcut exists.

Tier 1 is where parity lives. The model fills the slots and calls a typed tool; Rust does the work. **The model never writes AppleScript for an app that has a typed tool.** Scripts are constants, variable input goes in as `argv` (same rule as local intents). `run_applescript` stays for the long tail.

Tier 2 is the fallback the system prompt names when a typed tool does not exist or a permission was declined. It is never silent: Juno says what it is about to do in one line ("I'll do it through the Messages window.").

### The toolset: `mac_apps`

One new `ToolCategory::MacApps` in `tool_config.rs`, on by default, with these tools and nothing more. Names are nouns first so the model's tool list reads like a table of contents.

| Tool | Inputs | Consequence |
| --- | --- | --- |
| `reminders_list` | list, due window | read |
| `reminders_create` | title, due, list, notes | write, reversible |
| `reminders_complete` | id | write, reversible |
| `calendar_events` | from, to | read |
| `calendar_create_event` | title, start, end, location, invitees | write, reversible |
| `calendar_move_event` | id, new start | write, reversible |
| `calendar_delete_event` | id | destructive, asks |
| `contacts_find` | name or relationship | read |
| `messages_send` | to, body | **send, always asks** |
| `messages_recent` | from, limit | read, needs Full Disk Access |
| `facetime_call` | contact | acts at once (Siri does the same) |
| `mail_unread` | limit | read |
| `mail_search` | query, from, since | read |
| `mail_send` | to, subject, body | **send, always asks** |
| `mail_draft` | to, subject, body | write, free (drafts are free, #648) |
| `notes_create` | title, body, folder | write, reversible |
| `notes_append` | note, text | write, reversible |
| `notes_search` | query | read |
| `music_play` | query, kind (song, album, artist, playlist) | acts at once |
| `maps_directions` | to, from, mode | opens Maps |
| `shortcuts_list` | none | read |
| `shortcuts_run` | name, input | acts at once; fuzzy name match, exact run |

Every write reads back and reports what it observed, not what it sent (#646). A reminder that did not land is a failure, not a success with a null id.

### The send gate

`risk_classifier.rs` gains a `Send` consequence. `messages_send` and `mail_send` carry it. Send asks in every mode, including Don't Ask, and the per-conversation "do not ask again" grant does not cover it. This is the one line permissions-by-consequence already draws, now wired to the tools that can cross it (the dead-control rule: pin the link with a test that fails if either tool loses the class).

The ask is one card and one phrase. The card shows the recipient as Contacts resolved them, the body, and a Send button. The phrase "send it" (or the button) sends. A bare "yes" does not, decided by Lacy 2026-10-08: half a sentence followed by a pause ("yes, but change...") must never send. "Send it" is a whole phrase a person does not say by accident. Juno's prompt tells them the phrase. Anything else is a correction and re-renders the card. No "are you sure" after that.

### Permissions, progressively, right before the act

Decided by Lacy 2026-10-08: after the basics (microphone, accessibility, screen recording), every permission is disclosed progressively. Nothing is requested at install or in onboarding. Juno asks for a permission in the moment it is about to run the command that needs it, says why in one spoken line ("I need Reminders for that."), and brings the dialog up itself. The person is never sent to find a setting on their own.

How each kind is brought up:

- **Reminders, Calendar, Contacts.** The system dialog appears when the tool first calls the framework. Juno speaks the line, calls, and the dialog is the next thing on screen.
- **Controlling a specific app (Messages, Mail, Notes, Music).** The Automation dialog comes from macOS the first time Juno scripts that app, one per app. Juno says "I need to control Messages for that" and runs the script; the dialog is the next thing on screen. It must come from the Tauri main process so the dialog names Juno (see gotchas).
- **Full Disk Access (reading Messages).** macOS has no prompt for it. Juno says "I need Full Disk Access to read your messages", opens the Privacy pane on that row, and shows the one card with that action. When the person comes back, the read runs without being asked again.

Declined is designed, not an error: one sentence, one action that changes it, never a framework name, a plist key, or the word TCC (capability-shaped UX). Nothing already granted is asked again, and nothing is asked that the current request does not need.

Info.plist needs three strings it does not have: `NSRemindersFullAccessUsageDescription`, `NSCalendarsFullAccessUsageDescription`, `NSContactsUsageDescription`. Automation and Apple Events are already declared.

### Cards

Two new cards, not twelve. `AgendaCard` renders events and reminders with the same shape (title, when, where, done). `MessageCard` renders a text or an email in its three states: draft (with Send), sending, sent. Notes and Music reuse `LinkCard` and `NowPlayingCard`. Both new cards go on the `JsxMessageRenderer` whitelist and nowhere else.

### Local first, cloud second

If Calendar.app has the person's Google account, EventKit already sees it. The rule for the system prompt: a Mac app tool wins over a Composio tool for the same thing. Composio is for what is not on this Mac.

## Where Juno is better, and the plan keeps it that way

1. Any app. Siri's third-party actions need the developer to opt in. Tier 2 does not.
2. Compound requests. "Find Katie's invoice email and remind me Friday to pay it" is two tool calls in one turn. Siri does one thing per request.
3. The person extends it. `shortcuts_run` plus `bash` plus `run_applescript` means anything they build, Juno can call.
4. Files. Rename, move, find, open. Siri cannot.
5. No cap. Local intents and local models run with no daily limit.
6. It says what happened. Read-back after every write.

None of these need new work. They need the parity rows done so they are not the only thing Juno can do.

## Speed

Siri is instant on simple commands because its grammar is fixed. Tier 0 matches that today. Tier 1 costs a model round trip, so every row in the scorecard records key-release to first audio (`TurnTiming`), and the target for a single-intent tier-1 request is first audio within 1.5 seconds. Two levers, taken in order, only if the number misses:

1. A small-turn router: a single-intent utterance goes to the fastest model with only the `mac_apps` tools in context.
2. Slot-filled tier 0 for the top five utterances, using the fast-orchestration research (Jev classifier) already filed.

Neither is built until the scorecard says it is needed.

## The scorecard

`~/repo/juno/docs/parity/siri-utterances.md`: one row per Siri capability on the Mac, about sixty rows. Columns: utterance, Siri on Mac (yes/no), Juno tier, status, first-audio ms, recording. Each slice's PR updates its rows with a real-run recording in `docs/changelog/media/<PR>/`. Parity is done when every row Siri passes, Juno passes. A green row without a recording is not green.

## Slices, each one ships whole

**Slice 1: Reminders, Calendar, Contacts.** EventKit and Contacts through objc2 (the 0.3 generation already in `Cargo.toml`). Nine tools, `AgendaCard`, three plist strings, decline states. Demo: "what's on my calendar this afternoon", "remind me to call Katie tomorrow at nine", "move my three o'clock to four", and "add this to my calendar" while an invite is on screen. Scorecard rows for the three domains.

**Slice 2: Messages, FaceTime, Mail, Notes, and the send gate.** Nine tools, `Send` consequence with its pinning test, `MessageCard`, Full Disk Access ask for reading. Demo: "text Doug I'm running late" through "send it" to "Sent.", "read me my unread mail", "what did Katie text me" (the Full Disk Access ask, in the moment), "make a note: pick up the drawings Thursday", "FaceTime Mom". This is the keynote slice.

**Slice 3: Music, Maps, Shortcuts, Focus.** Four tools, the signed "Juno Focus" shortcut and its one-click import, the four tier-0 phrasings above. Demo: "play Boards of Canada", "directions to the airport", "turn on the porch lights" (Lacy's existing shortcut), "turn on do not disturb" the first time (import) and the second (instant).

**Slice 4: Speed and close-out.** Fill every scorecard row with a number and a recording. Pull the two speed levers only where rows miss 1.5 s. Then record the three "beyond Siri" demos (compound, any-app, files) as the proof the floor was worth building. This slice produces the growth-push recording (LAC-4173), decided by Lacy 2026-10-08: the recording shows the whole floor plus what is above it, not one slice.

Build rule: one agent per slice, Sonnet unless the send gate is in scope (slice 2 gets Opus for the classifier work). Each slice is one PR against `main`, CI only, no local cargo (juno-remote-builds).

## Considered and cut

| Candidate | Why it was cut |
| --- | --- |
| Registering Juno's own App Intents so Siri can call Juno | A different product (being a Siri extension). Revisit after parity. |
| MediaRemote for now-playing | Private API, notarization risk. Music AppleScript already works. |
| Brightness and Bluetooth typed tools | No public API. Tier 2 can drive System Settings; not promised, not in the scorecard as a pass. |
| Weather, Stocks, Translate tools | The model answers all three. Measure latency first; build only if a row misses. |
| Spotify "play X" by search | Spotify's AppleScript has no search. Needs the Web API and OAuth. Composio has a Spotify toolkit if it is ever asked for. |
| Own slot-filling NLU | The model fills slots. Tier 0 stays bounded-type so a misfire is impossible by construction. |
| Siri Suggestions, proactive anything | Not a request. Out of scope for a request-driven assistant. |
| HomeKit direct | No public macOS API. Shortcuts is the only door and it is a good one. |
| Reading Messages through the UI by default | Full Disk Access is one click and the read-back is exact. Tier 2 stays the fallback when it is declined. |
| A Mac Apps settings page | Nothing to configure. Permissions are system dialogs and the tools are on. |

## Gotchas known going in

- `chat.db` stores recent message bodies in the `attributedBody` blob with `text` null. The reader must decode both.
- EventKit has no read-only mode; full access is the only ask for Reminders and Calendar.
- Messages AppleScript `send` targets a participant or chat, not a phone string. Resolve through Contacts first, and pick the iMessage handle over SMS when both exist.
- A Shortcuts run that needs input stalls the CLI. Only run shortcuts whose input type is none or text, and pass text on stdin.
- Automation prompts come from the target app, one per app, and only when Juno is the frontmost process's responsible app. Trigger them from the Tauri main process, never from a spawned `osascript` with a different responsible PID.

## Decided by Lacy, 2026-10-08

1. Send needs the phrase "send it" or the button. A bare "yes" never sends, so half a sentence and a pause cannot misfire.
2. Juno asks for Full Disk Access, and for control of each app, right before the command that needs it, and brings the dialog up itself. Progressive disclosure after the basics.
3. Slice 4 produces the growth-push recording.

## Demo test

1. Ten seconds: hold the key, say the reminder, hear it back, see the card. Say the text, see the card, say "send it", hear "Sent."
2. Removed: a settings page, twelve cards, model-written AppleScript for the seven apps, a custom NLU, private APIs, and every domain without a public path.
3. One primary action: Send on the message card. Everything else acts on the request itself.
4. Defaults: every tool on, every permission asked right before the act that needs it, send always asks for "send it".
5. States: declined permission (one sentence, one action), empty calendar ("Nothing until 3."), tool failure (read-back says what it saw), offline (tier 0 still works, tier 1 says it needs a connection once).
6. Instant on tier 0; first audio within 1.5 s on tier 1 or the speed slice runs.
7. Seams to close: the prompt's "write AppleScript" instruction, the blanket High rating, the Composio-versus-local overlap on Calendar.
8. Stage test: the two exchanges in the first ten seconds are keynote-grade. The rest is the floor beneath them.
9. Evidence: scorecard recordings per slice in `docs/changelog/media/<PR>/`.
10. DRI: Lacy.
