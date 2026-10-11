//! Directions in Maps.
//!
//! One URL, opened with `open` and the URL as an argument. Nothing the person
//! said touches a shell or a script: it is percent-encoded into the query.

use std::time::Duration;

use serde_json::{json, Value};

use super::proc;
use super::{required_text, text_arg};

const OPEN_WAIT: Duration = Duration::from_secs(10);

/// How the person is getting there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Driving,
    Walking,
    Transit,
}

impl Mode {
    /// The `dirflg` value Maps reads.
    pub fn flag(self) -> &'static str {
        match self {
            Mode::Driving => "d",
            Mode::Walking => "w",
            Mode::Transit => "r",
        }
    }

    pub fn parse(text: &str) -> Option<Mode> {
        match text.trim().to_lowercase().as_str() {
            "driving" | "drive" | "car" => Some(Mode::Driving),
            "walking" | "walk" | "on foot" => Some(Mode::Walking),
            "transit" | "public transit" | "bus" | "train" => Some(Mode::Transit),
            _ => None,
        }
    }
}

/// Percent-encode one query value: letters, digits and `-._~` stay, every other
/// byte of the UTF-8 text becomes `%XX`.
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The URL Maps opens for a route. `from` left out means "where I am".
pub fn directions_url(to: &str, from: Option<&str>, mode: Option<Mode>) -> String {
    let mut url = format!("maps://?daddr={}", encode(to));
    if let Some(from) = from {
        url.push_str(&format!("&saddr={}", encode(from)));
    }
    if let Some(mode) = mode {
        url.push_str(&format!("&dirflg={}", mode.flag()));
    }
    url
}

/// The card: a link tag with the title as a JSON string, so a quote in a place
/// name cannot break the markup.
pub fn link_card(url: &str, title: &str) -> String {
    let url_json = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".to_string());
    let title_json = serde_json::to_string(title).unwrap_or_else(|_| "\"\"".to_string());
    format!("<LinkCard url={{{url_json}}} title={{{title_json}}} />")
}

/// `maps_directions`
pub fn directions(input: &Value) -> Result<Value, String> {
    let to = required_text(input, "to")?;
    let from = text_arg(input, "from");
    let mode = match text_arg(input, "mode") {
        Some(text) => Some(Mode::parse(text).ok_or_else(|| {
            format!("{text:?} is not a way to travel. Use driving, walking or transit.")
        })?),
        None => None,
    };
    let url = directions_url(to, from, mode);
    let ran = proc::run("open", std::slice::from_ref(&url), None, OPEN_WAIT)?;
    if !ran.ok {
        return Err("Maps would not open.".to_string());
    }
    let title = format!("Directions to {to}");
    Ok(json!({
        "ok": true,
        "summary": format!("Directions to {to} in Maps."),
        "card": link_card(&url, &title),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_is_percent_encoded_byte_by_byte() {
        assert_eq!(
            encode("Charlotte Douglas Airport"),
            "Charlotte%20Douglas%20Airport"
        );
        assert_eq!(
            encode("Joe's & Sons, 5th Ave"),
            "Joe%27s%20%26%20Sons%2C%205th%20Ave"
        );
        assert_eq!(encode("Café"), "Caf%C3%A9");
        assert_eq!(encode("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn nothing_can_add_a_parameter_or_end_the_url() {
        let url = directions_url("home&dirflg=w#x", None, None);
        assert_eq!(url, "maps://?daddr=home%26dirflg%3Dw%23x");
        assert!(!url.contains("&dirflg"));
    }

    #[test]
    fn modes_map_to_the_three_flags() {
        assert_eq!(
            directions_url("A", None, Some(Mode::Driving)),
            "maps://?daddr=A&dirflg=d"
        );
        assert_eq!(
            directions_url("A", None, Some(Mode::Walking)),
            "maps://?daddr=A&dirflg=w"
        );
        assert_eq!(
            directions_url("A", None, Some(Mode::Transit)),
            "maps://?daddr=A&dirflg=r"
        );
        assert_eq!(Mode::parse("Walking"), Some(Mode::Walking));
        assert_eq!(Mode::parse("by helicopter"), None);
    }

    #[test]
    fn a_start_point_goes_in_as_saddr_before_the_mode() {
        assert_eq!(
            directions_url("The Airport", Some("123 Main St"), Some(Mode::Transit)),
            "maps://?daddr=The%20Airport&saddr=123%20Main%20St&dirflg=r"
        );
    }

    #[test]
    fn the_card_survives_a_quote_in_the_name() {
        let card = link_card("maps://?daddr=x", "Directions to \"Joe's\"");
        assert_eq!(
            card,
            "<LinkCard url={\"maps://?daddr=x\"} title={\"Directions to \\\"Joe's\\\"\"} />"
        );
    }

    #[test]
    fn a_missing_destination_is_a_plain_error() {
        assert_eq!(directions(&json!({})), Err("Missing to.".to_string()));
        assert!(directions(&json!({"to": "A", "mode": "teleport"})).is_err());
    }
}
