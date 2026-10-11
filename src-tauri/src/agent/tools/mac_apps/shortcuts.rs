//! Shortcuts: list the person's own, run one by (spoken) name, and turn Focus
//! on or off through a shortcut Juno ships.
//!
//! This is the "the person extends it" row of `docs/plans/siri-parity.md`: a
//! shortcut can do anything, but the person built it, so a run is Medium risk.
//!
//! Two rules keep it honest. The spoken name is only ever *compared* with the
//! list; what reaches `shortcuts run` is the exact name from that list, as one
//! argument, never through a shell. And a name that fits more than one
//! shortcut runs nothing: the candidates go back so the person picks.
//!
//! ## Focus
//!
//! macOS has no public Focus API, so Do Not Disturb goes through a shortcut
//! named [`FOCUS_SHORTCUT`], bundled with Juno and signed. The first request
//! opens it, which Shortcuts turns into one "Add Shortcut" click; the tool then
//! waits for it to appear and runs it. Every request after that is instant.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::proc;
use super::{required_text, text_arg};

/// A shortcut that needs more than this is stopped. Shortcuts that wait on a
/// person would otherwise hold the call for good.
const RUN_WAIT: Duration = Duration::from_secs(30);
const LIST_WAIT: Duration = Duration::from_secs(10);
/// How long to wait for the person to click Add Shortcut.
const IMPORT_WAIT: Duration = Duration::from_secs(45);
const IMPORT_POLL: Duration = Duration::from_secs(1);

/// The name the bundled Focus shortcut has once imported. Its file name too.
pub const FOCUS_SHORTCUT: &str = "Juno Focus";

/// Most output a shortcut may hand back to the model.
const MAX_OUTPUT_CHARS: usize = 2000;

/// How many near matches are offered.
const MAX_CANDIDATES: usize = 5;

/// A match must beat the runner-up by this much to run without asking.
const MARGIN: f64 = 0.25;

// ---------------------------------------------------------------------------
// Matching a spoken name
// ---------------------------------------------------------------------------

/// Words that carry no weight in a shortcut's name.
const STOP_WORDS: &[&str] = &["the", "a", "an", "my", "shortcut", "please", "run"];

/// What a spoken name came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Matched {
    /// One shortcut, by its exact name.
    One(String),
    /// More than one fits equally well. Nothing runs.
    Ambiguous(Vec<String>),
    /// Nothing fits, but these share a word with it. Nothing runs.
    Near(Vec<String>),
    /// Nothing shares a word with it.
    Nothing,
}

/// A name as a set of words: lower case, punctuation gone, a trailing plural
/// folded, filler dropped. Falls back to every word if dropping filler would
/// leave nothing.
fn words(text: &str) -> BTreeSet<String> {
    let spaced: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let all: Vec<String> = spaced
        .split_whitespace()
        .map(|w| {
            let mut w = w.to_string();
            if w.chars().count() > 3 && w.ends_with('s') && !w.ends_with("ss") {
                w.pop();
            }
            w
        })
        .collect();
    let kept: BTreeSet<String> = all
        .iter()
        .filter(|w| !STOP_WORDS.contains(&w.as_str()))
        .cloned()
        .collect();
    if kept.is_empty() {
        all.into_iter().collect()
    } else {
        kept
    }
}

fn overlap(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    let shared = a.intersection(b).count() as f64;
    let total = a.union(b).count() as f64;
    if total == 0.0 {
        0.0
    } else {
        shared / total
    }
}

/// Fit a spoken name to the person's shortcuts.
///
/// A shortcut fits when every word of one name is inside the other: "porch
/// lights" fits "Front Porch Lights", and "turn on the front porch lights"
/// fits it too (when the shortcut's name has two words or more, so a
/// one-word name is not set off by a long sentence). "Back porch lights" does
/// not fit it: that is a different shortcut, or none.
pub fn match_shortcut(spoken: &str, names: &[String]) -> Matched {
    let said = words(spoken);
    if said.is_empty() {
        return Matched::Nothing;
    }
    let named: Vec<(&String, BTreeSet<String>)> = names.iter().map(|n| (n, words(n))).collect();

    // The same words in the same shortcut name, ignoring case and punctuation.
    let same: Vec<&String> = named
        .iter()
        .filter(|(_, w)| *w == said)
        .map(|(n, _)| *n)
        .collect();
    match same.as_slice() {
        [one] => return Matched::One((*one).clone()),
        [] => {}
        many => {
            // Two shortcuts that read the same: the one written exactly as said wins.
            let literal: Vec<&&String> =
                many.iter().filter(|n| n.trim() == spoken.trim()).collect();
            return match literal.as_slice() {
                [one] => Matched::One((***one).clone()),
                _ => Matched::Ambiguous(many.iter().map(|n| (*n).clone()).collect()),
            };
        }
    }

    let mut fitting: Vec<(f64, &String)> = named
        .iter()
        .filter(|(_, w)| {
            !w.is_empty() && (said.is_subset(w) || (w.len() >= 2 && w.is_subset(&said)))
        })
        .map(|(n, w)| (overlap(&said, w), *n))
        .collect();
    fitting.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    match fitting.as_slice() {
        [] => {}
        [(_, only)] => return Matched::One((*only).clone()),
        [(top, best), (next, _), ..] if top - next >= MARGIN => {
            return Matched::One((*best).clone())
        }
        several => {
            return Matched::Ambiguous(
                several
                    .iter()
                    .take(MAX_CANDIDATES)
                    .map(|(_, n)| (*n).clone())
                    .collect(),
            )
        }
    }

    let mut near: Vec<(usize, &String)> = named
        .iter()
        .map(|(n, w)| (said.intersection(w).count(), *n))
        .filter(|(shared, _)| *shared > 0)
        .collect();
    near.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    if near.is_empty() {
        Matched::Nothing
    } else {
        Matched::Near(
            near.into_iter()
                .take(MAX_CANDIDATES)
                .map(|(_, n)| n.clone())
                .collect(),
        )
    }
}

// ---------------------------------------------------------------------------
// The tools
// ---------------------------------------------------------------------------

/// The person's shortcut names, one per line of `shortcuts list`.
fn list_names() -> Result<Vec<String>, String> {
    let ran = proc::run("shortcuts", &["list".to_string()], None, LIST_WAIT)?;
    if !ran.ok {
        return Err("Shortcuts did not answer.".to_string());
    }
    Ok(ran
        .stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

/// Whether the Juno Focus shortcut is imported. A failed check is a no, so a
/// local intent never fires on a guess.
pub fn focus_installed() -> bool {
    list_names().is_ok_and(|names| names.iter().any(|n| n == FOCUS_SHORTCUT))
}

/// `shortcuts_list`
pub fn list(_input: &Value) -> Result<Value, String> {
    let names = list_names()?;
    let summary = if names.is_empty() {
        "There are no shortcuts in Shortcuts yet.".to_string()
    } else {
        names.join(", ")
    };
    Ok(json!({ "ok": true, "count": names.len(), "summary": summary, "shortcuts": names }))
}

fn clip(text: &str) -> String {
    text.chars().take(MAX_OUTPUT_CHARS).collect()
}

/// Run one shortcut by its exact name.
fn run_exact(name: &str, input: Option<&str>) -> Result<Value, String> {
    let args = vec!["run".to_string(), name.to_string()];
    let ran = proc::run("shortcuts", &args, input, RUN_WAIT)?;
    if ran.timed_out {
        return Ok(json!({
            "ok": false,
            "ran": name,
            "summary": format!("{name} was still going after 30 seconds, so I stopped waiting."),
        }));
    }
    if !ran.ok {
        let why = if ran.stderr.is_empty() {
            String::new()
        } else {
            format!(" {}", clip(&ran.stderr))
        };
        return Err(format!("{name} did not run.{why}"));
    }
    let summary = if ran.stdout.is_empty() {
        format!("Ran {name}.")
    } else {
        clip(&ran.stdout)
    };
    Ok(json!({ "ok": true, "ran": name, "summary": summary }))
}

fn which_one(candidates: &[String]) -> String {
    format!("Which one: {}?", candidates.join(", "))
}

/// `shortcuts_run`
pub fn run(input: &Value) -> Result<Value, String> {
    let spoken = required_text(input, "name")?;
    let names = list_names()?;
    match match_shortcut(spoken, &names) {
        Matched::One(name) => run_exact(&name, text_arg(input, "input")),
        Matched::Ambiguous(candidates) | Matched::Near(candidates) => Ok(json!({
            "ok": false,
            "ran": null,
            "candidates": candidates,
            "summary": which_one(&candidates),
        })),
        Matched::Nothing => Ok(json!({
            "ok": false,
            "ran": null,
            "summary": format!("You have no shortcut called {spoken}."),
        })),
    }
}

/// Where the bundled shortcut lives: inside the app, or in the source tree in
/// a development build.
fn bundled_focus_file() -> Option<PathBuf> {
    let file = format!("{FOCUS_SHORTCUT}.shortcut");
    let exe = std::env::current_exe().ok()?;
    let resources = exe.parent()?.parent()?.join("Resources");
    [
        resources.join("resources").join("shortcuts").join(&file),
        resources.join("shortcuts").join(&file),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("shortcuts")
            .join(&file),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Open the bundled shortcut so Shortcuts offers it, then wait for the person's
/// one click. `true` once it is in the list.
fn offer_focus_shortcut() -> Result<bool, String> {
    let file = bundled_focus_file()
        .ok_or_else(|| "The Juno Focus shortcut is missing from this copy of Juno.".to_string())?;
    let args = vec![file.to_string_lossy().into_owned()];
    let opened = proc::run("open", &args, None, LIST_WAIT)?;
    if !opened.ok {
        return Err("Shortcuts would not open the Juno Focus shortcut.".to_string());
    }
    let started = Instant::now();
    while started.elapsed() < IMPORT_WAIT {
        thread::sleep(IMPORT_POLL);
        if focus_installed() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// What on or off the person asked for.
pub fn focus_state(input: &Value) -> Result<&'static str, String> {
    match required_text(input, "state")?.to_lowercase().as_str() {
        "on" | "true" | "enable" | "enabled" => Ok("on"),
        "off" | "false" | "disable" | "disabled" => Ok("off"),
        other => Err(format!("{other:?} is not a Focus state. Use on or off.")),
    }
}

/// `focus_set`
pub fn focus(input: &Value) -> Result<Value, String> {
    let state = focus_state(input)?;
    let names = list_names()?;
    let mut asked = None;
    if !names.iter().any(|n| n == FOCUS_SHORTCUT) {
        if !offer_focus_shortcut()? {
            return Ok(json!({
                "ok": false,
                "summary": "Click Add Shortcut in the Shortcuts window, then ask me again.",
            }));
        }
        asked = Some("Added the Juno Focus shortcut.");
    }
    let mut out = run_exact(FOCUS_SHORTCUT, Some(state))?;
    if out["ok"] == true {
        out["summary"] = Value::String(format!("Do Not Disturb is {state}."));
    }
    if let (Some(line), Some(map)) = (asked, out.as_object_mut()) {
        map.insert("asked".to_string(), Value::String(line.to_string()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> Vec<String> {
        [
            "Front Porch Lights",
            "Back Porch Lights Off",
            "Morning Routine",
            "Log Water",
            "Resize Image",
            "Juno Focus",
            "Backup",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn one(spoken: &str) -> Option<String> {
        match match_shortcut(spoken, &lib()) {
            Matched::One(n) => Some(n),
            _ => None,
        }
    }

    #[test]
    fn case_and_punctuation_do_not_matter() {
        assert_eq!(one("front porch lights"), Some("Front Porch Lights".into()));
        assert_eq!(
            one("FRONT-PORCH, LIGHTS!"),
            Some("Front Porch Lights".into())
        );
        assert_eq!(one("log water"), Some("Log Water".into()));
    }

    #[test]
    fn a_plural_or_filler_word_does_not_break_the_match() {
        assert_eq!(
            one("the front porch light"),
            Some("Front Porch Lights".into())
        );
        assert_eq!(
            one("run my morning routine shortcut"),
            Some("Morning Routine".into())
        );
        assert_eq!(one("resize images"), Some("Resize Image".into()));
    }

    #[test]
    fn some_of_the_words_is_enough_when_only_one_shortcut_has_them() {
        assert_eq!(one("morning"), Some("Morning Routine".into()));
        assert_eq!(one("water"), Some("Log Water".into()));
    }

    #[test]
    fn a_long_sentence_around_a_two_word_name_still_finds_it() {
        assert_eq!(
            one("turn on the front porch lights now"),
            Some("Front Porch Lights".into())
        );
    }

    #[test]
    fn a_long_sentence_does_not_set_off_a_one_word_name() {
        // "Backup" is one word; "back up my photos" is a different request.
        assert_eq!(one("backup photos to the drive"), None);
        assert_eq!(one("backup"), Some("Backup".into()));
    }

    #[test]
    fn near_misses_run_nothing_and_offer_the_close_ones() {
        // Neither porch shortcut is "side porch lights".
        match match_shortcut("side porch lights", &lib()) {
            Matched::Near(c) => {
                assert!(c.contains(&"Front Porch Lights".to_string()));
                assert!(c.contains(&"Back Porch Lights Off".to_string()));
            }
            other => panic!("expected near matches, got {other:?}"),
        }
        // Different word, same shape.
        assert_eq!(one("rear porch lights"), None);
        assert_eq!(one("front door lights"), None);
    }

    #[test]
    fn two_equal_fits_are_ambiguous() {
        match match_shortcut("porch lights", &lib()) {
            Matched::Ambiguous(c) => assert_eq!(c.len(), 2),
            other => panic!("expected ambiguity, got {other:?}"),
        }
    }

    #[test]
    fn a_clearly_better_fit_wins_over_a_loose_one() {
        let names: Vec<String> = ["Lights", "Front Porch Lights Off Now", "Porch Lights Dim"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            match_shortcut("porch lights", &names),
            Matched::One("Porch Lights Dim".into())
        );
    }

    #[test]
    fn nothing_in_common_is_nothing() {
        assert_eq!(
            match_shortcut("launch the rockets", &lib()),
            Matched::Nothing
        );
        assert_eq!(match_shortcut("   ", &lib()), Matched::Nothing);
        assert_eq!(match_shortcut("porch", &[]), Matched::Nothing);
    }

    #[test]
    fn two_names_that_read_the_same_prefer_the_one_said_exactly() {
        let names: Vec<String> = ["Lights On", "lights on!"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            match_shortcut("Lights On", &names),
            Matched::One("Lights On".into())
        );
        assert!(matches!(
            match_shortcut("lights   on", &names),
            Matched::Ambiguous(_)
        ));
    }

    #[test]
    fn a_name_is_never_changed_on_the_way_to_the_run() {
        // What comes back is the list's own spelling, not the spoken one.
        assert_eq!(
            one("FRONT PORCH LIGHTS"),
            Some("Front Porch Lights".to_string())
        );
    }

    #[test]
    fn focus_states_are_on_or_off_only() {
        assert_eq!(focus_state(&json!({"state": "On"})), Ok("on"));
        assert_eq!(focus_state(&json!({"state": "disable"})), Ok("off"));
        assert!(focus_state(&json!({"state": "until noon"})).is_err());
        assert!(focus_state(&json!({})).is_err());
    }

    #[test]
    fn the_bundled_file_is_named_for_the_shortcut_it_becomes() {
        // Shortcuts names an imported file by its file name.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("shortcuts")
            .join(format!("{FOCUS_SHORTCUT}.shortcut"));
        assert!(
            path.is_file(),
            "{} is not in the source tree",
            path.display()
        );
    }

    #[test]
    fn the_bundled_shortcut_is_in_the_bundle_config() {
        let raw = include_str!("../../../../tauri.conf.json");
        let config: Value = serde_json::from_str(raw).unwrap_or_default();
        let resources = config["bundle"]["resources"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            resources
                .iter()
                .any(|r| r.as_str() == Some("resources/shortcuts/*")),
            "resources/shortcuts/* is not bundled"
        );
    }
}
