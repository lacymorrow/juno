use crate::agent::providers::claude_cli;
use crate::settings::{manager::SettingsManager, OnboardingSettings};
use serde::{Deserialize, Serialize};
use std::sync::{atomic::{AtomicU64, Ordering}, Arc, LazyLock};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info, warn};
use uuid::Uuid;
use crate::constants::events;

// ── Onboarding state machine ──────────────────────────────────────────────────

/// The sequential phases of the guided onboarding flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingPhase {
    Greeting,
    ScreenRecording,
    Accessibility,
    OptionalPermissions,
    Provider,
    Ready,
    Complete,
}

impl OnboardingPhase {
    fn advance(&self) -> Option<Self> {
        match self {
            Self::Greeting => Some(Self::ScreenRecording),
            Self::ScreenRecording => Some(Self::Accessibility),
            Self::Accessibility => Some(Self::OptionalPermissions),
            Self::OptionalPermissions => Some(Self::Provider),
            Self::Provider => Some(Self::Ready),
            Self::Ready => Some(Self::Complete),
            Self::Complete => None,
        }
    }

    fn intro_message(&self) -> &'static str {
        match self {
            Self::Greeting => {
                "Welcome to Juno! I'm your AI desktop assistant. I'll guide you through a quick setup to make sure everything works perfectly."
            }
            Self::ScreenRecording => {
                "First, let's set up **Screen Recording** permission. This lets me see your screen so I can help you with visual tasks.\n\nClick the button below to grant access."
            }
            Self::Accessibility => {
                "Next, I need **Accessibility** permission. This lets me interact with your Mac — clicking buttons, typing text, and navigating apps on your behalf.\n\nGrant Accessibility access to continue."
            }
            Self::OptionalPermissions => {
                "Almost there! A couple of **optional** permissions improve your experience:\n\n- **Microphone** — enables voice commands so you can talk to me\n- **Input Monitoring** — lets me detect your keyboard shortcuts globally\n\nGrant these now or skip if you prefer."
            }
            Self::Provider => {
                "Choose how you'd like to connect to the AI:\n\n- **Claude CLI** — use your existing Claude subscription (recommended)\n- **API Key** — enter an Anthropic API key directly\n\nSelect your preferred option below."
            }
            Self::Ready => {
                "You're all set! Here's what you can do:\n\n- Press **⌥D** to summon me\n- Press **⌥Space** to dictate\n- Press **Escape** to stop me\n\nLet's get started!"
            }
            Self::Complete => "Setup complete. Welcome to Juno!",
        }
    }

    /// Whether this phase can be skipped without completing its primary action.
    fn is_skippable(&self) -> bool {
        matches!(self, Self::OptionalPermissions | Self::Provider)
    }
}

/// State snapshot returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingStateInfo {
    pub phase: OnboardingPhase,
    pub can_advance: bool,
    pub can_skip: bool,
}

static ONBOARDING_PHASE: LazyLock<TokioMutex<OnboardingPhase>> =
    LazyLock::new(|| TokioMutex::new(OnboardingPhase::Greeting));

// ── Cursor animation cancellation ────────────────────────────────────────────

/// Monotonically increasing generation counter. Each `animate_cursor_to` call
/// increments this and records its own generation. A running animation task
/// self-aborts when it detects a newer generation has started, making
/// cancellation race-free without any sleep-based synchronization.
static ANIMATION_GENERATION: LazyLock<Arc<AtomicU64>> =
    LazyLock::new(|| Arc::new(AtomicU64::new(0)));

/// Check if we're running in development mode
fn is_development_mode() -> bool {
    #[cfg(debug_assertions)]
    {
        true
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

/// Check if the user has completed onboarding
/// In development mode, this will always return false to show onboarding
#[tauri::command]
pub async fn check_onboarding_status(app: AppHandle) -> Result<bool, String> {
    // In development mode, always show onboarding
    if is_development_mode() {
        info!("Development mode detected - onboarding will always be shown");
        return Ok(false);
    }

    let settings_manager = SettingsManager::new(app).map_err(|e| e.to_string())?;
    let onboarding_settings = settings_manager
        .get_onboarding_settings()
        .await
        .map_err(|e| e.to_string())?;

    Ok(onboarding_settings.completed)
}

/// Mark onboarding as completed
#[tauri::command]
pub async fn complete_onboarding(app: AppHandle) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone()).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    let current_settings = settings_manager
        .get_onboarding_settings()
        .await
        .map_err(|e| e.to_string())?;

    // Preserve skip_count — it is historical data (how many times the user
    // previously skipped). reset_onboarding() zeroes it explicitly.
    let onboarding_settings = OnboardingSettings {
        completed: true,
        completed_at: Some(now.clone()),
        skipped: false,
        skip_count: current_settings.skip_count,
        user_role: current_settings.user_role,
    };

    settings_manager
        .set_onboarding_settings(&onboarding_settings)
        .await
        .map_err(|e| e.to_string())?;

    info!("Onboarding marked as completed at {}", now);

    // Advance the in-memory state machine to Complete so the main window's
    // useOnboardingState hook sees the correct phase and unblocks the chat input.
    set_phase_complete(&app).await;

    // Clear onboarding active state so shortcut handlers resume normal behavior
    if let Err(e) = set_onboarding_active(app.clone(), false).await {
        warn!("Failed to clear onboarding active state on completion: {}", e);
    }

    // Show the main window now that onboarding is done
    if let Err(e) = crate::window_management::open_main_window(app.clone()).await {
        warn!("Failed to show main window after onboarding completion: {}", e);
    }

    Ok(())
}

/// Mark onboarding as skipped (still counts as completed)
#[tauri::command]
pub async fn skip_onboarding(app: AppHandle) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone()).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    // Get current settings to preserve skip count
    let current_settings = settings_manager
        .get_onboarding_settings()
        .await
        .map_err(|e| e.to_string())?;

    let onboarding_settings = OnboardingSettings {
        completed: true,
        completed_at: Some(now.clone()),
        skipped: true,
        skip_count: current_settings.skip_count + 1,
        user_role: current_settings.user_role.clone(),
    };

    settings_manager
        .set_onboarding_settings(&onboarding_settings)
        .await
        .map_err(|e| e.to_string())?;

    info!(
        "Onboarding skipped at {} (skip count: {})",
        now, onboarding_settings.skip_count
    );

    // Advance the in-memory state machine to Complete so the main window's
    // useOnboardingState hook sees the correct phase and unblocks the chat input.
    set_phase_complete(&app).await;

    // Clear onboarding active state so shortcut handlers resume normal behavior
    if let Err(e) = set_onboarding_active(app.clone(), false).await {
        warn!("Failed to clear onboarding active state on skip: {}", e);
    }

    // Show the main window now that onboarding is done (skipped)
    if let Err(e) = crate::window_management::open_main_window(app.clone()).await {
        warn!("Failed to show main window after onboarding skip: {}", e);
    }

    Ok(())
}

/// Reset onboarding (for testing/development and user-requested restart)
#[tauri::command]
pub async fn reset_onboarding(app: AppHandle) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone()).map_err(|e| e.to_string())?;

    let onboarding_settings = OnboardingSettings {
        completed: false,
        completed_at: None,
        skipped: false,
        skip_count: 0,
        user_role: None,
    };

    settings_manager
        .set_onboarding_settings(&onboarding_settings)
        .await
        .map_err(|e| e.to_string())?;

    // Reset permissions state so the permissions flow can be shown again during onboarding
    let app_state = app.state::<crate::state::AppState>();

    // Clear the permissions state in the app state
    app_state
        .update_permissions_state(crate::commands::permissions::PermissionsState {
            accessibility: crate::commands::permissions::PermissionStatus {
                permission_type: "accessibility".to_string(),
                granted: false,
                required: true,
                description: "Accessibility permission needs to be rechecked".to_string(),
                instructions: "Grant accessibility permission during onboarding".to_string(),
            },
            screen_recording: crate::commands::permissions::PermissionStatus {
                permission_type: "screen_recording".to_string(),
                granted: false,
                required: true,
                description: "Screen recording permission needs to be rechecked".to_string(),
                instructions: "Grant screen recording permission during onboarding".to_string(),
            },
            microphone: crate::commands::permissions::PermissionStatus {
                permission_type: "microphone".to_string(),
                granted: false,
                required: false,
                description: "Microphone permission needs to be rechecked".to_string(),
                instructions: "Grant microphone permission if needed".to_string(),
            },
            input_monitoring: crate::commands::permissions::PermissionStatus {
                permission_type: "input_monitoring".to_string(),
                granted: false,
                required: true,
                description: "Input monitoring permission needs to be rechecked".to_string(),
                instructions: "Grant input monitoring permission during onboarding".to_string(),
            },
            all_granted: false,
            app_name: app.package_info().name.clone(),
        })
        .await;

    // Mark permissions as not checked so they will be re-evaluated
    // Reset the permissions checked flag
    if let Ok(mut checked_guard) = app_state.permissions_checked.lock() {
        *checked_guard = false;
    }

    info!("Onboarding reset - permissions state also cleared for fresh onboarding experience");
    Ok(())
}

/// Restart onboarding flow (reset and open onboarding window)
#[tauri::command]
pub async fn restart_onboarding(app: AppHandle) -> Result<(), String> {
    info!("Restarting onboarding flow...");

    // Reset onboarding status (persisted settings)
    reset_onboarding(app.clone()).await?;

    // Reset the in-memory state machine back to Greeting
    {
        let mut phase = ONBOARDING_PHASE.lock().await;
        *phase = OnboardingPhase::Greeting;
    }

    // Open the onboarding window
    if let Err(e) = crate::window_management::open_onboarding_window(app.clone()).await {
        warn!("Failed to open onboarding window: {}", e);
        return Err(format!("Failed to open onboarding window: {}", e));
    }

    info!("Onboarding flow restarted successfully");
    Ok(())
}

/// Get detailed onboarding information
#[tauri::command]
pub async fn get_onboarding_info(app: AppHandle) -> Result<serde_json::Value, String> {
    let settings_manager = SettingsManager::new(app.clone()).map_err(|e| e.to_string())?;
    let onboarding_settings = settings_manager
        .get_onboarding_settings()
        .await
        .map_err(|e| e.to_string())?;

    // Get current keyboard shortcuts for the onboarding display
    let app_state = app.state::<crate::state::AppState>();
    let shortcuts = app_state
        .get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    Ok(serde_json::json!({
        "completed": onboarding_settings.completed,
        "skip_count": onboarding_settings.skip_count,
        "completed_at": onboarding_settings.completed_at,
        "is_development_mode": is_development_mode(),
        "shortcuts": {
            "agent_mode": shortcuts.agent_mode,
            "dictation_input": shortcuts.dictation_input,
            "stop_current_task": shortcuts.stop_current_task
        }
    }))
}

/// Test if global shortcuts are working during onboarding
#[tauri::command]
pub async fn test_global_shortcuts_working(app: AppHandle) -> Result<bool, String> {
    // Check if we have Input Monitoring permissions first
    #[cfg(target_os = "macos")]
    {
        let has_permissions =
            crate::commands::shortcuts::check_input_monitoring_permissions().unwrap_or(false);

        if !has_permissions {
            info!("Input Monitoring permissions not granted - shortcuts won't work");
            return Ok(false);
        }
    }

    // Check if global shortcuts are registered
    let app_state = app.state::<crate::state::AppState>();
    let shortcuts = app_state
        .get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    // Attempt to parse the shortcuts to see if they're valid
    let agent_shortcut_valid =
        crate::events::shortcuts::parse_shortcut_string(&shortcuts.agent_mode).is_some();
    let dictation_shortcut_valid =
        crate::events::shortcuts::parse_shortcut_string(&shortcuts.dictation_input).is_some();

    Ok(agent_shortcut_valid && dictation_shortcut_valid)
}

/// Set onboarding as active and start a listen-only escape key monitor.
/// Controls whether shortcut handlers suppress their normal actions (agent mode,
/// dictation, stop) and only emit visual feedback.
///
/// Uses a `CGEventTap` with `kCGEventTapOptionListenOnly` instead of a global
/// shortcut. This lets the Rust backend detect escape while the key still passes
/// through to HTML dropdowns, dialogs, and other applications.
///
/// Called by `initialize_onboarding_system` before the window opens (sets active=true)
/// and by `complete_onboarding`/`skip_onboarding` when the flow ends (sets active=false).
/// The frontend only calls this on real unmount (window destroyed). Idempotent — safe to
/// call multiple times with the same `active` value.
#[tauri::command]
pub async fn set_onboarding_active(app: AppHandle, active: bool) -> Result<(), String> {
    let app_state = app.state::<crate::state::AppState>();
    let was_active = app_state.is_onboarding_active();

    // Update the flag — shortcut handlers check this to suppress actions during onboarding
    app_state.set_onboarding_active(active);

    // Start/stop the listen-only escape key monitor on state transitions
    if active && !was_active {
        if let Err(e) = crate::platform::escape_key_monitor::start(&app) {
            error!("[Onboarding] Failed to start escape key monitor: {}", e);
        }
    } else if !active && was_active {
        crate::platform::escape_key_monitor::stop();
    }

    info!("[Onboarding] Active state set to: {} (was: {})", active, was_active);
    Ok(())
}

#[derive(serde::Serialize)]
pub struct ClaudeCliStatus {
    pub available: bool,
    pub authenticated: bool,
}

/// Check if Claude CLI is installed and optionally authenticated.
#[tauri::command]
pub async fn check_claude_cli_available() -> Result<ClaudeCliStatus, String> {
    let available = claude_cli::is_claude_cli_available();

    if !available {
        return Ok(ClaudeCliStatus {
            available: false,
            authenticated: false,
        });
    }

    let authenticated = match claude_cli::check_cli_auth_status().await {
        Ok(()) => true,
        Err(e) => {
            info!("Claude CLI found but not authenticated: {}", e);
            false
        }
    };

    Ok(ClaudeCliStatus {
        available,
        authenticated,
    })
}

/// Initialize the onboarding system and check if onboarding should be shown
pub async fn initialize_onboarding_system(app_handle: AppHandle) -> Result<(), String> {
    info!("Initializing onboarding system...");

    // Check if onboarding has been completed (respects development mode)
    let onboarding_completed = check_onboarding_status(app_handle.clone()).await?;

    if !onboarding_completed {
        let mode = if is_development_mode() {
            "development"
        } else {
            "production"
        };
        info!(
            "Onboarding not completed in {} mode, opening onboarding window",
            mode
        );

        // CRITICAL: Set onboarding_active in the backend BEFORE opening the window.
        // This ensures shortcut handlers (dictation, agent) will see onboarding as active
        // immediately. The frontend no longer calls set_onboarding_active(true) on mount —
        // it only clears the flag on real unmount (window destroyed).
        if let Err(e) = set_onboarding_active(app_handle.clone(), true).await {
            error!("[Onboarding] Failed to set onboarding active during init: {}", e);
        }

        // Open the onboarding window and give it focus
        if let Err(e) = crate::window_management::open_onboarding_window(app_handle.clone()).await {
            warn!("Failed to open onboarding window: {}", e);
            return Err(format!("Failed to open onboarding window: {}", e));
        }
    } else {
        info!("Onboarding already completed, showing main window");

        // Sync the in-memory phase to Complete so get_onboarding_state returns the
        // correct value when the main window mounts.
        {
            let mut phase = ONBOARDING_PHASE.lock().await;
            *phase = OnboardingPhase::Complete;
        }

        // Hide the onboarding window (it starts visible from tauri.conf.json)
        if let Err(e) = crate::window_management::close_onboarding_window(app_handle.clone()).await
        {
            warn!("Failed to close onboarding window: {}", e);
        }

        // Show the main window now that we know onboarding is done
        if let Err(e) = crate::window_management::open_main_window(app_handle.clone()).await {
            warn!("Failed to open main window: {}", e);
        }
    }

    Ok(())
}

// ── State machine commands ────────────────────────────────────────────────────

/// Advance the in-memory state machine to `Complete` and emit the
/// `onboarding-state-changed` event so the main window unblocks the chat input.
/// Called by `complete_onboarding` and `skip_onboarding`.
async fn set_phase_complete(app: &AppHandle) {
    {
        let mut phase = ONBOARDING_PHASE.lock().await;
        *phase = OnboardingPhase::Complete;
    }
    let info = OnboardingStateInfo {
        phase: OnboardingPhase::Complete,
        can_advance: false,
        can_skip: false,
    };
    if let Err(e) = app.emit(events::onboarding::STATE_CHANGED, &info) {
        warn!("Failed to emit onboarding state change to Complete: {}", e);
    }
}

/// Return the current onboarding phase without advancing.
#[tauri::command]
pub async fn get_onboarding_state() -> Result<OnboardingStateInfo, String> {
    let phase = ONBOARDING_PHASE.lock().await;
    Ok(OnboardingStateInfo {
        can_advance: *phase != OnboardingPhase::Complete,
        can_skip: phase.is_skippable(),
        phase: phase.clone(),
    })
}

/// Advance or reset the onboarding state machine.
///
/// `action` values:
/// - `"next"` — advance to the next phase (idempotent on `Complete`)
/// - `"skip"` — same as next; only meaningful on skippable phases
/// - `"reset"` — return to `Greeting` (used by restart_onboarding)
#[tauri::command]
pub async fn onboarding_action(app: AppHandle, action: String) -> Result<OnboardingStateInfo, String> {
    let new_phase = {
        let mut phase = ONBOARDING_PHASE.lock().await;
        match action.as_str() {
            "reset" => {
                *phase = OnboardingPhase::Greeting;
            }
            "next" | "skip" => {
                if let Some(next) = phase.advance() {
                    *phase = next;
                }
            }
            other => {
                return Err(format!("Unknown onboarding action: {}", other));
            }
        }
        phase.clone()
    }; // lock released before any .await

    let info = OnboardingStateInfo {
        can_advance: new_phase != OnboardingPhase::Complete,
        can_skip: new_phase.is_skippable(),
        phase: new_phase.clone(),
    };

    if let Err(e) = app.emit(events::onboarding::STATE_CHANGED, &info) {
        warn!("Failed to emit onboarding state change: {}", e);
    }

    emit_onboarding_message(&app, new_phase.intro_message());

    Ok(info)
}

/// Stream an onboarding message through the standard agent-text-stream pipeline
/// so it renders naturally in the chat UI.
fn emit_onboarding_message(app: &AppHandle, message: &str) {
    let message_id = Uuid::new_v4().to_string();

    if let Err(e) = app.emit(
        events::streaming::STREAM_START,
        serde_json::json!({ "message_id": message_id }),
    ) {
        warn!("Failed to emit stream start: {}", e);
    }

    if let Err(e) = app.emit(
        events::streaming::TEXT_STREAM,
        serde_json::json!({
            "chunk": message,
            "message_id": message_id,
            "tts_content": null,
            "metadata": { "has_spoken_content": false, "spoken_text": null }
        }),
    ) {
        warn!("Failed to emit text stream: {}", e);
    }

    if let Err(e) = app.emit(
        events::streaming::STREAM_END,
        serde_json::json!({
            "message_id": message_id,
            "complete_text": message,
            "is_jsx": false
        }),
    ) {
        warn!("Failed to emit stream end: {}", e);
    }
}

// ── Cursor animation commands ─────────────────────────────────────────────────

/// Animate the Juno cursor sprite along a quadratic Bezier arc from
/// `(from_x, from_y)` to `(to_x, to_y)`, emitting `cursor-animation-frame`
/// events at ~60fps.
///
/// Duration scales linearly with distance: clamp(0.4 + 0.8 * dist/2000, 0.4, 1.2) seconds.
/// Each frame applies smoothstep easing: t² × (3 - 2t).
/// The control point arcs upward by 25% of the chord length to create a natural arc.
#[tauri::command]
pub async fn animate_cursor_to(
    app: AppHandle,
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
    style: Option<String>,
) -> Result<(), String> {
    // Claim a new generation — any older task will self-abort when it next checks.
    let my_gen = ANIMATION_GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    let gen_ref = ANIMATION_GENERATION.clone();
    let style_str = style.unwrap_or_else(|| "arc".to_string());

    tauri::async_runtime::spawn(async move {
        let dx = to_x - from_x;
        let dy = to_y - from_y;
        let distance = (dx * dx + dy * dy).sqrt();

        if distance < 1.0 {
            return; // Already at target
        }

        // Duration proportional to distance, clamped to [0.4, 1.2] seconds
        let duration_secs = (0.4_f64 + 0.8 * (distance / 2000.0)).clamp(0.4, 1.2);
        let total_frames = (duration_secs * 60.0).ceil() as u64;
        let frame_ms = (duration_secs * 1000.0 / total_frames as f64).max(1.0) as u64;

        // Control point: midpoint offset perpendicular to the chord (arcs upward/left)
        let arc_height = distance * 0.25;
        let perp_x = -dy / distance * arc_height;
        let perp_y =  dx / distance * arc_height;
        let cx = (from_x + to_x) / 2.0 + perp_x;
        let cy = (from_y + to_y) / 2.0 + perp_y;

        for frame in 0..=total_frames {
            // Self-abort if a newer animation has been requested
            if gen_ref.load(Ordering::Acquire) != my_gen {
                break;
            }

            // Linear t in [0, 1]
            let t_linear = frame as f64 / total_frames as f64;
            // Smoothstep easing: t² × (3 - 2t)
            let t = t_linear * t_linear * (3.0 - 2.0 * t_linear);

            // Quadratic Bezier: B(t) = (1-t)²P0 + 2(1-t)tP1 + t²P2
            let inv_t = 1.0 - t;
            let x = inv_t * inv_t * from_x + 2.0 * inv_t * t * cx + t * t * to_x;
            let y = inv_t * inv_t * from_y + 2.0 * inv_t * t * cy + t * t * to_y;

            if let Err(e) = app.emit(
                events::cursor::ANIMATION_FRAME,
                serde_json::json!({ "x": x, "y": y, "t": t_linear, "style": style_str }),
            ) {
                warn!("Failed to emit cursor animation frame: {}", e);
                break;
            }

            if frame < total_frames {
                tokio::time::sleep(tokio::time::Duration::from_millis(frame_ms)).await;
            }
        }
    });

    Ok(())
}

/// Show a pulsing highlight ring at `(x, y)` on the cursor overlay.
#[tauri::command]
pub async fn show_cursor_highlight(
    app: AppHandle,
    x: f64,
    y: f64,
    radius: Option<f64>,
) -> Result<(), String> {
    app.emit(
        events::cursor::HIGHLIGHT,
        serde_json::json!({
            "x": x,
            "y": y,
            "radius": radius.unwrap_or(30.0)
        }),
    )
    .map_err(|e| format!("Failed to emit cursor highlight: {}", e))
}

/// Show a speech bubble at `(x, y)` on the cursor overlay.
#[tauri::command]
pub async fn show_cursor_bubble(
    app: AppHandle,
    x: f64,
    y: f64,
    text: String,
) -> Result<(), String> {
    app.emit(
        events::cursor::BUBBLE,
        serde_json::json!({
            "x": x,
            "y": y,
            "text": text
        }),
    )
    .map_err(|e| format!("Failed to emit cursor bubble: {}", e))
}

/// Cancel any running animation and dismiss the cursor overlay with a fade-out.
#[tauri::command]
pub async fn dismiss_cursor_overlay(app: AppHandle) -> Result<(), String> {
    // Bump generation so any in-flight animation task self-aborts
    ANIMATION_GENERATION.fetch_add(1, Ordering::AcqRel);

    app.emit(events::cursor::DISMISS_OVERLAY, serde_json::json!({ "animate": true }))
        .map_err(|e| format!("Failed to emit cursor dismiss: {}", e))
}

/// Save the user's selected role during onboarding.
/// Persists to OnboardingSettings so it survives restarts.
#[tauri::command]
pub async fn save_user_role(app: AppHandle, role: String) -> Result<(), String> {
    let role = role.trim().to_string();
    if role.is_empty() {
        return Err("Role cannot be empty".to_string());
    }
    if role.chars().count() > 64 {
        return Err("Role too long (max 64 characters)".to_string());
    }

    let settings_manager = SettingsManager::new(app.clone()).map_err(|e| e.to_string())?;

    let mut onboarding_settings = settings_manager
        .get_onboarding_settings()
        .await
        .map_err(|e| e.to_string())?;

    info!("User role saved: {}", role);
    onboarding_settings.user_role = Some(role);

    settings_manager
        .set_onboarding_settings(&onboarding_settings)
        .await
        .map_err(|e| e.to_string())
}
