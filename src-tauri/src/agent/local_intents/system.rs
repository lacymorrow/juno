//! Mac system controls and facts: volume, dark mode, lock, display and Mac
//! sleep, battery, time and date.
//!
//! Every script here is a constant or is built from a validated integer.
//! Nothing the person said is ever spliced into AppleScript.

use std::time::Duration;

use chrono::{Local, NaiveDate, NaiveTime};
use once_cell::sync::Lazy;
use regex::Regex;
use tauri::AppHandle;

use super::{compile, osascript, run, Reply};

/// Volume change for "turn it up / down", in percent of output volume.
const VOLUME_STEP: i32 = 10;

/// How long a sleep command waits so the spoken confirmation can start first.
const SLEEP_GRACE: Duration = Duration::from_millis(1200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    On,
    Off,
    Flip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemIntent {
    VolumeUp,
    VolumeDown,
    VolumeSet(u8),
    Mute,
    Unmute,
    VolumeQuery,
    DarkMode(Toggle),
    LockScreen,
    SleepDisplay,
    SleepMac,
    Battery,
    Time,
    Date,
}

struct Patterns {
    volume_up: Regex,
    volume_down: Regex,
    volume_set: Regex,
    mute: Regex,
    unmute: Regex,
    volume_query: Regex,
    dark_on: Regex,
    dark_off: Regex,
    dark_flip: Regex,
    lock: Regex,
    sleep_display: Regex,
    sleep_mac: Regex,
    battery: Regex,
    time: Regex,
    date: Regex,
}

impl Patterns {
    fn compile() -> Result<Self, regex::Error> {
        Ok(Self {
            volume_up: Regex::new(
                r"^(?:(?:turn|crank|bump) (?:volume|sound) up|(?:turn|crank|bump) up (?:volume|sound)|(?:volume|sound) up|(?:raise|increase) (?:volume|sound))(?: (?:bit|little|little bit|some))?$",
            )?,
            volume_down: Regex::new(
                r"^(?:turn (?:volume|sound) down|turn down (?:volume|sound)|(?:volume|sound) down|(?:lower|decrease|reduce) (?:volume|sound))(?: (?:bit|little|little bit|some))?$",
            )?,
            volume_set: Regex::new(
                r"^(?:(?:set|turn|put|change) )?(?:volume|sound)(?: level)? (?:to |at )?(?:(\d{1,3})%?|([a-z]+))(?: percent)?$",
            )?,
            mute: Regex::new(
                r"^(?:(?:mute|silence) (?:volume|sound|audio|mac|computer|speakers?)|(?:turn|switch) (?:volume|sound|audio) off|(?:turn|switch) off (?:volume|sound|audio))$",
            )?,
            unmute: Regex::new(
                r"^(?:unmute (?:volume|sound|audio|mac|computer|speakers?)|(?:turn|switch) (?:volume|sound|audio) (?:back )?on|(?:turn|switch) on (?:volume|sound|audio))$",
            )?,
            volume_query: Regex::new(
                r"^(?:what is (?:volume|volume level|sound level)(?: at)?|what volume is it(?: at)?)$",
            )?,
            dark_on: Regex::new(
                r"^(?:(?:turn|switch) on dark mode|(?:turn|switch) dark mode on|dark mode on|enable dark mode|(?:switch|change|go) to dark mode|(?:turn|switch) off light mode)$",
            )?,
            dark_off: Regex::new(
                r"^(?:(?:turn|switch) off dark mode|(?:turn|switch) dark mode off|dark mode off|disable dark mode|(?:switch|change|go) to light mode|(?:turn|switch) on light mode|light mode on|enable light mode)$",
            )?,
            dark_flip: Regex::new(r"^(?:toggle|switch|flip) (?:dark|light) mode$")?,
            lock: Regex::new(
                r"^lock (?:screen|mac|computer|laptop|macbook|mac screen|computer screen)$",
            )?,
            sleep_display: Regex::new(
                r"^(?:(?:turn|switch) off (?:screen|display|monitor)s?|(?:turn|switch) (?:screen|display|monitor)s? off|(?:put )?(?:screen|display|monitor)s? to sleep|sleep (?:screen|display|monitor)s?)$",
            )?,
            sleep_mac: Regex::new(
                r"^(?:put (?:mac|computer|laptop|macbook) to sleep|sleep (?:mac|computer|laptop|macbook))$",
            )?,
            battery: Regex::new(
                r"^(?:battery(?: level| status| percentage| life)?|how much battery(?: is left| do i have(?: left)?| left)?|what is battery(?: level| at| percentage| status)?|how is battery(?: doing)?|is (?:mac|laptop|computer|macbook) charging)$",
            )?,
            time: Regex::new(
                r"^(?:what time is it|what is time|tell me time|what is current time|current time)$",
            )?,
            date: Regex::new(
                r"^(?:what is date(?: today)?|what is today date|what date is it(?: today)?|what day is it(?: today)?|what is today|what day is today|today date|tell me date)$",
            )?,
        })
    }
}

static PATTERNS: Lazy<Option<Patterns>> = Lazy::new(|| compile("system", Patterns::compile));

/// Parse a normalized utterance (see `utterance::normalize`).
pub fn parse(utterance: &str) -> Option<SystemIntent> {
    let p = PATTERNS.as_ref()?;
    let u = utterance;
    if p.volume_up.is_match(u) {
        return Some(SystemIntent::VolumeUp);
    }
    if p.volume_down.is_match(u) {
        return Some(SystemIntent::VolumeDown);
    }
    if p.mute.is_match(u) {
        return Some(SystemIntent::Mute);
    }
    if p.unmute.is_match(u) {
        return Some(SystemIntent::Unmute);
    }
    if let Some(caps) = p.volume_set.captures(u) {
        let level = caps
            .get(1)
            .or_else(|| caps.get(2))
            .and_then(|m| super::utterance::number_value(m.as_str()))
            .filter(|n| *n <= 100)
            .and_then(|n| u8::try_from(n).ok());
        return level.map(SystemIntent::VolumeSet);
    }
    if p.volume_query.is_match(u) {
        return Some(SystemIntent::VolumeQuery);
    }
    if p.dark_on.is_match(u) {
        return Some(SystemIntent::DarkMode(Toggle::On));
    }
    if p.dark_off.is_match(u) {
        return Some(SystemIntent::DarkMode(Toggle::Off));
    }
    if p.dark_flip.is_match(u) {
        return Some(SystemIntent::DarkMode(Toggle::Flip));
    }
    if p.lock.is_match(u) {
        return Some(SystemIntent::LockScreen);
    }
    if p.sleep_display.is_match(u) {
        return Some(SystemIntent::SleepDisplay);
    }
    if p.sleep_mac.is_match(u) {
        return Some(SystemIntent::SleepMac);
    }
    if p.battery.is_match(u) {
        return Some(SystemIntent::Battery);
    }
    if p.time.is_match(u) {
        return Some(SystemIntent::Time);
    }
    if p.date.is_match(u) {
        return Some(SystemIntent::Date);
    }
    None
}

/// AppleScript that changes output volume by `delta` percent, unmutes, and
/// returns the new level. `delta` is an integer, never user text.
fn volume_step_script(delta: i32) -> String {
    format!(
        "set cur to output volume of (get volume settings)\n\
         set newV to cur + ({})\n\
         if newV > 100 then set newV to 100\n\
         if newV < 0 then set newV to 0\n\
         set volume output volume newV\n\
         if newV > 0 then set volume output muted false\n\
         return newV",
        delta
    )
}

fn volume_set_script(level: u8) -> String {
    format!(
        "set volume output volume {}\n\
         if {} > 0 then set volume output muted false\n\
         return output volume of (get volume settings)",
        level, level
    )
}

const VOLUME_QUERY_SCRIPT: &str = "set s to get volume settings\n\
     if output muted of s then return \"muted\"\n\
     return (output volume of s) as text";

fn dark_mode_script(toggle: Toggle) -> String {
    let value = match toggle {
        Toggle::On => "true",
        Toggle::Off => "false",
        Toggle::Flip => "not dark mode",
    };
    format!(
        "tell application \"System Events\" to tell appearance preferences\n\
         set dark mode to {}\n\
         return dark mode\n\
         end tell",
        value
    )
}

/// Control-Command-Q, the system "Lock Screen" shortcut.
const LOCK_SCRIPT: &str =
    "tell application \"System Events\" to keystroke \"q\" using {control down, command down}";

fn volume_reply(output: &str) -> Reply {
    match output.trim().parse::<u32>() {
        Ok(level) => Reply::text(format!("Volume {}%.", level)),
        Err(_) => Reply::failure("I can't change the volume on this output."),
    }
}

/// "It's 3:05 PM."
pub fn spoken_time(time: NaiveTime) -> String {
    format!("It's {}.", time.format("%-I:%M %p"))
}

/// "It's Tuesday, September 29."
pub fn spoken_date(date: NaiveDate) -> String {
    format!("It's {}.", date.format("%A, %B %-d"))
}

/// Battery facts parsed from `pmset -g batt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryInfo {
    pub percent: u32,
    /// `charging`, `discharging`, `charged`, `finishing charge`, `AC attached`.
    pub state: String,
}

/// Parse `pmset -g batt`. `None` means the Mac has no internal battery.
pub fn parse_pmset_batt(output: &str) -> Option<BatteryInfo> {
    let line = output.lines().find(|l| l.contains("InternalBattery"))?;
    let (_, after_tab) = line.split_once('\t')?;
    let mut fields = after_tab.split(';').map(str::trim);
    let percent = fields.next()?.trim_end_matches('%').trim().parse().ok()?;
    let state = fields.next().unwrap_or_default().to_string();
    Some(BatteryInfo { percent, state })
}

pub fn spoken_battery(info: Option<&BatteryInfo>) -> String {
    let Some(info) = info else {
        return "This Mac doesn't have a battery.".to_string();
    };
    match info.state.as_str() {
        "charging" | "finishing charge" => {
            format!("Battery is at {}% and charging.", info.percent)
        }
        "charged" => format!("Battery is full, {}%.", info.percent),
        "AC attached" => format!("Battery is at {}%, plugged in.", info.percent),
        _ => format!("Battery is at {}%.", info.percent),
    }
}

/// Sleep the display or the Mac after a short grace period, so the spoken
/// confirmation starts before the screen goes dark.
fn sleep_later(arg: &'static str) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SLEEP_GRACE).await;
        if let Err(e) = run("pmset", vec![arg.to_string()]).await {
            log::warn!("local_intents: pmset {} failed: {}", arg, e);
        }
    });
}

pub(super) async fn handle(_app_handle: &AppHandle, intent: SystemIntent) -> Option<Reply> {
    let reply = match intent {
        SystemIntent::VolumeUp => match osascript(&volume_step_script(VOLUME_STEP), vec![]).await {
            Ok(out) => volume_reply(&out),
            Err(e) => failed("change the volume", &e),
        },
        SystemIntent::VolumeDown => {
            match osascript(&volume_step_script(-VOLUME_STEP), vec![]).await {
                Ok(out) => volume_reply(&out),
                Err(e) => failed("change the volume", &e),
            }
        }
        SystemIntent::VolumeSet(level) => {
            match osascript(&volume_set_script(level), vec![]).await {
                Ok(out) => volume_reply(&out),
                Err(e) => failed("change the volume", &e),
            }
        }
        SystemIntent::Mute => match osascript("set volume output muted true", vec![]).await {
            Ok(_) => Reply::text("Muted."),
            Err(e) => failed("mute", &e),
        },
        SystemIntent::Unmute => {
            match osascript(
                "set volume output muted false\nreturn output volume of (get volume settings)",
                vec![],
            )
            .await
            {
                Ok(out) => match out.trim().parse::<u32>() {
                    Ok(level) => Reply::text(format!("Unmuted. Volume {}%.", level)),
                    Err(_) => Reply::text("Unmuted."),
                },
                Err(e) => failed("unmute", &e),
            }
        }
        SystemIntent::VolumeQuery => match osascript(VOLUME_QUERY_SCRIPT, vec![]).await {
            Ok(out) if out.trim() == "muted" => Reply::text("Sound is muted."),
            Ok(out) => match out.trim().parse::<u32>() {
                Ok(level) => Reply::text(format!("Volume is {}%.", level)),
                Err(_) => Reply::failure("I can't read the volume on this output."),
            },
            Err(e) => failed("read the volume", &e),
        },
        SystemIntent::DarkMode(toggle) => {
            match osascript(&dark_mode_script(toggle), vec![]).await {
                Ok(out) if out.trim() == "true" => Reply::text("Dark mode on."),
                Ok(_) => Reply::text("Dark mode off."),
                Err(e) => failed("change the appearance", &e),
            }
        }
        SystemIntent::LockScreen => match osascript(LOCK_SCRIPT, vec![]).await {
            Ok(_) => Reply::text("Locked."),
            Err(e) => failed("lock the screen", &e),
        },
        SystemIntent::SleepDisplay => {
            sleep_later("displaysleepnow");
            Reply::text("Turning off the display.")
        }
        SystemIntent::SleepMac => {
            sleep_later("sleepnow");
            Reply::text("Going to sleep.")
        }
        SystemIntent::Battery => match run("pmset", vec!["-g".into(), "batt".into()]).await {
            Ok(out) => Reply::text(spoken_battery(parse_pmset_batt(&out).as_ref())),
            Err(e) => failed("read the battery", &e),
        },
        SystemIntent::Time => Reply::text(spoken_time(Local::now().time())),
        SystemIntent::Date => Reply::text(spoken_date(Local::now().date_naive())),
    };
    Some(reply)
}

fn failed(what: &str, error: &str) -> Reply {
    log::warn!("local_intents: couldn't {}: {}", what, error);
    Reply::failure(format!("I couldn't {}.", what))
}

#[cfg(test)]
mod tests {
    use super::super::utterance::normalize;
    use super::*;

    fn p(q: &str) -> Option<SystemIntent> {
        normalize(q).and_then(|u| parse(&u))
    }

    #[test]
    fn volume() {
        for q in [
            "turn up the volume",
            "turn the volume up a bit",
            "volume up",
            "Increase the volume.",
            "hey juno, crank up the sound",
        ] {
            assert_eq!(p(q), Some(SystemIntent::VolumeUp), "{:?}", q);
        }
        for q in [
            "turn down the volume",
            "volume down",
            "lower the volume please",
        ] {
            assert_eq!(p(q), Some(SystemIntent::VolumeDown), "{:?}", q);
        }
        assert_eq!(p("set the volume to 50"), Some(SystemIntent::VolumeSet(50)));
        assert_eq!(p("volume 30%"), Some(SystemIntent::VolumeSet(30)));
        assert_eq!(
            p("set volume to twenty percent"),
            Some(SystemIntent::VolumeSet(20))
        );
        assert_eq!(p("volume to 0"), Some(SystemIntent::VolumeSet(0)));
        assert_eq!(p("mute the sound"), Some(SystemIntent::Mute));
        assert_eq!(p("mute my Mac"), Some(SystemIntent::Mute));
        assert_eq!(p("unmute the volume"), Some(SystemIntent::Unmute));
        assert_eq!(p("turn the sound back on"), Some(SystemIntent::Unmute));
        assert_eq!(p("what's the volume?"), Some(SystemIntent::VolumeQuery));
        assert_eq!(p("what's the volume at"), Some(SystemIntent::VolumeQuery));
    }

    #[test]
    fn volume_near_misses_reach_the_agent() {
        for q in [
            "how do I mute Zoom?",
            "mute Zoom",
            "mute",
            "unmute",
            "mute the call",
            "turn up the volume on YouTube",
            "turn the volume down in Spotify",
            "set the volume to 150",
            "set the volume to max",
            "volume",
            "why is the volume so low",
            "turn it up",
            "louder",
            "turn up the volume and play some jazz",
        ] {
            assert_eq!(p(q), None, "{:?} should reach the agent", q);
        }
    }

    #[test]
    fn dark_mode() {
        assert_eq!(
            p("turn on dark mode"),
            Some(SystemIntent::DarkMode(Toggle::On))
        );
        assert_eq!(
            p("switch to dark mode"),
            Some(SystemIntent::DarkMode(Toggle::On))
        );
        assert_eq!(
            p("Dark mode off, please"),
            Some(SystemIntent::DarkMode(Toggle::Off))
        );
        assert_eq!(
            p("switch to light mode"),
            Some(SystemIntent::DarkMode(Toggle::Off))
        );
        assert_eq!(
            p("toggle dark mode"),
            Some(SystemIntent::DarkMode(Toggle::Flip))
        );
        for q in [
            "turn on dark mode in Slack",
            "does Safari have a dark mode",
            "dark mode",
            "how do I turn on dark mode",
            "make the website dark mode",
        ] {
            assert_eq!(p(q), None, "{:?} should reach the agent", q);
        }
    }

    #[test]
    fn lock_and_sleep() {
        assert_eq!(p("lock the screen"), Some(SystemIntent::LockScreen));
        assert_eq!(p("lock my Mac"), Some(SystemIntent::LockScreen));
        assert_eq!(p("Lock my computer."), Some(SystemIntent::LockScreen));
        assert_eq!(p("turn off the display"), Some(SystemIntent::SleepDisplay));
        assert_eq!(
            p("put the display to sleep"),
            Some(SystemIntent::SleepDisplay)
        );
        assert_eq!(p("sleep the screen"), Some(SystemIntent::SleepDisplay));
        assert_eq!(p("put my Mac to sleep"), Some(SystemIntent::SleepMac));
        for q in [
            "lock",
            "lock the file",
            "how do I lock my screen",
            "lock the screen when I walk away",
            "go to sleep",
            "sleep",
            "turn off the screen saver",
            "don't let the display sleep",
        ] {
            assert_eq!(p(q), None, "{:?} should reach the agent", q);
        }
    }

    #[test]
    fn facts() {
        assert_eq!(p("what time is it?"), Some(SystemIntent::Time));
        assert_eq!(p("What's the time"), Some(SystemIntent::Time));
        assert_eq!(p("hey juno what time is it now"), Some(SystemIntent::Time));
        assert_eq!(p("what's the date"), Some(SystemIntent::Date));
        assert_eq!(p("what's today's date?"), Some(SystemIntent::Date));
        assert_eq!(p("what day is it"), Some(SystemIntent::Date));
        assert_eq!(p("battery"), Some(SystemIntent::Battery));
        assert_eq!(
            p("how much battery do I have left"),
            Some(SystemIntent::Battery)
        );
        assert_eq!(p("what's my battery at"), Some(SystemIntent::Battery));
        assert_eq!(p("is my laptop charging"), Some(SystemIntent::Battery));
        for q in [
            "what time is it in Tokyo",
            "what time does the store close",
            "what day is Christmas",
            "what's the date of the meeting",
            "time",
            "how do I check my battery health",
            "battery replacement cost",
            "what's the time zone",
        ] {
            assert_eq!(p(q), None, "{:?} should reach the agent", q);
        }
    }

    #[test]
    fn spoken_facts() {
        let t = NaiveTime::from_hms_opt(15, 5, 0).unwrap_or_default();
        assert_eq!(spoken_time(t), "It's 3:05 PM.");
        let d = NaiveDate::from_ymd_opt(2026, 9, 29).unwrap_or_default();
        assert_eq!(spoken_date(d), "It's Tuesday, September 29.");
    }

    #[test]
    fn battery_parsing() {
        let charging = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=123)\t82%; charging; 1:02 remaining present: true\n";
        let info = parse_pmset_batt(charging);
        assert_eq!(
            info,
            Some(BatteryInfo {
                percent: 82,
                state: "charging".into()
            })
        );
        assert_eq!(
            spoken_battery(info.as_ref()),
            "Battery is at 82% and charging."
        );
        let on_battery = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=1)\t47%; discharging; 3:10 remaining present: true\n";
        assert_eq!(
            spoken_battery(parse_pmset_batt(on_battery).as_ref()),
            "Battery is at 47%."
        );
        let desktop = "Now drawing from 'AC Power'\n";
        assert_eq!(parse_pmset_batt(desktop), None);
        assert_eq!(spoken_battery(None), "This Mac doesn't have a battery.");
    }

    #[test]
    fn scripts_are_built_from_integers_only() {
        assert!(volume_step_script(-10).contains("cur + (-10)"));
        assert!(volume_set_script(40).starts_with("set volume output volume 40"));
        assert!(dark_mode_script(Toggle::Flip).contains("not dark mode"));
    }
}
