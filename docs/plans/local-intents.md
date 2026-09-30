# Local intents

Short, whole commands that Juno answers itself in well under a second, with no model call. Code: `src-tauri/src/agent/local_intents/`. Entry point: `try_handle_local_intent`, called once from `submit_query` in `anthropic.rs`.

## The matching rule

A local intent fires only when the **whole utterance** is one of a small set of command phrasings, with nothing left over.

- Courtesy at the edges is fine: "hey Juno", "could you please", "thanks", trailing punctuation.
- A clause word after the command starts (`and`, `then`, `if`, `after`, `when`, `or`, `so`, ...) refuses the whole utterance. "Open Safari and find me flights to Denver" goes to the agent.
- An extra object or qualifier we do not model is not a match: "mute Zoom", "turn up the volume on YouTube", "what time is it in Tokyo".
- Questions about the thing are not commands: "how do I mute Zoom?", "how do I lock my screen".
- Names must resolve exactly. "Open X" only fires when X is an installed app (or a vendor-less or aliased form of one). "Open the flight tracker" finds nothing and goes to the agent.

A miss is always the safe outcome: the agent can do everything a local intent can, only slower. Every grammar is anchored at both ends, and each domain's tests carry near-miss negatives that must fall through.

Nothing the person said is spliced into a script. AppleScript is either constant or built from a validated integer. An app name reaches AppleScript only as `argv`, after it matched an installed bundle. `open` gets a bundle path or an `https://` URL on a short TLD allowlist as an argument, never through a shell.

## Commands

| Domain | Command | Example phrasings | Native call | Reply |
|---|---|---|---|---|
| Media (existing) | play, pause, next, previous, now playing | "pause Spotify", "skip this song", "what's playing?" | AppleScript to the player | `<NowPlayingCard>` + line |
| Volume | up / down by 10% | "turn up the volume", "volume down", "lower the volume a bit" | `osascript` (constant script) | "Volume 60%." |
| Volume | set | "set the volume to 50", "volume 30%", "volume to twenty percent" | `osascript`, integer 0 to 100 | "Volume 50%." |
| Volume | mute / unmute | "mute the sound", "mute my Mac", "unmute the volume" | `osascript` | "Muted." / "Unmuted. Volume 40%." |
| Volume | read | "what's the volume?" | `osascript` | "Volume is 40%." |
| Appearance | dark mode on / off / toggle | "turn on dark mode", "switch to light mode", "toggle dark mode" | System Events appearance preferences | "Dark mode on." |
| Security | lock screen | "lock the screen", "lock my Mac" | System Events Control-Command-Q | "Locked." |
| Power | display sleep | "turn off the display", "put the display to sleep" | `pmset displaysleepnow` after 1.2 s | "Turning off the display." |
| Power | Mac sleep | "put my Mac to sleep" | `pmset sleepnow` after 1.2 s | "Going to sleep." |
| Facts | battery | "battery", "how much battery do I have left", "is my laptop charging" | `pmset -g batt` | "Battery is at 82% and charging." |
| Facts | time / date | "what time is it?", "what's today's date?", "what day is it" | `chrono::Local` | "It's 3:05 PM." |
| Timers | start | "set a timer for 5 minutes", "5 minute timer", "timer for 1 hour 30 minutes" | backend task | `<TimerCard>` + "Timer set for 5 minutes." |
| Timers | time left | "how much time is left on the timer?", "check my timer" | in-memory | `<TimerCard>` + "3 minutes and 12 seconds left." |
| Timers | cancel | "cancel the timer", "stop the timer", "cancel all timers" | aborts the task | "Timer cancelled." |
| Apps | open / switch to | "open Safari", "launch Chrome", "switch to Slack", "open settings" | `open -a <bundle path>` | "Opening Safari." |
| Apps | quit | "quit Slack", "exit out of Zoom" | AppleScript with the name as `argv`, only if running | "Quitting Slack." |
| Web | open a site | "open github.com", "go to github dot com" | `open https://<domain>` | `<LinkCard>` + "Opening github.com." |

Timers are owned by the backend: one task per timer on the Tauri runtime, kept in a process-wide list so they can be read and cancelled. When one finishes, Juno speaks "Your 5 minute timer is done." and posts a notification (through the one `notify` door, which honours the notifications setting). Timers do not survive quitting Juno. "How much time is left?" without the word timer is only answered when a timer is running; otherwise it goes to the agent.

"Open" and "quit" never act on Juno itself. Two different apps answering to the same spoken name (for example "Docs") is ambiguous and goes to the agent.

## Considered and cut

| Candidate | Why it was cut |
|---|---|
| Screen brightness | No public API. The options are the private DisplayServices framework or synthetic brightness keys, which only work on some keyboards and displays. Not reliable enough to promise. |
| Do Not Disturb / Focus | No public API to toggle Focus. It needs a user-made Shortcut, so it cannot be one native call out of the box. |
| Show desktop | Only reachable through a synthetic key or gesture that users rebind. Unreliable. |
| Screenshot to Desktop | The Juno bar is on screen at that moment and would be in the shot, and Command-Shift-3 already exists. |
| Empty trash | Destructive and irreversible. A voice misfire must never delete files. |
| Wi-Fi on / off | The interface name varies per Mac, and turning Wi-Fi off by voice can cut off the very connection the agent needs. |
| Web search ("search for X") | Captures free text, and the person often wants an answer rather than a tab. The agent does it better. |
| Caffeinate / keep awake | Adds long-lived process state for a rare request. Worth revisiting with a real "stop" story. |
| Hide app | Process names often differ from bundle names, so it fails in ways a person cannot see. "Switch to" covers the common need. |
| "Close X" | On the Mac, close means a window. Treating it as quit would surprise people. Only "quit" and "exit" quit. |
| Force quit | Loses unsaved work. |
| Bare "mute" / "unmute" / "louder" / "turn it up" | In a call, "mute" usually means the microphone in Zoom or Meet, and "it" can mean anything. The phrase must name the sound or the Mac. |
| Bare "time", bare "lock", bare "dark mode", "go to sleep" | Too easy to hit in passing, or ambiguous ("go to sleep" can be addressed to Juno). |
| Shuffle, like, repeat | Spotify's AppleScript has no "like", and shuffle state is unreliable across players. Next and previous already exist. |
| Timer labels, pause and resume, "5 and a half minutes" | Labels capture free text; pause and resume add state with little use. Durations with "and" hit the clause rule and go to the agent, which handles them fine. |
| System sounds on timer finish | The notification already plays one; adding another doubles it. |

## Adding an intent

1. Add the phrasings to the domain's `Patterns`, anchored with `^...$`.
2. Add positive tests and at least as many near-miss negatives.
3. Keep scripts constant; pass variable input as `argv` or a validated integer.
4. Reply with one short line, and an existing card from the `JsxMessageRenderer` whitelist when one fits.
