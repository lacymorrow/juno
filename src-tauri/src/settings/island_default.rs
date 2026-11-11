//! Move anyone saved on the Pill to the Island, once.
//!
//! The Island is the default look now and the Pill is being retired as the
//! Island takes over what it does. A new install starts on the Island through
//! `bar_appearances::DEFAULT`; this module is for the people who already have
//! a settings file that says Pill, most of whom never chose it: it was simply
//! the default when they installed.
//!
//! It runs at launch, before the bar reads its look, so the bar comes up as
//! the Island with no flash of the Pill first. It runs once: a marker is
//! written whether or not anything changed, so a person who goes back to the
//! Pill in Settings afterwards keeps it. Nothing is shown; the look is the
//! announcement.

use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;
use tracing::{info, warn};

use crate::constants::settings::{store_keys, SETTINGS_STORE_FILE};
use crate::constants::ui::bar_appearances;

const APPEARANCE_FIELD: &str = "bar_appearance";

/// The stored bar section with the Pill swapped for the Island, or `None`
/// when there is nothing to change: no section yet (a new install takes the
/// default), or any other look, which was a choice and stays.
fn moved_to_island(section: Option<&Value>) -> Option<Value> {
    let section = section?;
    if section.get(APPEARANCE_FIELD)?.as_str()? != bar_appearances::FLOATING {
        return None;
    }
    let mut moved = section.clone();
    moved.as_object_mut()?.insert(
        APPEARANCE_FIELD.to_string(),
        Value::String(bar_appearances::DYNAMIC.to_string()),
    );
    Some(moved)
}

/// Apply the move if it has not been applied before. Never fails the launch:
/// a store that cannot be opened or saved is logged and left for next time.
pub fn run(app: &AppHandle) {
    let store = match app.store(SETTINGS_STORE_FILE) {
        Ok(store) => store,
        Err(e) => {
            warn!("[IslandDefault] Could not open the settings store: {e}");
            return;
        }
    };
    if store
        .get(store_keys::ISLAND_DEFAULT_APPLIED)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return;
    }

    if let Some(moved) = moved_to_island(store.get(store_keys::FLOATING_BAR).as_ref()) {
        store.set(store_keys::FLOATING_BAR, moved);
        info!("[IslandDefault] The saved look was the Pill; it is the Island now");
    }
    store.set(store_keys::ISLAND_DEFAULT_APPLIED, Value::Bool(true));
    if let Err(e) = store.save() {
        warn!("[IslandDefault] Could not save the settings store: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_saved_pill_becomes_the_island_and_keeps_everything_else() {
        let stored = json!({ "bar_appearance": "floating", "opacity": 0.8, "auto_hide": true });
        let moved = moved_to_island(Some(&stored));
        assert_eq!(
            moved,
            Some(json!({ "bar_appearance": "dynamic", "opacity": 0.8, "auto_hide": true }))
        );
    }

    #[test]
    fn any_other_look_was_a_choice_and_stays() {
        for look in [
            bar_appearances::DYNAMIC,
            bar_appearances::PERSONA,
            bar_appearances::NOTCH,
            bar_appearances::SHADER_ORB,
            bar_appearances::ORB,
            bar_appearances::REACT_ORB,
        ] {
            let stored = json!({ "bar_appearance": look });
            assert_eq!(moved_to_island(Some(&stored)), None, "{look} should stay");
        }
    }

    #[test]
    fn a_new_install_has_nothing_to_move() {
        assert_eq!(moved_to_island(None), None);
        assert_eq!(moved_to_island(Some(&json!({}))), None);
        assert_eq!(moved_to_island(Some(&json!("not a section"))), None);
    }

    #[test]
    fn the_island_is_what_a_new_install_starts_on() {
        assert_eq!(bar_appearances::DEFAULT, bar_appearances::DYNAMIC);
    }
}
