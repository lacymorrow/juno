//! # Did the click do anything?
//!
//! A click posted straight to a background process is accepted by the window
//! server whether or not the app does anything with it. Music, and other SwiftUI
//! and Catalyst apps, drop such events (double clicks most of all), and the post
//! still returns success. Reporting that as a success sent the agent on with a
//! state that never happened.
//!
//! This module holds the platform-neutral decisions, so they can be tested
//! without a Mac: which accessibility action fits a click, how two fingerprints
//! of a window compare, and what to do next when a click changed nothing. The
//! fingerprint itself is read in `ax_elements::window_fingerprint`.

use crate::input_tier::InputTier;

/// The kind of click being delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickKind {
    Left,
    Double,
}

/// Actions that mean "open this" on a row, cell or list item, best first.
const OPEN_ACTIONS: [&str; 2] = ["AXOpen", "AXShowDefaultUI"];

/// The accessibility action that stands in for this click, if the element
/// offers one. `actions` are the names the element reported.
///
/// A double click means "open", so it only takes an open-style action: pressing
/// a row would select it and call that a double click. A single click takes
/// `AXPress`.
pub fn pick_ax_action(kind: ClickKind, actions: &[String]) -> Option<&'static str> {
    let has = |name: &str| actions.iter().any(|a| a == name);
    match kind {
        ClickKind::Left => has("AXPress").then_some("AXPress"),
        ClickKind::Double => OPEN_ACTIONS.into_iter().find(|name| has(name)),
    }
}

/// How many ancestors above an element are tried for an action. A title in a
/// row is a text node; the row or its cell is what carries the action.
pub const MAX_ANCESTORS: usize = 3;

/// What a before/after pair of window fingerprints says about a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The window's accessibility state moved: the app reacted.
    Changed,
    /// Identical before and after: the app very likely ignored the click.
    Unchanged,
    /// A fingerprint could not be read, so nothing can be said either way.
    Unknown,
}

/// Compare the fingerprints taken before and after a click.
pub fn compare(before: Option<u64>, after: Option<u64>) -> Verdict {
    match (before, after) {
        (Some(b), Some(a)) if b == a => Verdict::Unchanged,
        (Some(_), Some(_)) => Verdict::Changed,
        _ => Verdict::Unknown,
    }
}

/// What to do once a click has been delivered and read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextStep {
    /// Report the click as delivered.
    Accept,
    /// Ask to take the mouse, activate the app and click again with real input.
    RetryForeground,
    /// Report the honest failure: the app ignored the click.
    ReportIgnored,
}

/// Only a click posted to a process can be silently dropped while claiming
/// success. An accessibility action reports its own refusal, and a physical
/// click goes through the system's own event path.
pub fn needs_verification(tier: InputTier) -> bool {
    tier == InputTier::ProcessTargeted
}

/// The decision after a click: accept it, retry it in the foreground, or give
/// up honestly.
///
/// `foreground_tried` is true once the foreground retry has already run, so the
/// retry happens at most once.
pub fn next_step(verdict: Verdict, tier: InputTier, foreground_tried: bool) -> NextStep {
    if !needs_verification(tier) && !foreground_tried {
        return NextStep::Accept;
    }
    match verdict {
        Verdict::Changed | Verdict::Unknown => NextStep::Accept,
        Verdict::Unchanged if foreground_tried => NextStep::ReportIgnored,
        Verdict::Unchanged => NextStep::RetryForeground,
    }
}

/// The one-line hint an ignored click carries.
pub fn ignored_hint(foreground_tried: bool, declined: bool) -> &'static str {
    if declined {
        "The app ignored a background click and taking the mouse was declined. \
         Use a keyboard shortcut, an accessibility action on a named element, or ask the person to do this step."
    } else if foreground_tried {
        "The app did not react even to a foreground click. Re-read the elements to see where things stand, or try another route."
    } else {
        "The app ignored a click posted in the background. Try a keyboard shortcut, or an element with a press action."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_left_click_takes_press() {
        let actions = names(&["AXShowMenu", "AXPress"]);
        assert_eq!(pick_ax_action(ClickKind::Left, &actions), Some("AXPress"));
        assert_eq!(pick_ax_action(ClickKind::Left, &names(&["AXOpen"])), None);
    }

    #[test]
    fn a_double_click_takes_an_open_action_never_press() {
        assert_eq!(
            pick_ax_action(ClickKind::Double, &names(&["AXPress", "AXOpen"])),
            Some("AXOpen")
        );
        assert_eq!(
            pick_ax_action(ClickKind::Double, &names(&["AXShowDefaultUI"])),
            Some("AXShowDefaultUI")
        );
        assert_eq!(
            pick_ax_action(ClickKind::Double, &names(&["AXPress"])),
            None
        );
        assert_eq!(pick_ax_action(ClickKind::Double, &[]), None);
    }

    #[test]
    fn identical_fingerprints_mean_ignored() {
        assert_eq!(compare(Some(7), Some(7)), Verdict::Unchanged);
        assert_eq!(compare(Some(7), Some(8)), Verdict::Changed);
    }

    #[test]
    fn an_unreadable_fingerprint_is_not_a_verdict() {
        assert_eq!(compare(None, Some(1)), Verdict::Unknown);
        assert_eq!(compare(Some(1), None), Verdict::Unknown);
        assert_eq!(compare(None, None), Verdict::Unknown);
    }

    #[test]
    fn only_process_targeted_clicks_are_verified() {
        assert!(needs_verification(InputTier::ProcessTargeted));
        assert!(!needs_verification(InputTier::Accessibility));
        assert!(!needs_verification(InputTier::PhysicalCursor));
    }

    #[test]
    fn an_unchanged_background_click_retries_in_the_foreground_once() {
        assert_eq!(
            next_step(Verdict::Unchanged, InputTier::ProcessTargeted, false),
            NextStep::RetryForeground
        );
        assert_eq!(
            next_step(Verdict::Unchanged, InputTier::PhysicalCursor, true),
            NextStep::ReportIgnored
        );
    }

    #[test]
    fn a_changed_or_unknown_click_is_accepted() {
        for verdict in [Verdict::Changed, Verdict::Unknown] {
            assert_eq!(
                next_step(verdict, InputTier::ProcessTargeted, false),
                NextStep::Accept
            );
            assert_eq!(
                next_step(verdict, InputTier::PhysicalCursor, true),
                NextStep::Accept
            );
        }
    }

    #[test]
    fn clicks_that_cannot_be_dropped_silently_are_not_second_guessed() {
        assert_eq!(
            next_step(Verdict::Unchanged, InputTier::Accessibility, false),
            NextStep::Accept
        );
        assert_eq!(
            next_step(Verdict::Unchanged, InputTier::PhysicalCursor, false),
            NextStep::Accept
        );
    }

    #[test]
    fn the_hint_says_what_happened() {
        assert!(ignored_hint(false, true).contains("declined"));
        assert!(ignored_hint(true, false).contains("foreground"));
        assert!(ignored_hint(false, false).contains("background"));
        assert!(!ignored_hint(false, false).contains('\u{2014}'));
    }
}
