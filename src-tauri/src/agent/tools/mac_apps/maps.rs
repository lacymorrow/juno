//! Directions in Maps.
//!
//! One URL, opened with `open` and the URL as an argument. Nothing the person
//! said touches a shell or a script: it is percent-encoded into the query.

use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use super::proc;
use super::{required_text, text_arg};

const OPEN_WAIT: Duration = Duration::from_secs(10);
const READ_WAIT: Duration = Duration::from_secs(5);

/// Maps' bundle id.
const MAPS_BUNDLE: &str = "com.apple.Maps";

/// `open` returns as Maps launches, before it has come to the front. The
/// read-back is of a state this call just caused, so it waits a short, bounded
/// moment for the front app to be Maps.
const FRONT_TRIES: u32 = 6;
const FRONT_EVERY: Duration = Duration::from_millis(250);

/// What the system says about one app right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    pub running: bool,
    pub frontmost: bool,
}

/// Is `lsappinfo find` output a running app? It prints an `ASN:` handle, or
/// nothing when the app is not running.
pub fn parse_running(find_output: &str) -> bool {
    find_output.contains("ASN:")
}

/// Does `lsappinfo info -only bundleid` output name this bundle?
pub fn parse_is_bundle(info_output: &str, bundle: &str) -> bool {
    info_output
        .split(|c: char| c == '"' || c.is_whitespace() || c == '=')
        .any(|part| part == bundle)
}

fn lsappinfo(args: &[&str]) -> Option<String> {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    match proc::run("lsappinfo", &args, None, READ_WAIT) {
        Ok(r) if r.ok => Some(r.stdout),
        _ => None,
    }
}

/// Read back whether an app is running and in front. `lsappinfo` needs no permission.
fn observe(bundle: &str) -> Seen {
    let running = lsappinfo(&["find", &format!("bundleid={bundle}")])
        .map(|out| parse_running(&out))
        .unwrap_or(false);
    let frontmost = running
        && lsappinfo(&["front"])
            .map(|front| front.trim().to_string())
            .filter(|front| !front.is_empty())
            .and_then(|front| lsappinfo(&["info", "-only", "bundleid", &front]))
            .map(|info| parse_is_bundle(&info, bundle))
            .unwrap_or(false);
    Seen { running, frontmost }
}

/// The sentence for what was seen, stated plainly.
pub fn open_summary(to: &str, seen: Seen) -> String {
    match (seen.running, seen.frontmost) {
        (true, true) => format!("Maps is open with directions to {to}."),
        (true, false) => {
            format!("Maps is running with directions to {to}, but another app is still in front.")
        }
        (false, _) => format!("Asked Maps for directions to {to}, but Maps is not running."),
    }
}

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
    let mut seen = observe(MAPS_BUNDLE);
    for _ in 1..FRONT_TRIES {
        if seen.frontmost {
            break;
        }
        thread::sleep(FRONT_EVERY);
        seen = observe(MAPS_BUNDLE);
    }
    let title = format!("Directions to {to}");
    Ok(json!({
        "ok": seen.running,
        "summary": open_summary(to, seen),
        "observed": { "app": "Maps", "running": seen.running, "frontmost": seen.frontmost },
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
    fn the_read_back_says_it_plainly() {
        assert!(parse_running("ASN:0x0-0x1a01a:\n"));
        assert!(!parse_running(""));
        assert!(parse_is_bundle(
            "\"CFBundleIdentifier\"=\"com.apple.Maps\"",
            MAPS_BUNDLE
        ));
        assert!(!parse_is_bundle(
            "\"CFBundleIdentifier\"=\"com.apple.MapsHelper\"",
            MAPS_BUNDLE
        ));
        let up = Seen {
            running: true,
            frontmost: true,
        };
        assert_eq!(
            open_summary("Target", up),
            "Maps is open with directions to Target."
        );
        let behind = Seen {
            running: true,
            frontmost: false,
        };
        assert!(open_summary("Target", behind).contains("another app is still in front"));
        let down = Seen {
            running: false,
            frontmost: false,
        };
        assert!(open_summary("Target", down).contains("not running"));
    }

    #[test]
    fn a_missing_destination_is_a_plain_error() {
        assert_eq!(directions(&json!({})), Err("Missing to.".to_string()));
        assert!(directions(&json!({"to": "A", "mode": "teleport"})).is_err());
    }
}
