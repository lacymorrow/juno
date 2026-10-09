//! Calendar through EventKit.
//!
//! EventKit sees every account Calendar has (iCloud, Google, Exchange), which
//! is why these tools beat a cloud calendar tool for the same question.
//! Every write is read back through a fresh store before it is reported.

use chrono::{Duration, Local, TimeZone};
use objc2::rc::Retained;
use objc2_event_kit::{EKEvent, EKEventStore, EKSpan};
use objc2_foundation::NSString;
use serde_json::{json, Value};

use super::eventkit::{self, from_ns_date, ns_date, ns_error};
use super::*;

/// The most events one answer carries.
const MAX_ITEMS: usize = 50;

/// An hour, the length of an event nobody gave an end to.
const DEFAULT_LENGTH_SECS: i64 = 3600;

/// How far a saved time may differ from the asked time before it is an error.
const READ_BACK_SLACK_SECS: i64 = 60;

fn asked_or_stop(cleared: Cleared) -> Result<Option<String>, Value> {
    match cleared {
        Cleared::Go { asked } => Ok(asked),
        Cleared::Stop(answer) => Err(answer),
    }
}

/// An event's start and end as moments. `None` if EventKit gave a date that
/// does not exist on this Mac's clock.
fn span(event: &EKEvent) -> Option<(DateTime<Local>, DateTime<Local>)> {
    // SAFETY: property reads on a live event.
    let (start, end) = unsafe { (event.startDate(), event.endDate()) };
    Some((from_ns_date(&start)?, from_ns_date(&end)?))
}

/// Read an event into the one shape.
fn event_item(event: &EKEvent, now: DateTime<Local>) -> AgendaItem {
    // SAFETY: property reads on a live event.
    let (identifier, title, place, list, all_day) = unsafe {
        (
            event
                .eventIdentifier()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            event.title().to_string(),
            event
                .location()
                .map(|s| s.to_string())
                .filter(|s| !s.trim().is_empty()),
            event.calendar().map(|c| c.title().to_string()),
            event.isAllDay(),
        )
    };
    let (id, when, start, end) = match span(event) {
        Some((start, end)) => {
            let clock = if all_day { Clock::AllDay } else { Clock::Timed };
            (
                event_id(&identifier, start),
                relative_when(start, now, clock),
                Some(format_iso(start)),
                Some(format_iso(end)),
            )
        }
        None => (identifier, String::new(), None, None),
    };
    AgendaItem {
        kind: "event",
        id,
        title,
        when,
        start,
        end,
        list,
        place,
        done: false,
    }
}

/// Find one event by the id a tool handed out. A repeating event is found at
/// the occurrence the id names, never at its first occurrence.
fn find_event(id: &str) -> Option<(Retained<EKEventStore>, Retained<EKEvent>)> {
    let (identifier, stamp) = split_event_id(id);
    let store = eventkit::new_store();
    if let Some(secs) = stamp {
        let center = Local.timestamp_opt(secs, 0).single()?;
        let slack = Duration::seconds(OCCURRENCE_SLACK_SECS);
        let (low, high) = (ns_date(center - slack), ns_date(center + slack));
        // SAFETY: reads on a live store with live dates.
        let found = unsafe {
            let predicate =
                store.predicateForEventsWithStartDate_endDate_calendars(&low, &high, None);
            store
                .eventsMatchingPredicate(&predicate)
                .to_vec()
                .into_iter()
                .find(|e| e.eventIdentifier().map(|s| s.to_string()).as_deref() == Some(identifier))
        };
        if let Some(event) = found {
            return Some((store, event));
        }
    }
    // SAFETY: a read on a live store.
    let event = unsafe { store.eventWithIdentifier(&NSString::from_str(identifier)) }?;
    // The first occurrence is the right answer only for an event that does not repeat.
    // SAFETY: a property read on a live event.
    if stamp.is_some() && unsafe { event.hasRecurrenceRules() } {
        return None;
    }
    Some((store, event))
}

/// `calendar_events`
pub fn events(input: &Value) -> Result<Value, String> {
    let from = parse_when(required_text(input, "from")?)?.start()?;
    let to = parse_when(required_text(input, "to")?)?.end_exclusive()?;
    if to <= from {
        return Err("The end of the window is not after its start.".to_string());
    }
    let asked = match asked_or_stop(eventkit::clear_access(Domain::Calendar)) {
        Ok(asked) => asked,
        Err(answer) => return Ok(answer),
    };

    let store = eventkit::new_store();
    // SAFETY: reads on a live store with live dates.
    let found = unsafe {
        let predicate = store.predicateForEventsWithStartDate_endDate_calendars(
            &ns_date(from),
            &ns_date(to),
            None,
        );
        store.eventsMatchingPredicate(&predicate)
    };
    let now = Local::now();
    let mut items: Vec<AgendaItem> = found.to_vec().iter().map(|e| event_item(e, now)).collect();
    sort_items(&mut items);
    items.truncate(MAX_ITEMS);
    Ok(listing(
        items,
        "Nothing on your calendar for that time.",
        asked,
    ))
}

/// `calendar_create_event`
pub fn create(input: &Value) -> Result<Value, String> {
    let title = required_text(input, "title")?;
    let start = parse_when(required_text(input, "start")?)?;
    let end = text_arg(input, "end").map(parse_when).transpose()?;
    let invitees = list_arg(input, "invitees");

    // A bare date is an all-day event; EventKit ends it on its last second.
    let (all_day, start_at, end_at) = match start {
        When::Day(_) => {
            let first = start.start()?;
            let last = start.end_exclusive()? - Duration::seconds(1);
            (true, first, last)
        }
        When::Moment(first) => {
            let last = match end {
                Some(end) => end.start()?,
                None => first + Duration::seconds(DEFAULT_LENGTH_SECS),
            };
            (false, first, last)
        }
    };
    if end_at < start_at {
        return Err("The event would end before it starts.".to_string());
    }

    let asked = match asked_or_stop(eventkit::clear_access(Domain::Calendar)) {
        Ok(asked) => asked,
        Err(answer) => return Ok(answer),
    };

    let store = eventkit::new_store();
    // SAFETY: a read on a live store.
    let calendar = unsafe { store.defaultCalendarForNewEvents() }
        .ok_or_else(|| "Calendar has no calendar to add events to.".to_string())?;
    // SAFETY: a new event on a live store, then setters with live arguments.
    let event = unsafe { EKEvent::eventWithEventStore(&store) };
    unsafe {
        event.setTitle(Some(&NSString::from_str(title)));
        event.setCalendar(Some(&calendar));
        event.setAllDay(all_day);
        event.setStartDate(Some(&ns_date(start_at)));
        event.setEndDate(Some(&ns_date(end_at)));
        if let Some(place) = text_arg(input, "location") {
            event.setLocation(Some(&NSString::from_str(place)));
        }
        if !invitees.is_empty() {
            event.setNotes(Some(&NSString::from_str(&format!(
                "With: {}",
                invitees.join(", ")
            ))));
        }
    }
    // SAFETY: saving an event created from this store.
    unsafe { store.saveEvent_span_error(&event, EKSpan::ThisEvent) }
        .map_err(|e| format!("Calendar would not save it: {}", ns_error(&e)))?;

    // SAFETY: a property read on the event just saved.
    let identifier = unsafe { event.eventIdentifier() }
        .map(|s| s.to_string())
        .ok_or_else(|| "Calendar did not give the event an id.".to_string())?;
    let (_seen_store, seen) =
        find_event(&identifier).ok_or_else(|| "Calendar did not keep the event.".to_string())?;
    let item = event_item(&seen, Local::now());
    check_start(&seen, start_at)?;

    let invitees_note = (!invitees.is_empty()).then(|| {
        (
            "invitees",
            json!({
                "names": invitees,
                "note": "Calendar cannot send invitations from here. Their names are in the event's notes."
            }),
        )
    });
    Ok(observed(
        "Added to your calendar",
        item,
        invitees_note,
        asked,
    ))
}

/// The saved start must be the asked start, or the answer would be a guess.
fn check_start(event: &EKEvent, wanted: DateTime<Local>) -> Result<(), String> {
    let (start, _) =
        span(event).ok_or_else(|| "Calendar saved a time that does not exist.".to_string())?;
    if (start - wanted).num_seconds().abs() > READ_BACK_SLACK_SECS {
        return Err(format!(
            "Calendar saved it for {} instead of {}.",
            start.format("%b %-d %-I:%M %p"),
            wanted.format("%b %-d %-I:%M %p")
        ));
    }
    Ok(())
}

/// `calendar_move_event`
pub fn move_event(input: &Value) -> Result<Value, String> {
    let id = required_text(input, "id")?;
    let new_start = parse_when(required_text(input, "new_start")?)?;
    let asked = match asked_or_stop(eventkit::clear_access(Domain::Calendar)) {
        Ok(asked) => asked,
        Err(answer) => return Ok(answer),
    };

    let (store, event) =
        find_event(id).ok_or_else(|| "There is no event with that id.".to_string())?;
    let (old_start, old_end) =
        span(&event).ok_or_else(|| "That event has a time that does not exist.".to_string())?;
    let (start, end) = moved_range(old_start, old_end, new_start)?;
    // SAFETY: setters on a live event with live dates, then a save from its own store.
    unsafe {
        event.setStartDate(Some(&ns_date(start)));
        event.setEndDate(Some(&ns_date(end)));
        store
            .saveEvent_span_error(&event, EKSpan::ThisEvent)
            .map_err(|e| format!("Calendar would not move it: {}", ns_error(&e)))?;
    }

    let moved_id = event_id(
        // SAFETY: a property read on the event just saved.
        &unsafe { event.eventIdentifier() }
            .map(|s| s.to_string())
            .unwrap_or_default(),
        start,
    );
    let (_seen_store, seen) = find_event(&moved_id)
        .ok_or_else(|| "Calendar lost the event after moving it.".to_string())?;
    check_start(&seen, start)?;
    Ok(observed(
        "Moved",
        event_item(&seen, Local::now()),
        None,
        asked,
    ))
}

/// `calendar_delete_event`
pub fn delete(input: &Value) -> Result<Value, String> {
    let id = required_text(input, "id")?;
    let asked = match asked_or_stop(eventkit::clear_access(Domain::Calendar)) {
        Ok(asked) => asked,
        Err(answer) => return Ok(answer),
    };

    let (store, event) =
        find_event(id).ok_or_else(|| "There is no event with that id.".to_string())?;
    let item = event_item(&event, Local::now());
    // SAFETY: removing an event read from this store.
    unsafe { store.removeEvent_span_error(&event, EKSpan::ThisEvent) }
        .map_err(|e| format!("Calendar would not delete it: {}", ns_error(&e)))?;

    if find_event(id).is_some() {
        return Err(format!("Calendar still shows {}.", item.title));
    }
    let mut out = observed("Deleted", item, None, asked);
    // The event is gone, so there is nothing to draw.
    if let Some(map) = out.as_object_mut() {
        map.remove("card");
    }
    Ok(out)
}
