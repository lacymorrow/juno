//! The send gate: what a text or an email looks like while it waits, the one
//! phrase that sends it, and what Juno says afterwards.
//!
//! Decided by Lacy, 2026-10-08: a send needs the phrase "send it" (or "send"
//! as the whole utterance) or the Send button. A bare "yes" never sends, so
//! half a sentence and a pause ("yes, but change...") cannot misfire. Anything
//! else the person says while the card is up is a correction: nothing is sent,
//! and the agent is handed their words to redo the message.
//!
//! Pure, so every rule here is tested without a Mac.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde::Serialize;
use serde_json::{json, Value};

use super::recipients::Recipient;

/// How long a send waits for "send it". Longer than other asks: the person
/// may be reading the message back before they answer.
pub const SEND_TIMEOUT_SECS: u64 = 120;

/// The line every send prompt ends with, so the phrase is never a secret.
pub const SAY_SEND_IT: &str = "Say send it.";

// ---------------------------------------------------------------------------
// The phrase
// ---------------------------------------------------------------------------

/// Lower case, punctuation folded to spaces (apostrophes kept so "don't" stays
/// one word), whitespace collapsed.
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '\'' || c == '\u{2019}' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether the whole utterance is the phrase that sends.
///
/// "send it" and "send", alone, with "please" allowed either side. Nothing else:
/// "yes", "yes send", "don't send it", "send it later" and "send it to Katie
/// instead" do not send.
pub fn is_send_phrase(text: &str) -> bool {
    // A question is never the answer: "send it?" is someone checking.
    if text.contains('?') {
        return false;
    }
    let normalized = normalize(text);
    let core = normalized
        .strip_prefix("please ")
        .unwrap_or(&normalized)
        .trim();
    let core = core.strip_suffix(" please").unwrap_or(core).trim();
    matches!(core, "send it" | "send")
}

/// What the person's words mean while a send is waiting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendReply {
    /// "send it": send.
    Send,
    /// "no", "cancel", "don't send it": nothing goes out.
    Cancel,
    /// A clean yes that is not the phrase: keep the card up and say the phrase.
    SayThePhrase,
    /// Anything else: nothing goes out and the agent redoes the message with
    /// these words.
    Correction(String),
}

/// Classify a reply to a waiting send.
///
/// `clean_yes_or_no` is the general approval parser
/// (`cli_approval::parse_spoken_approval`), passed in so this stays pure and
/// the two cannot disagree about what "no" means.
pub fn classify_reply(
    text: &str,
    clean_yes_or_no: impl FnOnce(&str) -> Option<bool>,
) -> Option<SendReply> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_send_phrase(trimmed) {
        return Some(SendReply::Send);
    }
    match clean_yes_or_no(trimmed) {
        Some(false) => Some(SendReply::Cancel),
        Some(true) => Some(SendReply::SayThePhrase),
        None => Some(SendReply::Correction(trimmed.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Corrections, handed from the voice path to the waiting gate
// ---------------------------------------------------------------------------

/// The person's words for a send they changed, keyed by the approval id. The
/// voice path writes, the gate that is waiting reads once and forgets.
static CORRECTIONS: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Remember what the person said instead of "send it".
pub fn record_correction(approval_id: &str, text: &str) {
    if let Ok(mut map) = CORRECTIONS.lock() {
        map.insert(approval_id.to_string(), text.to_string());
    }
}

/// The correction for this approval, if there was one. Forgets it.
pub fn take_correction(approval_id: &str) -> Option<String> {
    CORRECTIONS
        .lock()
        .ok()
        .and_then(|mut map| map.remove(approval_id))
}

// ---------------------------------------------------------------------------
// The message as it will go out
// ---------------------------------------------------------------------------

/// Text or email.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SendKind {
    Text,
    Email,
}

/// Where the card is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CardState {
    /// Waiting for "send it".
    Draft,
    /// Approved, going out.
    Sending,
    /// Read back from the app.
    Sent,
}

impl CardState {
    pub fn as_str(self) -> &'static str {
        match self {
            CardState::Draft => "draft",
            CardState::Sending => "sending",
            CardState::Sent => "sent",
        }
    }
}

/// A text or an email, with the recipient as Contacts resolved them.
#[derive(Debug, Clone, PartialEq)]
pub struct SendDraft {
    pub kind: SendKind,
    pub recipient: Recipient,
    pub subject: Option<String>,
    pub body: String,
}

/// The fields the card draws.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CardData {
    pub kind: SendKind,
    pub to: String,
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub body: String,
    /// The line Juno says while the card waits.
    pub prompt: String,
}

fn clip(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push_str("...");
    out
}

/// "Doug" for "Doug Keesler": the name a person says out loud.
fn first_name(name: &str) -> &str {
    if name.contains('@') || name.starts_with('+') {
        return name;
    }
    name.split_whitespace().next().unwrap_or(name)
}

impl SendDraft {
    /// What Juno says while the card waits: "To Doug: I'm running late. Say send it."
    pub fn prompt(&self) -> String {
        let to = first_name(&self.recipient.name);
        let body = clip(self.body.trim(), 240);
        let body = body.trim_end_matches(['.', '!', '?']);
        match (&self.kind, &self.subject) {
            (SendKind::Email, Some(subject)) if !subject.trim().is_empty() => format!(
                "Email to {to}, {}: {body}. {SAY_SEND_IT}",
                clip(subject.trim(), 80)
            ),
            (SendKind::Email, _) => format!("Email to {to}: {body}. {SAY_SEND_IT}"),
            (SendKind::Text, _) => format!("To {to}: {body}. {SAY_SEND_IT}"),
        }
    }

    pub fn card_data(&self) -> CardData {
        CardData {
            kind: self.kind,
            to: self.recipient.name.clone(),
            address: self.recipient.address.clone(),
            service: self.recipient.service.map(|s| s.as_str().to_string()),
            subject: self.subject.clone(),
            body: self.body.clone(),
            prompt: self.prompt(),
        }
    }

    /// The approval payload: the card's fields as JSON.
    pub fn approval_payload(&self) -> Value {
        serde_json::to_value(self.card_data()).unwrap_or(Value::Null)
    }

    /// The finished `<MessageCard>` tag, the way `<AgendaCard>` is emitted.
    pub fn card_tag(&self, state: CardState) -> String {
        let data = self.card_data();
        let attr = |v: &str| serde_json::to_string(v).unwrap_or_else(|_| "\"\"".to_string());
        let mut tag = format!(
            "<MessageCard state=\"{}\" kind=\"{}\" to={{{}}} address={{{}}}",
            state.as_str(),
            match data.kind {
                SendKind::Text => "text",
                SendKind::Email => "email",
            },
            attr(&data.to),
            attr(&data.address),
        );
        if let Some(subject) = &data.subject {
            tag.push_str(&format!(" subject={{{}}}", attr(subject)));
        }
        tag.push_str(&format!(" body={{{}}} />", attr(&data.body)));
        tag
    }

    /// The sentence a gate shows where it cannot draw the card.
    pub fn description(&self) -> String {
        let to = &self.recipient.name;
        let body = clip(self.body.trim(), 80);
        match self.kind {
            SendKind::Text => format!("Text {to}: {body}"),
            SendKind::Email => match &self.subject {
                Some(subject) if !subject.trim().is_empty() => {
                    format!("Email {to}: {}", clip(subject.trim(), 60))
                }
                _ => format!("Email {to}: {body}"),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// After the ask
// ---------------------------------------------------------------------------

/// What the model reads when a send did not go out because of the ask.
pub fn not_sent_text(correction: Option<&str>, timed_out: bool) -> String {
    match correction {
        Some(words) => format!(
            "Not sent. While the message was waiting the person said: \"{words}\". Treat that as \
             a change to the message, or to who it goes to, and call the send tool again with the \
             change so they see the new version. If they meant to stop, say it was not sent."
        ),
        None if timed_out => "Not sent: nobody said send it in time. Tell the person it was not \
             sent; they can ask again."
            .to_string(),
        None => "Not sent: the person cancelled it. Say it was not sent and move on.".to_string(),
    }
}

/// When there is more than one send in a turn, or a send next to other tools.
pub const ONE_SEND_AT_A_TIME: &str = "Not sent. Each message has to be shown to the person on \
     its own before it goes: call the send tool by itself, one message per turn.";

/// What the app showed after a send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence {
    /// Read back from the app as gone out.
    Confirmed,
    /// The app has it but it has not gone out yet.
    Waiting,
    /// The app says it failed.
    Failed,
    /// Nothing could be read back. Carries what was observed, in a sentence.
    Unseen(String),
}

/// The answer to a send, worded from what was read back. "Sent." only on
/// evidence.
pub fn sent_result(draft: &SendDraft, evidence: &Evidence) -> Value {
    let (ok, sent, summary, state) = match evidence {
        Evidence::Confirmed => (true, true, "Sent.".to_string(), Some(CardState::Sent)),
        Evidence::Waiting => (
            true,
            false,
            match draft.kind {
                SendKind::Text => "Messages has it, but it has not gone out yet.".to_string(),
                SendKind::Email => "Mail has it in the outbox, but it has not gone out yet.".to_string(),
            },
            Some(CardState::Sending),
        ),
        Evidence::Failed => (
            false,
            false,
            match draft.kind {
                SendKind::Text => "Messages could not send it.".to_string(),
                SendKind::Email => "Mail could not send it.".to_string(),
            },
            None,
        ),
        Evidence::Unseen(observed) => (true, false, observed.clone(), Some(CardState::Sending)),
    };
    let mut out = json!({
        "ok": ok,
        "sent": sent,
        "summary": summary,
        "to": draft.recipient.name,
        "address": draft.recipient.address,
    });
    if let (Some(state), Some(map)) = (state, out.as_object_mut()) {
        map.insert("card".to_string(), Value::String(draft.card_tag(state)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools::mac_apps::recipients::Service;

    /// A stand-in for the general yes/no parser with the same answers on these.
    fn yes_no(text: &str) -> Option<bool> {
        match normalize(text).as_str() {
            "yes" | "yeah" | "ok" | "okay" | "sure" | "yes send" | "send it later" => Some(true),
            "no" | "cancel" | "don't send it" | "do not send it" | "nope" => Some(false),
            _ => None,
        }
    }

    fn doug() -> SendDraft {
        SendDraft {
            kind: SendKind::Text,
            recipient: Recipient {
                name: "Doug Keesler".to_string(),
                address: "+17045550100".to_string(),
                service: Some(Service::IMessage),
            },
            subject: None,
            body: "I'm running ten minutes late".to_string(),
        }
    }

    #[test]
    fn send_it_sends_and_so_does_send_alone() {
        for phrase in [
            "send it",
            "Send it.",
            "send it!",
            "  SEND IT  ",
            "send",
            "Send.",
            "please send it",
            "send it please",
        ] {
            assert!(is_send_phrase(phrase), "{phrase:?} should send");
        }
    }

    #[test]
    fn near_misses_never_send() {
        for near in [
            "yes",
            "Yes.",
            "yes send",
            "yes, send it",
            "don't send it",
            "do not send it",
            "send it later",
            "send it to Katie instead",
            "sent",
            "sending",
            "send it?",
            "okay",
            "",
            "it",
            "send send",
            "resend it",
        ] {
            assert!(!is_send_phrase(near), "{near:?} must not send");
            assert_ne!(
                classify_reply(near, yes_no),
                Some(SendReply::Send),
                "{near:?} classified as send"
            );
        }
    }

    #[test]
    fn a_bare_yes_asks_for_the_phrase_and_a_no_cancels() {
        assert_eq!(classify_reply("yes", yes_no), Some(SendReply::SayThePhrase));
        assert_eq!(classify_reply("okay", yes_no), Some(SendReply::SayThePhrase));
        assert_eq!(classify_reply("no", yes_no), Some(SendReply::Cancel));
        assert_eq!(
            classify_reply("don't send it", yes_no),
            Some(SendReply::Cancel)
        );
        assert_eq!(classify_reply("   ", yes_no), None);
    }

    #[test]
    fn anything_else_is_a_correction_and_carries_the_words() {
        assert_eq!(
            classify_reply("make it twenty minutes", yes_no),
            Some(SendReply::Correction("make it twenty minutes".to_string()))
        );
        assert_eq!(
            classify_reply("send it to Katie instead", yes_no),
            Some(SendReply::Correction("send it to Katie instead".to_string()))
        );
    }

    #[test]
    fn a_correction_is_handed_over_once() {
        record_correction("abc", "make it twenty");
        assert_eq!(take_correction("abc").as_deref(), Some("make it twenty"));
        assert_eq!(take_correction("abc"), None);
        let text = not_sent_text(Some("make it twenty"), false);
        assert!(text.starts_with("Not sent."));
        assert!(text.contains("make it twenty"));
    }

    #[test]
    fn the_prompt_names_the_person_the_message_and_the_phrase() {
        assert_eq!(
            doug().prompt(),
            "To Doug: I'm running ten minutes late. Say send it."
        );
        let mut email = doug();
        email.kind = SendKind::Email;
        email.subject = Some("Thursday".to_string());
        email.body = "See you at nine.".to_string();
        assert_eq!(
            email.prompt(),
            "Email to Doug, Thursday: See you at nine. Say send it."
        );
    }

    #[test]
    fn the_card_tag_carries_every_field_as_json() {
        let tag = doug().card_tag(CardState::Draft);
        assert!(tag.starts_with("<MessageCard state=\"draft\" kind=\"text\""), "{tag}");
        assert!(tag.contains("to={\"Doug Keesler\"}"), "{tag}");
        assert!(tag.contains("body={\"I'm running ten minutes late\"}"), "{tag}");
        assert!(tag.ends_with(" />"), "{tag}");
        let mut quoted = doug();
        quoted.body = "say \"hi\" {now}".to_string();
        let tag = quoted.card_tag(CardState::Sent);
        assert!(tag.contains("body={\"say \\\"hi\\\" {now}\"}"), "{tag}");
    }

    #[test]
    fn sent_is_said_only_on_evidence() {
        let draft = doug();
        let confirmed = sent_result(&draft, &Evidence::Confirmed);
        assert_eq!(confirmed["summary"], "Sent.");
        assert_eq!(confirmed["sent"], true);
        assert!(confirmed["card"]
            .as_str()
            .unwrap_or_default()
            .contains("state=\"sent\""));

        for evidence in [
            Evidence::Waiting,
            Evidence::Failed,
            Evidence::Unseen("Messages took it, but I could not see it go out.".to_string()),
        ] {
            let out = sent_result(&draft, &evidence);
            assert_eq!(out["sent"], false, "{evidence:?}");
            assert_ne!(out["summary"], "Sent.", "{evidence:?}");
            assert!(
                !out["card"].as_str().unwrap_or_default().contains("state=\"sent\""),
                "{evidence:?}"
            );
        }
    }
}
