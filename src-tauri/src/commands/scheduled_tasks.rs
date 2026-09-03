//! # Scheduled Task Commands
//!
//! Tauri commands exposing user-facing scheduled automations (LAC-1431).
//! Thin CRUD layer over `crate::scheduler` — all persistence lives in the
//! Tauri Store, and the background scheduler loop picks changes up on its
//! next tick because it reloads the store every cycle.

use tauri::AppHandle;
use tracing::info;
use uuid::Uuid;

use crate::scheduler::{
    self, compute_next_run, load_automations, preview_next_runs, with_automations,
    ScheduledAutomation,
};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs()
}

/// Creates a new scheduled automation and returns it.
#[tauri::command]
pub async fn create_scheduled_task(
    app: AppHandle,
    name: String,
    query: String,
    cron: String,
    natural_language: Option<String>,
    notify: Option<bool>,
) -> Result<ScheduledAutomation, String> {
    let name = name.trim().to_string();
    let query = query.trim().to_string();
    if name.is_empty() {
        return Err("Automation name cannot be empty".to_string());
    }
    if query.is_empty() {
        return Err("Automation query cannot be empty".to_string());
    }

    let next_run_at = compute_next_run(&cron)?;
    scheduler::validate_cron_interval(&cron)?;
    let automation = ScheduledAutomation {
        id: Uuid::new_v4().to_string(),
        name,
        query,
        cron: scheduler::normalize_cron(&cron),
        natural_language,
        enabled: true,
        notify: notify.unwrap_or(true),
        created_at: now_secs(),
        last_run_at: None,
        next_run_at: Some(next_run_at),
        last_result: None,
    };

    with_automations(&app, |automations| {
        automations.push(automation.clone());
        Ok(((), true))
    })
    .await?;

    info!(
        "Created scheduled automation '{}' ({}) with cron '{}'",
        automation.name, automation.id, automation.cron
    );
    Ok(automation)
}

/// Lists all scheduled automations.
#[tauri::command]
pub async fn list_scheduled_tasks(app: AppHandle) -> Result<Vec<ScheduledAutomation>, String> {
    load_automations(&app)
}

/// Updates fields of an existing automation. Only provided fields change.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_scheduled_task(
    app: AppHandle,
    id: String,
    name: Option<String>,
    query: Option<String>,
    cron: Option<String>,
    natural_language: Option<String>,
    enabled: Option<bool>,
    notify: Option<bool>,
) -> Result<ScheduledAutomation, String> {
    with_automations(&app, move |automations| {
        let automation = automations
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("No scheduled automation with id '{}'", id))?;

        if let Some(name) = name {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err("Automation name cannot be empty".to_string());
            }
            automation.name = name;
        }
        if let Some(query) = query {
            let query = query.trim().to_string();
            if query.is_empty() {
                return Err("Automation query cannot be empty".to_string());
            }
            automation.query = query;
        }
        if let Some(cron) = cron {
            scheduler::validate_cron_interval(&cron)?;
            automation.next_run_at = Some(compute_next_run(&cron)?);
            automation.cron = scheduler::normalize_cron(&cron);
        }
        if let Some(natural_language) = natural_language {
            automation.natural_language = Some(natural_language);
        }
        if let Some(enabled) = enabled {
            automation.enabled = enabled;
            if enabled {
                // Always recompute from the cron — reusing a next_run_at that
                // went stale while the automation was paused would fire it
                // immediately on the next tick instead of on schedule.
                automation.next_run_at = Some(compute_next_run(&automation.cron)?);
            } else {
                automation.next_run_at = None;
            }
        }
        if let Some(notify) = notify {
            automation.notify = notify;
        }

        Ok((automation.clone(), true))
    })
    .await
}

/// Deletes a scheduled automation by id.
#[tauri::command]
pub async fn delete_scheduled_task(app: AppHandle, id: String) -> Result<(), String> {
    with_automations(&app, |automations| {
        let before = automations.len();
        automations.retain(|a| a.id != id);
        if automations.len() == before {
            return Err(format!("No scheduled automation with id '{}'", id));
        }
        Ok(((), true))
    })
    .await?;
    info!("Deleted scheduled automation {}", id);
    Ok(())
}

/// Fires a scheduled automation immediately (without changing its schedule).
#[tauri::command]
pub async fn run_scheduled_task_now(app: AppHandle, id: String) -> Result<(), String> {
    // Claim the id first so the tick loop cannot fire the same automation
    // while this manual run is in flight (and vice versa).
    let _claim = scheduler::try_claim_firing(&id)
        .ok_or_else(|| "This automation is already running".to_string())?;

    let automations = load_automations(&app)?;
    let mut automation = automations
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| format!("No scheduled automation with id '{}'", id))?;

    scheduler::fire_automation(&app, &mut automation).await;
    // Merge the result by id instead of saving the pre-run snapshot: the agent
    // run above can take minutes, during which the user may edit other automations.
    let (last_run_at, last_result) = (automation.last_run_at, automation.last_result);
    let result = last_result.clone();
    scheduler::update_automation(&app, &id, |a| {
        a.last_run_at = last_run_at;
        a.last_result = last_result;
    })
    .await?;

    match result.as_deref() {
        Some(r) if r.starts_with("error") => Err(r.to_string()),
        _ => Ok(()),
    }
}

/// Validates a cron expression and returns its next few run times (Unix seconds)
/// so the UI can preview the schedule before saving.
#[tauri::command]
pub async fn preview_cron_schedule(cron: String) -> Result<Vec<u64>, String> {
    preview_next_runs(&cron, 3)
}
