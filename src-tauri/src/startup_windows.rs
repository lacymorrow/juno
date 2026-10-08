//! # Which windows exist at launch, and when the rest arrive
//!
//! Every window in `tauri.conf.json` is declared `create: false`. Tauri used
//! to build all eight of them on the main thread before `setup` even started
//! (0.7 to 1.7 s of a launch, in the logs), and then all eight loaded the same
//! three-megabyte frontend at once, the bar competing with a chat window, a
//! settings window and four full-screen overlays nobody could see.
//!
//! Now the bar is built first, alone, at the top of setup, with its config and
//! its last position written into the page (`window.__JUNO_BAR_BOOT__`) so it
//! can paint without asking. The smoke window is preloaded next to it
//! (`intro::preload`). Everything else is built once the bar is on screen
//! ([`bar_is_up`]), one window at a time, or after [`DEFERRED_FALLBACK`] if the
//! bar never comes up (setup is on screen, or its page failed).
//!
//! Settings and onboarding are not on that list: both are built on demand by
//! `WindowManager::create_or_show_window`, as they already were.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewWindowBuilder};
use tracing::{info, warn};

use crate::commands::bar_position::BarPosition;
use crate::commands::ui_commands::FloatingBarConfig;
use crate::constants::ui::window_labels;

/// Built after the bar is up, in this order: the chat window first, because
/// it is the one the bar opens.
pub const DEFERRED: [&str; 5] = [
    window_labels::MAIN,
    window_labels::FLOATING_PANEL,
    crate::window_management::DESKTOP_CURSOR_OVERLAY_LABEL,
    window_labels::SNAP_WELLS_OVERLAY,
    "listening-overlay",
];

/// Build the deferred windows by now even if the bar never came up.
pub const DEFERRED_FALLBACK: Duration = Duration::from_secs(5);

/// A breath between deferred windows, so the main thread is never held for
/// all of them at once while the smoke is still clearing.
const DEFERRED_GAP: Duration = Duration::from_millis(60);

static DEFERRED_STARTED: AtomicBool = AtomicBool::new(false);

/// What the bar's page knows before it asks anything.
#[derive(Debug, Serialize)]
pub struct BarBoot {
    pub bar_config: FloatingBarConfig,
    pub bar_position: Option<BarPosition>,
}

/// The script that hands the page its boot payload, read once by the page.
pub fn boot_script(boot: &BarBoot) -> String {
    let json = serde_json::to_string(boot).unwrap_or_else(|_| "null".to_string());
    format!("window.__JUNO_BAR_BOOT__ = {json};")
}

/// Build the floating bar, hidden, from its declared config. The first thing
/// setup does.
pub fn create_bar(app: &AppHandle) -> Result<(), String> {
    if app
        .get_webview_window(window_labels::FLOATING_BAR)
        .is_some()
    {
        return Ok(());
    }
    let mut config =
        crate::window_management::declared_window_config(app, window_labels::FLOATING_BAR)
            .ok_or("the floating bar is not declared in tauri.conf.json")?;
    config.title = crate::demo::window_title(&config.title);

    let boot = BarBoot {
        bar_config: crate::commands::ui_commands::bar_config_now(app),
        bar_position: crate::commands::bar_position::stored_bar_position(app),
    };
    WebviewWindowBuilder::from_config(app, &config)
        .map_err(|e| e.to_string())?
        .initialization_script(boot_script(&boot))
        .build()
        .map_err(|e| e.to_string())?;
    crate::startup_timing::mark("bar window created");
    Ok(())
}

/// The bar is on screen: build the rest. Safe to call more than once.
pub fn bar_is_up(app: &AppHandle) {
    start_deferred(app);
}

/// Build the rest after [`DEFERRED_FALLBACK`] whatever happens. Called from
/// setup.
pub fn schedule_fallback(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(DEFERRED_FALLBACK).await;
        start_deferred(&app);
    });
}

fn start_deferred(app: &AppHandle) {
    if DEFERRED_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        for label in DEFERRED {
            build_hidden(&app, label);
            tokio::time::sleep(DEFERRED_GAP).await;
        }
        crate::startup_timing::mark("deferred windows created");
    });
}

/// Build one declared window, hidden, unless something already built it (a
/// person opening the chat before its turn, say).
fn build_hidden(app: &AppHandle, label: &str) {
    if app.get_webview_window(label).is_some() {
        return;
    }
    let Some(mut config) = crate::window_management::declared_window_config(app, label) else {
        warn!("[Startup] '{}' is not declared; not built", label);
        return;
    };
    config.visible = false;
    config.title = crate::demo::window_title(&config.title);
    let built = WebviewWindowBuilder::from_config(app, &config).and_then(|b| b.build());
    match built {
        Ok(_) => {
            info!("[Startup] Built the {} window", label);
            #[cfg(target_os = "macos")]
            crate::platform::configure_deferred_window(app, label);
        }
        Err(e) => warn!("[Startup] Could not build the {} window: {}", label, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window_management::find_declared_window;
    use tauri::utils::config::WindowConfig;

    fn declared_windows() -> Vec<WindowConfig> {
        let raw = include_str!("../tauri.conf.json");
        let value: serde_json::Value =
            serde_json::from_str(raw).expect("tauri.conf.json is not valid JSON");
        serde_json::from_value(value["app"]["windows"].clone())
            .expect("app.windows in tauri.conf.json does not parse as Tauri window configs")
    }

    #[test]
    fn tauri_builds_no_window_by_itself() {
        // A declared window with `create` left on is built before setup runs,
        // on the main thread, ahead of the bar. That is the cost this module
        // exists to remove.
        for window in declared_windows() {
            assert!(
                !window.create,
                "'{}' would be built by Tauri before setup; declare it create: false",
                window.label
            );
        }
    }

    #[test]
    fn every_deferred_window_is_declared() {
        let windows = declared_windows();
        for label in DEFERRED {
            assert!(
                find_declared_window(&windows, label).is_some(),
                "'{}' is deferred but not declared",
                label
            );
        }
    }

    #[test]
    fn every_declared_window_has_a_way_to_be_built() {
        // The bar at setup, the deferred list after it, settings and
        // onboarding on demand. A declared window in none of these would
        // silently never exist.
        let on_demand = [window_labels::SETTINGS, window_labels::ONBOARDING];
        for window in declared_windows() {
            let label = window.label.as_str();
            assert!(
                label == window_labels::FLOATING_BAR
                    || DEFERRED.contains(&label)
                    || on_demand.contains(&label),
                "nothing builds the declared window '{}'",
                label
            );
        }
    }

    #[test]
    fn the_boot_script_is_a_plain_assignment_the_page_can_read() {
        let boot = BarBoot {
            bar_config: FloatingBarConfig::default(),
            bar_position: Some(BarPosition { x: 10, y: 20 }),
        };
        let script = boot_script(&boot);
        let json = script
            .strip_prefix("window.__JUNO_BAR_BOOT__ = ")
            .and_then(|rest| rest.strip_suffix(';'))
            .expect("assignment shape");
        let value: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
        assert_eq!(value["bar_position"]["x"], 10);
        assert!(value["bar_config"]["bar_appearance"].is_string());
    }
}
