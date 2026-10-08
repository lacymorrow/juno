//! # The `settings` tool: Juno drives its own Settings window
//!
//! One tool with six actions, so "change my voice", "make the shortcut
//! Fn+Space" and "show me where the model setting is" each take one call:
//!
//! | action | does |
//! |---|---|
//! | `list` | every setting the agent can reach, with pane, value type, advanced, protected |
//! | `get` | one setting's current value (secrets only as set or not set) |
//! | `set` | change it, through the same Rust function the Settings window calls |
//! | `open` | open the Settings window, optionally on a pane |
//! | `highlight` | open it on a setting's pane, scroll the row into view, flash it |
//! | `set_advanced` | turn "Show advanced settings" on or off |
//!
//! The settings themselves are the typed table in [`crate::settings::registry`].
//! Writes go through [`registry::authorize_set`], which refuses every protected
//! setting, and [`apply`] takes only what that returns.
//!
//! Offered to the Claude CLI over Juno's MCP server (`juno_mcp.rs`). The
//! window side is `ModularSettingsWindow.tsx`, which listens for
//! [`events::SETTINGS_NAVIGATE`] and, when it was not open yet, asks for the
//! navigation it missed with `take_pending_settings_navigation`.

use std::sync::Mutex;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, warn};

use crate::agent::core::ToolDefinition;
use crate::constants::agent::tool_names;
use crate::constants::settings::events;
use crate::constants::ui::window_labels;
use crate::settings::manager::SettingsManager;
use crate::settings::registry::{
    self, all_specs, AuthorizedChange, McpChange, NewValue, Refusal, SettingKey, SettingSpec,
};
use crate::state::AppState;
use crate::triggers::{Binding, Gesture, TriggerTarget};

/// What the Settings window should show. Serialized to the window as-is.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Navigation {
    /// Sidebar id of the pane, or `None` to leave the pane alone.
    pub pane: Option<&'static str>,
    /// Row anchor to scroll to and flash.
    pub row: Option<&'static str>,
    /// Re-read every setting from Rust before drawing: the agent changed one.
    pub reload: bool,
}

/// A navigation asked for while the window did not exist. The window takes it
/// once it has mounted, because an event emitted before then reaches nobody.
static PENDING: Mutex<Option<Navigation>> = Mutex::new(None);

/// Hand the window the navigation it missed, once.
pub fn take_pending_navigation() -> Option<Navigation> {
    PENDING.lock().ok().and_then(|mut pending| pending.take())
}

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: tool_names::SETTINGS.to_string(),
        description: "Juno's own settings. Use it when the person asks to change, find or \
            explain one of Juno's settings: \"change my voice\", \"make the shortcut Fn+Space\", \
            \"show me where the model setting is\". Actions: `list` (every setting, its key, \
            pane and allowed values), `get` (current value and choices for one key), `set` \
            (change it; applies live, exactly as if the person changed it), `open` (the \
            Settings window, optionally on a pane), `highlight` (open the window on one \
            setting and point at it; use this for \"show me\" and \"where is\"), \
            `set_advanced` (show or hide advanced settings). Tool categories take \
            {\"category\": \"Browser\", \"enabled\": false}; MCP servers take one change at a \
            time, {\"add\": {\"<name>\": {\"command\": \"npx\", \"args\": [...], \"env\": {}}}} \
            or {\"remove\": \"<name>\"}, and an added server waits for the person's approval \
            before it runs. Settings marked protected (permission mode, ask before sending, \
            provider, model, API key, system prompt) cannot be changed by you: highlight \
            them and tell the person to change them. Call `list` first if unsure of a key."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "get", "set", "open", "highlight", "set_advanced"]
                },
                "key": {
                    "type": "string",
                    "description": "A setting key from `list`, e.g. \"audio.voice\". For get, set and highlight."
                },
                "value": {
                    "description": "The new value for `set`: true/false, a choice, a number, or a shortcut like \"Option+Space\"."
                },
                "pane": {
                    "type": "string",
                    "description": "For `open`: General, Triggers, Audio, Providers, Models, Notifications, Tools, Automations, Network, Security, Advanced."
                },
                "enabled": {
                    "type": "boolean",
                    "description": "For `set_advanced`."
                }
            },
            "required": ["action"]
        }),
        api_type: None,
        beta_flag: None,
    }
}

/// Run one call. An `Err` is a message for the model to read and act on.
pub async fn run(app: &AppHandle, input: Value) -> Result<Value, String> {
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match action.as_str() {
        "list" => list(app).await,
        "get" => get(app, key_from(&input)?).await,
        "set" => {
            set(
                app,
                key_from(&input)?,
                input.get("value").unwrap_or(&Value::Null),
            )
            .await
        }
        "open" => open(app, input.get("pane").and_then(Value::as_str)).await,
        "highlight" => highlight(app, key_from(&input)?).await,
        "set_advanced" => {
            let enabled = input
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or("set_advanced needs \"enabled\": true or false.")?;
            set_advanced(app, enabled).await?;
            Ok(json!({ "advanced": enabled }))
        }
        other => Err(format!(
            "Unknown action \"{other}\". Use list, get, set, open, highlight or set_advanced."
        )),
    }
}

fn key_from(input: &Value) -> Result<SettingKey, String> {
    let id = input
        .get("key")
        .and_then(Value::as_str)
        .ok_or("This action needs a \"key\". Call action \"list\" for the keys.")?;
    SettingKey::from_id(id)
}

fn manager(app: &AppHandle) -> Result<SettingsManager, String> {
    SettingsManager::new(app.clone())
}

async fn advanced_on(app: &AppHandle) -> bool {
    match manager(app) {
        Ok(m) => m.get_advanced_settings_enabled().await.unwrap_or(false),
        Err(_) => false,
    }
}

async fn list(app: &AppHandle) -> Result<Value, String> {
    let settings: Vec<Value> = all_specs().map(|spec| spec.describe()).collect();
    Ok(json!({
        "advanced_settings_shown": advanced_on(app).await,
        "settings": settings,
    }))
}

async fn get(app: &AppHandle, key: SettingKey) -> Result<Value, String> {
    let spec = key.spec();
    let mut out = spec.describe();
    let (current, choices) = read(app, key).await?;
    out["current"] = current;
    if let Some(choices) = choices {
        out["choices"] = choices;
    }
    Ok(out)
}

async fn set(app: &AppHandle, key: SettingKey, raw: &Value) -> Result<Value, String> {
    let change = match registry::authorize_set(key, raw) {
        Ok(change) => change,
        Err(refusal) => {
            if matches!(refusal, Refusal::Protected { .. }) {
                warn!(
                    "[SettingsTool] Refused a change to protected {}",
                    key.spec().id
                );
            }
            return Err(refusal.to_string());
        }
    };
    apply(app, &change).await?;
    info!("[SettingsTool] Changed {}", key.spec().id);

    let spec = key.spec();
    // An open window redraws from Rust and points at what changed, so the
    // person sees it happen. A closed one is left closed: a change asked for
    // out loud does not need a window in the way.
    if app.get_webview_window(window_labels::SETTINGS).is_some() {
        let row_visible = !spec.needs_advanced() || advanced_on(app).await;
        emit_navigation(
            app,
            &Navigation {
                pane: row_visible.then_some(spec.pane.id()),
                row: row_visible.then_some(spec.row),
                reload: true,
            },
        );
    }

    let (current, _) = read(app, key).await?;
    Ok(json!({ "key": spec.id, "changed": true, "current": current }))
}

async fn open(app: &AppHandle, pane: Option<&str>) -> Result<Value, String> {
    let pane = match pane.map(str::trim).filter(|p| !p.is_empty()) {
        Some(name) => Some(registry::Pane::from_name(name).ok_or_else(|| {
            let names: Vec<&str> = registry::Pane::ALL.iter().map(|p| p.name()).collect();
            format!("There is no \"{name}\" pane. Panes: {}.", names.join(", "))
        })?),
        None => None,
    };
    let mut turned_on_advanced = false;
    if let Some(pane) = pane {
        if pane.advanced() && !advanced_on(app).await {
            set_advanced(app, true).await?;
            turned_on_advanced = true;
        }
    }
    show(
        app,
        Navigation {
            pane: pane.map(registry::Pane::id),
            row: None,
            reload: false,
        },
    )
    .await?;
    Ok(json!({
        "opened": pane.map(registry::Pane::name).unwrap_or("Settings"),
        "turned_on_advanced_settings": turned_on_advanced,
    }))
}

async fn highlight(app: &AppHandle, key: SettingKey) -> Result<Value, String> {
    let spec = key.spec();
    // A row behind the advanced toggle does not exist on screen until the
    // toggle is on, and "show me where it is" has to show it.
    let mut turned_on_advanced = false;
    if spec.needs_advanced() && !advanced_on(app).await {
        set_advanced(app, true).await?;
        turned_on_advanced = true;
    }
    show(
        app,
        Navigation {
            pane: Some(spec.pane.id()),
            row: Some(spec.row),
            reload: false,
        },
    )
    .await?;
    Ok(json!({
        "highlighted": spec.label,
        "pane": spec.pane.name(),
        "protected": spec.protected,
        "turned_on_advanced_settings": turned_on_advanced,
    }))
}

/// The sidebar switch's own write path, which also tells an open window.
async fn set_advanced(app: &AppHandle, enabled: bool) -> Result<(), String> {
    crate::commands::settings::set_advanced_settings_enabled(app.clone(), enabled).await
}

/// Open (or raise) the window and point it at `nav`.
async fn show(app: &AppHandle, nav: Navigation) -> Result<(), String> {
    let already_open = app.get_webview_window(window_labels::SETTINGS).is_some();
    if !already_open {
        if let Ok(mut pending) = PENDING.lock() {
            *pending = Some(nav.clone());
        }
    }
    crate::window_management::open_settings_window(app.clone()).await?;
    if already_open {
        emit_navigation(app, &nav);
    }
    Ok(())
}

fn emit_navigation(app: &AppHandle, nav: &Navigation) {
    if let Err(e) = app.emit(events::SETTINGS_NAVIGATE, nav) {
        warn!("[SettingsTool] Could not reach the Settings window: {e}");
    }
}

/* ------------------------------- reading ------------------------------- */

fn app_state(app: &AppHandle) -> Result<tauri::State<'_, AppState>, String> {
    app.try_state::<AppState>()
        .ok_or_else(|| "Juno's state is not available yet.".to_string())
}

fn set_or_not(present: bool) -> Value {
    json!(if present { "set" } else { "not set" })
}

/// The first key-or-mouse row for a target, which is the row the Triggers
/// pane anchors as `trigger-<target>`.
fn shortcut_row(triggers: &[crate::triggers::Trigger], target: TriggerTarget) -> Option<usize> {
    triggers
        .iter()
        .position(|t| t.target == target && t.gesture != Gesture::Say)
}

fn binding_value(binding: Option<&Binding>) -> Value {
    match binding {
        Some(Binding::Keyboard { shortcut }) => json!(shortcut),
        Some(Binding::Mouse { button }) => json!(format!("mouse button {button}")),
        None => Value::Null,
    }
}

fn shortcut_target(key: SettingKey) -> TriggerTarget {
    if key == SettingKey::DictationShortcut {
        TriggerTarget::Dictation
    } else {
        TriggerTarget::Agent
    }
}

/// The current value, and for a live choice the choices.
async fn read(app: &AppHandle, key: SettingKey) -> Result<(Value, Option<Value>), String> {
    use SettingKey as K;
    let value = match key {
        K::JunoVoice => {
            let list = crate::tts::voices::get_juno_voices(app.clone(), app_state(app)?).await?;
            let current = list
                .options
                .iter()
                .find(|o| o.selected)
                .map(|o| json!({ "id": o.id, "name": o.name }))
                .unwrap_or(Value::Null);
            let choices: Vec<Value> = list
                .options
                .iter()
                .map(|o| json!({ "id": o.id, "name": o.name }))
                .collect();
            return Ok((current, Some(json!(choices))));
        }
        K::SpeakingSpeed => {
            let list = crate::tts::voices::get_juno_voices(app.clone(), app_state(app)?).await?;
            match list.speed {
                Some(speed) => json!(speed.value),
                None => json!("The current voice cannot change speed."),
            }
        }
        K::InputDevice | K::OutputDevice => {
            let devices = crate::commands::audio_devices::list_audio_devices(app.clone()).await?;
            let (chosen, entries) = if key == K::InputDevice {
                (devices.chosen_input, devices.inputs)
            } else {
                (devices.chosen_output, devices.outputs)
            };
            let mut choices = vec![json!("system")];
            choices.extend(entries.into_iter().map(|d| json!(d.name)));
            let current = chosen.map(Value::String).unwrap_or_else(|| json!("system"));
            return Ok((current, Some(json!(choices))));
        }
        K::PlaySounds => json!(manager(app)?.get_audio_settings().await?.sound_enabled),
        K::OpenAtLogin => {
            json!(crate::commands::autostart::is_autostart_enabled(app.clone()).await?)
        }
        K::CursorColor => json!(
            manager(app)?
                .get_floating_bar_settings()
                .await?
                .agent_cursor_color
        ),
        K::AgentShortcut | K::DictationShortcut => {
            let triggers = app_state(app)?.get_triggers()?;
            shortcut_row(&triggers, shortcut_target(key))
                .and_then(|i| triggers.get(i))
                .map(|t| binding_value(t.binding.as_ref()))
                .unwrap_or(Value::Null)
        }
        K::BackgroundMode => json!(manager(app)?.get_agent_settings().await?.background_mode),
        K::PersistentSession => json!(
            crate::agent::providers::claude_cli_session::get_cli_persistent_session_enabled(
                app.clone()
            )
            .await?
        ),
        K::SmoothMouseMovement => {
            json!(
                manager(app)?
                    .get_tool_settings()
                    .await?
                    .smooth_mouse_movement
            )
        }
        K::PermissionMode => json!(manager(app)?.get_agent_settings().await?.permission_mode),
        K::AskBeforeSend => json!(crate::agent::providers::cli_approval::is_enabled(app)),
        K::MouseControl => json!(manager(app)?.get_agent_settings().await?.mouse_control),
        K::ActiveProvider => json!(manager(app)?.get_provider_settings().await?.active_provider),
        K::Model | K::ApiKey | K::SystemPrompt | K::AccountConnectors => {
            let providers = manager(app)?.get_provider_settings().await?;
            let active = providers
                .providers
                .iter()
                .find(|p| p.id == providers.active_provider);
            match key {
                K::Model => json!(active.and_then(|p| p.model.clone())),
                // Never the key or the prompt themselves: only whether there is one.
                K::ApiKey => set_or_not(
                    active
                        .and_then(|p| p.api_key.as_deref())
                        .is_some_and(|k| !k.trim().is_empty()),
                ),
                K::SystemPrompt => set_or_not(
                    active
                        .and_then(|p| p.system_prompt.as_deref())
                        .is_some_and(|s| !s.trim().is_empty()),
                ),
                _ => json!(active.map(|p| p.load_account_mcp)),
            }
        }
        K::ToolCategories => json!(manager(app)?.get_tool_settings().await?.category_enabled),
        // Names and state only: a server's env often carries an API key.
        K::McpServers => {
            let servers = crate::commands::mcp::get_mcp_servers(app_state(app)?).await?;
            json!(servers
                .iter()
                .map(|s| json!({ "id": s.id, "name": s.name, "enabled": s.enabled, "approved": s.approved }))
                .collect::<Vec<Value>>())
        }
    };
    Ok((value, None))
}

/* ------------------------------- writing ------------------------------- */

fn want_bool(change: &AuthorizedChange) -> Result<bool, String> {
    match change.value() {
        NewValue::Bool(b) => Ok(*b),
        other => Err(format!("Expected on or off, got {other:?}.")),
    }
}

fn want_text(change: &AuthorizedChange) -> Result<&str, String> {
    match change.value() {
        NewValue::Text(s) => Ok(s.as_str()),
        other => Err(format!("Expected text, got {other:?}.")),
    }
}

fn want_number(change: &AuthorizedChange) -> Result<f64, String> {
    match change.value() {
        NewValue::Number(n) => Ok(*n),
        other => Err(format!("Expected a number, got {other:?}.")),
    }
}

/// Pick from a list the running app knows: exact id or name first, then a
/// name that starts with what was asked ("Daniel" finds "Daniel (Enhanced)").
fn pick<'a>(wanted: &str, choices: &'a [(String, String)]) -> Option<&'a (String, String)> {
    let wanted = wanted.trim().to_lowercase();
    choices
        .iter()
        .find(|(id, name)| id.to_lowercase() == wanted || name.to_lowercase() == wanted)
        .or_else(|| {
            choices
                .iter()
                .find(|(_, name)| name.to_lowercase().starts_with(&wanted))
        })
}

fn follows_system(wanted: &str) -> bool {
    matches!(
        wanted.trim().to_lowercase().as_str(),
        "system" | "default" | "system default" | "follow system" | "automatic"
    )
}

/// Make an authorized change through the same Rust function the Settings
/// window invokes for it, so side effects (re-registering a shortcut, the
/// voice audition, the cursor follower) happen exactly as from the UI.
async fn apply(app: &AppHandle, change: &AuthorizedChange) -> Result<(), String> {
    use SettingKey as K;
    let key = change.key();
    match key {
        K::JunoVoice => {
            let wanted = want_text(change)?;
            let list = crate::tts::voices::get_juno_voices(app.clone(), app_state(app)?).await?;
            let choices: Vec<(String, String)> = list
                .options
                .iter()
                .map(|o| (o.id.clone(), o.name.clone()))
                .collect();
            let (id, _) = pick(wanted, &choices).ok_or_else(|| {
                let names: Vec<&str> = choices.iter().map(|(_, n)| n.as_str()).collect();
                format!(
                    "No voice called \"{wanted}\". Voices: {}.",
                    names.join(", ")
                )
            })?;
            crate::tts::voices::set_juno_voice(id.clone(), app.clone(), app_state(app)?).await?;
        }
        K::SpeakingSpeed => {
            crate::tts::voices::set_juno_voice_rate(
                want_number(change)?,
                app.clone(),
                app_state(app)?,
            )
            .await?;
        }
        K::InputDevice | K::OutputDevice => {
            let wanted = want_text(change)?;
            let name = if follows_system(wanted) {
                None
            } else {
                let devices =
                    crate::commands::audio_devices::list_audio_devices(app.clone()).await?;
                let entries = if key == K::InputDevice {
                    devices.inputs
                } else {
                    devices.outputs
                };
                let choices: Vec<(String, String)> = entries
                    .into_iter()
                    .map(|d| (d.name.clone(), d.name))
                    .collect();
                let (name, _) = pick(wanted, &choices).ok_or_else(|| {
                    let names: Vec<&str> = choices.iter().map(|(n, _)| n.as_str()).collect();
                    format!(
                        "No device called \"{wanted}\". Connected: system, {}.",
                        names.join(", ")
                    )
                })?;
                Some(name.clone())
            };
            if key == K::InputDevice {
                crate::commands::audio_devices::set_audio_input_device(
                    name,
                    app.clone(),
                    app_state(app)?,
                )
                .await?;
            } else {
                crate::commands::audio_devices::set_audio_output_device(
                    name,
                    app.clone(),
                    app_state(app)?,
                )
                .await?;
            }
        }
        K::PlaySounds => {
            crate::commands::sound::set_sound_enabled(
                app.clone(),
                want_bool(change)?,
                app_state(app)?,
            )
            .await?;
        }
        K::OpenAtLogin => {
            if want_bool(change)? {
                crate::commands::autostart::enable_autostart(app.clone()).await?;
            } else {
                crate::commands::autostart::disable_autostart(app.clone()).await?;
            }
        }
        K::CursorColor => {
            let mut bar = manager(app)?.get_floating_bar_settings().await?;
            bar.agent_cursor_color = want_text(change)?.to_string();
            crate::commands::settings::set_floating_bar_settings(app.clone(), bar).await?;
        }
        K::AgentShortcut | K::DictationShortcut => {
            let shortcut = want_text(change)?.to_string();
            let target = shortcut_target(key);
            let mut triggers = app_state(app)?.get_triggers()?;
            let index = shortcut_row(&triggers, target).ok_or(
                "There is no key or mouse trigger for that yet. Ask the person to add one in \
                 Settings > Triggers.",
            )?;
            if let Some(row) = triggers.get_mut(index) {
                row.binding = Some(Binding::Keyboard { shortcut });
            }
            crate::commands::triggers::set_triggers(app.clone(), triggers, app_state(app)?).await?;
        }
        K::BackgroundMode => {
            crate::input_control::commands::set_background_mode(want_bool(change)?, app.clone())
                .await?;
        }
        K::PersistentSession => {
            crate::agent::providers::claude_cli_session::set_cli_persistent_session_enabled(
                app.clone(),
                want_bool(change)?,
            )
            .await?;
        }
        K::SmoothMouseMovement => {
            crate::commands::mouse::set_smooth_mouse_movement_setting(
                app.clone(),
                app_state(app)?,
                want_bool(change)?,
            )
            .await?;
        }
        K::MouseControl => {
            crate::input_control::commands::set_mouse_control(
                want_text(change)?.to_string(),
                app.clone(),
            )
            .await?;
        }
        K::AccountConnectors => {
            let provider = manager(app)?.get_provider_settings().await?.active_provider;
            crate::commands::providers::update_provider_load_account_mcp(
                app.clone(),
                provider,
                want_bool(change)?,
            )
            .await?;
        }
        K::ToolCategories => {
            let (category, enabled) = match change.value() {
                NewValue::Category { category, enabled } => (*category, *enabled),
                other => return Err(format!("Expected a tool category, got {other:?}.")),
            };
            crate::commands::tools::set_tool_category_enabled(
                category.to_string(),
                enabled,
                app.clone(),
                app_state(app)?,
            )
            .await?;
        }
        K::McpServers => {
            let mcp = match change.value() {
                NewValue::Mcp(mcp) => mcp.clone(),
                other => return Err(format!("Expected an MCP server change, got {other:?}.")),
            };
            apply_mcp(app, mcp).await?;
        }
        // `authorize_set` never builds a change for these. Listed, not
        // wildcarded, so a new key has to come through here and be decided.
        K::PermissionMode
        | K::AskBeforeSend
        | K::ActiveProvider
        | K::Model
        | K::ApiKey
        | K::SystemPrompt => {
            let spec: SettingSpec = key.spec();
            return Err(Refusal::Protected {
                label: spec.label,
                pane: spec.pane.name(),
            }
            .to_string());
        }
    }
    Ok(())
}

/// Add or remove one MCP server through the window's own commands. An added
/// server is built the way the window's "Add server" box builds it, so it is
/// saved unapproved: its command does not run until the person approves it.
async fn apply_mcp(app: &AppHandle, change: McpChange) -> Result<(), String> {
    match change {
        McpChange::Add {
            name,
            command,
            args,
            env,
            description,
        } => {
            let id = format!(
                "mcp-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            );
            let mut config = crate::agent::tools::MCPServerConfig::new(name.clone(), command, args);
            config.id = id;
            config.description = Some(description.unwrap_or_else(|| format!("MCP Server: {name}")));
            config.environment_variables = env;
            // As the window sends it: on, not auto-started, never pre-approved.
            config.enabled = true;
            config.auto_start = false;
            config.approved = false;
            crate::commands::mcp::add_mcp_server(app.clone(), app_state(app)?, config).await
        }
        McpChange::Remove(target) => {
            let servers = crate::commands::mcp::get_mcp_servers(app_state(app)?).await?;
            let wanted = target.to_lowercase();
            let server = servers
                .iter()
                .find(|s| s.id.to_lowercase() == wanted || s.name.to_lowercase() == wanted)
                .ok_or_else(|| {
                    let names: Vec<&str> = servers.iter().map(|s| s.name.as_str()).collect();
                    format!(
                        "No MCP server called \"{target}\". Servers: {}.",
                        if names.is_empty() {
                            "none".to_string()
                        } else {
                            names.join(", ")
                        }
                    )
                })?;
            crate::commands::mcp::remove_mcp_server(app.clone(), app_state(app)?, server.id.clone())
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_definition_offers_every_action_and_names_the_tool() {
        let def = definition();
        assert_eq!(def.name, "settings");
        let actions = &def.input_schema["properties"]["action"]["enum"];
        for action in ["list", "get", "set", "open", "highlight", "set_advanced"] {
            assert!(
                actions
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v == action)),
                "{action} missing"
            );
        }
        assert!(def.description.contains("protected"));
    }

    #[test]
    fn pick_prefers_an_exact_match_then_a_prefix() {
        let voices = vec![
            (
                "com.apple.daniel".to_string(),
                "Daniel (Enhanced)".to_string(),
            ),
            ("Dan".to_string(), "Dan".to_string()),
        ];
        assert_eq!(pick("dan", &voices).map(|(id, _)| id.as_str()), Some("Dan"));
        assert_eq!(
            pick("Daniel", &voices).map(|(id, _)| id.as_str()),
            Some("com.apple.daniel")
        );
        assert!(pick("Moira", &voices).is_none());
    }

    #[test]
    fn system_words_follow_the_system_device() {
        assert!(follows_system("System"));
        assert!(follows_system(" default "));
        assert!(!follows_system("AirPods"));
    }

    #[test]
    fn a_pending_navigation_is_taken_once() {
        if let Ok(mut pending) = PENDING.lock() {
            *pending = Some(Navigation {
                pane: Some("voice"),
                row: Some("juno-voice"),
                reload: false,
            });
        }
        assert!(take_pending_navigation().is_some());
        assert!(take_pending_navigation().is_none());
    }

    #[test]
    fn the_shortcut_row_skips_voice_triggers() {
        let triggers: Vec<crate::triggers::Trigger> = serde_json::from_value(json!([
            { "id": "a", "gesture": "say", "target": "agent", "phrase": "juno" },
            { "id": "b", "gesture": "hold", "target": "dictation",
              "binding": { "kind": "keyboard", "shortcut": "Fn" } },
            { "id": "c", "gesture": "tap", "target": "agent",
              "binding": { "kind": "keyboard", "shortcut": "Option+Space" } }
        ]))
        .expect("triggers parse");
        assert_eq!(shortcut_row(&triggers, TriggerTarget::Agent), Some(2));
        assert_eq!(shortcut_row(&triggers, TriggerTarget::Dictation), Some(1));
        assert_eq!(
            binding_value(triggers[2].binding.as_ref()),
            json!("Option+Space")
        );
    }

    /// Every protected key is refused before any writer runs, for a value
    /// that would otherwise be valid. The registry tests pin the list; this
    /// pins that the tool's `set` goes through that guard.
    #[test]
    fn set_goes_through_the_guard() {
        let source = include_str!("settings_tool.rs");
        let set_fn = source
            .split("async fn set(")
            .nth(1)
            .and_then(|rest| rest.split("\nasync fn ").next())
            .expect("set exists");
        let guard_at = set_fn
            .find("authorize_set")
            .expect("set calls authorize_set");
        let apply_at = set_fn.find("apply(").expect("set calls apply");
        assert!(guard_at < apply_at, "the guard must run before the write");
        for key in SettingKey::ALL.iter().filter(|k| k.spec().protected) {
            assert!(registry::authorize_set(*key, &json!(true)).is_err());
        }
    }
}
