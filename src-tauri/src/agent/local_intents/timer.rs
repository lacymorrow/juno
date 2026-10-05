//! Countdown timers: "set a timer for 5 minutes", "how much time is left?",
//! "cancel the timer".
//!
//! Timers are owned by the backend: each one is a task on the Tauri runtime,
//! held in a process-wide list so it can be read and cancelled. When one
//! finishes Juno speaks and posts a notification, with or without any window
//! open. Timers do not survive a quit; that is stated, not hidden, in the
//! plan doc.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use regex::Regex;
use tauri::AppHandle;

use super::utterance::number_value;
use super::{compile, Reply};

/// Longest timer accepted locally. Anything longer is a reminder, which is
/// the agent's (and the scheduler's) job.
const MAX_TIMER_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerIntent {
    Start {
        secs: u64,
    },
    Cancel,
    /// `names_timer` is false for "how much time is left?", which is only
    /// about a timer when one is running.
    Remaining {
        names_timer: bool,
    },
}

struct Patterns {
    start_for: Regex,
    start_prefix: Regex,
    cancel: Regex,
    remaining_timer: Regex,
    remaining_bare: Regex,
}

/// One or more "<number> <unit>" pairs.
const DURATION: &str = r"((?:\S+ (?:hours?|hrs?|minutes?|mins?|seconds?|secs?) ?)+)";

impl Patterns {
    fn compile() -> Result<Self, regex::Error> {
        Ok(Self {
            start_for: Regex::new(&format!(
                r"^(?:(?:set|start|make|create) )?timer (?:for )?{}$",
                DURATION
            ))?,
            start_prefix: Regex::new(&format!(
                r"^(?:(?:set|start|make|create) )?{}timer$",
                DURATION
            ))?,
            cancel: Regex::new(
                r"^(?:cancel|stop|delete|clear|remove|end|kill|turn off) (?:all )?timers?$",
            )?,
            remaining_timer: Regex::new(
                r"^(?:how much time is left on timer|how much time left on timer|how long is left on timer|how long left on timer|time left on timer|how is timer doing|check timer|timer status|how long until timer (?:goes off|is done|ends)|how much longer on timer)$",
            )?,
            remaining_bare: Regex::new(
                r"^(?:how much time is left|how much time left|how long is left|how much longer)$",
            )?,
        })
    }
}

static PATTERNS: Lazy<Option<Patterns>> = Lazy::new(|| compile("timer", Patterns::compile));

/// Sum "<number> <unit>" pairs into seconds. `None` when any pair is not a
/// number followed by a unit, or the total is out of range.
pub fn parse_duration(text: &str) -> Option<u64> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let mut total: u64 = 0;
    for pair in tokens.chunks(2) {
        // A trailing odd token leaves a one-element chunk, which fails here.
        let [amount, unit] = pair else {
            return None;
        };
        let amount = u64::from(number_value(amount)?);
        let per = match *unit {
            "hour" | "hours" | "hr" | "hrs" => 3600,
            "minute" | "minutes" | "min" | "mins" => 60,
            "second" | "seconds" | "sec" | "secs" => 1,
            _ => return None,
        };
        total = total.checked_add(amount.checked_mul(per)?)?;
    }
    (1..=MAX_TIMER_SECS).contains(&total).then_some(total)
}

pub fn parse(utterance: &str) -> Option<TimerIntent> {
    let p = PATTERNS.as_ref()?;
    if let Some(caps) = p
        .start_for
        .captures(utterance)
        .or_else(|| p.start_prefix.captures(utterance))
    {
        let secs = caps.get(1).and_then(|m| parse_duration(m.as_str()))?;
        return Some(TimerIntent::Start { secs });
    }
    if p.cancel.is_match(utterance) {
        return Some(TimerIntent::Cancel);
    }
    if p.remaining_timer.is_match(utterance) {
        return Some(TimerIntent::Remaining { names_timer: true });
    }
    if p.remaining_bare.is_match(utterance) {
        return Some(TimerIntent::Remaining { names_timer: false });
    }
    None
}

/// "5 minutes", "1 hour and 30 minutes", "45 seconds".
pub fn spoken_duration(secs: u64) -> String {
    let parts: Vec<String> = [
        (secs / 3600, "hour"),
        ((secs % 3600) / 60, "minute"),
        (secs % 60, "second"),
    ]
    .iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, unit)| format!("{} {}{}", n, unit, if *n == 1 { "" } else { "s" }))
    .collect();
    match parts.as_slice() {
        [] => "0 seconds".to_string(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {}", init.join(", "), last),
    }
}

/// "5:00", "1:30:00", "0:45".
pub fn clock(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}

fn timer_card(secs: u64, status: &str) -> String {
    format!(
        "<TimerCard label=\"Timer\" duration=\"{}\" status=\"{}\" />",
        clock(secs),
        status
    )
}

struct RunningTimer {
    id: u64,
    ends_at: Instant,
    task: tauri::async_runtime::JoinHandle<()>,
}

static TIMERS: Lazy<Mutex<Vec<RunningTimer>>> = Lazy::new(|| Mutex::new(Vec::new()));
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn with_timers<T>(f: impl FnOnce(&mut Vec<RunningTimer>) -> T) -> Option<T> {
    match TIMERS.lock() {
        Ok(mut guard) => Some(f(&mut guard)),
        Err(e) => {
            log::error!("local_intents: timer list lock poisoned: {}", e);
            None
        }
    }
}

fn start(app_handle: &AppHandle, secs: u64) -> Reply {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let app = app_handle.clone();
    let task = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(secs)).await;
        with_timers(|timers| timers.retain(|t| t.id != id));
        let message = format!("Your {} timer is done.", spoken_duration(secs));
        log::info!("local_intents: timer {} finished ({}s)", id, secs);
        crate::commands::notifications::notify(&app, "Timer done", &message);
        crate::tts::enqueue_speech_always(message, app);
    });
    let stored = with_timers(|timers| {
        timers.push(RunningTimer {
            id,
            ends_at: Instant::now() + Duration::from_secs(secs),
            task,
        })
    });
    if stored.is_none() {
        return Reply::failure("I couldn't set the timer.");
    }
    Reply::card(
        timer_card(secs, "running"),
        format!("Timer set for {}.", spoken_duration(secs)),
    )
}

fn cancel() -> Reply {
    let cancelled = with_timers(|timers| {
        let n = timers.len();
        for t in timers.drain(..) {
            t.task.abort();
        }
        n
    });
    match cancelled {
        None => Reply::failure("I couldn't cancel the timer."),
        Some(0) => Reply::text("No timer is running."),
        Some(1) => Reply::text("Timer cancelled."),
        Some(n) => Reply::text(format!("Cancelled {} timers.", n)),
    }
}

/// Seconds left on the timer that ends soonest, and how many are running.
fn soonest() -> Option<(u64, usize)> {
    with_timers(|timers| {
        let now = Instant::now();
        let left = timers
            .iter()
            .map(|t| t.ends_at.saturating_duration_since(now))
            .min()?;
        // Round up so a timer never reads "0 seconds left" while running.
        let secs = left.as_secs() + u64::from(left.subsec_nanos() > 0);
        Some((secs, timers.len()))
    })
    .flatten()
}

fn remaining(names_timer: bool) -> Option<Reply> {
    match soonest() {
        Some((secs, count)) => {
            let spoken = if count > 1 {
                format!(
                    "{} left on the next of {} timers.",
                    spoken_duration(secs),
                    count
                )
            } else {
                format!("{} left.", spoken_duration(secs))
            };
            Some(Reply::card(timer_card(secs, "running"), spoken))
        }
        // "How much time is left?" with no timer is about something else.
        None if !names_timer => None,
        None => Some(Reply::text("No timer is running.")),
    }
}

pub(super) fn handle(app_handle: &AppHandle, intent: TimerIntent) -> Option<Reply> {
    match intent {
        TimerIntent::Start { secs } => Some(start(app_handle, secs)),
        TimerIntent::Cancel => Some(cancel()),
        TimerIntent::Remaining { names_timer } => remaining(names_timer),
    }
}

#[cfg(test)]
mod tests {
    use super::super::utterance::normalize;
    use super::*;

    fn p(q: &str) -> Option<TimerIntent> {
        normalize(q).and_then(|u| parse(&u))
    }

    #[test]
    fn start_phrasings() {
        for (q, secs) in [
            ("set a timer for 5 minutes", 300),
            ("Set a timer for five minutes.", 300),
            ("timer for 10 min", 600),
            ("start a 5 minute timer", 300),
            ("5-minute timer", 300),
            ("set a timer for a minute", 60),
            ("set a timer for half an hour", 1800),
            ("timer 90 seconds", 90),
            ("hey juno set a timer for 1 hour 30 minutes please", 5400),
            ("set a 2 hour timer", 7200),
        ] {
            assert_eq!(p(q), Some(TimerIntent::Start { secs }), "{:?}", q);
        }
    }

    #[test]
    fn cancel_and_remaining() {
        assert_eq!(p("cancel the timer"), Some(TimerIntent::Cancel));
        assert_eq!(p("stop the timer"), Some(TimerIntent::Cancel));
        assert_eq!(p("cancel all timers"), Some(TimerIntent::Cancel));
        assert_eq!(
            p("how much time is left on the timer?"),
            Some(TimerIntent::Remaining { names_timer: true })
        );
        assert_eq!(
            p("check my timer"),
            Some(TimerIntent::Remaining { names_timer: true })
        );
        assert_eq!(
            p("how much time is left"),
            Some(TimerIntent::Remaining { names_timer: false })
        );
    }

    #[test]
    fn near_misses_reach_the_agent() {
        for q in [
            "set a timer",
            "set a timer for 5",
            "set a timer for 5 minutes and 30 seconds",
            "set a timer for 5 minutes to check the oven",
            "remind me in 5 minutes",
            "set a timer for 3 days",
            "set a timer for 0 minutes",
            "how do I set a timer on my iPhone",
            "pause the timer",
            "cancel the meeting",
            "timer",
            "how much time is left in the game",
        ] {
            assert_eq!(p(q), None, "{:?} should reach the agent", q);
        }
    }

    #[test]
    fn durations_read_and_speak() {
        assert_eq!(parse_duration("1 hour 30 minutes"), Some(5400));
        assert_eq!(parse_duration("25 hours"), None);
        assert_eq!(parse_duration("5"), None);
        assert_eq!(spoken_duration(300), "5 minutes");
        assert_eq!(spoken_duration(60), "1 minute");
        assert_eq!(spoken_duration(5400), "1 hour and 30 minutes");
        assert_eq!(spoken_duration(3725), "1 hour, 2 minutes and 5 seconds");
        assert_eq!(clock(300), "5:00");
        assert_eq!(clock(45), "0:45");
        assert_eq!(clock(5400), "1:30:00");
    }

    #[test]
    fn card_uses_the_existing_timer_card() {
        assert_eq!(
            timer_card(300, "running"),
            "<TimerCard label=\"Timer\" duration=\"5:00\" status=\"running\" />"
        );
    }

    #[test]
    fn remaining_without_a_timer_falls_through_unless_the_timer_was_named() {
        // No timers are started in unit tests (they need a Tauri runtime).
        assert!(soonest().is_none());
        assert!(remaining(false).is_none());
        assert!(remaining(true).is_some());
    }
}
