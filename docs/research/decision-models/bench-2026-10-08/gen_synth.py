"""Hand-written synthetic rows for the Juno decision bench. Run: python -I gen_synth.py"""
import json, random, re, os
R = random.Random(7)
HERE = os.path.dirname(os.path.abspath(__file__))

SET = {
 "juno_general": ["open Juno's general settings", "make Juno start when I log in", "show me the Juno settings", "where do I change launch at login for Juno", "i want to change Juno's language in settings"],
 "juno_voice": ["change Juno's voice", "I want a different voice for you", "you sound too robotic, open the voice settings", "switch your speaking voice to something deeper", "slow down how fast you talk, in your voice settings"],
 "juno_triggers": ["change the push to talk key", "open trigger settings", "I want to hold a different key to talk to you", "change the shortcut that wakes you up", "set the dictation hotkey to something else"],
 "juno_ai_provider": ["switch your provider to anthropic", "open the AI provider settings", "let me put in my open router key", "change which company's model you use", "where do I paste my API key for you"],
 "juno_models": ["pick a different model for you", "change the default model to the smaller one", "open the models page", "let me download a different speech model", "which transcription model, I want to change it"],
 "juno_tools": ["open the tools settings", "turn off the browser tool for you", "show me which tools you can use in settings", "disable file tools in your settings", "enable the shell tool in Juno settings"],
 "juno_automations": ["open automations", "show me my scheduled tasks settings", "edit the morning routine automation", "take me to where the automations live", "manage my recurring automations in Juno"],
 "juno_notifications": ["change how Juno notifies me", "turn off your notification sounds in your settings", "open Juno notification settings", "stop Juno from popping up alerts, in settings", "set Juno's alerts to quiet"],
 "juno_permissions": ["what permissions does Juno have, open that", "open Juno's permission settings", "where's the toggle to let you control my screen", "revoke Juno's file access in settings", "change what Juno is allowed to do without asking"],
 "juno_advanced": ["open advanced settings", "show me the developer options for Juno", "reset Juno's settings on the advanced page", "turn on debug logging in your settings", "take me to Juno's advanced tab"],
 "mac_wifi": ["open wifi settings", "I want to join a different wifi network", "take me to the network settings", "connect to another wireless network", "open system settings to wi-fi"],
 "mac_bluetooth": ["open bluetooth settings", "pair my new headphones", "I need to connect my AirPods, open bluetooth", "show me the bluetooth devices", "forget that bluetooth speaker in settings"],
 "mac_sound": ["open sound settings", "change my output device in system settings", "pick a different microphone input in the sound settings", "take me to the sound preferences", "change the alert sound for the Mac"],
 "mac_displays": ["open display settings", "change my screen resolution", "set up my external monitor arrangement", "turn on night shift in the display settings", "make the text on my display bigger in display preferences"],
 "mac_keyboard": ["open keyboard settings", "change the keyboard shortcuts for my Mac", "make key repeat faster in settings", "add a new keyboard language layout", "turn off autocorrect in the keyboard settings"],
 "mac_notifications": ["open mac notification settings", "stop Slack notifications in system settings", "change the notification style for Messages", "turn off notifications for Mail in settings", "show me the system notification center settings"],
 "mac_privacy_security": ["open privacy and security", "which apps have camera access, in system settings", "allow Juno screen recording in privacy settings", "turn on FileVault", "open the full disk access page"],
 "mac_battery": ["open battery settings", "turn on low power mode in settings", "show battery health in system settings", "change when my Mac sleeps on battery", "open the energy settings"],
 "mac_appearance": ["switch to dark mode in system settings", "open appearance settings", "change my accent color on the Mac", "change the highlight color in settings", "make scroll bars always show in appearance"],
 "mac_software_update": ["check for macOS updates", "open software update", "install the latest macOS update from settings", "turn on automatic updates in system settings", "is there a new version of macOS, open updates"],
 "mac_accessibility": ["open accessibility settings", "turn on voiceover", "make the cursor bigger in accessibility settings", "turn on reduce motion", "enable live captions from settings"],
 "mac_desktop_dock": ["open desktop and dock settings", "make the dock smaller in settings", "hide the dock automatically in settings", "change hot corners in system settings", "move the dock to the left side"],
 "mac_focus": ["open focus settings", "set up a work focus mode", "edit my do not disturb schedule", "change what's allowed through in focus", "open focus filters"],
}
# (text, optional component label for the near-miss)
SET_NEG = [
 ("how do I change my voice?", None), ("what's my wifi called", "system_status"), ("the sound in this video is bad", None),
 ("my wifi is really slow today", None), ("what's the range on those bluetooth headphones", None), ("tell me about the history of the keyboard", None),
 ("the display on that TV looks great", None), ("what battery percentage do I have", "system_status"), ("is dark mode actually better for your eyes", None),
 ("the notifications from my phone are so annoying", None), ("I can't focus today", None), ("what's a good software update strategy for a company", None),
 ("the dock at the harbor closes at six", None), ("turn the volume down", None), ("mute", None), ("play something on my bluetooth speaker", None),
 ("what's the weather", "weather"), ("set a timer for ten minutes", "timer"), ("how do you spell accessibility", None),
 ("write an email about our privacy policy", None), ("explain what end to end encryption means", None), ("the voice in this podcast is great", None),
 ("my keyboard is sticky, how do I clean it", None), ("who invented wifi", None), ("what model of Mac do I have", "system_status"),
 ("what's the best AI provider for coding", None), ("why is my battery draining so fast", None), ("I like your voice", None), ("what is automation testing", None),
 ("the permissions on this file are wrong, fix them", None), ("the advanced calculus homework is hard", None), ("general question, what year was the moon landing", None),
 ("how do models get trained", None), ("play the next track", None), ("what does a wake word do", None), ("how do I change the notification settings on my phone", None),
 ("is my bluetooth on", "system_status"), ("tell me about the new software update rumors", None), ("which shortcut do people use for screenshots", None),
 ("how do tools work in an agent", None),
]
COMP = {
 "now_playing": ["what song is this", "what's playing right now", "who sings this", "what's this song called", "name that tune", "which album is this from", "what am I listening to"],
 "timer": ["set a timer for ten minutes", "how much time is left on my timer", "start a five minute countdown", "timer for the pasta, eight minutes", "cancel the timer", "egg timer three minutes please", "set a twenty minute timer"],
 "weather": ["what's the weather", "is it going to rain tomorrow", "how cold is it outside", "do I need an umbrella", "weather in Denver this weekend", "what's the forecast for Charlotte", "will it snow tonight"],
 "calendar_day": ["what's on my calendar today", "what do I have tomorrow", "am I free at three", "show me my schedule for Friday", "what meetings do I have this afternoon", "when's my next meeting", "do I have anything on Monday"],
 "reminders": ["remind me to call mom at five", "what are my reminders", "add buy milk to my reminders", "remind me tomorrow to send the invoice", "show my to do list", "don't let me forget the dentist", "remind me to take the trash out tonight"],
 "calculation": ["what's 15 percent of 240", "twelve times thirty four", "split 86 dollars four ways", "what's the square root of 144", "256 divided by 16", "what's 18 plus 27 minus 9", "tip on 64 dollars at 20 percent"],
 "app_opened": ["open Safari", "launch Spotify", "open the calculator", "bring up Notes", "open Slack for me", "start Zoom", "pull up Finder", "open Photos"],
 "web_answer": ["who won the game last night", "what's the stock price of Apple", "when is the next full moon", "how tall is Mount Everest", "what's the score of the Panthers game", "who is the CEO of Nvidia", "what time does Target close"],
 "directions": ["how do I get to the airport", "directions to Whole Foods", "navigate home", "how far is it to Raleigh", "take me to the nearest gas station", "how long to drive to Atlanta", "what's the best route to work"],
 "system_status": ["how much battery do I have left", "what's my wifi connection", "how much storage is left", "is my bluetooth on", "how much memory is being used", "what's my IP address", "how is my CPU doing", "what Mac is this"],
}
COMP_NEG = ["tell me a joke", "what is a timer in programming", "the weather in this movie is gloomy", "I love this song", "explain how a calendar works",
 "I need to remind myself to breathe", "calculus is hard", "open my mind to new ideas", "write an essay about giving directions", "my system of filing is a mess",
 "who sings better, Adele or Beyonce", "tell me the history of the weather channel", "this music is too loud", "I'll be back in a few minutes", "what does the word app mean",
 "the web is such a time sink", "how are you doing today", "I got lost on the way here yesterday", "do you like math", "what's the meaning of life"]

TASKS = ["Researching flights to Denver", "Writing an email to Sam about the invoice", "Booking a table for four on Friday", "Organizing the Downloads folder",
 "Summarizing the PDF on screen", "Playing chess in the Chess app", "Filling out the expense form in Chrome", "Creating a calendar event for the team lunch",
 "Searching for hotels in Austin", "Renaming photos on the desktop"]
STOP = ["stop", "cancel that", "never mind", "actually forget it", "hold on, stop", "wait don't do that", "abort", "kill it", "that's enough", "stop doing that",
 "no no no stop", "cancel the task", "scratch that", "okay that's fine, stop", "pause, leave it", "undo that and stop", "stop it Juno", "enough", "halt", "nope, cancel",
 "forget the whole thing", "stop right there", "Juno stop", "cut it out", "hang on, cancel"]
ADD = [(0, "make it a window seat"), (0, "and keep it under four hundred dollars"), (0, "only direct flights please"), (1, "also mention the due date"), (1, "make it sound more friendly"),
 (1, "cc Maria on it"), (2, "make it seven thirty"), (2, "outside seating if possible"), (3, "skip the screenshots folder"), (3, "also sort them by date"),
 (4, "keep it to three bullets"), (4, "focus on the pricing section"), (5, "play more aggressively"), (5, "use the white pieces"), (6, "use the corporate card"),
 (6, "the date is October third"), (7, "add Priya as an attendee"), (7, "make it an hour long"), (8, "near downtown"), (8, "with free parking"),
 (9, "start with the vacation ones"), (9, "use the date as the name"), (2, "actually make it five people"), (1, "say I'll send the file tomorrow"), (4, "and list the action items")]
NEW = ["what's the weather in Denver", "set a timer for ten minutes", "what song is this", "open Spotify", "what's on my calendar tomorrow", "how much battery do I have",
 "tell me a joke", "who won the game last night", "remind me to call mom", "what's 15 percent of 80", "turn the volume down", "open bluetooth settings", "directions to the airport",
 "what time is it in Tokyo", "send a message to Sam", "play some jazz", "what's the capital of Peru", "write me a haiku", "take a screenshot", "can you also check the news",
 "open the calculator", "how do I get home", "what's the forecast this weekend", "mute the mic", "start a focus session"]
NFJ = ["hey honey did you take the dog out", "yeah I'll be there in five", "no I told him we'd call back", "can you pass me the charger", "uh huh yeah", "what do you want for dinner",
 "turn the TV down", "sorry I was talking to my wife", "mm hm", "yeah okay sounds good", "I'll call you later", "did you see the game", "oh my god that's hilarious",
 "babe where are my keys", "tell Mike to call me back", "ha ha ha", "that's what she said", "are you coming to lunch", "thanks buddy see you tomorrow",
 "no no not you Juno", "I know right", "let me think", "sorry, coughing", "can you close the door", "hold on one second I'm talking to someone"]

HOMO = {"wifi": "why fi", "bluetooth": "blue tooth", "dock": "dark", "settings": "setting", "timer": "time her", "Spotify": "spot a fie", "Juno": "Juneau",
        "calendar": "calender", "weather": "whether", "two": "to", "their": "there", "write": "right", "airport": "air port", "Denver": "Dever"}
def noise(t, rng):
    if rng.random() > 0.4: return t
    op = rng.choice(["lower", "homo", "filler", "dup", "drop", "homo", "lower"])
    if op == "lower": return re.sub(r"[.,?!']", "", t).lower()
    if op == "homo":
        for k, v in HOMO.items():
            if k in t: return t.replace(k, v, 1)
        return t.lower()
    w = t.split()
    if op == "filler": return rng.choice(["uh", "um", "okay so", "hey", "so"]) + " " + t[0].lower() + t[1:]
    if op == "dup" and len(w) > 2:
        i = rng.randrange(len(w)); w.insert(i, w[i]); return " ".join(w)
    if op == "drop" and len(w) > 3:
        del w[rng.randrange(1, len(w) - 1)]; return " ".join(w)
    return t

rows = []
def add(q, text, label, **kw):
    rows.append({"id": f"syn-{q}-{len([r for r in rows if r['question']==q]):03d}", "synthetic": True, "question": q, "text": noise(text, R), "label": label, **kw})
for dest, ts in SET.items():
    for t in ts: add("settings", t, dest, near_miss=False, addressed="for_juno", component="none")
for t, c in SET_NEG:
    kw = {"component": c} if c else {}
    add("settings", t, "none", near_miss=True, **kw)
for comp, ts in COMP.items():
    for t in ts: add("component", t, comp, near_miss=False, addressed="for_juno", settings="none")
for t in COMP_NEG: add("component", t, "none", near_miss=True)
for t in STOP: add("midrun", t, "stop", running_task=R.choice(TASKS), addressed="for_juno")
for i, t in ADD: add("midrun", t, "add_to_task", running_task=TASKS[i], addressed="for_juno")
for t in NEW: add("midrun", t, "new_request", running_task=R.choice(TASKS), addressed="for_juno")
for t in NFJ: add("midrun", t, "not_for_juno", running_task=R.choice(TASKS), addressed="not_for_juno")
with open(os.path.join(HERE, "synthetic.jsonl"), "w") as f:
    for r in rows: f.write(json.dumps(r) + "\n")
import collections
print(collections.Counter(r["question"] for r in rows), sum(r.get("near_miss", False) for r in rows))
