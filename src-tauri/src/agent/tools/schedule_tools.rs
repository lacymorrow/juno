//! # Schedule Tools Module
//!
//! Agent-facing tools for user-visible scheduled automations (LAC-1431).
//! Lets the agent turn natural-language requests like "check my emails every
//! morning" into persistent cron schedules that fire the agent back later.
//!
//! The agent performs the natural-language → cron conversion itself (guided by
//! the tool descriptions); these tools only validate, persist, and manage the
//! schedules via `crate::scheduler`. Schedules created here are the same ones
//! users see and manage in Settings → Automations.
//!
//! Registration: `register_schedule_tools()` called from the provider factory.

use crate::agent::core::ToolDefinition;
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::constants::agent;
use crate::scheduler::{
    self, compute_next_run, load_automations, with_automations, ScheduledAutomation,
};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};
use uuid::Uuid;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

fn create_definition() -> ToolDefinition {
    ToolDefinition {
        name: agent::tool_names::CREATE_SCHEDULED_AUTOMATION.to_string(),
        description: "Creates a recurring scheduled automation that runs an agent query on a cron schedule. Use this when the user asks for something to happen repeatedly at specific times, e.g. 'check my emails every morning' or 'remind me to review PRs on Friday'. Convert the user's natural-language schedule to a cron expression yourself using the LOCAL timezone. Cron format is 6 fields with seconds: 'sec min hour day-of-month month day-of-week' (standard 5-field expressions are also accepted). Examples: every day at 9am = '0 0 9 * * *'; every Monday at 9am = '0 0 9 * * MON'; every hour = '0 0 * * * *'; weekdays at 5:30pm = '0 30 17 * * MON-FRI'. The query should be a complete, self-contained instruction the agent can execute later without this conversation's context.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Short human-readable name shown to the user, e.g. 'Morning email check'"
                },
                "query": {
                    "type": "string",
                    "description": "Self-contained agent query to run when the schedule fires, e.g. 'Check my emails and summarize anything important'"
                },
                "cron": {
                    "type": "string",
                    "description": "Cron expression (6-field with seconds, or standard 5-field), evaluated in the user's local timezone"
                },
                "natural_language": {
                    "type": "string",
                    "description": "The user's original schedule phrasing, e.g. 'every Monday at 9am' (shown in the UI)"
                },
                "notify": {
                    "type": "boolean",
                    "description": "Show a system notification when the automation runs (default true)"
                }
            },
            "required": ["name", "query", "cron"]
        }),
        api_type: None,
        beta_flag: None,
    }
}

async fn create_exec(input: Value, app_handle: AppHandle) -> Result<Value, String> {
    let name = input
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("Missing required parameter: name")?
        .to_string();
    let query = input
        .get("query")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("Missing required parameter: query")?
        .to_string();
    let cron = input
        .get("cron")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: cron")?;
    let natural_language = input
        .get("natural_language")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let notify = input.get("notify").and_then(|v| v.as_bool()).unwrap_or(true);

    let next_run_at = compute_next_run(cron)?;
    scheduler::validate_cron_interval(cron)?;
    let automation = ScheduledAutomation {
        id: Uuid::new_v4().to_string(),
        name,
        query,
        cron: scheduler::normalize_cron(cron),
        natural_language,
        enabled: true,
        notify,
        created_at: now_secs(),
        last_run_at: None,
        next_run_at: Some(next_run_at),
        last_result: None,
    };

    with_automations(&app_handle, |automations| {
        automations.push(automation.clone());
        Ok(((), true))
    })
    .await?;

    info!(
        "Agent created scheduled automation '{}' ({}) with cron '{}'",
        automation.name, automation.id, automation.cron
    );

    // Surface agent-initiated persistence at creation time: the `notify` flag
    // only covers firing, and the user may never open Settings → Automations.
    let notification = app_handle
        .notification()
        .builder()
        .title("Juno: automation created")
        .body(format!(
            "The agent scheduled '{}' ({})",
            automation.name,
            automation
                .natural_language
                .as_deref()
                .unwrap_or(&automation.cron)
        ))
        .show();
    if let Err(e) = notification {
        warn!("Failed to show automation-created notification: {}", e);
    }
    Ok(json!({
        "success": true,
        "automation": automation,
        "message": format!(
            "Scheduled automation '{}' created. Next run at Unix timestamp {}.",
            automation.name, next_run_at
        )
    }))
}

fn list_definition() -> ToolDefinition {
    ToolDefinition {
        name: agent::tool_names::LIST_SCHEDULED_AUTOMATIONS.to_string(),
        description: "Lists all scheduled automations (recurring agent tasks) with their schedules, status, and next run times. Use before creating a new automation to avoid duplicates, or when the user asks what is scheduled.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
        api_type: None,
        beta_flag: None,
    }
}

async fn list_exec(_input: Value, app_handle: AppHandle) -> Result<Value, String> {
    let automations = load_automations(&app_handle)?;
    Ok(json!({
        "count": automations.len(),
        "automations": automations
    }))
}

fn delete_definition() -> ToolDefinition {
    ToolDefinition {
        name: agent::tool_names::DELETE_SCHEDULED_AUTOMATION.to_string(),
        description: "Deletes a scheduled automation by id. Use list_scheduled_automations first to find the id. Only delete when the user explicitly asks to cancel or remove a scheduled task.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The id of the scheduled automation to delete"
                }
            },
            "required": ["id"]
        }),
        api_type: None,
        beta_flag: None,
    }
}

async fn delete_exec(input: Value, app_handle: AppHandle) -> Result<Value, String> {
    let id = input
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: id")?;

    with_automations(&app_handle, |automations| {
        let before = automations.len();
        automations.retain(|a| a.id != id);
        if automations.len() == before {
            return Err(format!("No scheduled automation with id '{}'", id));
        }
        Ok(((), true))
    })
    .await?;

    info!("Agent deleted scheduled automation {}", id);
    Ok(json!({ "success": true, "deleted_id": id }))
}

/// Registers the scheduled automation tools with the given tool provider.
///
/// # Tools Registered
/// - `create_scheduled_automation`: Create a recurring cron-scheduled agent task
/// - `list_scheduled_automations`: List all schedules with status and next runs
/// - `delete_scheduled_automation`: Remove a schedule by id
pub async fn register_schedule_tools(provider: &mut LocalToolProvider, app_handle: AppHandle) {
    let create_def = create_definition();
    let create_handle = app_handle.clone();
    let create_fn = move |input| {
        let handle = create_handle.clone();
        async move { create_exec(input, handle).await }
    };
    provider.register_async_tool(create_def, create_fn).await;

    let list_def = list_definition();
    let list_handle = app_handle.clone();
    let list_fn = move |input| {
        let handle = list_handle.clone();
        async move { list_exec(input, handle).await }
    };
    provider.register_async_tool(list_def, list_fn).await;

    let delete_def = delete_definition();
    let delete_handle = app_handle.clone();
    let delete_fn = move |input| {
        let handle = delete_handle.clone();
        async move { delete_exec(input, handle).await }
    };
    provider.register_async_tool(delete_def, delete_fn).await;

    info!("Registered schedule tools: create_scheduled_automation, list_scheduled_automations, delete_scheduled_automation");
}
