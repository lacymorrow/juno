//! Which window an input action is meant for.
//!
//! A click carries only a screen point, and keystrokes carry nothing at all. With
//! several windows stacked at the same spot (six terminal windows of one app, say)
//! the point says which window is on top, not which one the agent meant, and a
//! keystroke posted to a process lands in that process's key window, whichever it
//! is. This module holds the platform-neutral half of fixing that: naming a window,
//! picking it out of the window list, and pinning it for the duration of one
//! action so the platform layer routes events to it.
//!
//! The pin is thread-local on purpose. The no-warp input calls run synchronously
//! on the thread that set the pin, so a pin can never leak into another agent
//! session's action running on a different thread.

use serde_json::Value;
use std::cell::Cell;

/// One on-screen window, as the window server lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowRecord {
    /// CGWindowID: stable for the life of the window.
    pub id: u32,
    pub pid: i32,
    pub owner: String,
    /// Absent without Screen Recording permission.
    pub title: Option<String>,
    /// x, y, width, height in global screen points.
    pub bounds: (f64, f64, f64, f64),
    pub layer: i32,
}

impl WindowRecord {
    fn contains(&self, x: f64, y: f64) -> bool {
        let (wx, wy, ww, wh) = self.bounds;
        x >= wx && x < wx + ww && y >= wy && y < wy + wh
    }

    /// Ordinary app windows and their panels. Menu bar, Dock and overlays at
    /// system levels are never input targets.
    fn is_user_layer(&self) -> bool {
        (-1..20).contains(&self.layer)
    }
}

/// How the agent named a window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowSelector {
    Id(u32),
    Title(String),
}

impl WindowSelector {
    /// Read the optional `window` input parameter.
    ///
    /// Accepts a number (window id), a string (a title, or digits meaning an id),
    /// or an object with `id` and/or `title`. Absent or null means "no window
    /// named", which keeps every existing call working unchanged.
    pub fn from_input(input: &Value) -> Result<Option<Self>, String> {
        let window = match input.get("window") {
            None | Some(Value::Null) => return Ok(None),
            Some(window) => window,
        };
        match window {
            Value::Number(n) => n
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .filter(|id| *id > 0)
                .map(|id| Some(WindowSelector::Id(id)))
                .ok_or_else(|| format!("'window' id {} is not a valid window id", n)),
            Value::String(s) => {
                let s = s.trim();
                if s.is_empty() {
                    return Err("'window' is empty: give a window id or title".to_string());
                }
                match s.parse::<u32>() {
                    Ok(id) if id > 0 => Ok(Some(WindowSelector::Id(id))),
                    _ => Ok(Some(WindowSelector::Title(s.to_string()))),
                }
            }
            Value::Object(map) => {
                if let Some(id) = map.get("id").filter(|v| !v.is_null()) {
                    let mut probe = serde_json::Map::new();
                    probe.insert("window".to_string(), id.clone());
                    return Self::from_input(&Value::Object(probe));
                }
                match map.get("title").and_then(Value::as_str) {
                    Some(title) if !title.trim().is_empty() => {
                        Ok(Some(WindowSelector::Title(title.trim().to_string())))
                    }
                    _ => Err("'window' object needs an 'id' or a 'title'".to_string()),
                }
            }
            other => Err(format!(
                "'window' must be a window id or title, got {}",
                other
            )),
        }
    }
}

/// Pick the window a selector names. Juno's own windows are never candidates.
///
/// Titles match exactly (ignoring case) first; failing that, a unique substring
/// match is accepted. More than one substring match is refused with the
/// candidates listed, because guessing is how keystrokes end up in the wrong
/// terminal.
pub fn select_window<'a>(
    records: &'a [WindowRecord],
    selector: &WindowSelector,
    own_pid: i32,
) -> Result<&'a WindowRecord, String> {
    let mut candidates = records
        .iter()
        .filter(|w| w.pid != own_pid && w.is_user_layer());
    match selector {
        WindowSelector::Id(id) => candidates
            .find(|w| w.id == *id)
            .ok_or_else(|| format!("No window with id {} is on screen", id)),
        WindowSelector::Title(title) => {
            let wanted = title.to_lowercase();
            let titled: Vec<&WindowRecord> = candidates
                .filter(|w| w.title.as_deref().is_some_and(|t| !t.is_empty()))
                .collect();
            if let Some(exact) = titled.iter().find(|w| {
                w.title
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase() == wanted)
            }) {
                return Ok(*exact);
            }
            let partial: Vec<&&WindowRecord> = titled
                .iter()
                .filter(|w| {
                    w.title
                        .as_deref()
                        .is_some_and(|t| t.to_lowercase().contains(&wanted))
                })
                .collect();
            match partial.as_slice() {
                [only] => Ok(**only),
                [] => by_app_name(records, title, own_pid),
                many => Err(format!(
                    "'{}' matches {} windows; pass one of these ids as 'window': {}",
                    title,
                    many.len(),
                    many.iter()
                        .map(|w| format!(
                            "{} ({}: {})",
                            w.id,
                            w.owner,
                            w.title.as_deref().unwrap_or_default()
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        }
    }
}

/// No title matched: an app name ("Calculator", "Zed") names its window when
/// the app has exactly one. More than one is refused with the ids, as for
/// titles.
fn by_app_name<'a>(
    records: &'a [WindowRecord],
    name: &str,
    own_pid: i32,
) -> Result<&'a WindowRecord, String> {
    let wanted = name.to_lowercase();
    let owned: Vec<&WindowRecord> = records
        .iter()
        .filter(|w| w.pid != own_pid && w.is_user_layer() && w.owner.to_lowercase() == wanted)
        .collect();
    match owned.as_slice() {
        [only] => Ok(*only),
        [] => Err(format!(
            "No on-screen window title contains '{}', and no app by that name has a window",
            name
        )),
        many => Err(format!(
            "{} has {} windows; pass one of these ids as 'window': {}",
            name,
            many.len(),
            many.iter()
                .map(|w| format!("{} ({})", w.id, w.title.as_deref().unwrap_or("untitled")))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Whether a point falls on any window of `pid`: the target window itself or
/// one of its app's sheets, popovers and menus. Used to refuse a click aimed at
/// one app that would land on another.
pub fn app_window_contains(records: &[WindowRecord], pid: i32, x: f64, y: f64) -> bool {
    records.iter().any(|w| w.pid == pid && w.contains(x, y))
}

/// The topmost window under a point, front-to-back order assumed.
///
/// Juno's own windows are skipped. Its full-screen overlays (listening, snap
/// wells) are click-through for a real pointer, but the window list still
/// reports them, so without this skip a process-targeted click aimed at the
/// app underneath was posted to Juno instead.
pub fn window_at_point(
    records: &[WindowRecord],
    x: f64,
    y: f64,
    own_pid: i32,
) -> Option<&WindowRecord> {
    records
        .iter()
        .find(|w| w.pid != own_pid && w.is_user_layer() && w.contains(x, y))
}

/// The window an action has been pinned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinnedWindow {
    pub pid: i32,
    pub window_id: u32,
}

thread_local! {
    static PINNED: Cell<Option<PinnedWindow>> = const { Cell::new(None) };
}

/// Restores the previous pin when dropped, so nested or failed actions cannot
/// leave a stale pin behind.
#[must_use = "the window is only pinned while this guard is alive"]
pub struct PinGuard {
    previous: Option<PinnedWindow>,
}

impl Drop for PinGuard {
    fn drop(&mut self) {
        let previous = self.previous;
        PINNED.with(|p| p.set(previous));
    }
}

/// Route this thread's input to `target` until the guard drops. `None` pins
/// nothing but still returns a guard, so call sites need no branch.
pub fn pin(target: Option<PinnedWindow>) -> PinGuard {
    let previous = PINNED.with(|p| p.replace(target.or(p.get())));
    PinGuard { previous }
}

/// The window pinned on this thread, if any.
pub fn pinned() -> Option<PinnedWindow> {
    PINNED.with(|p| p.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn win(id: u32, pid: i32, title: &str, bounds: (f64, f64, f64, f64)) -> WindowRecord {
        WindowRecord {
            id,
            pid,
            owner: "Ghostty".to_string(),
            title: Some(title.to_string()),
            bounds,
            layer: 0,
        }
    }

    const JUNO: i32 = 1;

    fn stacked() -> Vec<WindowRecord> {
        vec![
            // Juno's full-screen overlay is first in front-to-back order.
            WindowRecord {
                id: 900,
                pid: JUNO,
                owner: "Juno".to_string(),
                title: None,
                bounds: (0.0, 0.0, 2560.0, 1600.0),
                layer: 3,
            },
            win(11, 50, "zsh: build", (0.0, 0.0, 800.0, 600.0)),
            win(12, 50, "zsh: logs", (0.0, 0.0, 800.0, 600.0)),
            win(13, 50, "vim notes.md", (0.0, 0.0, 800.0, 600.0)),
        ]
    }

    #[test]
    fn absent_and_null_window_mean_no_selector() {
        assert_eq!(WindowSelector::from_input(&json!({})), Ok(None));
        assert_eq!(
            WindowSelector::from_input(&json!({ "window": null })),
            Ok(None)
        );
    }

    #[test]
    fn numbers_and_digit_strings_are_ids_other_strings_are_titles() {
        assert_eq!(
            WindowSelector::from_input(&json!({ "window": 12 })),
            Ok(Some(WindowSelector::Id(12)))
        );
        assert_eq!(
            WindowSelector::from_input(&json!({ "window": "12" })),
            Ok(Some(WindowSelector::Id(12)))
        );
        assert_eq!(
            WindowSelector::from_input(&json!({ "window": "logs" })),
            Ok(Some(WindowSelector::Title("logs".to_string())))
        );
        assert_eq!(
            WindowSelector::from_input(&json!({ "window": { "title": "logs" } })),
            Ok(Some(WindowSelector::Title("logs".to_string())))
        );
        assert_eq!(
            WindowSelector::from_input(&json!({ "window": { "id": 13 } })),
            Ok(Some(WindowSelector::Id(13)))
        );
    }

    #[test]
    fn nonsense_window_values_are_refused_not_ignored() {
        assert!(WindowSelector::from_input(&json!({ "window": 0 })).is_err());
        assert!(WindowSelector::from_input(&json!({ "window": -3 })).is_err());
        assert!(WindowSelector::from_input(&json!({ "window": "  " })).is_err());
        assert!(WindowSelector::from_input(&json!({ "window": true })).is_err());
        assert!(WindowSelector::from_input(&json!({ "window": {} })).is_err());
    }

    #[test]
    fn a_window_is_found_by_id_regardless_of_stacking() {
        let records = stacked();
        let w = select_window(&records, &WindowSelector::Id(13), JUNO).unwrap();
        assert_eq!(w.id, 13);
    }

    #[test]
    fn titles_match_exactly_first_then_by_unique_substring() {
        let records = stacked();
        let exact =
            select_window(&records, &WindowSelector::Title("ZSH: LOGS".into()), JUNO).unwrap();
        assert_eq!(exact.id, 12);
        let partial = select_window(&records, &WindowSelector::Title("vim".into()), JUNO).unwrap();
        assert_eq!(partial.id, 13);
    }

    #[test]
    fn an_ambiguous_title_is_refused_with_the_candidate_ids() {
        let records = stacked();
        let err = select_window(&records, &WindowSelector::Title("zsh".into()), JUNO).unwrap_err();
        assert!(err.contains("11") && err.contains("12"), "{}", err);
    }

    fn calculator_over_zed() -> Vec<WindowRecord> {
        vec![
            WindowRecord {
                id: 11502,
                pid: 70,
                owner: "Calculator".to_string(),
                title: Some("Calculator".to_string()),
                bounds: (368.0, 410.0, 230.0, 408.0),
                layer: 0,
            },
            WindowRecord {
                id: 106,
                pid: 80,
                owner: "Zed".to_string(),
                title: Some("juno".to_string()),
                bounds: (0.0, 30.0, 934.0, 870.0),
                layer: 0,
            },
        ]
    }

    #[test]
    fn an_app_name_names_its_only_window() {
        let records = calculator_over_zed();
        let zed = select_window(&records, &WindowSelector::Title("zed".into()), JUNO).unwrap();
        assert_eq!(zed.id, 106);
    }

    #[test]
    fn an_app_with_several_windows_is_refused_with_the_ids() {
        let mut records = calculator_over_zed();
        records.push(WindowRecord {
            id: 107,
            pid: 80,
            owner: "Zed".to_string(),
            title: Some("notes".to_string()),
            bounds: (0.0, 30.0, 500.0, 500.0),
            layer: 0,
        });
        let err = select_window(&records, &WindowSelector::Title("Zed".into()), JUNO).unwrap_err();
        assert!(err.contains("106") && err.contains("107"), "{}", err);
    }

    /// The 2026-10-08 Calculator run: the "1" key click landed at screen
    /// x=361, 7 points left of Calculator, on the Zed window beneath it.
    #[test]
    fn a_point_just_outside_the_target_app_is_not_on_it() {
        let records = calculator_over_zed();
        assert!(!app_window_contains(&records, 70, 361.0, 730.0));
        assert!(app_window_contains(&records, 70, 410.0, 730.0));
        assert!(app_window_contains(&records, 80, 361.0, 730.0));
    }

    #[test]
    fn juno_never_selects_its_own_window() {
        let records = stacked();
        assert!(select_window(&records, &WindowSelector::Id(900), JUNO).is_err());
    }

    #[test]
    fn the_point_lookup_skips_junos_overlays_and_returns_the_top_window() {
        let records = stacked();
        let w = window_at_point(&records, 100.0, 100.0, JUNO).unwrap();
        assert_eq!(w.id, 11);
        assert!(window_at_point(&records, 2000.0, 1000.0, JUNO).is_none());
    }

    #[test]
    fn a_pin_lasts_exactly_as_long_as_its_guard() {
        assert_eq!(pinned(), None);
        {
            let _outer = pin(Some(PinnedWindow {
                pid: 50,
                window_id: 12,
            }));
            assert_eq!(pinned().map(|p| p.window_id), Some(12));
            {
                let _inner = pin(Some(PinnedWindow {
                    pid: 50,
                    window_id: 13,
                }));
                assert_eq!(pinned().map(|p| p.window_id), Some(13));
                // A `None` pin keeps the current one rather than clearing it.
                let _noop = pin(None);
                assert_eq!(pinned().map(|p| p.window_id), Some(13));
            }
            assert_eq!(pinned().map(|p| p.window_id), Some(12));
        }
        assert_eq!(pinned(), None);
    }

    #[test]
    fn a_pin_never_crosses_threads() {
        let _guard = pin(Some(PinnedWindow {
            pid: 50,
            window_id: 12,
        }));
        let seen = std::thread::spawn(pinned).join().unwrap();
        assert_eq!(seen, None);
    }
}
