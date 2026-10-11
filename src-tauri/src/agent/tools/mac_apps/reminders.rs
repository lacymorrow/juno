//! Reminders through EventKit.
//!
//! Each public function is one tool, run start to finish on a blocking thread
//! (see the module notes in `mod.rs`). Every write is read back through a fresh
//! store before it is reported.

use std::sync::Mutex;

use block2::RcBlock;
use chrono::{Datelike, Local, Timelike};
use objc2::rc::Retained;
use objc2_event_kit::{EKAlarm, EKCalendar, EKEntityType, EKEventStore, EKReminder};
use objc2_foundation::{NSArray, NSDateComponentUndefined, NSDateComponents, NSString};
use serde_json::Value;
use tokio::sync::oneshot;

use super::eventkit::{self, ns_date, ns_error};
use super::*;

/// The most reminders one answer carries. A longer list is read aloud as noise.
const MAX_ITEMS: usize = 50;

/// How long to wait for the reminders fetch before giving up.
const FETCH_WAIT_SECS: u64 = 20;

/// Read a reminder into the one shape.
fn reminder_item(reminder: &EKReminder, now: DateTime<Local>) -> AgendaItem {
    // SAFETY: plain property reads on a live reminder.
    let (id, title, list, done, due) = unsafe {
        (
            reminder.calendarItemIdentifier().to_string(),
            reminder.title().to_string(),
            reminder.calendar().map(|c| c.title().to_string()),
            reminder.isCompleted(),
            reminder.dueDateComponents(),
        )
    };
    let due = due.and_then(|c| {
        let part = |v: isize| (v != NSDateComponentUndefined).then_some(v as i64);
        from_components(
            part(c.year())?,
            part(c.month())?,
            part(c.day())?,
            part(c.hour()),
            part(c.minute()),
        )
    });
    let (when, start) = match due {
        Some((date, time)) => match components_to_local(date, time) {
            Some(dt) => {
                let clock = if time.is_some() {
                    Clock::Timed
                } else {
                    Clock::DateOnly
                };
                (relative_when(dt, now, clock), Some(format_iso(dt)))
            }
            None => (String::new(), None),
        },
        None => (String::new(), None),
    };
    AgendaItem {
        kind: "reminder",
        id,
        title,
        when,
        start,
        end: None,
        list,
        place: None,
        done,
    }
}

/// Every Reminders list, by the name the person sees.
fn lists(store: &EKEventStore) -> Vec<(String, Retained<EKCalendar>)> {
    // SAFETY: a read on a live store.
    let calendars = unsafe { store.calendarsForEntityType(EKEntityType::Reminder) };
    calendars
        .to_vec()
        .into_iter()
        // SAFETY: a property read on a live calendar.
        .map(|c| (unsafe { c.title() }.to_string(), c))
        .collect()
}

fn unknown_list(name: &str, named: &[(String, Retained<EKCalendar>)]) -> String {
    let names: Vec<String> = named.iter().map(|(n, _)| n.clone()).collect();
    format!(
        "There is no Reminders list called {name}. The lists are: {}.",
        name_list(&names)
    )
}

/// An optional date argument, as its earliest moment.
fn window_start(input: &Value, key: &str) -> Result<Option<DateTime<Local>>, String> {
    text_arg(input, key)
        .map(|t| parse_when(t)?.start())
        .transpose()
}

/// An optional date argument, as the first moment after it.
fn window_end(input: &Value, key: &str) -> Result<Option<DateTime<Local>>, String> {
    text_arg(input, key)
        .map(|t| parse_when(t)?.end_exclusive())
        .transpose()
}

fn asked_or_stop(cleared: Cleared) -> Result<Option<String>, Value> {
    match cleared {
        Cleared::Go { asked } => Ok(asked),
        Cleared::Stop(answer) => Err(answer),
    }
}

/// `reminders_list`
pub fn list(input: &Value) -> Result<Value, String> {
    let from = window_start(input, "due_from")?;
    let to = window_end(input, "due_to")?;
    let asked = match asked_or_stop(eventkit::clear_access(Domain::Reminders)) {
        Ok(asked) => asked,
        Err(answer) => return Ok(answer),
    };

    let store = eventkit::new_store();
    let wanted = text_arg(input, "list");
    let calendars = match wanted {
        Some(name) => {
            let named = lists(&store);
            let found = find_by_name(&named, name).ok_or_else(|| unknown_list(name, &named))?;
            Some(NSArray::from_retained_slice(std::slice::from_ref(found)))
        }
        None => None,
    };
    let from_ns = from.map(ns_date);
    let to_ns = to.map(ns_date);
    // SAFETY: the arguments are live objects or nil, which the predicate allows.
    let predicate = unsafe {
        store.predicateForIncompleteRemindersWithDueDateStarting_ending_calendars(
            from_ns.as_deref(),
            to_ns.as_deref(),
            calendars.as_deref(),
        )
    };

    let (tx, rx) = oneshot::channel::<Vec<AgendaItem>>();
    let slot = Mutex::new(Some(tx));
    let now = Local::now();
    let block = RcBlock::new(move |found: *mut NSArray<EKReminder>| {
        // SAFETY: EventKit hands the handler a live array or nil.
        let items: Vec<AgendaItem> = match unsafe { found.as_ref() } {
            Some(array) => array
                .to_vec()
                .iter()
                .map(|r| reminder_item(r, now))
                .collect(),
            None => Vec::new(),
        };
        if let Ok(mut guard) = slot.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(items);
            }
        }
    });
    // SAFETY: the store and the block both outlive the wait below.
    let _request = unsafe { store.fetchRemindersMatchingPredicate_completion(&predicate, &block) };
    let mut items = eventkit::wait(rx, FETCH_WAIT_SECS)
        .ok_or_else(|| "Reminders did not answer in time.".to_string())?;

    sort_items(&mut items);
    items.truncate(MAX_ITEMS);
    let empty = match wanted {
        Some(name) => format!("Nothing open in {name}."),
        None => "Nothing open in Reminders.".to_string(),
    };
    Ok(listing(items, &empty, asked))
}

/// Set the due date, and a notification at the moment when there is one.
fn apply_due(reminder: &EKReminder, due: When) {
    let components = NSDateComponents::new();
    let (date, moment) = match due {
        When::Day(day) => (day, None),
        When::Moment(dt) => (dt.date_naive(), Some(dt)),
    };
    components.setYear(date.year() as isize);
    components.setMonth(date.month() as isize);
    components.setDay(date.day() as isize);
    if let Some(dt) = moment {
        components.setHour(dt.hour() as isize);
        components.setMinute(dt.minute() as isize);
    }
    // SAFETY: setters on a live reminder with live arguments.
    unsafe {
        reminder.setDueDateComponents(Some(&components));
        if let Some(dt) = moment {
            let alarm = EKAlarm::alarmWithAbsoluteDate(&ns_date(dt));
            reminder.addAlarm(&alarm);
        }
    }
}

/// Look a reminder up in a store of its own. The store comes back with it,
/// because a reminder must not outlive the store it was read from.
fn lookup(id: &str) -> Option<(Retained<EKEventStore>, Retained<EKReminder>)> {
    let store = eventkit::new_store();
    // SAFETY: a read on a live store.
    let item = unsafe { store.calendarItemWithIdentifier(&NSString::from_str(id)) }?;
    let reminder = item.downcast::<EKReminder>().ok()?;
    Some((store, reminder))
}

/// `reminders_create`
pub fn create(input: &Value) -> Result<Value, String> {
    let title = required_text(input, "title")?;
    let due = text_arg(input, "due").map(parse_when).transpose()?;
    let asked = match asked_or_stop(eventkit::clear_access(Domain::Reminders)) {
        Ok(asked) => asked,
        Err(answer) => return Ok(answer),
    };

    let store = eventkit::new_store();
    let calendar = match text_arg(input, "list") {
        Some(name) => {
            let named = lists(&store);
            find_by_name(&named, name)
                .cloned()
                .ok_or_else(|| unknown_list(name, &named))?
        }
        // SAFETY: a read on a live store.
        None => unsafe { store.defaultCalendarForNewReminders() }
            .ok_or_else(|| "Reminders has no list to put it in.".to_string())?,
    };

    // SAFETY: a new reminder on a live store, then setters with live arguments.
    let reminder = unsafe { EKReminder::reminderWithEventStore(&store) };
    unsafe {
        reminder.setTitle(Some(&NSString::from_str(title)));
        reminder.setCalendar(Some(&calendar));
        if let Some(notes) = text_arg(input, "notes") {
            reminder.setNotes(Some(&NSString::from_str(notes)));
        }
    }
    if let Some(due) = due {
        apply_due(&reminder, due);
    }
    // SAFETY: saving a reminder created from this store.
    unsafe { store.saveReminder_commit_error(&reminder, true) }
        .map_err(|e| format!("Reminders would not save it: {}", ns_error(&e)))?;

    // SAFETY: a property read on the reminder just saved.
    let id = unsafe { reminder.calendarItemIdentifier() }.to_string();
    let (_seen_store, seen) =
        lookup(&id).ok_or_else(|| "Reminders did not keep the reminder.".to_string())?;
    Ok(observed(
        "Reminder saved",
        reminder_item(&seen, Local::now()),
        None,
        asked,
    ))
}

/// `reminders_complete`
pub fn complete(input: &Value) -> Result<Value, String> {
    let id = required_text(input, "id")?;
    let asked = match asked_or_stop(eventkit::clear_access(Domain::Reminders)) {
        Ok(asked) => asked,
        Err(answer) => return Ok(answer),
    };

    let (store, reminder) =
        lookup(id).ok_or_else(|| "There is no reminder with that id.".to_string())?;
    // SAFETY: a read, then a write and a save on a reminder from this store.
    unsafe {
        if !reminder.isCompleted() {
            reminder.setCompleted(true);
            store
                .saveReminder_commit_error(&reminder, true)
                .map_err(|e| format!("Reminders would not mark it done: {}", ns_error(&e)))?;
        }
    }

    let (_seen_store, seen) =
        lookup(id).ok_or_else(|| "Reminders lost the reminder after saving.".to_string())?;
    let item = reminder_item(&seen, Local::now());
    if !item.done {
        return Err(format!("Reminders still shows {} as open.", item.title));
    }
    Ok(observed("Marked done", item, None, asked))
}
