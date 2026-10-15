//! Timers belong to Clock: "set a timer for 5 minutes" opens Clock on its
//! Timers tab and says so.
//!
//! Juno used to run its own timers here, in a process-wide list of Tauri
//! tasks that spoke and posted a notification on expiry and died on quit.
//! They are gone (Lacy, 2026-10-12): Juno drives the Mac's own apps and
//! features and never rebuilds one, and a timer that vanishes when the app
//! quits is a worse timer than the one already on the Mac.
//!
//! # Why opening the tab is the whole feature
//!
//! Clock.app ships no scripting dictionary (`sdef` returns -192 and
//! `OSAScriptingDefinition` is absent from its `Info.plist`), so there is no
//! way to start a Clock timer or read one back. Its only automation surface
//! is `WorldClockWidget.OpenTabIntent`, which declares
//! `com.apple.link.systemProtocol.URLRepresentable` -- and LaunchServices
//! shows what that URL is: Clock claims `clock-timer:` along with
//! `clock-alarm:`, `clock-stopwatch:` and `clock-worldclock:`. Opening
//! `clock-timer:` selects the Timers tab, verified on macOS 26 on 2026-10-14
//! by reading the toolbar back over the accessibility API (Timers was the
//! radio button with value 1).
//!
//! # The duration is deliberately not parsed
//!
//! Nothing can carry it. `clock-timer://300`, `clock-timer:300` and
//! `clock-timer://?duration=300` were all tried on the same Mac and every one
//! opens the tab and ignores the rest. So a number the person said cannot
//! reach Clock, and reading one here only to throw it away would be
//! pretending. The duration shape still appears in the patterns below,
//! because it is what tells a timer command apart from a sentence that merely
//! contains the word, but no value is ever taken out of it.

use once_cell::sync::Lazy;
use regex::Regex;
use tauri::AppHandle;

use super::{compile, run, Reply};

/// The URL that opens Clock on its Timers tab. See the module docs for how
/// this scheme was found and verified.
const TIMERS_URL: &str = "clock-timer:";

/// What Juno says. One sentence: the Timers tab is on screen by the time it
/// lands, and the tab shows the timers and the Start button better than a
/// second sentence could.
const SPOKEN: &str = "Clock is open on Timers.";

/// A timer request. Every phrasing lands in the same place now, because
/// everything Juno can do about a timer is put the person on Clock's Timers
/// tab. The enum stays so the intent still reads as a timer intent in
/// [`super::LocalIntent`] and in the served-intent log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerIntent {
    OpenTimers,
}

struct Patterns {
    start_verb: Regex,
    start_bare: Regex,
    cancel: Regex,
    status: Regex,
}

/// One or more "<number> <unit>" pairs. A shape, not a value: it keeps "set a
/// timer for 3 days" and "set a timer for 5" away from Clock, since neither
/// is a duration this grammar recognizes and both are better answered by the
/// agent.
const DURATION: &str = r"(?:\S+ (?:hours?|hrs?|minutes?|mins?|seconds?|secs?) ?)+";

impl Patterns {
    fn compile() -> Result<Self, regex::Error> {
        Ok(Self {
            // "set a timer", "set a timer for 5 minutes", "start a 2 hour timer".
            // The duration is optional on purpose: with nothing to pass to
            // Clock, requiring a number the person then watches get discarded
            // would be a leftover from when Juno owned the countdown.
            start_verb: Regex::new(&format!(
                r"^(?:set|start|make|create) (?:{} )?timers?(?: for {})?$",
                DURATION, DURATION
            ))?,
            // The same request without a verb: "timer for 10 min", "5 minute
            // timer". A bare "timer" is not included; one word is as likely
            // to be a fragment as a command, so it goes to the agent.
            start_bare: Regex::new(&format!(
                r"^(?:timers? (?:for )?{}|{} timers?)$",
                DURATION, DURATION
            ))?,
            cancel: Regex::new(
                r"^(?:cancel|stop|delete|clear|remove|end|kill|turn off) (?:all )?timers?$",
            )?,
            // Only phrasings that name the timer. A bare "how much time is
            // left?" used to be answered when one of Juno's own timers
            // happened to be running; with no timer state to key on it is
            // just as likely to be about a game or a meeting, so it is no
            // longer treated as a timer command at all.
            status: Regex::new(
                r"^(?:how much time is left on timer|how much time left on timer|how long is left on timer|how long left on timer|time left on timer|how is timer doing|check timer|timer status|how long until timer (?:goes off|is done|ends)|how much longer on timer)$",
            )?,
        })
    }
}

static PATTERNS: Lazy<Option<Patterns>> = Lazy::new(|| compile("timer", Patterns::compile));

pub fn parse(utterance: &str) -> Option<TimerIntent> {
    let p = PATTERNS.as_ref()?;
    let names_a_timer = p.start_verb.is_match(utterance)
        || p.start_bare.is_match(utterance)
        || p.cancel.is_match(utterance)
        || p.status.is_match(utterance);
    names_a_timer.then_some(TimerIntent::OpenTimers)
}

pub(super) async fn handle(app_handle: &AppHandle, intent: TimerIntent) -> Option<Reply> {
    let TimerIntent::OpenTimers = intent;
    // Speak first; the acknowledgement does not depend on the result, and the
    // person hears it while the window is still coming up.
    super::speak_ahead(app_handle, SPOKEN);
    Some(match run("open", vec![TIMERS_URL.into()]).await {
        Ok(_) => Reply::text(SPOKEN).unspoken(),
        Err(e) => {
            log::warn!("local_intents: open {} failed: {}", TIMERS_URL, e);
            Reply::failure("I couldn't open Clock.").unspoken()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::utterance::normalize;
    use super::*;

    fn p(q: &str) -> Option<TimerIntent> {
        normalize(q).and_then(|u| parse(&u))
    }

    #[test]
    fn every_timer_phrasing_opens_clock() {
        for q in [
            // Starting one, with and without a duration.
            "set a timer",
            "set a timer for 5 minutes",
            "Set a timer for five minutes.",
            "timer for 10 min",
            "start a 5 minute timer",
            "5-minute timer",
            "set a timer for a minute",
            "set a timer for half an hour",
            "timer 90 seconds",
            "hey juno set a timer for 1 hour 30 minutes please",
            "set a 2 hour timer",
            // Cancelling one.
            "cancel the timer",
            "stop the timer",
            "cancel all timers",
            // Asking after one by name.
            "how much time is left on the timer?",
            "check my timer",
            "timer status",
        ] {
            assert_eq!(p(q), Some(TimerIntent::OpenTimers), "{:?}", q);
        }
    }

    #[test]
    fn near_misses_reach_the_agent() {
        for q in [
            // Not a duration this grammar knows, so the agent can say what
            // Clock actually supports.
            "set a timer for 3 days",
            "set a timer for 5",
            // A labelled timer. Clock takes no label through the URL, so
            // sending the person to a blank Timers tab would lose the part
            // they cared about.
            "set a timer for 5 minutes to check the oven",
            // A question about a timer, not a request for one.
            "how do I set a timer on my iPhone",
            // One word is as likely a fragment as a command.
            "timer",
            // Reminders' job, not Clock's.
            "remind me in 5 minutes",
            // Two clauses; normalize refuses these outright.
            "set a timer for 5 minutes and 30 seconds",
            "cancel the meeting",
            // No timer is named, so this is about whatever else is going on.
            "how much time is left",
            "how much time is left in the game",
            "how much longer",
        ] {
            assert_eq!(p(q), None, "{:?} should reach the agent", q);
        }
    }

    /// Juno opens the Timers tab and nothing else. A regression here would
    /// most likely be a well-meant "improvement" that passes the duration
    /// along; see the module docs for why no such URL works.
    #[test]
    fn the_url_is_the_bare_timers_tab() {
        assert_eq!(TIMERS_URL, "clock-timer:");
    }
}
