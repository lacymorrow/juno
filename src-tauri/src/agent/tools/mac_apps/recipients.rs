//! Who a text, an email or a FaceTime call goes to.
//!
//! Pure: Contacts hands over the people a name matched, the Messages history
//! (when it can be read) says which addresses already talk over iMessage, and
//! this decides. A name that matches more than one person never picks one; it
//! comes back with the candidates and nothing is sent.

use std::collections::HashMap;

use super::fold;

/// How the message travels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// Messages: iMessage, or SMS when that is the only way this address has
    /// ever been reached.
    Text,
    /// Mail.
    Email,
    /// FaceTime.
    Call,
}

/// The Messages service an address goes out on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    IMessage,
    Sms,
}

impl Service {
    /// The word Messages' dictionary and chat.db both use.
    pub fn as_str(self) -> &'static str {
        match self {
            Service::IMessage => "iMessage",
            Service::Sms => "SMS",
        }
    }

    /// chat.db's `handle.service` column.
    pub fn from_db(raw: &str) -> Option<Self> {
        match raw {
            "iMessage" => Some(Service::IMessage),
            "SMS" | "RCS" => Some(Service::Sms),
            _ => None,
        }
    }
}

/// One person Contacts matched, reduced to what choosing an address needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub name: String,
    /// (label, number) as Contacts stores them.
    pub phones: Vec<(String, String)>,
    /// (label, address).
    pub emails: Vec<(String, String)>,
}

/// Where a message will go.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipient {
    /// The name a person reads on the card.
    pub name: String,
    /// The phone number or email address it goes to.
    pub address: String,
    /// Messages only.
    pub service: Option<Service>,
}

/// What resolving a recipient came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolved {
    One(Recipient),
    /// More than one person matched. Their names, so the model can ask which.
    Ambiguous(Vec<String>),
    /// The one match has no number or address for this channel.
    NoAddress(String),
    /// Nobody matched.
    NotFound,
}

/// What kind of address a typed recipient already is, if it is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressKind {
    Phone,
    Email,
}

/// A plausible email: one `@`, something either side, a dot after it, no space.
pub fn is_email(text: &str) -> bool {
    let text = text.trim();
    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains('@')
        && !text.chars().any(char::is_whitespace)
}

/// Keep a leading `+` and the digits. "(704) 555-0100" becomes "7045550100".
pub fn normalize_phone(raw: &str) -> String {
    let trimmed = raw.trim();
    let plus = trimmed.starts_with('+');
    let digits: String = trimmed.chars().filter(char::is_ascii_digit).collect();
    if plus {
        format!("+{digits}")
    } else {
        digits
    }
}

/// A typed phone number: only digits and phone punctuation, at least 7 digits.
pub fn is_phone(text: &str) -> bool {
    let text = text.trim();
    let digits = text.chars().filter(char::is_ascii_digit).count();
    digits >= 7
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '(' | ')' | '.' | ' '))
}

/// Whether the person typed an address rather than a name.
pub fn address_kind(text: &str) -> Option<AddressKind> {
    if is_email(text) {
        Some(AddressKind::Email)
    } else if is_phone(text) {
        Some(AddressKind::Phone)
    } else {
        None
    }
}

/// The key two spellings of one address share: an email folded, a phone's last
/// ten digits (so "+1 704 555 0100" and "(704) 555-0100" are one number).
pub fn address_key(address: &str) -> String {
    if is_email(address) {
        return fold(address);
    }
    let digits: Vec<char> = address.chars().filter(char::is_ascii_digit).collect();
    let start = digits.len().saturating_sub(10);
    digits[start..].iter().collect()
}

/// Labels that mean "this number is a phone in someone's pocket".
fn is_mobile_label(label: &str) -> bool {
    let label = fold(label);
    label.contains("mobile") || label.contains("iphone") || label.contains("cell")
}

/// The address and service a text should use for one person.
///
/// `known` maps [`address_key`] to the services Messages has used with it.
/// Preference: an address already on iMessage; then, if the only history is
/// SMS, that SMS number; then a mobile number on iMessage; then any number;
/// then an email on iMessage.
pub fn pick_for_text(
    person: &Candidate,
    known: &HashMap<String, Vec<Service>>,
) -> Option<(String, Service)> {
    let all: Vec<&String> = person
        .phones
        .iter()
        .chain(person.emails.iter())
        .map(|(_, v)| v)
        .collect();

    let has = |address: &str, service: Service| {
        known
            .get(&address_key(address))
            .is_some_and(|s| s.contains(&service))
    };

    if let Some(address) = all.iter().find(|a| has(a, Service::IMessage)) {
        return Some(((*address).clone(), Service::IMessage));
    }
    if let Some(address) = all.iter().find(|a| has(a, Service::Sms)) {
        return Some(((*address).clone(), Service::Sms));
    }
    if let Some((_, number)) = person.phones.iter().find(|(l, _)| is_mobile_label(l)) {
        return Some((number.clone(), Service::IMessage));
    }
    if let Some((_, number)) = person.phones.first() {
        return Some((number.clone(), Service::IMessage));
    }
    person
        .emails
        .first()
        .map(|(_, email)| (email.clone(), Service::IMessage))
}

/// The address an email should use for one person: the first one Contacts has.
pub fn pick_for_email(person: &Candidate) -> Option<String> {
    person.emails.first().map(|(_, e)| e.clone())
}

/// The address a FaceTime call should use: what a text would use, since
/// FaceTime reaches the same Apple ID, without the SMS fallback.
pub fn pick_for_call(person: &Candidate, known: &HashMap<String, Vec<Service>>) -> Option<String> {
    pick_for_text(person, known).map(|(address, _)| address)
}

/// Narrow what Contacts matched to one person, or say why not.
///
/// An exact name match wins over partial ones ("Doug" matches "Doug Keesler"
/// and "Doug Smith": ambiguous; "Doug Keesler" matches one). Two cards with the
/// same name are still two people, so they stay ambiguous.
pub fn narrow(query: &str, mut people: Vec<Candidate>) -> Result<Candidate, Resolved> {
    match people.len() {
        0 => Err(Resolved::NotFound),
        1 => Ok(people.remove(0)),
        _ => {
            let wanted = fold(query);
            let exact: Vec<usize> = people
                .iter()
                .enumerate()
                .filter(|(_, p)| fold(&p.name) == wanted)
                .map(|(i, _)| i)
                .collect();
            if exact.len() == 1 {
                Ok(people.remove(exact[0]))
            } else {
                Err(Resolved::Ambiguous(
                    people.into_iter().map(|p| p.name).collect(),
                ))
            }
        }
    }
}

/// The whole decision, for a name the person said and the people it matched.
pub fn resolve(
    query: &str,
    people: Vec<Candidate>,
    channel: Channel,
    known: &HashMap<String, Vec<Service>>,
) -> Resolved {
    let person = match narrow(query, people) {
        Ok(person) => person,
        Err(why) => return why,
    };
    let picked = match channel {
        Channel::Text => pick_for_text(&person, known).map(|(a, s)| (a, Some(s))),
        Channel::Email => pick_for_email(&person).map(|a| (a, None)),
        Channel::Call => pick_for_call(&person, known).map(|a| (a, None)),
    };
    match picked {
        Some((address, service)) => Resolved::One(Recipient {
            name: person.name,
            address,
            service,
        }),
        None => Resolved::NoAddress(person.name),
    }
}

/// A recipient typed as an address, no Contacts needed. A phone number cannot
/// be emailed, so that comes back `None` for [`Channel::Email`].
pub fn from_typed_address(
    text: &str,
    channel: Channel,
    known: &HashMap<String, Vec<Service>>,
) -> Option<Recipient> {
    let kind = address_kind(text)?;
    let address = match kind {
        AddressKind::Email => text.trim().to_string(),
        AddressKind::Phone => normalize_phone(text),
    };
    match (channel, kind) {
        (Channel::Email, AddressKind::Phone) => None,
        (Channel::Email, AddressKind::Email) => Some(Recipient {
            name: address.clone(),
            address,
            service: None,
        }),
        (Channel::Call, _) => Some(Recipient {
            name: address.clone(),
            address,
            service: None,
        }),
        (Channel::Text, _) => {
            let services = known.get(&address_key(&address));
            let service = match services {
                Some(s) if !s.contains(&Service::IMessage) && s.contains(&Service::Sms) => {
                    Service::Sms
                }
                _ => Service::IMessage,
            };
            Some(Recipient {
                name: address.clone(),
                address,
                service: Some(service),
            })
        }
    }
}

/// The answer when a recipient could not be settled. Nothing was sent.
pub fn unresolved_result(query: &str, why: &Resolved, channel: Channel) -> serde_json::Value {
    let what = match channel {
        Channel::Text => "a number to text",
        Channel::Email => "an email address",
        Channel::Call => "a number or address to call",
    };
    let summary = match why {
        Resolved::Ambiguous(names) => format!(
            "More than one person matches {query}: {}. Which one? Nothing was sent.",
            names.join(", ")
        ),
        Resolved::NoAddress(name) => format!("{name} has no {what} in Contacts. Nothing was sent."),
        Resolved::NotFound => format!("Nobody in Contacts matches {query}. Nothing was sent."),
        Resolved::One(_) => String::new(),
    };
    let candidates = match why {
        Resolved::Ambiguous(names) => names.clone(),
        _ => Vec::new(),
    };
    serde_json::json!({
        "ok": false,
        "sent": false,
        "summary": summary,
        "candidates": candidates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(name: &str, phones: &[(&str, &str)], emails: &[(&str, &str)]) -> Candidate {
        Candidate {
            name: name.to_string(),
            phones: phones
                .iter()
                .map(|(l, v)| (l.to_string(), v.to_string()))
                .collect(),
            emails: emails
                .iter()
                .map(|(l, v)| (l.to_string(), v.to_string()))
                .collect(),
        }
    }

    fn none() -> HashMap<String, Vec<Service>> {
        HashMap::new()
    }

    #[test]
    fn typed_addresses_are_recognised() {
        assert_eq!(address_kind("doug@example.com"), Some(AddressKind::Email));
        assert_eq!(address_kind("(704) 555-0100"), Some(AddressKind::Phone));
        assert_eq!(address_kind("+1 704 555 0100"), Some(AddressKind::Phone));
        assert_eq!(address_kind("Doug"), None);
        assert_eq!(address_kind("555"), None);
        assert_eq!(address_kind("doug @example.com"), None);
        assert_eq!(normalize_phone("+1 (704) 555-0100"), "+17045550100");
        assert_eq!(address_key("+1 (704) 555-0100"), address_key("704.555.0100"));
        assert_eq!(address_key("Doug@Example.com"), "doug@example.com");
    }

    #[test]
    fn imessage_is_preferred_over_sms_when_both_exist() {
        let doug = person(
            "Doug Keesler",
            &[("home", "(704) 555-0199"), ("mobile", "(704) 555-0100")],
            &[("work", "doug@example.com")],
        );
        let mut known = HashMap::new();
        known.insert(address_key("+17045550199"), vec![Service::Sms]);
        known.insert(
            address_key("doug@example.com"),
            vec![Service::IMessage],
        );
        assert_eq!(
            pick_for_text(&doug, &known),
            Some(("doug@example.com".to_string(), Service::IMessage))
        );

        // Same number known on both: iMessage.
        let mut both = HashMap::new();
        both.insert(
            address_key("7045550100"),
            vec![Service::Sms, Service::IMessage],
        );
        assert_eq!(
            pick_for_text(&doug, &both),
            Some(("(704) 555-0100".to_string(), Service::IMessage))
        );
    }

    #[test]
    fn sms_only_history_stays_on_sms() {
        let sam = person("Sam", &[("mobile", "704-555-0123")], &[]);
        let mut known = HashMap::new();
        known.insert(address_key("+17045550123"), vec![Service::Sms]);
        assert_eq!(
            pick_for_text(&sam, &known),
            Some(("704-555-0123".to_string(), Service::Sms))
        );
    }

    #[test]
    fn with_no_history_a_mobile_number_goes_first() {
        let katie = person(
            "Katie",
            &[("home", "704 555 0001"), ("_$!<Mobile>!$_", "704 555 0002")],
            &[("home", "k@example.com")],
        );
        assert_eq!(
            pick_for_text(&katie, &none()),
            Some(("704 555 0002".to_string(), Service::IMessage))
        );
        let email_only = person("Ann", &[], &[("home", "ann@example.com")]);
        assert_eq!(
            pick_for_text(&email_only, &none()),
            Some(("ann@example.com".to_string(), Service::IMessage))
        );
        assert_eq!(pick_for_text(&person("Nobody", &[], &[]), &none()), None);
    }

    #[test]
    fn an_ambiguous_name_returns_the_candidates_and_does_not_pick() {
        let people = vec![
            person("Doug Keesler", &[("mobile", "1")], &[]),
            person("Doug Smith", &[("mobile", "2")], &[]),
        ];
        assert_eq!(
            resolve("Doug", people.clone(), Channel::Text, &none()),
            Resolved::Ambiguous(vec!["Doug Keesler".to_string(), "Doug Smith".to_string()])
        );
        // The full name settles it.
        let one = resolve("doug keesler", people, Channel::Text, &none());
        assert!(matches!(one, Resolved::One(ref r) if r.name == "Doug Keesler"), "{one:?}");

        // Two cards with one name are still two people.
        let twins = vec![
            person("Sam Lee", &[("mobile", "1")], &[]),
            person("Sam Lee", &[("mobile", "2")], &[]),
        ];
        assert!(matches!(
            resolve("Sam Lee", twins, Channel::Text, &none()),
            Resolved::Ambiguous(_)
        ));
    }

    #[test]
    fn an_email_needs_an_email_address() {
        let doug = person("Doug", &[("mobile", "704 555 0100")], &[]);
        assert_eq!(
            resolve("Doug", vec![doug], Channel::Email, &none()),
            Resolved::NoAddress("Doug".to_string())
        );
        assert_eq!(resolve("Zed", vec![], Channel::Email, &none()), Resolved::NotFound);
        assert!(from_typed_address("704 555 0100", Channel::Email, &none()).is_none());
        let typed = from_typed_address("doug@example.com", Channel::Email, &none());
        assert_eq!(typed.map(|r| r.address), Some("doug@example.com".to_string()));
    }

    #[test]
    fn unresolved_says_nothing_was_sent() {
        let out = unresolved_result(
            "Doug",
            &Resolved::Ambiguous(vec!["Doug A".to_string(), "Doug B".to_string()]),
            Channel::Text,
        );
        assert_eq!(out["sent"], false);
        assert_eq!(out["candidates"][1], "Doug B");
        let summary = out["summary"].as_str().unwrap_or_default();
        assert!(summary.contains("Which one?") && summary.contains("Nothing was sent."));
    }
}
