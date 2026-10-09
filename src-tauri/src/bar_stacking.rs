//! # Where the floating bar sits in the window stack
//!
//! The bar is an accessory window that normally floats above every ordinary
//! window, Juno's own included and every other app's. That is right while the
//! person is working somewhere else, and wrong the moment they are working in
//! Juno: a settings window, which is where the permission rows live, cannot be
//! read through a bar sitting on top of it.
//!
//! So the bar's place in the stack is a decision, not a constant. The facts
//! that feed it are recorded here as they are observed ([`note_focus`],
//! [`note_window_gone`], [`note_agent_driving`], [`hold_for_system_prompt`]),
//! one pure function turns them into an intent ([`stacking_for`]), and one
//! function applies it ([`apply`]).
//!
//! Before this module the same decision was made in two places that could not
//! see each other. A permission command lowered the bar for the duration of a
//! system prompt and raised it again on a timer. A window helper ordered the
//! settings window above the *chat* window, and left the bar floating above
//! both. Settings was raised above the wrong thing, nothing ever lowered the
//! bar for it, and the prompt timer's job was to put the bar back on top of
//! whatever was there, settings included. One owner is the fix.
//!
//! ## Levels are system-wide, ordering is not
//!
//! An `NSWindowLevel` is a band across the whole system, not an order within
//! an app. Anything above `NSNormalWindowLevel` floats over every other
//! application's ordinary windows too, so no level means "above Juno's windows
//! and nothing else", and the bar has already paid for guessing: at level 5 it
//! sat above a system permission prompt, which arrived behind it, unreadable
//! and unclickable. Level here is therefore only ever floating or normal, and
//! "below that particular window" is done with `orderWindow:relativeTo:`,
//! which is app-relative and touches nothing else.
//!
//! ## Overlays never hold the keyboard
//!
//! The front window is read from AppKit's key-window events, so anything that
//! can become key can confuse it. Tauri's `show()` is `makeKeyAndOrderFront:`
//! on macOS, and an overlay that can become key takes the keyboard the moment
//! it appears; when it hides, AppKit gives the keyboard to whichever window it
//! likes. The drop indicator did exactly that: it took key from the bar at the
//! start of a drag and handed it to settings on the drop, so the bar the
//! person had just put down went straight back under settings. The overlays
//! (the cursor overlay, the snap wells, the listening glow, the intro) are
//! therefore declared `focusable: false`, which makes `canBecomeKeyWindow`
//! answer no, and a test below reads `tauri.conf.json` so a new overlay cannot
//! forget it.

use crate::constants::ui::window_labels;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

/// What the floating bar should be doing about its place in the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarStacking {
    /// `NSFloatingWindowLevel`: above ordinary windows in every app, below
    /// anything the system needs to put in front of a person. The resting
    /// state, and the state while the agent is driving another application.
    Floating,
    /// `NSNormalWindowLevel`, left where it is in the ordinary stack. For when
    /// something that is not Juno's window has to be able to cover the bar,
    /// which in practice means a system alert.
    Normal,
    /// `NSNormalWindowLevel`, and ordered directly below the Juno window the
    /// person is working in.
    ///
    /// The level alone is not enough: `setLevel:` orders a window to the front
    /// of its new level, so a bar dropped from floating to normal can land
    /// *above* the very window it was getting out from under. The explicit
    /// re-order is what makes "get out of the way" true rather than likely.
    BelowFront,
}

impl BarStacking {
    /// The `NSWindowLevel` this intent asks for.
    ///
    /// Only two values appear, and floating is 3 rather than anything higher
    /// on purpose: 3 is above ordinary windows and below system alerts. Raising
    /// it is how a screen-recording prompt ended up behind the bar.
    pub const fn ns_window_level(self) -> i64 {
        match self {
            BarStacking::Floating => 3,
            BarStacking::Normal | BarStacking::BelowFront => 0,
        }
    }

    /// Whether this intent needs the bar ordered below the front Juno window.
    pub const fn orders_below_front(self) -> bool {
        matches!(self, BarStacking::BelowFront)
    }
}

/// Which of Juno's own windows the person is currently working in.
///
/// Only windows a person reads or types into appear here. Juno's overlays (the
/// floating panel, the agent cursor overlay, the snap wells, the listening
/// overlay) are accessories like the bar itself: they are never "the window
/// the person is in", and the bar must not duck under one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrontWindow {
    /// None of Juno's person-facing windows has the keyboard.
    #[default]
    None,
    /// The full-size chat window: the same conversation the bar holds, in a
    /// window of its own.
    Chat,
    /// A window the person is reading or filling in, which the bar must never
    /// cover: settings (where the permission rows are) and onboarding.
    Reading,
}

/// Classify a window label, as AppKit hands it to us.
///
/// This is the list the old fix was missing. Lowering the bar for settings and
/// forgetting onboarding would have been the same bug with a different window,
/// so every person-facing window is named in one place and a test reads
/// `tauri.conf.json` to check none has been added without landing here.
pub fn front_window_for(label: &str) -> FrontWindow {
    if label == window_labels::MAIN {
        FrontWindow::Chat
    } else if label == window_labels::SETTINGS || label == window_labels::ONBOARDING {
        FrontWindow::Reading
    } else {
        FrontWindow::None
    }
}

/// What is happening, as far as the bar's stacking is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BarSituation {
    /// macOS is expected to raise a dialog on Juno's behalf. Those alerts are
    /// not always-on-top, so anything of Juno's that is must stand down.
    pub system_prompt_expected: bool,
    /// The Juno window the person is working in, if any.
    pub front: FrontWindow,
    /// The agent is driving another application through computer use. The bar
    /// is the state readout and the brake, so it stays where it can be seen
    /// and pressed.
    pub agent_driving: bool,
}

/// Decide where the bar belongs. The whole rule, in one pure function.
///
/// | situation | result |
/// | --- | --- |
/// | a system prompt is expected, no Juno window in front | `Normal` |
/// | a system prompt is expected, a Juno window in front | `BelowFront` |
/// | settings or onboarding is in front | `BelowFront` |
/// | the chat window is in front and the agent is idle | `BelowFront` |
/// | the chat window is in front and the agent is driving | `Floating` |
/// | the agent is driving, no Juno window in front | `Floating` |
/// | nothing in particular | `Floating` |
///
/// The two rows worth defending: settings is `BelowFront` whatever else is
/// true, because a permission row that cannot be read is the bug this exists
/// to kill; and the chat window is the one front window the agent can
/// override, because while the agent is driving another app the bar is the only
/// place the work shows and the only place it can be stopped, and the chat
/// window is already showing the same conversation underneath.
pub fn stacking_for(situation: BarSituation) -> BarStacking {
    let step_aside = situation.system_prompt_expected
        || situation.front == FrontWindow::Reading
        || (situation.front == FrontWindow::Chat && !situation.agent_driving);

    if !step_aside {
        return BarStacking::Floating;
    }

    match situation.front {
        // Nothing of Juno's to order against, so the level is the whole answer.
        FrontWindow::None => BarStacking::Normal,
        FrontWindow::Chat | FrontWindow::Reading => BarStacking::BelowFront,
    }
}

/// The facts, as last observed.
#[derive(Debug, Clone, Copy)]
struct Observed {
    /// How many system prompts are believed to be outstanding. A count rather
    /// than a flag because several permission paths can ask at once, and a
    /// flag let whichever one finished first put the bar back over a prompt
    /// another one was still waiting on.
    prompt_holds: u32,
    /// The person-facing Juno window that holds the keyboard.
    front: Option<&'static str>,
    agent_driving: bool,
    /// What was last handed to AppKit, so repeated reports cost nothing. The
    /// agent-driving fact arrives on every mouse action during computer use.
    applied: Option<(BarStacking, Option<&'static str>)>,
}

static OBSERVED: Mutex<Observed> = Mutex::new(Observed {
    prompt_holds: 0,
    front: None,
    agent_driving: false,
    applied: None,
});

impl Observed {
    fn situation(&self) -> BarSituation {
        BarSituation {
            system_prompt_expected: self.prompt_holds > 0,
            front: self.front.map(front_window_for).unwrap_or_default(),
            agent_driving: self.agent_driving,
        }
    }
}

/// The canonical `'static` spelling of a label we track, or `None` for a
/// window whose focus the bar does not care about.
fn tracked(label: &str) -> Option<&'static str> {
    [
        window_labels::MAIN,
        window_labels::SETTINGS,
        window_labels::ONBOARDING,
    ]
    .into_iter()
    .find(|known| *known == label)
}

/// Record that a Juno window gained or lost the keyboard.
///
/// Key status is AppKit's own answer to "which window is the person in", which
/// is why the fact is observed here rather than inferred from whichever code
/// path opened the window. It also means the bar recovers by itself: click
/// another app, or the bar, and the front window resigns key, the fact clears,
/// and the bar floats again.
pub fn note_focus(app: &AppHandle, label: &str, focused: bool) {
    let Some(label) = tracked(label) else {
        return;
    };
    {
        let Ok(mut observed) = OBSERVED.lock() else {
            return;
        };
        if focused {
            observed.front = Some(label);
        } else if observed.front == Some(label) {
            observed.front = None;
        } else {
            return;
        }
    }
    apply(app);
}

/// Record that a Juno window has been closed, hidden or destroyed.
///
/// A window that is gone is not the front window, and a destroyed window does
/// not reliably resign key on the way out.
pub fn note_window_gone(app: &AppHandle, label: &str) {
    let Some(label) = tracked(label) else {
        return;
    };
    {
        let Ok(mut observed) = OBSERVED.lock() else {
            return;
        };
        if observed.front != Some(label) {
            return;
        }
        observed.front = None;
    }
    apply(app);
}

/// Record that a Juno window has been put in front of the person.
///
/// Belt and braces alongside [`note_focus`]: the open paths say so directly so
/// the bar is out of the way by the time the window is drawn, rather than one
/// focus event later.
pub fn note_front(app: &AppHandle, label: &str) {
    note_focus(app, label, true);
}

/// Record whether the agent is driving another application.
pub fn note_agent_driving(app: &AppHandle, driving: bool) {
    {
        let Ok(mut observed) = OBSERVED.lock() else {
            return;
        };
        if observed.agent_driving == driving {
            return;
        }
        observed.agent_driving = driving;
    }
    apply(app);
}

/// Stand down while macOS is expected to raise a dialog on Juno's behalf.
///
/// Balanced by [`release_system_prompt_hold`] (one prompt answered or timed
/// out) or [`clear_system_prompt_holds`] (all of them).
pub fn hold_for_system_prompt(app: &AppHandle) {
    {
        let Ok(mut observed) = OBSERVED.lock() else {
            return;
        };
        observed.prompt_holds = observed.prompt_holds.saturating_add(1);
    }
    apply(app);
}

/// One expected prompt is no longer expected.
pub fn release_system_prompt_hold(app: &AppHandle) {
    {
        let Ok(mut observed) = OBSERVED.lock() else {
            return;
        };
        observed.prompt_holds = observed.prompt_holds.saturating_sub(1);
    }
    apply(app);
}

/// No prompt is outstanding: the answer is in.
pub fn clear_system_prompt_holds(app: &AppHandle) {
    {
        let Ok(mut observed) = OBSERVED.lock() else {
            return;
        };
        if observed.prompt_holds == 0 {
            return;
        }
        observed.prompt_holds = 0;
    }
    apply(app);
}

/// Work out where the bar belongs now, and put it there.
///
/// Safe to call for a fact that did not change: an intent identical to the one
/// already applied returns before touching AppKit.
pub fn apply(app: &AppHandle) {
    // No bar to restack. Deliberately not recorded as a decision that has been
    // applied: the bar is built and shown during startup, and remembering an
    // answer worked out before it existed would leave it wherever it was built.
    if app
        .get_webview_window(window_labels::FLOATING_BAR)
        .is_none()
    {
        return;
    }

    let decided = {
        let Ok(mut observed) = OBSERVED.lock() else {
            warn!("The floating bar's stacking state is poisoned; leaving the bar where it is");
            return;
        };
        let situation = observed.situation();
        let stacking = stacking_for(situation);
        let front = observed.front;
        if observed.applied == Some((stacking, front)) {
            None
        } else {
            observed.applied = Some((stacking, front));
            Some((stacking, front, situation))
        }
    };

    let Some((stacking, front, situation)) = decided else {
        return;
    };

    info!(
        "Floating bar stacking: {:?} (prompt expected: {}, front window: {}, agent driving: {})",
        stacking,
        situation.system_prompt_expected,
        front.unwrap_or("none"),
        situation.agent_driving
    );

    // Only macOS has a window stack to argue with; elsewhere the decision is
    // made and logged and there is nothing to apply it to. Every value above is
    // used on both paths, so nothing needs silencing here.
    #[cfg(target_os = "macos")]
    crate::platform::macos::apply_bar_stacking(app, stacking, front);
}

/// Whether the bar should be ordered above a drop overlay, given the stacking
/// last applied. Only a floating bar shares the overlay's level; a bar that
/// stepped aside for a window or a system prompt stays aside. Nothing applied
/// yet is the resting state, which is floating.
pub fn bar_goes_over_overlay(applied: Option<BarStacking>) -> bool {
    matches!(applied, None | Some(BarStacking::Floating))
}

/// Keep the bar above one of Juno's own drop overlays.
///
/// The overlay and the bar are both at the floating level, and showing the
/// overlay orders it to the front of that level, over the bar being dragged.
/// Ordering within a level is app-relative and touches nothing else on the
/// system, so this is the one restack the drag needs, and it is decided here
/// with every other one.
pub fn raise_over_overlay(app: &AppHandle, overlay_label: &str) {
    let applied = match OBSERVED.lock() {
        Ok(observed) => observed.applied.map(|(stacking, _)| stacking),
        Err(_) => return,
    };
    if !bar_goes_over_overlay(applied) {
        return;
    }
    #[cfg(target_os = "macos")]
    crate::platform::macos::order_bar_above(app, overlay_label);
    #[cfg(not(target_os = "macos"))]
    let _ = (app, overlay_label);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::utils::config::WindowConfig as DeclaredWindowConfig;

    #[test]
    fn only_a_floating_bar_is_put_over_the_drop_overlay() {
        assert!(bar_goes_over_overlay(None));
        assert!(bar_goes_over_overlay(Some(BarStacking::Floating)));
        assert!(!bar_goes_over_overlay(Some(BarStacking::Normal)));
        assert!(!bar_goes_over_overlay(Some(BarStacking::BelowFront)));
    }

    fn situation() -> BarSituation {
        BarSituation::default()
    }

    #[test]
    fn an_idle_bar_floats() {
        assert_eq!(stacking_for(situation()), BarStacking::Floating);
    }

    #[test]
    fn a_window_the_person_is_reading_is_never_covered() {
        // Settings and onboarding, under every combination of everything else.
        // This is the whole point of the module: a bar that floats over the
        // permission rows is a permission the person cannot grant.
        for system_prompt_expected in [false, true] {
            for agent_driving in [false, true] {
                let decided = stacking_for(BarSituation {
                    system_prompt_expected,
                    front: FrontWindow::Reading,
                    agent_driving,
                });
                assert_eq!(
                    decided,
                    BarStacking::BelowFront,
                    "reading window with prompt={} driving={}",
                    system_prompt_expected,
                    agent_driving
                );
                assert_eq!(decided.ns_window_level(), 0);
                assert!(decided.orders_below_front());
            }
        }
    }

    #[test]
    fn the_chat_window_is_the_one_front_window_the_agent_can_override() {
        assert_eq!(
            stacking_for(BarSituation {
                front: FrontWindow::Chat,
                agent_driving: false,
                ..situation()
            }),
            BarStacking::BelowFront
        );
        assert_eq!(
            stacking_for(BarSituation {
                front: FrontWindow::Chat,
                agent_driving: true,
                ..situation()
            }),
            BarStacking::Floating
        );
    }

    #[test]
    fn driving_another_app_keeps_the_bar_on_top() {
        assert_eq!(
            stacking_for(BarSituation {
                agent_driving: true,
                ..situation()
            }),
            BarStacking::Floating
        );
    }

    #[test]
    fn an_expected_system_prompt_lowers_the_bar_even_mid_task() {
        // The blood-paid case: a screen-recording prompt arrived behind the bar
        // and could not be read. It outranks everything, computer use included.
        let decided = stacking_for(BarSituation {
            system_prompt_expected: true,
            front: FrontWindow::None,
            agent_driving: true,
        });
        assert_eq!(decided, BarStacking::Normal);
        assert_eq!(decided.ns_window_level(), 0);
        assert!(!decided.orders_below_front());
    }

    #[test]
    fn floating_stays_below_system_alerts() {
        // NSFloatingWindowLevel. Not 5 (above NSModalPanelWindowLevel), which
        // is where a system permission prompt came up behind the bar.
        assert_eq!(BarStacking::Floating.ns_window_level(), 3);
        assert!(BarStacking::Floating.ns_window_level() < 8);
    }

    #[test]
    fn only_the_bar_itself_can_be_lowered() {
        // Every stacking intent is one of two levels. A third level creeping in
        // is how this breaks again.
        for stacking in [
            BarStacking::Floating,
            BarStacking::Normal,
            BarStacking::BelowFront,
        ] {
            assert!(matches!(stacking.ns_window_level(), 0 | 3));
        }
    }

    #[test]
    fn juno_overlays_are_not_front_windows() {
        for label in [
            window_labels::FLOATING_BAR,
            window_labels::FLOATING_PANEL,
            "desktop-cursor-overlay",
            "snap-wells-overlay",
            "listening-overlay",
        ] {
            assert_eq!(
                front_window_for(label),
                FrontWindow::None,
                "'{}' is an accessory window; the bar must not duck under it",
                label
            );
        }
    }

    #[test]
    fn the_person_facing_windows_are_classified() {
        assert_eq!(front_window_for(window_labels::MAIN), FrontWindow::Chat);
        assert_eq!(
            front_window_for(window_labels::SETTINGS),
            FrontWindow::Reading
        );
        assert_eq!(
            front_window_for(window_labels::ONBOARDING),
            FrontWindow::Reading
        );
    }

    /// The real `tauri.conf.json`, parsed the way Tauri parses it.
    fn declared_windows() -> Vec<DeclaredWindowConfig> {
        let raw = include_str!("../tauri.conf.json");
        let value: serde_json::Value =
            serde_json::from_str(raw).expect("tauri.conf.json is not valid JSON");
        serde_json::from_value(value["app"]["windows"].clone())
            .expect("app.windows in tauri.conf.json does not parse as Tauri window configs")
    }

    #[test]
    fn every_declared_ordinary_window_is_a_front_window() {
        // A window someone can work in is a window the bar has to get out from
        // under. Juno's accessories declare alwaysOnTop, so anything that does
        // not is person-facing and has to be classified above — otherwise
        // adding a window silently reintroduces this bug for it. Handling
        // settings and forgetting onboarding would have been exactly that.
        let windows = declared_windows();
        let ordinary: Vec<&DeclaredWindowConfig> =
            windows.iter().filter(|w| !w.always_on_top).collect();

        assert!(
            ordinary.len() >= 3,
            "expected the chat, settings and onboarding windows to be declared ordinary"
        );

        for window in ordinary {
            assert_ne!(
                front_window_for(&window.label),
                FrontWindow::None,
                "window '{}' is an ordinary window the person can work in, but \
                 bar_stacking::front_window_for does not know it, so the floating \
                 bar will sit on top of it",
                window.label
            );
        }
    }

    #[test]
    fn no_overlay_can_take_the_keyboard() {
        // Every always-on-top window is either a surface the person types into
        // (the bar and the panel both carry a text field) or an overlay, and an
        // overlay must be `focusable: false`. An overlay that can become key
        // takes the keyboard when it is shown and AppKit hands it on when it
        // hides; the snap-wells overlay handed it to settings on every drop,
        // which put the bar the person had just dropped back under settings.
        let typed_into = [window_labels::FLOATING_BAR, window_labels::FLOATING_PANEL];
        let windows = declared_windows();
        let overlays: Vec<&DeclaredWindowConfig> = windows
            .iter()
            .filter(|w| w.always_on_top && !typed_into.contains(&w.label.as_str()))
            .collect();

        assert!(
            overlays.len() >= 3,
            "expected the cursor, snap-wells and listening overlays to be declared always-on-top"
        );

        for window in overlays {
            assert!(
                !window.focusable,
                "overlay '{}' can become the key window; declare it \"focusable\": false \
                 in tauri.conf.json, or it will take the keyboard when shown and hand \
                 it to a window of AppKit's choosing when hidden",
                window.label
            );
            assert!(
                !window.focus,
                "overlay '{}' is declared to take focus when created",
                window.label
            );
        }
    }
}
