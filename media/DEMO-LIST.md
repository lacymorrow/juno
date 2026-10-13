# Juno demo list

Every short demo worth recording, one feature per clip. Each clip is 10 to 30 seconds, made for an X post first, and can be cut into the longer videos later.

**Rules for every clip**
- Film only paths that have run on a real Mac. A clip counts as proven once Lacy has done it for real, or a rig take exists.
- Juno must start with an empty chat. Show only the staged `~/Desktop/@` folder, test contacts and test events. Never show real messages, mail or files.
- Record the screen silently first and add the voice afterwards (see [video-juno-demo/storyboard.md](video-juno-demo/storyboard.md)).
- Each clip makes one point, and the post leads with that point, not with the feature name.

Status key: **proven** = run for real · **built** = merged, never run against the real app · **held** = waiting on a ticket or a decision.

## Siri parity (Juno drives the Mac's own apps)

These come from the scorecard walk ([LAC-4244](https://zero.hyrax-boga.ts.net/LAC-4244)). They also serve as the test pass before slice 4.

| # | Clip | What happens on screen | The point | Status |
|---|---|---|---|---|
| 1 | Juno asks before macOS does | "What's on my calendar today?" A single line with a Continue button, then the Mac's own Calendar dialog, then today's agenda. Ask again: the answer comes straight back with no banner. | No surprise permission popups. Juno tells you first and asks only once. | built |
| 2 | Reminders by voice | "Remind me to call Mom tomorrow at 9." The reminder appears in the real Reminders app. | It uses your real Reminders, not a copy inside Juno. | built |
| 3 | Contacts lookup | "What's Sarah's number?" A card shows the number with a Call button. Needs a test contact. | Ask it and get a button back. | built |
| 4 | Read my texts | "Read my last texts from [test contact]." Juno opens the Full Disk Access pane and you add Juno there. Then it reads the texts aloud. | It walks you through the one permission the Mac can't ask for in a dialog. | built |
| 5 | "Yes" never sends | "Text [self] hi from Juno." A draft card appears. Say "yes" and nothing happens. Say "send it" and it sends. | A pause after "yes" can't fire off a half-finished message. **Strongest trust clip.** | built |
| 6 | Email by voice, same gate | "Email [self] a test." Draft card, then "send it." | One send rule for everything. | built |
| 7 | Notes | "Add a note: buy film." It lands in Notes. | | built |
| 8 | FaceTime without surprises | "FaceTime [test contact]." FaceTime opens and stops before the call starts. | It never calls anyone you didn't confirm. | built |
| 9 | Music | "Play some jazz," then "pause." | | built |
| 10 | Directions | "Directions to Target." Maps opens with the route. | | built |
| 11 | Do Not Disturb | "Turn on Do Not Disturb." The first time, the Mac asks to import the Juno Focus shortcut. | Juno uses the Mac's own Focus, not its own version. | built |
| 12 | Your shortcuts by voice | "What shortcuts do I have?" Then run a harmless test shortcut. Never one of Lacy's real ones. | Every shortcut you've already built becomes a voice command. | built |
| 13 | Instant system controls | "Volume to 30," "mute," "what's my next meeting." Each answers before you finish blinking, with no model call. | Fast because it skips the AI for simple things. | built (volume proven earlier) |
| 14 | Siri vs Juno, side by side | The same five requests to each. | Parity first, then where Juno is better: cards, buttons, the send rule. | held (needs 1 to 13 proven) |

## Answers you can press (the growth hook, [LAC-4173](https://zero.hyrax-boga.ts.net/LAC-4173))

| # | Clip | What happens on screen | The point | Status |
|---|---|---|---|---|
| 15 | Folder card with an Open button | "What's in the @ folder on my desktop?" A card of files appears; press Open and Finder shows the folder. | Ask a question and get a button back. | proven (rig, [LAC-4085](https://zero.hyrax-boga.ts.net/LAC-4085)) |
| 16 | Calculator, hands off | "Open Calculator and multiply 64 by 64." The app does the math and Juno says the answer. | It drives the app; your mouse never moves. | proven |
| 17 | It never touches your mouse | Juno works in an app while you keep typing somewhere else. The ghost cursor shows what it's doing. | It uses the accessibility tree, not your pointer. | built (#749, #750, #722) |
| 18 | Something groovy | "Find something groovy to listen to." Music starts playing. | | rig scene exists, take not yet reviewed |

## Dictation and voice

| # | Clip | What happens on screen | The point | Status |
|---|---|---|---|---|
| 19 | Hold to dictate, anywhere | Hold the key and speak; the words land in a real text field in another app. | It's the daily habit. | needs one rehearsal |
| 20 | Pauses don't become "..." | Talk with long pauses. The text comes out clean, with on-device grammar cleanup. | It's on device and the output is clean. | built (#791, default off per #798) |
| 21 | First word fast | A spoken question. Time the gap to the first audio. | It speaks the first sentence while it's still thinking. | built (#713, #773, #795) |
| 22 | Speaks in the Mac's own voice | Juno answers in the system Siri voice. | It sounds native, not like a chatbot. | proven |

## The app itself

| # | Clip | What happens on screen | The point | Status |
|---|---|---|---|---|
| 23 | Launch reveal | The bar rises out of smoke and the greeting lands on the beat. | First impression. | proven |
| 24 | Escape steps down | Each press of Esc closes one layer, newest first. The Dock or tray icon brings it back. | It's calm and never loses your work. | built (#797) |
| 25 | "Open settings" and it points | "Turn on [setting]." Juno opens Settings, points at the switch and flips it. | It can change its own settings. | built (#759, #761) |
| 26 | Your Claude login, no new bill | Setup uses an existing Claude Pro or Max login. | Already paid for. | proven |

## Held: waiting on Lacy's go (slice 5)

| # | Clip | Status |
|---|---|---|
| 27 | "What's the weather?" through a Shortcuts weather action, with Weather.app opened for detail ([LAC-4301](https://zero.hyrax-boga.ts.net/LAC-4301)) | held |
| 28 | Brightness by voice ([LAC-4302](https://zero.hyrax-boga.ts.net/LAC-4302)) | held |
| 29 | "What's playing?" for any app ([LAC-4303](https://zero.hyrax-boga.ts.net/LAC-4303)) | held, spike |
| 30 | "Hey Siri, ask Juno..." through an App Intent ([LAC-4304](https://zero.hyrax-boga.ts.net/LAC-4304)) | held, spike; the best clip on this list if it works |
| 31 | Guided permission helper: a floating window over System Settings, then drag the Juno icon in ([LAC-4234](https://zero.hyrax-boga.ts.net/LAC-4234)) | not built |
| 32 | Paste a photo and edit it (image edits, the first Pro reason) | not built (LAC-4173 child) |

## Longer cuts these feed

- **The 75-second Juno demo**: [video-juno-demo/storyboard.md](video-juno-demo/storyboard.md) uses #15, #16 and #19.
- **"Siri, but it does the thing"**: three Mac apps by voice with the cursor untouched. Uses #1, #2, #5, #10 and #13 once they're proven. This is the slice 4 growth recording.
- **Lacy's Lab channel**: the other four storyboarded videos (lacy shell, ShipX, Hitchhiker's Guide, Paperclip fleet) are listed in `~/repo/marketing-videos/docs/RECORDING-DAY-2026-10-11.md`.
