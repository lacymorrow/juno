//! The system prompts as they read on Windows.
//!
//! The prompts are written for a Mac, and most of what they say holds anywhere.
//! So Windows takes the finished text and changes what doesn't hold, rather
//! than keeping a second copy of every prompt that would drift from the first.
//!
//! On a Mac [`adapt`] returns its input untouched, which is what keeps the Mac
//! prompts byte-identical. Everything else is pure text and runs on any
//! platform, so CI's macOS job tests the Windows wording.

use super::templates::PromptFragments;

/// Windows' own version of `PromptFragments::accessibility_first_strategy`.
const WINDOWS_STRATEGY: &str = r#"🎯 **HOW TO WORK ON WINDOWS** - CRITICAL FOR ACCURACY

This computer runs Windows 11. The person keeps using it while you work, so do your work in the windows it belongs to, behind whatever they are doing.

**Look before you act.** Call `list_visible_windows` to see what's open. If they ask for email and Gmail is in an Edge window, use that window. If they say "the browser" and Chrome is open, use Chrome. Prefer what is already open over launching something new.

**Use the first of these that can do the job:**

1. **PowerShell, for anything with no window to look at.** Files and folders, reading system state, running programs. Your shell tool takes PowerShell commands: `Get-ChildItem`, `Rename-Item`, `Get-Process`, `Get-Clipboard`, `Invoke-RestMethod`.
   - Open an app with `Start-Process notepad`, a file or folder with `Invoke-Item`, a Settings page with `Start-Process ms-settings:display`. Juno gives the foreground back to the person afterwards, so the app opens behind what they're using.
   - Find installed apps with `Get-StartApps`. There are no Mac apps here: mail is Outlook or a site, files are in File Explorer, settings are in Windows Settings.
   - `Remove-Item` is permanent: it skips the Recycle Bin. Say what would be lost before you delete anything you didn't create.

2. **The `computer` tool, by element, for anything in a window.** List a window's controls and act on them by id. It works on a window that is covered by another one, and it doesn't move the person's cursor.
   ```
   {"action": "elements", "window": "Notepad"}
   {"action": "left_click", "element": "e7"}
   {"action": "type", "element": "e3", "text": "hello"}
   ```
   Read results from the values in the listing. Name the window on every action until it's your working window.

3. **Browser tools, for the web.** The `browser_` tools drive Chrome and Edge directly and are faster and more exact than clicking on a page.

4. **Screenshots and coordinates, only when a window lists no controls.** Take the screenshot with `window` so it shows just that window, and click by coordinates in that picture. A full-screen screenshot shows what the person is looking at, so use one only when they refer to something on screen.

**Shortcuts** use `ctrl` where a Mac would use command: `ctrl+c`, `ctrl+s`. A shortcut with the Windows key needs real input and takes the foreground, so find another way.

**A window running as administrator can't be used.** Windows blocks other apps from it. When a result says so, tell the person "That window is running as administrator, so I can't use it." Then stop trying it.

**If an action did nothing,** list the window's elements again and read what changed. Don't repeat it blind, and don't report it as done."#;

/// What the observation fragment promises on a Mac and Windows doesn't do yet:
/// no `app_state` comes back from a launch here, so the agent looks for itself.
const WINDOWS_OBSERVATION: &str = "**Look for yourself.** A launch or a change comes back with nothing about what happened on screen. Check with `list_visible_windows` or the `computer` tool's `elements` action, and if you did not see it, do not tell the person it is open on their screen.";

/// Text cut from the first marker through the second, for a passage too long to
/// quote in [`PHRASES`].
const OBSERVATION_START: &str = "**Juno looks for you.**";
const OBSERVATION_END: &str = "Never upgrade `unknown` into success.";

/// Exact Mac phrases and what they say on Windows. A test fails when a phrase
/// here stops appearing in the prompts, so an edit to a Mac prompt shows up as
/// a Windows wording to revisit rather than as a Mac word left behind.
const PHRASES: &[(&str, &str)] = &[
    (
        "computer tasks on macOS through voice interaction",
        "computer tasks on Windows through voice interaction",
    ),
    (
        "`open -a \"Spotify\"` exits 0",
        "`Start-Process spotify` exits 0",
    ),
    ("Want me to try Apple Music instead?", "Want me to try it in the browser instead?"),
    ("a real app or site on this Mac", "a real app or site on this computer"),
    (
        "installed apps (`ls /Applications /System/Applications`), then a site in the browser. One obvious place (chess: /System/Applications/Chess.app): open it and go, no asking. None, or several equal ones: ask one short question with options, e.g. \"Chess.app isn't here. Play on chess.com or lichess.org?\"",
        "installed apps (`Get-StartApps`), then a site in the browser. One obvious place: open it and go, no asking. None, or several equal ones: ask one short question with options, e.g. \"There's no chess app here. Play on chess.com or lichess.org?\"",
    ),
    (
        "use bash to call a free weather API with the coordinates from your context:\n  `curl -s \"https://api.open-meteo.com",
        "use PowerShell to call a free weather API with the coordinates from your context:\n  `Invoke-RestMethod \"https://api.open-meteo.com",
    ),
    (
        "**NATIVE ACCESSIBILITY TOOLS** - macOS Element-Level",
        "**NATIVE ACCESSIBILITY TOOLS** - Windows Element-Level",
    ),
    (
        "You have access to native macOS accessibility tools",
        "You have access to native Windows accessibility tools",
    ),
    (
        "These tools leverage macOS accessibility APIs",
        "These tools leverage Windows UI Automation",
    ),
    ("app_name: \"TextEdit\" ", "app_name: \"Notepad\"  "),
    (
        "Direct macOS accessibility framework integration",
        "Direct Windows UI Automation integration",
    ),
    ("- System Preferences configuration", "- Windows Settings configuration"),
    ("- Finder file operations", "- File Explorer operations"),
    ("- **Use with safari tools** for web-specific interactions\n", ""),
    (
        "the most reliable element interaction on macOS",
        "the most reliable element interaction on Windows",
    ),
    ("why AppleScript instead of clicking.", "why PowerShell instead of clicking."),
    ("a disk that is 94% full on a Mac with", "a disk that is 94% full on a computer with"),
    (
        "| ✅ `<NowPlayingCard>` — never hand-rolled buttons |",
        "| skip: say it in one line, never hand-rolled buttons |",
    ),
    (
        "It is NOT for playback control (use `<NowPlayingCard>`) and never for",
        "It is never for",
    ),
    (
        "**Playback example** (correct — live card, honest text):\n```xml\n<TTS>Paused.</TTS>\n\n<NowPlayingCard app=\"Spotify\" />\n```",
        "**Playback example** (correct — honest text):\n```xml\n<TTS>Paused.</TTS>\n```",
    ),
    (
        "<TTS>Playing. Midnight by 1991.</TTS>\n\n<NowPlayingCard app=\"Spotify\" />\n",
        "<TTS>Playing. Midnight by 1991.</TTS>\n",
    ),
    (
        "implies live state unless it is a live component (`<NowPlayingCard>`).",
        "implies live state.",
    ),
    (
        "- We're on mac, you can use stickies, notes, textedit, etc.",
        "- We're on Windows, you can use Notepad, Sticky Notes, etc.",
    ),
    ("(Open in Preview)", "(Open)"),
    ("opens file/folder in Finder", "opens file/folder in File Explorer"),
    (
        "\"AppleScript instead of clicking because…\"",
        "\"PowerShell instead of clicking because…\"",
    ),
    (
        "Spotify was already running, so `osascript` hit its scripting dictionary directly — no screenshot, no coordinate guessing, no stolen focus from your editor.",
        "Spotify was already running, so I pressed its play button by element, with no screenshot, no coordinate guessing and no stolen focus from your editor.",
    ),
    ("**Why AppleScript here:**", "**Why PowerShell here:**"),
    ("(why AppleScript, why not clicking,", "(why PowerShell, why not clicking,"),
    (
        "registered on the user's macOS for that file type",
        "registered on the user's computer for that file type",
    ),
    ("`\"cmd+c\"`", "`\"ctrl+c\"`"),
    ("`\"shift\"`, `\"cmd\"`, `\"ctrl\"`", "`\"shift\"`, `\"win\"`, `\"ctrl\"`"),
    ("Always prefer: AppleScript, keyboard", "Always prefer: PowerShell, keyboard"),
    (
        "[If screenshot fails due to permissions]\n\n<TTS>I couldn't take a screenshot. Juno needs permission to record the screen first.</TTS>\n\nHere's how to turn it on:\n1. Open System Settings → Privacy & Security\n2. Click \"Screen & System Audio Recording\"\n3. Turn on Juno",
        "[If the window is running as administrator]\n\n<TTS>That window is running as administrator, so I can't use it.</TTS>\n\nHere's how to fix it:\n1. Close the app\n2. Open it again without \"Run as administrator\"",
    ),
    ("Mail (not visible, not frontmost)", "Outlook (not visible, not frontmost)"),
    ("use that instead of launching Mail", "use that instead of launching Outlook"),
    ("<TTS>Searching in Safari.</TTS>", "<TTS>Searching in Edge.</TTS>"),
    (
        "Check visible_windows: Safari is open and frontmost",
        "Check visible_windows: Edge is open and frontmost",
    ),
    ("use Safari directly", "use Edge directly"),
    ("safari_navigate to search URL", "browser_navigate to search URL"),
    ("- Safari-optimized automation for enhanced performance\n", ""),
    (
        "**Native macOS Integration**: Leverage",
        "**Native Windows Integration**: Leverage",
    ),
];

/// Lines replaced whole, because the card they describe polls Mac players and
/// the line is mostly its details. An empty replacement removes the line.
const LINES: &[(&str, &str)] = &[
    (
        "- `<NowPlayingCard app=\"Spotify\" />` — live playback card",
        "- There is no live playback card on Windows. After a playback request, say what's playing in one line.",
    ),
    ("  - `<NowPlayingCard app=\"Spotify\" />` — Spotify, or", ""),
];

/// A finished prompt as this platform should read it.
pub fn adapt(prompt: String) -> String {
    if cfg!(target_os = "windows") {
        for_windows(&prompt)
    } else {
        prompt
    }
}

/// A Mac prompt rewritten for Windows. Changing nothing a second time, so it's
/// safe on a prompt that has already been through it.
pub(super) fn for_windows(prompt: &str) -> String {
    let mut text = prompt
        .replace(
            PromptFragments::accessibility_first_strategy(),
            WINDOWS_STRATEGY,
        )
        .replace(PromptFragments::safari_browser_automation(), "");
    if let (Some(start), Some(end)) = (text.find(OBSERVATION_START), text.find(OBSERVATION_END)) {
        if start < end {
            text.replace_range(start..end + OBSERVATION_END.len(), WINDOWS_OBSERVATION);
        }
    }
    for (mac, windows) in PHRASES {
        text = text.replace(mac, windows);
    }
    replace_lines(&text)
}

/// Applies [`LINES`], keeping every other line as it was.
fn replace_lines(text: &str) -> String {
    let mut out = Vec::new();
    for line in text.split('\n') {
        match LINES.iter().find(|(start, _)| line.starts_with(start)) {
            Some((_, "")) => {}
            Some((_, replacement)) => out.push(*replacement),
            None => out.push(line),
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::super::templates::DefaultPrompts;
    use super::*;

    /// Every prompt, in its Mac form on a Mac. On Windows `get_all` has already
    /// adapted these, and adapting twice changes nothing.
    fn prompts() -> Vec<(String, String)> {
        DefaultPrompts::get_all()
            .into_values()
            .map(|template| (template.id, template.content))
            .collect()
    }

    /// Lowercase, because the mistake reads the same in any case.
    const MAC_WORDS: &[&str] = &[
        "macos",
        "osascript",
        "applescript",
        "finder",
        "safari",
        "spotlight",
        "/applications",
        "open -a",
        "textedit",
        "system settings",
        "system preferences",
        "system events",
        "apple music",
        "this mac",
        "on a mac",
        "cmd+",
        "\"cmd\"",
        "chess.app",
        "app_state",
        "use bash",
        "nowplayingcard",
        "on mac",
    ];

    #[test]
    fn a_windows_prompt_has_no_mac_left_in_it() {
        let mut left = std::collections::BTreeSet::new();
        for (_, prompt) in prompts() {
            for line in for_windows(&prompt).lines() {
                let lower = line.to_lowercase();
                if MAC_WORDS.iter().any(|word| lower.contains(word)) {
                    left.insert(line.to_string());
                }
            }
        }
        assert!(left.is_empty(), "still Mac: {left:#?}");
    }

    #[test]
    fn a_windows_prompt_says_how_windows_works() {
        let windows = for_windows(&DefaultPrompts::system_default().content);
        for needle in [
            "PowerShell",
            "Start-Process",
            "File Explorer",
            "Get-StartApps",
            "running as administrator, so I can't use it.",
            // The rules that hold everywhere stay.
            "ACT IN THE REAL COMPUTER",
            "Never draw a board or game as ASCII",
            "**Never report a step as done because the tool call was accepted.**",
        ] {
            assert!(windows.contains(needle), "missing {needle:?}");
        }
    }

    /// A Mac prompt that changed under a phrase would otherwise leave Mac wording
    /// on Windows until someone noticed.
    #[test]
    fn every_phrase_still_appears_in_a_mac_prompt() {
        let all: String = prompts()
            .into_iter()
            .map(|(_, prompt)| prompt)
            .collect::<Vec<_>>()
            .join("\n");
        if cfg!(target_os = "windows") {
            return;
        }
        for (mac, _) in PHRASES.iter().chain(LINES) {
            assert!(all.contains(mac), "no prompt contains {mac:?}");
        }
        assert!(all.contains(OBSERVATION_START));
        assert!(all.contains(OBSERVATION_END));
    }

    #[test]
    fn adapting_twice_changes_nothing() {
        let once = for_windows(&DefaultPrompts::system_default().content);
        assert_eq!(for_windows(&once), once);
    }

    /// The whole of the Mac guarantee: `adapt` doesn't touch a Mac prompt, and
    /// the default prompt is still its fragments in order.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_mac_prompt_is_untouched() {
        let prompt = "osascript on macOS".to_string();
        assert_eq!(adapt(prompt.clone()), prompt);

        let expected = [
            PromptFragments::core_personality(),
            PromptFragments::honest_observation(),
            PromptFragments::act_in_the_real_computer(),
            PromptFragments::chain_of_thought_framework(),
            PromptFragments::multishot_examples(),
            PromptFragments::response_prefilling_patterns(),
            PromptFragments::tts_speech_format(),
            PromptFragments::tool_batching_optimization(),
            PromptFragments::official_computer_use_api(),
            PromptFragments::accessibility_first_strategy(),
            PromptFragments::native_accessibility_tools(),
            PromptFragments::safari_browser_automation(),
            PromptFragments::mcp_capabilities(),
            PromptFragments::jsx_capabilities(),
            PromptFragments::macos_file_handling(),
        ]
        .join("\n\n");
        assert_eq!(DefaultPrompts::system_default().content, expected);
    }
}
