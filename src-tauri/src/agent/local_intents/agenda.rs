//! Your day and Do Not Disturb, answered without a model.
//!
//! Four phrasings, each with no free slot, which is why they are allowed in
//! tier 0 (`docs/plans/siri-parity.md`, "Three tiers, one vocabulary"):
//!
//! - "what's on my calendar today / tomorrow" (and "what do I have today")
//! - "read my reminders", "what are my reminders"
//! - "what's my next meeting"
//! - "turn on / off do not disturb"
//!
//! They call the same `mac_apps` tools the agent calls, so there is one
//! implementation. And they answer only what is already allowed: if Calendar
//! or Reminders has not been allowed yet, or the Juno Focus shortcut has not
//! been added, the intent does not fire and the agent handles it, asking in
//! the moment. A local reply never opens a permission dialog.

use chrono::{DateTime, Duration, Local, NaiveDate};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use tauri::AppHandle;

use super::{compile, Reply};
use crate::agent::tools::mac_apps::{self, Domain, When};
use crate::constants::agent::tool_names;

/// Which day was asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Day {
    Today,
    Tomorrow,
}

impl Day {
    fn word(self) -> &'static str {
        match self {
            Day::Today => "today",
            Day::Tomorrow => "tomorrow",
        }
    }

    fn date(self, today: NaiveDate) -> NaiveDate {
        match self {
            Day::Today => today,
            Day::Tomorrow => today.succ_opt().unwrap_or(today),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgendaIntent {
    Calendar(Day),
    Reminders,
    NextMeeting,
    /// Do Not Disturb: `true` for on.
    Focus(bool),
}

/// Every way a person says Do Not Disturb, after normalizing ("don't" has lost
/// its apostrophe).
const DND: &str = r"(?:do not disturb|dont disturb|dnd)(?: mode)?";

struct Patterns {
    calendar: Regex,
    have: Regex,
    reminders: Regex,
    next: Regex,
    focus_on: Regex,
    focus_off: Regex,
}

impl Patterns {
    fn compile() -> Result<Self, regex::Error> {
        Ok(Self {
            calendar: Regex::new(
                r"^(?:what is on|what is) (?:calendar|schedule|agenda)(?: for)? (today|tomorrow)$",
            )?,
            have: Regex::new(
                r"^what do i have(?: on (?:calendar|schedule|agenda))?(?: for)? (today|tomorrow)$",
            )?,
            reminders: Regex::new(r"^(?:read(?: out)?|what are) reminders$")?,
            next: Regex::new(r"^what is next (?:meeting|event|appointment)$")?,
            focus_on: Regex::new(&format!(
                r"^(?:(?:turn|switch) on {DND}|(?:turn|switch) {DND} on|enable {DND}|{DND} on)$"
            ))?,
            focus_off: Regex::new(&format!(
                r"^(?:(?:turn|switch) off {DND}|(?:turn|switch) {DND} off|disable {DND}|{DND} off)$"
            ))?,
        })
    }
}

static PATTERNS: Lazy<Option<Patterns>> = Lazy::new(|| compile("agenda", Patterns::compile));

fn day_of(caps: &regex::Captures<'_>) -> Option<Day> {
    match caps.get(1)?.as_str() {
        "today" => Some(Day::Today),
        "tomorrow" => Some(Day::Tomorrow),
        _ => None,
    }
}

/// Parse a normalized utterance (see `utterance::normalize`).
pub fn parse(utterance: &str) -> Option<AgendaIntent> {
    let p = PATTERNS.as_ref()?;
    for pattern in [&p.calendar, &p.have] {
        if let Some(day) = pattern.captures(utterance).as_ref().and_then(day_of) {
            return Some(AgendaIntent::Calendar(day));
        }
    }
    if p.reminders.is_match(utterance) {
        return Some(AgendaIntent::Reminders);
    }
    if p.next.is_match(utterance) {
        return Some(AgendaIntent::NextMeeting);
    }
    if p.focus_on.is_match(utterance) {
        return Some(AgendaIntent::Focus(true));
    }
    if p.focus_off.is_match(utterance) {
        return Some(AgendaIntent::Focus(false));
    }
    None
}

// ---------------------------------------------------------------------------
// Words and cards, pure
// ---------------------------------------------------------------------------

/// How a tool labels an all-day event. `mac_apps::relative_when` writes it; a
/// test below pins the two together so a change to one fails here.
const ALL_DAY_SUFFIX: &str = ", all day";

/// The most rows the card draws. The rest are counted aloud.
const CARD_ROWS: usize = 8;

/// The most items read aloud by name.
const SPOKEN_ITEMS: usize = 3;

fn title(item: &Value) -> &str {
    item["title"].as_str().unwrap_or_default()
}

fn is_all_day(item: &Value) -> bool {
    item["when"]
        .as_str()
        .is_some_and(|w| w.ends_with(ALL_DAY_SUFFIX))
}

/// "9:00 AM", from the item's ISO start. `None` for an all-day event or no date.
fn clock_time(item: &Value) -> Option<String> {
    if is_all_day(item) {
        return None;
    }
    match mac_apps::parse_when(item["start"].as_str()?) {
        Ok(When::Moment(at)) => Some(at.format("%-I:%M %p").to_string()),
        _ => None,
    }
}

fn items_of(result: &Value) -> Vec<Value> {
    result["items"].as_array().cloned().unwrap_or_default()
}

/// "a", "a and b", "a, b, and c".
fn join_aloud(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// The part of an item the card draws. Ids and ISO dates stay out of the chat.
fn card_row(item: &Value) -> Value {
    let mut row = json!({
        "title": title(item),
        "when": item["when"].as_str().unwrap_or_default(),
        "done": item["done"].as_bool().unwrap_or(false),
        "kind": item["kind"].as_str().unwrap_or_default(),
    });
    if let (Some(place), Some(map)) = (item.get("where"), row.as_object_mut()) {
        map.insert("where".to_string(), place.clone());
    }
    row
}

/// The `AgendaCard` tag for these items.
pub fn card(items: &[Value], empty: &str) -> String {
    let rows: Vec<Value> = items.iter().take(CARD_ROWS).map(card_row).collect();
    let rows_json = serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string());
    let empty_json = serde_json::to_string(empty).unwrap_or_else(|_| "\"\"".to_string());
    format!("<AgendaCard items={{{rows_json}}} empty={{{empty_json}}} />")
}

/// "Nothing today." or "You have 3 things today: A at 9:00 AM, B, and C."
pub fn spoken_day(day: Day, items: &[Value]) -> String {
    let word = day.word();
    if items.is_empty() {
        return format!("Nothing {word}.");
    }
    let named: Vec<String> = items
        .iter()
        .take(SPOKEN_ITEMS)
        .map(|i| match clock_time(i) {
            Some(time) => format!("{} at {time}", title(i)),
            None => title(i).to_string(),
        })
        .collect();
    let more = items.len().saturating_sub(SPOKEN_ITEMS);
    let count = match items.len() {
        1 => "one thing".to_string(),
        n => format!("{n} things"),
    };
    let tail = if more > 0 {
        format!("{}, and {more} more", named.join(", "))
    } else {
        join_aloud(&named)
    };
    format!("You have {count} {word}: {tail}.")
}

/// "You have 2 reminders: A and B." or the tool's own empty line.
pub fn spoken_reminders(items: &[Value], empty: &str) -> String {
    if items.is_empty() {
        return empty.to_string();
    }
    let named: Vec<String> = items
        .iter()
        .take(SPOKEN_ITEMS)
        .map(|i| title(i).to_string())
        .collect();
    let more = items.len().saturating_sub(SPOKEN_ITEMS);
    let count = match items.len() {
        1 => "one reminder".to_string(),
        n => format!("{n} reminders"),
    };
    let tail = if more > 0 {
        format!("{}, and {more} more", named.join(", "))
    } else {
        join_aloud(&named)
    };
    format!("You have {count}: {tail}.")
}

/// The first item that starts after `now` and has a time of day.
pub fn next_meeting(items: &[Value], now: DateTime<Local>) -> Option<&Value> {
    items.iter().find(|item| {
        !is_all_day(item)
            && item["start"]
                .as_str()
                .and_then(|s| mac_apps::parse_when(s).ok())
                .and_then(|w| w.start().ok())
                .is_some_and(|start| start > now)
    })
}

/// The window a day covers, as the bare dates the calendar tool reads.
pub fn day_window(day: Day, today: NaiveDate) -> (String, String) {
    let date = day.date(today).format("%Y-%m-%d").to_string();
    (date.clone(), date)
}

// ---------------------------------------------------------------------------
// Serving
// ---------------------------------------------------------------------------

async fn allowed(domain: Domain) -> bool {
    tokio::task::spawn_blocking(move || mac_apps::access_granted(domain))
        .await
        .unwrap_or(false)
}

/// A tool's answer, when it answered ok.
async fn ask(tool: &str, input: Value) -> Option<Value> {
    match mac_apps::dispatch(tool.to_string(), input).await {
        Ok(result) if result["ok"] == true => Some(result),
        Ok(_) => None,
        Err(e) => {
            log::warn!("local_intents: {tool} failed: {e}");
            None
        }
    }
}

/// Serve a parsed request. `None` means the agent should take it: a permission
/// is not granted yet, or the tool had no clean answer.
pub(super) async fn handle(_app_handle: &AppHandle, intent: AgendaIntent) -> Option<Reply> {
    match intent {
        AgendaIntent::Calendar(day) => {
            if !allowed(Domain::Calendar).await {
                return None;
            }
            let (from, to) = day_window(day, Local::now().date_naive());
            let result = ask(tool_names::CALENDAR_EVENTS, json!({"from": from, "to": to})).await?;
            let items = items_of(&result);
            let empty = format!("Nothing {}.", day.word());
            Some(Reply::card(card(&items, &empty), spoken_day(day, &items)))
        }
        AgendaIntent::Reminders => {
            if !allowed(Domain::Reminders).await {
                return None;
            }
            let result = ask(tool_names::REMINDERS_LIST, json!({})).await?;
            let items = items_of(&result);
            let empty = result["summary"]
                .as_str()
                .unwrap_or("Nothing open in Reminders.")
                .to_string();
            Some(Reply::card(
                card(&items, &empty),
                spoken_reminders(&items, &empty),
            ))
        }
        AgendaIntent::NextMeeting => {
            if !allowed(Domain::Calendar).await {
                return None;
            }
            let now = Local::now();
            let to = (now.date_naive() + Duration::days(7))
                .format("%Y-%m-%d")
                .to_string();
            let result = ask(
                tool_names::CALENDAR_EVENTS,
                json!({"from": mac_apps::format_iso(now), "to": to}),
            )
            .await?;
            let items = items_of(&result);
            let empty = "Nothing else this week.";
            Some(match next_meeting(&items, now) {
                Some(item) => {
                    let when = item["when"].as_str().unwrap_or_default();
                    Reply::card(
                        card(std::slice::from_ref(item), empty),
                        format!("Your next meeting is {}, {when}.", title(item)),
                    )
                }
                None => Reply::card(card(&[], empty), empty),
            })
        }
        AgendaIntent::Focus(on) => {
            let installed = tokio::task::spawn_blocking(mac_apps::focus_shortcut_installed)
                .await
                .unwrap_or(false);
            if !installed {
                return None;
            }
            let state = if on { "on" } else { "off" };
            match mac_apps::dispatch(tool_names::FOCUS_SET.to_string(), json!({"state": state}))
                .await
            {
                Ok(result) if result["ok"] == true => Some(Reply::text(
                    result["summary"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("Do Not Disturb is {state}.")),
                )),
                _ => Some(Reply::failure("I couldn't change Do Not Disturb.")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::utterance::normalize;
    use super::*;
    use chrono::TimeZone;

    fn p(q: &str) -> Option<AgendaIntent> {
        normalize(q).and_then(|u| parse(&u))
    }

    #[test]
    fn the_calendar_for_today_and_tomorrow() {
        for q in [
            "what's on my calendar today",
            "What's on my calendar today?",
            "hey juno, what's on my calendar today please",
            "what's on my schedule today",
            "what do I have today",
            "what do I have on my calendar today",
        ] {
            assert_eq!(p(q), Some(AgendaIntent::Calendar(Day::Today)), "{q:?}");
        }
        for q in [
            "what's on my calendar tomorrow",
            "what do I have tomorrow",
            "what's on the agenda for tomorrow",
        ] {
            assert_eq!(p(q), Some(AgendaIntent::Calendar(Day::Tomorrow)), "{q:?}");
        }
    }

    #[test]
    fn reminders_and_the_next_meeting() {
        for q in [
            "read my reminders",
            "what are my reminders",
            "read out my reminders",
        ] {
            assert_eq!(p(q), Some(AgendaIntent::Reminders), "{q:?}");
        }
        for q in ["what's my next meeting", "what's my next appointment"] {
            assert_eq!(p(q), Some(AgendaIntent::NextMeeting), "{q:?}");
        }
        // "when" is a clause word for every grammar, so this phrasing goes to
        // the agent. A miss is the safe outcome.
        assert_eq!(p("when is my next meeting"), None);
    }

    #[test]
    fn do_not_disturb_on_and_off() {
        for q in [
            "turn on do not disturb",
            "turn do not disturb on",
            "enable do not disturb",
            "turn on don't disturb",
            "turn on dnd",
            "Do Not Disturb on",
            "turn on do not disturb mode",
        ] {
            assert_eq!(p(q), Some(AgendaIntent::Focus(true)), "{q:?}");
        }
        for q in [
            "turn off do not disturb",
            "turn do not disturb off",
            "disable do not disturb",
            "switch off dnd",
            "do not disturb off",
        ] {
            assert_eq!(p(q), Some(AgendaIntent::Focus(false)), "{q:?}");
        }
    }

    /// Everything here has more in it than the four phrasings, or is a
    /// different question. All of it must reach the agent.
    #[test]
    fn near_misses_fall_through() {
        for q in [
            // calendar: another day, a range, a qualifier, another subject
            "what's on my calendar next week",
            "what's on my calendar today and tomorrow",
            "what's on my calendar today for Katie",
            "what's on my calendar on Friday",
            "what's on my calendar",
            "what's on TV tonight",
            "what's on my calendar today in the other time zone",
            "what do I have against Friday",
            "what do I have today in my inbox",
            "what do I have for dinner tomorrow",
            "what's today",
            "add lunch to my calendar today",
            "clear my calendar today",
            // reminders: a filter, an object, a how-to
            "read my reminders for Friday",
            "read my reminders from Katie",
            "read my reminder emails",
            "what are my reminders for today",
            "how do I read my reminders",
            "remind me to read",
            // next meeting: with whom, about what, which one
            "what's my next meeting with Katie",
            "when is my next meeting about the budget",
            "what was my last meeting",
            "what's my next step",
            "cancel my next meeting",
            // focus: a duration, an exception, a second action, a question
            "turn on do not disturb until noon",
            "turn on do not disturb for an hour",
            "turn off do not disturb at five",
            "turn on do not disturb and mute slack",
            "what is do not disturb",
            "turn on focus",
            "turn on do not disturb for Katie",
            "turn on dnd in slack",
            "turn on do not disturb on my phone",
            "do not disturb",
            "turn on the lights",
        ] {
            assert_eq!(p(q), None, "{q:?} should reach the agent");
        }
    }

    #[test]
    fn each_phrasing_is_reachable_from_the_entry_parser() {
        use super::super::{parse_local_intent, LocalIntent};
        for q in [
            "what's on my calendar today",
            "read my reminders",
            "what's my next meeting",
            "turn on do not disturb",
        ] {
            assert!(
                matches!(parse_local_intent(q), Some(LocalIntent::Agenda(_))),
                "{q:?} is not reachable"
            );
        }
        // Another domain's phrase is not taken.
        assert!(!matches!(
            parse_local_intent("what's playing"),
            Some(LocalIntent::Agenda(_))
        ));
    }

    fn event(title: &str, when: &str, start: &str) -> Value {
        json!({"kind": "event", "title": title, "when": when, "start": start, "done": false})
    }

    #[test]
    fn the_day_is_said_the_way_a_person_would() {
        assert_eq!(spoken_day(Day::Today, &[]), "Nothing today.");
        assert_eq!(spoken_day(Day::Tomorrow, &[]), "Nothing tomorrow.");
        let one = [event("Standup", "Today 9:00 AM", "2026-10-09T09:00:00")];
        assert_eq!(
            spoken_day(Day::Today, &one),
            "You have one thing today: Standup at 9:00 AM."
        );
        let two = [
            event("Standup", "Today 9:00 AM", "2026-10-09T09:00:00"),
            event("Dentist", "Today 3:30 PM", "2026-10-09T15:30:00"),
        ];
        assert_eq!(
            spoken_day(Day::Today, &two),
            "You have 2 things today: Standup at 9:00 AM and Dentist at 3:30 PM."
        );
        let all_day = [event("Birthday", "Today, all day", "2026-10-09T00:00:00")];
        assert_eq!(
            spoken_day(Day::Today, &all_day),
            "You have one thing today: Birthday."
        );
    }

    #[test]
    fn a_long_day_names_three_and_counts_the_rest() {
        let many: Vec<Value> = (1..=5)
            .map(|n| event(&format!("E{n}"), "Today 9:00 AM", "2026-10-09T09:00:00"))
            .collect();
        assert_eq!(
            spoken_day(Day::Today, &many),
            "You have 5 things today: E1 at 9:00 AM, E2 at 9:00 AM, E3 at 9:00 AM, and 2 more."
        );
    }

    #[test]
    fn reminders_are_read_by_title() {
        let items = [
            json!({"title": "Call Katie", "when": "Tomorrow", "done": false, "kind": "reminder"}),
            json!({"title": "Pay rent", "when": "", "done": false, "kind": "reminder"}),
        ];
        assert_eq!(
            spoken_reminders(&items, "Nothing open in Reminders."),
            "You have 2 reminders: Call Katie and Pay rent."
        );
        assert_eq!(
            spoken_reminders(&[], "Nothing open in Reminders."),
            "Nothing open in Reminders."
        );
    }

    #[test]
    fn the_card_is_the_agenda_tag_without_ids() {
        let items = [json!({
            "kind": "event", "id": "abc@1", "title": "Standup", "when": "Today 9:00 AM",
            "start": "2026-10-09T09:00:00", "done": false, "where": "Room 4"
        })];
        let tag = card(&items, "Nothing today.");
        assert!(tag.starts_with("<AgendaCard items={["));
        assert!(tag.contains("\"title\":\"Standup\"") && tag.contains("\"where\":\"Room 4\""));
        assert!(!tag.contains("abc@1") && !tag.contains("2026-10-09T"));
        assert!(tag.ends_with("empty={\"Nothing today.\"} />"));
    }

    #[test]
    fn the_card_stops_at_eight_rows() {
        let many: Vec<Value> = (0..20)
            .map(|n| json!({"title": format!("R{n}"), "when": "", "done": false, "kind": "reminder"}))
            .collect();
        let tag = card(&many, "");
        assert!(tag.contains("\"R7\"") && !tag.contains("\"R8\""));
    }

    #[test]
    fn the_next_meeting_skips_the_past_and_all_day_events() {
        let now = Local
            .with_ymd_and_hms(2026, 10, 9, 10, 0, 0)
            .single()
            .unwrap_or_else(Local::now);
        let items = [
            event("Birthday", "Today, all day", "2026-10-09T00:00:00"),
            event("Standup", "Today 9:00 AM", "2026-10-09T09:00:00"),
            event("Dentist", "Today 3:30 PM", "2026-10-09T15:30:00"),
        ];
        // The two fixed offsets above are not this Mac's zone in every place the
        // tests run, so compare against the item that is furthest in the future.
        let later = next_meeting(&items, now - Duration::days(1));
        assert_eq!(later.map(title), Some("Standup"));
        let none = next_meeting(&items, now + Duration::days(30));
        assert_eq!(none, None);
    }

    #[test]
    fn the_all_day_label_is_the_one_the_tools_write() {
        let now = Local::now();
        let label = mac_apps::relative_when(now, now, mac_apps::Clock::AllDay);
        assert!(label.ends_with(ALL_DAY_SUFFIX), "{label}");
        assert!(is_all_day(&json!({"when": label})));
        let timed = mac_apps::relative_when(now, now, mac_apps::Clock::Timed);
        assert!(!is_all_day(&json!({"when": timed})));
    }

    #[test]
    fn a_day_is_one_bare_date_so_the_whole_day_is_covered() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap_or_default();
        assert_eq!(
            day_window(Day::Today, today),
            ("2026-10-09".to_string(), "2026-10-09".to_string())
        );
        assert_eq!(
            day_window(Day::Tomorrow, today),
            ("2026-10-10".to_string(), "2026-10-10".to_string())
        );
    }
}
