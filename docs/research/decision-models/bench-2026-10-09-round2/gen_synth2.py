"""Round 2 synthetic rows. Each list is 20 strings = 5 paraphrase families of 4 consecutive strings. Run: python -I gen_synth2.py"""
import json, random, re, os, hashlib, collections
R = random.Random(11); HERE = os.path.dirname(os.path.abspath(__file__))
S = {
"juno_general": ["make Juno start when I log in","have you open automatically when my Mac starts","turn on launch at login for Juno","I want you running every time I turn on my computer, where's that",
 "change the color of your cursor","make Juno's pointer green","what color is your cursor, I want to pick another","set the Juno cursor color to orange",
 "open Juno's general settings","take me to the general page in Juno","show me the general tab for Juno","go to Juno general preferences",
 "change Juno's language","can you speak a different language, show me the setting","Juno language preference please","switch Juno to Spanish in settings",
 "hide Juno from the dock","put Juno only in the menu bar","show Juno in the dock again","I want the Juno icon back in my dock"],
"juno_triggers": ["change the shortcut that summons you","what key opens Juno, I want a different one","rebind the Juno hotkey","set your shortcut to option space",
 "change the dictation shortcut","I want to hold a different key to dictate","set dictation to Fn plus control","dictation key setting please",
 "change the push to talk key","I want to hold a different key to talk to you","switch push to talk to something else","rebind hold to talk",
 "open trigger settings","take me to the triggers page","show Juno's triggers","go to the triggers tab",
 "the shortcut keeps conflicting with another app, change it","your hotkey clashes with Raycast, let me change it","I keep hitting your hotkey by accident, change it","stop me from triggering you by accident, trigger settings"],
"juno_audio": ["change which mic Juno uses","use my headset microphone for you","pick a different input device for Juno","you can't hear me, check the mic setting",
 "change Juno's voice","I want a different voice for you","switch your speaking voice to something deeper","pick another voice for Juno",
 "make you talk slower","can you speak faster, change that","slow down your speaking speed","turn your talking speed down in settings",
 "make your voice louder","turn Juno's volume up in settings","I can't hear you, turn your volume up","you're too quiet, raise your volume",
 "open the audio settings","take me to Juno's voice page","show audio settings for Juno","go to the audio tab"],
"juno_providers": ["where do I paste my API key","put in my open router key","add my anthropic key","enter my open AI API key for you",
 "switch your provider to anthropic","change which company's AI you use","use open router instead","swap the AI provider",
 "open the providers settings","take me to the AI providers page","show providers","go to providers in Juno settings",
 "log in with my Claude subscription instead of a key","connect my chat GPT account to Juno","sign in to my provider account","use my existing subscription for you",
 "you say there's no API key, where do I add it","Juno says it can't connect to the AI, let me fix the key","my key expired, let me enter a new one","I got a new API key, where does it go"],
"juno_models": ["pick a different model for you","switch your model to the bigger one","use a smarter model","change the default model",
 "change the transcription model","download a different speech model","use parakeet for dictation, model settings","which speech to text model are you using, change it",
 "open the models page","take me to models","show the models settings","go to the models tab",
 "use a cheaper model","I want a faster model even if it's dumber","switch to the small model to save money","stop using the expensive model",
 "you're too slow, use a quicker model","your answers are shallow, use a better model","can you think harder, switch the model","swap out the brain you're using"],
"juno_notifications": ["change how Juno notifies me","turn off your notification sounds","stop Juno from popping up alerts","set Juno's alerts to quiet",
 "mute the ding when you finish","turn off the completion sound","I don't want the chime when a task is done","change the sound Juno plays when done",
 "open Juno notification settings","go to Juno's notifications page","take me to the notifications tab in Juno","show Juno notification preferences",
 "you keep interrupting me with popups, stop that","too many alerts from Juno","stop pinging me when tasks finish","I only want alerts when you need me",
 "turn off the banner when a task finishes","let me pick which Juno alerts I get","tell me when the long task is done, notification setting","silence Juno's notifications while I'm presenting"],
"juno_tools": ["turn off the browser tool","disable the desktop tools for Juno","enable the timer tool","turn the computer use tool on",
 "smooth out your mouse movement","make the mouse move faster when you click things","turn off the smooth cursor animation","change how your mouse moves",
 "add an MCP server","remove that MCP server","show me my MCP servers","connect a new MCP server for Juno",
 "open the tools settings","take me to tools in Juno","show which tools you can use","go to the tools tab",
 "stop you from using my browser","don't let Juno touch the desktop apps","I don't want you to use any tools, change that in settings","which tools are on right now, let me change them"],
"juno_automations": ["open automations","take me to the automations page","show me my automations","go to the automations tab",
 "edit the morning routine automation","change my daily automation","turn off that scheduled automation","disable the Friday report automation",
 "add a new automation","make a recurring automation","set up a scheduled task for Juno","schedule something to run every day, automations",
 "manage my recurring automations","show scheduled tasks","where do I see what Juno runs on a schedule","list my scheduled jobs in settings",
 "stop the thing that runs every morning","that automation keeps firing, turn it off","I want Juno to do this every Monday, where do I set that","pause all my automations"],
"juno_network": ["open network settings for Juno","go to the network tab in Juno","take me to Juno's network page","show network settings in Juno",
 "set a proxy for Juno","route Juno through a proxy","change Juno's proxy","Juno needs to use my company proxy",
 "let Juno reach the web","block Juno from going online","turn off Juno's internet access","limit which sites Juno can visit",
 "Juno can't connect, check its network settings","Juno says offline, open its network settings","Juno connection keeps timing out, show network","fix Juno's connection settings",
 "change the port Juno uses","set the server address for Juno","point Juno at my local server","change the base URL Juno talks to"],
"juno_security": ["change Juno's permission mode","let Juno do things without asking","make you ask me before doing stuff","switch to the stricter permission mode",
 "turn off ask before send","make you ask before sending messages","stop asking me before you send emails","I want to approve everything you send",
 "open security and privacy for Juno","go to Juno's privacy settings","take me to the security tab in Juno","show Juno's security page",
 "delete my Juno history","show what Juno keeps about me in privacy settings","stop Juno saving my conversations","clear what you remember about me in settings",
 "I don't trust you with my files, restrict what you can do","lock Juno down","limit what you're allowed to touch","who can see my data in Juno, let me look"],
"juno_advanced": ["open advanced settings","take me to Juno's advanced tab","show the advanced page","go to advanced in Juno settings",
 "keep Juno working in the background","let Juno keep running when I close the window","turn on work in background","stop Juno working while I'm away",
 "let Juno use my real mouse","turn off mouse control","stop Juno from taking over my cursor","ask me before you take the mouse",
 "keep the Claude session alive between requests","turn on persistent Claude session","start a fresh Claude session each time","stop reusing the old session",
 "show advanced settings","turn on the developer options","reveal the hidden settings","unhide the advanced options"],
"mac_wifi": ["open wifi settings","take me to the wifi page","show wi-fi settings","go to wireless settings",
 "join a different wifi network","connect to another wireless network","switch to the guest wifi","connect me to the office network",
 "my wifi keeps dropping, open that","the internet is flaky, open wifi settings","wifi is acting up, show me the network settings","I can't get online, open the wifi page",
 "forget this wifi network","remove the old wifi from my Mac","delete the saved wifi password","stop auto joining that network",
 "turn wifi off","turn the wifi back on in settings","set up a static IP for wifi","change my DNS servers"],
"mac_bluetooth": ["open bluetooth settings","take me to the bluetooth page","show bluetooth settings","go to bluetooth preferences",
 "pair my new headphones","I need to connect my AirPods, open bluetooth","connect my bluetooth speaker","add a new bluetooth device",
 "forget that bluetooth speaker","remove my old headphones from bluetooth","unpair the mouse","delete the bluetooth keyboard",
 "my mouse won't connect, open bluetooth","my AirPods keep disconnecting, show me bluetooth","the headphones aren't showing up, open that","bluetooth is acting weird, take me there",
 "turn bluetooth off","turn on bluetooth in settings","rename my bluetooth device","show connected bluetooth devices"],
"mac_sound": ["open sound settings","take me to the sound page","show sound preferences","go to the sound tab in system settings",
 "change my output device","play sound through the speakers instead","send audio to my headphones, sound settings","switch the speaker output",
 "pick a different microphone input in the sound settings","use the external mic","change the input device for the Mac","select my USB mic",
 "I can't hear anything on my Mac, open sound","the sound is coming out of the wrong speaker","my Mac is too quiet, open the sound page","no audio from my laptop, check sound settings",
 "change the alert sound","turn off the startup chime","change the system beep","turn off sound effects on the Mac"],
"mac_displays": ["open display settings","take me to the displays page","show display preferences","go to displays in system settings",
 "change my screen resolution","make everything on screen bigger","scale the display to look smaller","set the display to default resolution",
 "set up my external monitor arrangement","mirror my display to the TV","extend my desktop to the second screen","arrange my two monitors",
 "turn on night shift","make the screen warmer at night","turn true tone off","change auto brightness",
 "my screen is blurry, open display settings","the second monitor isn't showing up, open displays","text on my screen is too small, display settings","the monitor is on the wrong side, fix it in settings"],
"mac_keyboard": ["open keyboard settings","take me to the keyboard page","show keyboard preferences","go to keyboard in system settings",
 "make key repeat faster","change keyboard repeat delay","slow down the key repeat","speed up how fast keys repeat",
 "change the keyboard shortcuts for my Mac","remap caps lock to escape","change the screenshot shortcut","set up a custom shortcut in keyboard settings",
 "turn off autocorrect","disable auto capitalization","turn off smart quotes","stop the keyboard from replacing text",
 "add a new keyboard language layout","add an emoji keyboard","switch the keyboard layout to Dvorak","add a Spanish keyboard"],
"mac_notifications": ["open mac notification settings","take me to system notifications","show notification settings on my Mac","open notification center preferences",
 "stop Slack notifications in system settings","turn off notifications for Mail","silence Messages alerts in settings","change the notification style for Calendar",
 "my Mac keeps buzzing with notifications, change that","too many popups from apps, open settings","stop Safari from asking to send notifications","banners won't go away, notification settings",
 "hide notification previews on the lock screen","don't show notifications when the screen is locked","turn off notification badges","stop notifications showing when sharing my screen",
 "group notifications by app","show notifications as alerts not banners","change how long banners stay","turn off notification sounds for all apps"],
"mac_privacy_security": ["open privacy and security","take me to Mac privacy settings","show security settings on my Mac","go to privacy and security",
 "which apps have camera access","give Juno screen recording permission","allow accessibility access for an app","let Zoom use my microphone in settings",
 "turn on FileVault","enable the Mac firewall","set up Lockdown mode","check if gatekeeper is on",
 "an app says it needs permission to see my screen, open that","the Mac keeps asking for permission, show me where","I clicked don't allow by accident, open the permission page","this app can't record my screen, open privacy",
 "open the full disk access page","show me the location services list","review which apps track my location","remove an app's access to my contacts"],
"mac_battery": ["open battery settings","take me to the battery page","show battery preferences","go to battery in system settings",
 "turn on low power mode","switch to low power mode in settings","enable high power mode","turn off low power mode",
 "show battery health","check my battery condition in system settings","open the battery usage page","show which apps drain the battery",
 "change when my Mac sleeps on battery","stop the screen from turning off on power","set the display sleep time","keep my Mac awake when plugged in",
 "my battery drains too fast, open battery settings","my laptop dies so quickly, show battery options","charging seems stuck, open battery","optimize my charging in settings"],
"mac_appearance": ["switch to dark mode in system settings","turn on dark mode","go back to light mode","set appearance to auto",
 "change my accent color","change the highlight color","make my Mac accent green","pick a new accent color for the Mac",
 "open appearance settings","take me to the appearance page","show appearance preferences","go to appearance in system settings",
 "make scroll bars always show","show scroll bars only when scrolling","change the sidebar icon size","turn on wallpaper tinting",
 "it's too bright in here, switch the Mac to dark","my eyes hurt, make everything dark","the Mac looks too bland, change the colors","switch the system theme for night"],
"mac_software_update": ["check for macOS updates","open software update","is there a new version of macOS, open updates","look for system updates",
 "install the latest macOS update","update my Mac now","download the macOS update","run the system update",
 "turn on automatic updates","stop my Mac from updating automatically","turn off automatic macOS downloads","turn on security updates",
 "my Mac says an update is available, open it","there's a red badge on settings, show me the update","the update keeps nagging me, open software update","I'm on an old macOS, update it",
 "show the update history","upgrade to the new macOS version","enable beta updates","defer the update for later"],
"mac_accessibility": ["open accessibility settings","go to the accessibility page","take me to accessibility preferences","show accessibility on my Mac",
 "turn on voiceover","enable screen zoom","turn on the screen reader","set up voice control",
 "make the cursor bigger","turn on reduce motion","increase contrast","reduce transparency",
 "enable live captions","turn on captions","show subtitles settings","set up spoken content",
 "I can't read the text on screen, open accessibility","the screen is hard to see, show me options","I can't use the mouse well, open accessibility","the animations make me dizzy, reduce them"],
"mac_desktop_dock": ["open desktop and dock settings","take me to the dock page","show dock preferences","go to desktop and dock",
 "make the dock smaller","make the dock bigger","shrink the dock","turn on dock magnification",
 "hide the dock automatically","move the dock to the left","put the dock on the right side","show the dock at the bottom",
 "change hot corners","turn on Stage Manager","set up mission control gestures","change the desktop widgets",
 "the dock is in my way, hide it","the dock takes up too much room, change it","my dock icons are too tiny, open that","I keep opening the dock by accident, change it"],
"mac_focus": ["open focus settings","take me to focus mode settings","show focus preferences","go to the focus page",
 "set up a work focus mode","make a new focus for sleeping","create a driving focus","add a gaming focus",
 "edit my do not disturb schedule","turn on do not disturb","change what's allowed through in focus","allow calls from favorites in focus",
 "I need to stop getting interrupted, set up focus","I want quiet time every night, set that up","silence my Mac while I work, open focus","stop alerts at bedtime, focus settings",
 "link my focus across devices","show focus status in the menu bar","open focus filters","share my focus status"],
}
NEG = [
 ["how do I change my voice?","how would someone change their voice settings?","what's the way to change your voice","can you explain how voices get switched","is it possible to change how you sound"],
 ["how do I connect to wifi","what are the steps to join a wifi network","how does a person switch wifi networks","explain how wifi joining works","how do wifi passwords get stored"],
 ["how do I turn on dark mode","what is dark mode","how does dark mode work","is dark mode better for battery","why do people like dark mode"],
 ["how do keyboard shortcuts work","what's the default shortcut for Juno","what key do I press to talk to you","what's your dictation hotkey","remind me what the push to talk key is"],
 ["how do notifications work on a Mac","what is notification center","why am I not getting notifications","explain notification badges","how do I know if I have a notification"],
 ["what does Juno do with my data","is my data private","does Juno record my screen","are you listening all the time","what permissions do you need"],
 ["what's my wifi called","what wifi am I on","is my wifi working","how strong is my wifi signal","what's my wifi speed"],
 ["the sound in this video is bad","the audio on this podcast is great","this song is too loud","the volume in that movie was low","the music was too quiet in the clip"],
 ["my bluetooth headphones sound great","what's the range on bluetooth","how does bluetooth work","who invented bluetooth","those bluetooth speakers are too expensive"],
 ["what battery percentage do I have","how long will my battery last","is my Mac charging","how much battery is left","what's the battery health on iPhones"],
 ["the display on that TV looks great","what resolution is 4K","my monitor is on sale","which monitor should I buy","that screen is so bright"],
 ["my keyboard is sticky","what's a good mechanical keyboard","type this out for me","the keys on this laptop feel mushy","how do I clean my keyboard"],
 ["I can't focus today","how do I focus better","focus on the task in front of you","what's a good focus playlist","I need to concentrate"],
 ["what's new in the latest macOS","is the update worth it","I hate software updates","how big is the macOS update","tell me about the new software update rumors"],
 ["how do you spell accessibility","write about web accessibility","what is WCAG","good accessibility practice for websites","accessibility matters in design"],
 ["the dock at the harbor closes at six","what's a loading dock","docking a boat is hard","I docked my laptop at the office","where's the nearest dock"],
 ["how do language models get trained","what's the best AI model for coding","which AI model is smartest","what model of Mac do I have","who makes the best models"],
 ["what is automation testing","what tools do I need for woodworking","best tools for a home garage","automate my life please","what is an automation engineer"],
 ["write an email about our privacy policy","explain end to end encryption","what's a good security question","is it safe to use public wifi","how do hackers steal passwords"],
 ["what color should I paint the room","how do I make my slides look nice","pick an accent color for my logo","is light mode bad","what theme is that editor using"],
 ["how do I change the notification settings on my phone","where's the wifi setting on my iPhone","change the ringtone on my iPhone","turn on dark mode on my phone","how do I update my Android"],
 ["turn off notifications inside Slack","change the Chrome default search engine","where's the privacy setting in Instagram","change my Spotify audio quality","change the zoom level in the browser"],
 ["how do I change wifi on Windows","where is bluetooth on my Android","set the TV display to cinema mode","change the sound settings on my Xbox","update my router firmware"],
 ["change Siri's voice","turn off Alexa notifications","change Google Assistant language","set up hey Siri","rename my Alexa"],
 ["change the graphics settings in this game","turn the brightness down on the TV","change my camera settings","set the thermostat to 70","turn up the volume on the speaker"],
 ["turn the volume down","mute","turn it up a little","unmute the Mac","lower the volume"],
 ["play something on my bluetooth speaker","play the next track","pause the music","skip this song","play my focus playlist"],
 ["set a timer for ten minutes","what's the weather","remind me to call mom","send a text to Sam","take a screenshot"],
 ["I like your voice","that was a good answer","you sound great today","thanks for the help","good morning"],
 ["the permissions on this file are wrong, fix them","the advanced calculus homework is hard","general question, what year was the moon landing","what does a wake word do","how do tools work in an agent"],
]
HELD_NEG = {1, 5, 9, 14, 20, 26}
C = {
"now_playing": ["what song is this","what's this song called","name this song","who sings this","who's the artist","which band is this","what's playing right now","what am I listening to","what's on right now","which album is this from","what record is this","what album is that song on","what's the name of this track","tell me what track this is","what was that song that just played"],
"timer": ["set a timer for ten minutes","timer for ten minutes","start a ten minute timer","how much time is left on my timer","how long's left on the timer","check the timer","cancel the timer","stop the timer","clear my timer","egg timer three minutes please","time my eggs for three minutes","three minute timer for the eggs","count down from twenty minutes","start a twenty minute countdown","give me a 20 minute countdown"],
"weather": ["what's the weather","how's the weather","weather please","is it going to rain tomorrow","will it rain tomorrow","tomorrow rain chance","how cold is it outside","what's the temperature outside","is it freezing out","do I need an umbrella","should I bring a jacket","do I need a coat today","what's the forecast for Charlotte this weekend","weekend forecast for Charlotte","weather in Charlotte Saturday"],
"calendar_day": ["what's on my calendar today","what do I have today","show my schedule for today","am I free at three","do I have anything at three","is three o'clock open","when's my next meeting","what's my next meeting","next thing on my calendar","what do I have tomorrow","tomorrow's schedule","what's on for tomorrow","do I have anything on Monday","what's Monday look like","anything scheduled Monday"],
"reminders": ["remind me to call mom at five","set a reminder to call mom at five","call mom at five, remind me","what are my reminders","show my reminders","read my reminders","add buy milk to my reminders","put milk on my to do list","add milk to my list","remind me tomorrow to send the invoice","tomorrow remind me about the invoice","set an invoice reminder for tomorrow","don't let me forget the dentist","remind me about the dentist appointment","dentist reminder please"],
"calculation": ["what's 15 percent of 240","fifteen percent of two forty","calculate 15% of 240","twelve times thirty four","what's 12 times 34","multiply twelve by thirty four","split 86 dollars four ways","divide 86 by 4 people","86 bucks between four people","what's the square root of 144","square root of one forty four","root of 144","tip on 64 dollars at 20 percent","20 percent tip on 64","what's a twenty percent tip on sixty four"],
"app_opened": ["open Safari","launch Safari","bring up Safari","launch Spotify","start Spotify","open up Spotify","open the calculator","pull up the calculator app","start Calculator","open Slack for me","get Slack open","switch to Slack","start Zoom","open Zoom please","launch Zoom"],
"web_answer": ["who won the game last night","who won last night's game","last night's game result","what's the stock price of Apple","how's Apple stock doing","Apple share price","when is the next full moon","next full moon date","when's the full moon","how tall is Mount Everest","height of Everest","what's Everest's elevation","what time does Target close","Target closing time","when does Target shut tonight"],
"directions": ["how do I get to the airport","directions to the airport","route to the airport","navigate home","take me home","directions home","how far is it to Raleigh","distance to Raleigh","how many miles to Raleigh","how long to drive to Atlanta","drive time to Atlanta","how long is the drive to Atlanta","take me to the nearest gas station","where's the closest gas station","directions to a gas station near me"],
"system_status": ["how much battery do I have left","battery level","what's my battery at","how much storage is left","free disk space","how full is my disk","what's my IP address","tell me my IP","my IP address please","how much memory is being used","RAM usage","how's my memory doing","is my bluetooth on","is wifi connected","what's my wifi connection"],
}
CN = [["tell me a joke","tell me a riddle","tell me a story"],["what is a timer in programming","how do timers work in javascript","explain setTimeout"],["the weather in this movie is gloomy","I love rainy weather","weather is a good small talk topic"],
 ["I love this song","this song is my jam","great song choice"],["explain how a calendar works","why do calendars have leap years","who invented the calendar"],["I need to remind myself to breathe","remind me what we were talking about","do you remember what I said"],
 ["calculus is hard","I hate math homework","math was my worst subject"],["open my mind to new ideas","open up about my feelings","I opened a window in my house"],["write an essay about giving directions","how do people give good directions","I'm bad at following directions"],
 ["my system of filing is a mess","how healthy is my immune system","the solar system is huge"]]

HOMO = {"wifi":"why fi","bluetooth":"blue tooth","dock":"dark","settings":"setting","timer":"time her","Spotify":"spot a fie","Juno":"Juneau","calendar":"calender","weather":"whether","write":"right","airport":"air port","mic":"mike","shortcut":"short cut","notifications":"notification"}
def noise(t, rng):
    if rng.random() > 0.35: return t
    op = rng.choice(["lower","homo","filler","dup","drop","homo","lower"])
    if op == "lower": return re.sub(r"[.,?!']", "", t).lower()
    if op == "homo":
        for k, v in HOMO.items():
            if k in t: return t.replace(k, v, 1)
        return t.lower()
    w = t.split()
    if op == "filler": return rng.choice(["uh","um","okay so","hey","so","Juno"]) + " " + t[0].lower() + t[1:]
    if op == "dup" and len(w) > 2:
        i = rng.randrange(len(w)); w.insert(i, w[i]); return " ".join(w)
    if op == "drop" and len(w) > 3:
        del w[rng.randrange(1, len(w) - 1)]; return " ".join(w)
    return t
h = lambda s: int(hashlib.md5(s.encode()).hexdigest(), 16)
rows = []
def add(q, text, label, fam, held, **kw):
    n = sum(1 for r in rows if r["question"] == q)
    rows.append({"id": f"syn2-{q}-{n:03d}", "synthetic": True, "round": 2, "question": q, "text": noise(text, R), "label": label, "family": fam, "split": "test" if held else "train", **kw})
for d, ts in S.items():
    assert len(ts) == 20, d
    hold = h(d) % 5
    for i, t in enumerate(ts): add("settings", t, d, f"{d}-{i//4}", i // 4 == hold, near_miss=False, addressed="for_juno", component="none")
for k, fam in enumerate(NEG):
    for t in fam: add("settings", t, "none", f"neg-{k}", k in HELD_NEG, near_miss=True)
for c, ts in C.items():
    assert len(ts) == 15, c
    hold = h(c) % 5
    for i, t in enumerate(ts): add("component", t, c, f"{c}-{i//3}", i // 3 == hold, near_miss=False, addressed="for_juno", settings="none")
for k, fam in enumerate(CN):
    for t in fam: add("component", t, "none", f"cneg-{k}", k in (2, 7), near_miss=True, settings="none")
with open(os.path.join(HERE, "synthetic_v2.jsonl"), "w") as f:
    for r in rows: f.write(json.dumps(r) + "\n")
print(sorted(collections.Counter((r["question"], r["split"], r["near_miss"]) for r in rows).items()))
