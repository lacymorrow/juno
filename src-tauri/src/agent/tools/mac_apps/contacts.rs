//! Contacts.
//!
//! Finds a person by name, or by how they relate to the person asking ("my
//! wife"), which is read off the person's own card. Read only.

use std::sync::Mutex;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2::Message;
use objc2_contacts::{
    CNAuthorizationStatus, CNContact, CNContactEmailAddressesKey, CNContactFamilyNameKey,
    CNContactGivenNameKey, CNContactNicknameKey, CNContactOrganizationNameKey,
    CNContactPhoneNumbersKey, CNContactRelationsKey, CNContactStore, CNEntityType, CNKeyDescriptor,
    CNPhoneNumber,
};
use objc2_foundation::{NSArray, NSError, NSString};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::oneshot;

use super::eventkit;
use super::*;

/// The most people one answer carries.
const MAX_PEOPLE: usize = 5;

/// How long to wait for a person to answer the system dialog.
const DIALOG_WAIT_SECS: u64 = 120;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Labeled {
    pub(crate) label: String,
    pub(crate) value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Person {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    organization: Option<String>,
    pub(crate) phones: Vec<Labeled>,
    pub(crate) emails: Vec<Labeled>,
    /// Other people this card says they are related to: "mother", "Jane Doe".
    related: Vec<Labeled>,
    #[serde(skip)]
    identifier: String,
}

// ---------------------------------------------------------------------------
// Pure: names, labels, relationships
// ---------------------------------------------------------------------------

/// Contacts stores its built-in labels wrapped, as `_$!<Mobile>!$_`.
pub fn plain_label(raw: &str) -> &str {
    raw.strip_prefix("_$!<")
        .and_then(|rest| rest.strip_suffix(">!$_"))
        .unwrap_or(raw)
}

/// The name a person would say: "Katie Berrier", else a nickname, else a company.
pub fn display_name(given: &str, family: &str, nickname: &str, organization: &str) -> String {
    let full = format!("{} {}", given.trim(), family.trim())
        .trim()
        .to_string();
    if !full.is_empty() {
        return full;
    }
    [nickname.trim(), organization.trim()]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// The relationship labels a phrase like "my wife" stands for, folded. `None`
/// when the phrase is not a relationship at all.
pub fn relationship_labels(query: &str) -> Option<Vec<&'static str>> {
    let folded = fold(query);
    let word = ["my ", "the "]
        .iter()
        .find_map(|p| folded.strip_prefix(p))
        .unwrap_or(&folded)
        .trim();
    let labels: &[&'static str] = match word {
        "wife" | "husband" | "spouse" | "partner" => {
            &["spouse", "wife", "husband", "partner", "significant other"]
        }
        "mom" | "mother" | "mum" | "mama" => &["mother", "mom"],
        "dad" | "father" | "papa" => &["father", "dad"],
        "son" => &["son", "child"],
        "daughter" => &["daughter", "child"],
        "kid" | "child" => &["child", "son", "daughter"],
        "brother" => &["brother", "sibling"],
        "sister" => &["sister", "sibling"],
        "boss" | "manager" => &["manager", "boss"],
        "assistant" => &["assistant"],
        "friend" => &["friend"],
        _ => return None,
    };
    Some(labels.to_vec())
}

/// Did the person say "my ..."? A bare "Mom" might be a contact's actual name.
pub fn is_possessive(query: &str) -> bool {
    fold(query).starts_with("my ")
}

/// Does a relation's label (`_$!<Spouse>!$_`, `wife`) match one of the wanted ones?
pub fn label_matches(raw_label: &str, wanted: &[&str]) -> bool {
    let label = fold(plain_label(raw_label));
    wanted.iter().any(|w| *w == label)
}

// ---------------------------------------------------------------------------
// Access
// ---------------------------------------------------------------------------

fn status() -> Access {
    // SAFETY: a class method that only reads the current answer.
    let raw: CNAuthorizationStatus =
        unsafe { CNContactStore::authorizationStatusForEntityType(CNEntityType::Contacts) };
    access_from_contacts(raw.0)
}

/// Show the system dialog and wait for the answer.
fn request() -> Option<Access> {
    let (tx, rx) = oneshot::channel::<bool>();
    let slot = Mutex::new(Some(tx));
    // SAFETY: `+new` on a plain NSObject subclass.
    let store = unsafe { CNContactStore::new() };
    let block = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
        if let Ok(mut guard) = slot.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(granted.as_bool());
            }
        }
    });
    // SAFETY: the block and the store both outlive the wait below.
    unsafe { store.requestAccessForEntityType_completionHandler(CNEntityType::Contacts, &block) };
    eventkit::wait(rx, DIALOG_WAIT_SECS)?;
    Some(status())
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Every field read below has to be named here first, or Contacts throws.
fn keys() -> Retained<NSArray<ProtocolObject<dyn CNKeyDescriptor>>> {
    // SAFETY: these are constant strings exported by the framework.
    let names: [&'static NSString; 7] = unsafe {
        [
            CNContactGivenNameKey,
            CNContactFamilyNameKey,
            CNContactNicknameKey,
            CNContactOrganizationNameKey,
            CNContactPhoneNumbersKey,
            CNContactEmailAddressesKey,
            CNContactRelationsKey,
        ]
    };
    let keys: Vec<Retained<ProtocolObject<dyn CNKeyDescriptor>>> = names
        .iter()
        .map(|key| ProtocolObject::from_retained(key.retain()))
        .collect();
    NSArray::from_retained_slice(&keys)
}

/// The relations a card lists: (label, name).
fn relations(contact: &CNContact) -> Vec<(String, String)> {
    // SAFETY: property reads on a card fetched with the relations key.
    let relations = unsafe { contact.contactRelations() };
    relations
        .to_vec()
        .iter()
        .map(|entry| {
            // SAFETY: property reads on live labeled values.
            unsafe {
                (
                    entry.label().map(|l| l.to_string()).unwrap_or_default(),
                    entry.value().name().to_string(),
                )
            }
        })
        .collect()
}

fn person(contact: &CNContact) -> Person {
    // SAFETY: property reads on a card fetched with these keys.
    let (name, organization, phones, emails, identifier) = unsafe {
        let org = contact.organizationName().to_string();
        let name = display_name(
            &contact.givenName().to_string(),
            &contact.familyName().to_string(),
            &contact.nickname().to_string(),
            &org,
        );
        let phones: Vec<Labeled> = contact
            .phoneNumbers()
            .to_vec()
            .iter()
            .map(|p| Labeled {
                label: p
                    .label()
                    .map(|l| plain_label(&l.to_string()).to_string())
                    .unwrap_or_default(),
                value: p.value().stringValue().to_string(),
            })
            .collect();
        let emails: Vec<Labeled> = contact
            .emailAddresses()
            .to_vec()
            .iter()
            .map(|e| Labeled {
                label: e
                    .label()
                    .map(|l| plain_label(&l.to_string()).to_string())
                    .unwrap_or_default(),
                value: e.value().to_string(),
            })
            .collect();
        (name, org, phones, emails, contact.identifier().to_string())
    };
    let related = relations(contact)
        .into_iter()
        .map(|(label, name)| Labeled {
            label: plain_label(&label).to_string(),
            value: name,
        })
        .collect();
    Person {
        name,
        organization: Some(organization).filter(|o| !o.is_empty()),
        phones,
        emails,
        related,
        identifier,
    }
}

/// Everyone Contacts matches to a name.
fn search(
    store: &CNContactStore,
    keys: &NSArray<ProtocolObject<dyn CNKeyDescriptor>>,
    name: &str,
) -> Result<Vec<Person>, String> {
    // SAFETY: a predicate from a live string, then a read on a live store.
    let found = unsafe {
        let predicate = CNContact::predicateForContactsMatchingName(&NSString::from_str(name));
        store.unifiedContactsMatchingPredicate_keysToFetch_error(&predicate, keys)
    }
    .map_err(|e| format!("Contacts would not search: {}", eventkit::ns_error(&e)))?;
    Ok(found.to_vec().iter().map(|c| person(c)).collect())
}

/// The names the person's own card lists under these relationship labels.
fn related_names(
    store: &CNContactStore,
    keys: &NSArray<ProtocolObject<dyn CNKeyDescriptor>>,
    wanted: &[&str],
) -> Option<Vec<String>> {
    // SAFETY: a read on a live store. Fails when no card is marked as the person's own.
    let me = unsafe { store.unifiedMeContactWithKeysToFetch_error(keys) }.ok()?;
    Some(
        relations(&me)
            .into_iter()
            .filter(|(label, _)| label_matches(label, wanted))
            .map(|(_, name)| name)
            .filter(|name| !name.trim().is_empty())
            .collect(),
    )
}

/// The people a query found and the line said if a dialog ran, or a finished
/// answer to hand back as is.
pub(crate) type Found = Result<(Vec<Person>, Option<String>), Value>;

/// Everyone a name or a relationship stands for, with access cleared first.
///
/// `Ok(Err(answer))` is a finished answer to hand back as is: access was
/// declined, or "my wife" was asked with no card marked as the person's own.
/// Shared by `contacts_find` and every tool that sends to a person by name.
pub(crate) fn people_for(query: &str) -> Result<Found, String> {
    let asked = match clear(Domain::Contacts, status(), request) {
        Cleared::Go { asked } => asked,
        Cleared::Stop(answer) => return Ok(Err(answer)),
    };

    // SAFETY: `+new` on a plain NSObject subclass.
    let store = unsafe { CNContactStore::new() };
    let keys = keys();

    let mut people: Vec<Person> = Vec::new();
    if let Some(wanted) = relationship_labels(query) {
        match related_names(&store, &keys, &wanted) {
            Some(names) => {
                for name in names {
                    people.extend(search(&store, &keys, &name)?);
                }
            }
            None if is_possessive(query) => {
                return Ok(Err(json!({
                    "ok": false,
                    "summary": format!("Contacts does not know which card is yours, so I cannot tell who your {} is. Ask for a name instead.",
                        fold(query).trim_start_matches("my ").trim_start_matches("the ")),
                })));
            }
            None => {}
        }
        // "Mom" on its own may be a contact's actual name.
        if people.is_empty() && !is_possessive(query) {
            people = search(&store, &keys, query)?;
        }
    } else {
        people = search(&store, &keys, query)?;
    }

    let mut seen: Vec<String> = Vec::new();
    people.retain(|p| {
        let fresh = !seen.contains(&p.identifier);
        seen.push(p.identifier.clone());
        fresh
    });
    people.truncate(MAX_PEOPLE);
    Ok(Ok((people, asked)))
}

/// A person as the recipient logic sees them.
pub(crate) fn candidate(person: &Person) -> super::recipients::Candidate {
    super::recipients::Candidate {
        name: person.name.clone(),
        phones: person
            .phones
            .iter()
            .map(|p| (p.label.clone(), p.value.clone()))
            .collect(),
        emails: person
            .emails
            .iter()
            .map(|e| (e.label.clone(), e.value.clone()))
            .collect(),
    }
}

/// The name on the card that has this phone number or email address, when
/// Contacts access is already on. Never asks: labelling a sender is not worth
/// a dialog, and the address is shown instead.
pub(crate) fn name_for_address(address: &str) -> Option<String> {
    if status() != Access::Granted {
        return None;
    }
    // SAFETY: `+new` on a plain NSObject subclass.
    let store = unsafe { CNContactStore::new() };
    let keys = keys();
    // SAFETY: predicates built from live strings, then a read on a live store.
    let found = unsafe {
        let predicate = if address.contains('@') {
            CNContact::predicateForContactsMatchingEmailAddress(&NSString::from_str(address))
        } else {
            let number = CNPhoneNumber::phoneNumberWithStringValue(&NSString::from_str(address))?;
            CNContact::predicateForContactsMatchingPhoneNumber(&number)
        };
        store.unifiedContactsMatchingPredicate_keysToFetch_error(&predicate, &keys)
    }
    .ok()?;
    found
        .to_vec()
        .first()
        .map(|c| person(c).name)
        .filter(|name| !name.is_empty())
}

/// `contacts_find`
pub fn find(input: &Value) -> Result<Value, String> {
    let query = required_text(input, "query")?;
    let (people, asked) = match people_for(query)? {
        Ok(found) => found,
        Err(answer) => return Ok(answer),
    };

    let summary = if people.is_empty() {
        format!("Nobody in Contacts matches {query}.")
    } else {
        people.iter().map(describe).collect::<Vec<_>>().join("; ")
    };
    let mut out = json!({
        "ok": true,
        "count": people.len(),
        "summary": summary,
        "people": people,
    });
    if let (Some(line), Some(map)) = (asked, out.as_object_mut()) {
        map.insert("asked".to_string(), Value::String(line));
    }
    Ok(out)
}

/// One person as a sentence the model can read out.
fn describe(person: &Person) -> String {
    let phones = person
        .phones
        .iter()
        .map(|p| format!("{} {}", p.label, p.value).trim().to_string());
    let emails = person.emails.iter().map(|e| e.value.clone());
    let details: Vec<String> = phones.chain(emails).collect();
    if details.is_empty() {
        format!("{} (no number or email)", person.name)
    } else {
        format!("{}: {}", person.name, details.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_labels_lose_their_wrapper() {
        assert_eq!(plain_label("_$!<Mobile>!$_"), "Mobile");
        assert_eq!(plain_label("iPhone"), "iPhone");
        assert_eq!(plain_label("_$!<Mobile"), "_$!<Mobile");
    }

    #[test]
    fn a_name_falls_back_to_nickname_then_company() {
        assert_eq!(display_name("Katie", "Berrier", "", ""), "Katie Berrier");
        assert_eq!(display_name("Katie", "", "", ""), "Katie");
        assert_eq!(display_name("", "", "Doug", "Purple Haze"), "Doug");
        assert_eq!(display_name("", "", "", "Purple Haze"), "Purple Haze");
        assert_eq!(display_name("", "", "", ""), "");
    }

    #[test]
    fn my_wife_is_a_relationship_and_a_name_is_not() {
        let spouse = relationship_labels("my wife").unwrap_or_default();
        assert!(spouse.contains(&"spouse") && spouse.contains(&"wife"));
        assert_eq!(
            relationship_labels("My Husband"),
            relationship_labels("my wife")
        );
        assert!(relationship_labels("Mom").is_some());
        assert!(relationship_labels("Katie").is_none());
        assert!(relationship_labels("my accountant").is_none());
    }

    #[test]
    fn only_my_makes_it_possessive() {
        assert!(is_possessive("My wife"));
        assert!(!is_possessive("Mom"));
        assert!(!is_possessive("the manager"));
    }

    #[test]
    fn relation_labels_match_wrapped_and_custom() {
        let wanted = relationship_labels("my wife").unwrap_or_default();
        assert!(label_matches("_$!<Spouse>!$_", &wanted));
        assert!(label_matches("Wife", &wanted));
        assert!(!label_matches("_$!<Mother>!$_", &wanted));
        assert!(!label_matches("", &wanted));
    }

    #[test]
    fn a_person_reads_as_one_sentence() {
        let katie = Person {
            name: "Katie Berrier".to_string(),
            organization: None,
            phones: vec![Labeled {
                label: "mobile".to_string(),
                value: "704 555 0100".to_string(),
            }],
            emails: vec![Labeled {
                label: "work".to_string(),
                value: "katie@example.com".to_string(),
            }],
            related: Vec::new(),
            identifier: "1".to_string(),
        };
        assert_eq!(
            describe(&katie),
            "Katie Berrier: mobile 704 555 0100, katie@example.com"
        );
        let bare = Person {
            phones: Vec::new(),
            emails: Vec::new(),
            ..katie
        };
        assert_eq!(describe(&bare), "Katie Berrier (no number or email)");
    }
}
