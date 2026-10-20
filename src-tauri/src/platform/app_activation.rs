//! # Coming back to Juno brings her back
//!
//! Escape on the idle bar takes Juno off the screen, and the tray's Hide Juno
//! does the same. Switching to Juno afterwards (Cmd+Tab, the app switcher,
//! anything that makes her the active app) is the person asking for her, but
//! no window of hers became key, so nothing appeared: Juno was active and
//! invisible.
//!
//! AppKit says when the app becomes active
//! (`NSApplicationDidBecomeActiveNotification`). On that, if none of Juno's
//! windows is on screen, the bar is put back, shown and not focused, the same
//! way the Dock and tray ladder shows it. Setup, the chat window and Settings
//! are left alone: they are Juno too, and whichever of them is coming forward
//! is what the person switched to. Nothing happens before the bar's first
//! appearance at launch, which belongs to the intro reveal.

use tauri::AppHandle;

/// Whether becoming the active app should put the bar back on the screen.
pub fn should_unhide_bar(
    bar_visible: bool,
    other_window_visible: bool,
    onboarding_open: bool,
    bar_has_appeared: bool,
) -> bool {
    bar_has_appeared && !bar_visible && !other_window_visible && !onboarding_open
}

/// Read the windows now and put the bar back if the rule says so.
fn on_became_active(app: &AppHandle) {
    use crate::constants::ui::window_labels;
    use crate::window_management::WindowManager;
    let other_window_visible = [window_labels::MAIN, window_labels::SETTINGS]
        .iter()
        .any(|label| WindowManager::is_window_visible(app, label));
    let unhide = should_unhide_bar(
        WindowManager::is_window_visible(app, window_labels::FLOATING_BAR),
        other_window_visible,
        crate::window_management::onboarding_is_open(app),
        crate::intro::bar_has_appeared() && !crate::intro::reveal_in_flight(),
    );
    if unhide {
        tracing::info!(
            "[AppActivation] Juno became active with nothing on screen; showing the bar"
        );
        crate::window_management::show_bar(app);
    }
}

/// Start watching for Juno becoming the active app. Once, from setup.
#[cfg(target_os = "macos")]
pub fn watch(app: &AppHandle) {
    use block::ConcreteBlock;
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::c_void;

    let app_for_main = app.clone();
    let dispatched = app.run_on_main_thread(move || {
        let app_for_block = app_for_main.clone();
        let block = ConcreteBlock::new(move |_note: id| {
            // Window reads wait on the main thread, which is where this block
            // runs, so they are made from a task instead.
            let app = app_for_block.clone();
            tauri::async_runtime::spawn(async move {
                on_became_active(&app);
            });
        })
        .copy();
        // SAFETY: main thread. The center copies the block and keeps the
        // observer for the life of the app; ours is never removed.
        unsafe {
            let center: id = msg_send![class!(NSNotificationCenter), defaultCenter];
            let name = NSString::alloc(nil).init_str("NSApplicationDidBecomeActiveNotification");
            let _observer: id = msg_send![
                center,
                addObserverForName: name
                object: nil
                queue: nil
                usingBlock: &*block as *const _ as *const c_void
            ];
        }
        std::mem::forget(block);
    });
    if let Err(e) = dispatched {
        tracing::warn!("[AppActivation] Could not start watching for activation: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::should_unhide_bar;

    #[test]
    fn switching_to_a_hidden_juno_brings_the_bar_back() {
        assert!(should_unhide_bar(false, false, false, true));
    }

    #[test]
    fn leaves_her_alone_when_something_of_hers_is_up_or_launch_is_not_done() {
        // The bar is already there.
        assert!(!should_unhide_bar(true, false, false, true));
        // The chat window or Settings is what was switched to.
        assert!(!should_unhide_bar(false, true, false, true));
        // Setup keeps the bar away on purpose.
        assert!(!should_unhide_bar(false, false, true, true));
        // The first appearance is the intro reveal's.
        assert!(!should_unhide_bar(false, false, false, false));
    }
}
