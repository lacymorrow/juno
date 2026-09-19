//! # Permission diagnostics
//!
//! What macOS answers about this process right now, per permission, plus the
//! one destructive repair for the case the answer exposes.
//!
//! ## Why this exists
//!
//! A TCC grant is keyed to the bundle id *and* the code signature. A
//! development build is re-signed on every rebuild, so the row System Settings
//! shows can belong to a signature that no longer exists. The switch stays on,
//! the live API says no, and nothing in the app admits the difference. That
//! gap is the bug, and it is unfalsifiable from the outside: "I granted this
//! and it still does not work" is exactly what a stale entry looks like.
//!
//! So this module reports one thing honestly: the answer the live API gives
//! this process, right now. It cannot read the checkbox in System Settings,
//! and it does not pretend to. It pairs each answer with the deep link to the
//! pane so a person can compare the two themselves.
//!
//! ## Two properties this module must keep
//!
//! 1. Looking never prompts. Every check here is a preflight-style call. None
//!    of them pass `kAXTrustedCheckOptionPrompt` or reach for the requesting
//!    variant, so opening this panel can never raise a system dialog as a side
//!    effect of being looked at. See the comment on
//!    `native_permissions::request_accessibility_permission` for the history.
//! 2. Looking never reads the cache. `check_permissions_status_native` has a
//!    short-TTL cache that is right for the UI and wrong for a diagnostic: a
//!    cached answer is the one thing this panel must not report.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tracing::{info, warn};

use crate::constants::permissions::{tcc_services, tools, types, urls};
use crate::state::AppState;

/// What macOS answered about this process.
///
/// `Denied` and `NotDetermined` are separate because only some APIs can tell
/// them apart. `IOHIDCheckAccess` and `AVCaptureDevice.authorizationStatus`
/// both report "never asked" distinctly; `AXIsProcessTrustedWithOptions` and
/// `CGPreflightScreenCaptureAccess` return a plain bool and cannot. Where the
/// API cannot tell, this reports `Denied` and the detail line says so rather
/// than inventing a confidence the call does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveAnswer {
    /// macOS trusts this process for this permission.
    Granted,
    /// macOS does not. A normal state, not an error.
    Denied,
    /// This process has never asked, so there is no row in the pane yet.
    NotDetermined,
    /// The check itself failed. Nothing is known either way.
    Unreadable,
    /// Not a macOS build, so there is no TCC to ask.
    NotApplicable,
}

/// One permission, as macOS sees it right now.
///
/// Field naming is snake_case to match `NativePermissionStatus`, which the
/// frontend already consumes that way.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDiagnostic {
    /// Stable key, matching `constants::permissions::types`.
    pub permission_type: String,
    /// Human name, matching the System Settings pane.
    pub label: String,
    /// Whether Juno needs it to act at all.
    pub required: bool,
    /// The live answer.
    pub answer: LiveAnswer,
    /// One plain sentence about what this answer does and does not establish.
    pub answer_detail: String,
    /// The API that produced the answer, named so it can be checked.
    pub api: String,
    /// The TCC service name macOS files this under, and the one `tccutil`
    /// takes. Shown because it is the word the person will see in any other
    /// tool that reads TCC.
    pub tcc_service: String,
    /// Deep link to the exact Privacy pane.
    pub settings_url: String,
    /// Where that link lands, spelled out.
    pub settings_label: String,
    /// What stops working without it. Keeps the panel diagnostic rather than
    /// abstract: Accessibility is the one the Fn key monitor gates on.
    pub gates: String,
    /// True once this launch has reset this permission. After a reset the
    /// running process keeps answering from the decision it already has, so
    /// the row above is known-stale until Juno relaunches.
    pub reset_this_launch: bool,
}

/// The whole report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDiagnosticsReport {
    /// The bundle id of the running app, read from the running app. The demo
    /// build ships under a different id, and a hardcoded one would diagnose
    /// (and reset) the wrong app's grants.
    pub bundle_id: String,
    pub app_name: String,
    pub app_version: String,
    /// True for a debug build. Worth showing: a debug build is re-signed on
    /// every rebuild, which is where a stale TCC entry comes from.
    pub debug_build: bool,
    pub platform_supported: bool,
    /// Whether the reset controls should be offered at all.
    pub reset_available: bool,
    /// Why not, when not. `None` when reset is available.
    pub reset_unavailable_reason: Option<String>,
    /// Whether `app.restart()` can be offered after a reset. Tauri 2.11 has
    /// it, and `restart_app_after_permissions` already uses it.
    pub relaunch_available: bool,
    pub permissions: Vec<PermissionDiagnostic>,
}

/// The result of one reset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResetOutcome {
    pub permission_type: String,
    pub tcc_service: String,
    pub bundle_id: String,
    /// True when macOS cleared a record or confirmed there was none to clear.
    pub succeeded: bool,
    /// Plain sentence, safe to show as-is.
    pub message: String,
    /// Always true on success: the running process cannot see the new state.
    pub relaunch_required: bool,
}

// ── Which permissions this reports on ────────────────────────────────────────
//
// Exactly the four Juno uses. `tccutil` accepts `SystemPolicyAllFiles` (Full
// Disk Access) and `PostEvent` too, and both are deliberately absent: Juno
// asks for neither, so offering to reset them would be a control that can only
// do harm to some other app's expectations.

/// Permissions in report order: required first, then optional.
const REPORTED: [&str; 4] = [
    types::ACCESSIBILITY,
    types::SCREEN_RECORDING,
    types::INPUT_MONITORING,
    types::MICROPHONE,
];

/// Map a Juno permission key to the TCC service name `tccutil` takes.
///
/// Returns `None` for anything unknown, which is what keeps a caller-supplied
/// string from reaching the command line.
pub fn tcc_service_for(permission_type: &str) -> Option<&'static str> {
    match permission_type {
        types::ACCESSIBILITY => Some(tcc_services::ACCESSIBILITY),
        types::INPUT_MONITORING => Some(tcc_services::INPUT_MONITORING),
        types::MICROPHONE => Some(tcc_services::MICROPHONE),
        types::SCREEN_RECORDING => Some(tcc_services::SCREEN_RECORDING),
        _ => None,
    }
}

fn label_for(permission_type: &str) -> &'static str {
    match permission_type {
        types::ACCESSIBILITY => "Accessibility",
        types::INPUT_MONITORING => "Input Monitoring",
        types::MICROPHONE => "Microphone",
        types::SCREEN_RECORDING => "Screen Recording",
        _ => "Unknown",
    }
}

fn settings_url_for(permission_type: &str) -> &'static str {
    match permission_type {
        types::ACCESSIBILITY => urls::ACCESSIBILITY_PANEL,
        types::INPUT_MONITORING => urls::INPUT_MONITORING_PANEL,
        types::MICROPHONE => urls::MICROPHONE_PANEL,
        types::SCREEN_RECORDING => urls::SCREEN_RECORDING_PANEL,
        _ => urls::SYSTEM_PREFERENCES_PRIVACY,
    }
}

fn settings_label_for(permission_type: &str) -> String {
    format!("Privacy & Security > {}", label_for(permission_type))
}

fn gates_for(permission_type: &str) -> &'static str {
    match permission_type {
        // Named specifically because this is the one that produces the
        // "I granted it and nothing happens" report: without Accessibility the
        // modifier key monitor installs its local half only, so the Fn key
        // fires solely while a Juno window is focused.
        types::ACCESSIBILITY => {
            "Clicking, typing and window control. The Fn key monitor gates its global half on this, so without it the key only fires while a Juno window is focused."
        }
        types::SCREEN_RECORDING => "Screenshots and anything that reads the screen.",
        types::INPUT_MONITORING => "Global shortcuts and key monitoring outside Juno's own windows.",
        types::MICROPHONE => "Dictation, voice sessions and wake words.",
        _ => "",
    }
}

fn is_required(permission_type: &str) -> bool {
    // Matches `check_permissions_status_native`: Accessibility and Screen
    // Recording gate whether Juno can act at all.
    matches!(
        permission_type,
        types::ACCESSIBILITY | types::SCREEN_RECORDING
    )
}

// ── Reset ledger ─────────────────────────────────────────────────────────────

/// Permissions reset during this launch.
///
/// After `tccutil reset`, macOS keeps answering the running process from the
/// decision it already handed out, and some of the checks cache on our side
/// too. So the answer above a reset row is known-stale, and saying that
/// plainly is more useful than re-reading a number that cannot have changed.
static RESET_THIS_LAUNCH: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

fn note_reset(permission_type: &str) {
    match RESET_THIS_LAUNCH.lock() {
        Ok(mut set) => {
            set.insert(permission_type.to_string());
        }
        Err(_) => warn!("Reset ledger lock poisoned; the relaunch notice may be missing"),
    }
}

fn was_reset_this_launch(permission_type: &str) -> bool {
    RESET_THIS_LAUNCH
        .lock()
        .map(|set| set.contains(permission_type))
        .unwrap_or(false)
}

// ── Live reads ───────────────────────────────────────────────────────────────

/// What answered, and what that answer does and does not establish.
struct LiveRead {
    answer: LiveAnswer,
    api: &'static str,
    detail: &'static str,
}

#[cfg(target_os = "macos")]
fn read_accessibility() -> LiveRead {
    use crate::commands::native_permissions::NativePermissionChecker;

    // Preflight only. The prompting variant is deliberately not used here.
    match NativePermissionChecker::check_accessibility_permission() {
        Ok(true) => LiveRead {
            answer: LiveAnswer::Granted,
            api: "AXIsProcessTrustedWithOptions, prompt option off",
            detail: "macOS trusts this process for Accessibility right now.",
        },
        Ok(false) => LiveRead {
            answer: LiveAnswer::Denied,
            api: "AXIsProcessTrustedWithOptions, prompt option off",
            detail: "macOS does not trust this process. This API returns a plain yes or no, so it cannot tell a switched-off row from one that was never created.",
        },
        Err(_) => LiveRead {
            answer: LiveAnswer::Unreadable,
            api: "AXIsProcessTrustedWithOptions, prompt option off",
            detail: "The check did not complete, so nothing is known either way.",
        },
    }
}

#[cfg(target_os = "macos")]
fn read_screen_recording() -> LiveRead {
    use crate::commands::native_permissions::NativePermissionChecker;

    match NativePermissionChecker::check_screen_recording_permission() {
        Ok(true) => LiveRead {
            answer: LiveAnswer::Granted,
            api: "CGPreflightScreenCaptureAccess",
            detail: "macOS grants this process screen capture right now.",
        },
        Ok(false) => LiveRead {
            answer: LiveAnswer::Denied,
            api: "CGPreflightScreenCaptureAccess",
            detail: "macOS does not. Preflight returns a plain yes or no, and a running process keeps hearing its first answer, so a grant made since launch shows here only after a relaunch.",
        },
        Err(_) => LiveRead {
            answer: LiveAnswer::Unreadable,
            api: "CGPreflightScreenCaptureAccess",
            detail: "The check did not complete, so nothing is known either way.",
        },
    }
}

#[cfg(target_os = "macos")]
fn read_input_monitoring() -> LiveRead {
    use crate::platform::input_monitoring::{check_input_monitoring_access, InputMonitoringAccess};

    // `IOHIDCheckAccess` asks. `IOHIDRequestAccess` is the one that prompts,
    // and it is not used here.
    match check_input_monitoring_access() {
        InputMonitoringAccess::Granted => LiveRead {
            answer: LiveAnswer::Granted,
            api: "IOHIDCheckAccess",
            detail: "macOS grants this process input monitoring right now.",
        },
        InputMonitoringAccess::Denied => LiveRead {
            answer: LiveAnswer::Denied,
            api: "IOHIDCheckAccess",
            detail: "Juno is listed and switched off, or the record no longer matches this build. A running process keeps hearing its first answer, so a grant made since launch shows here only after a relaunch.",
        },
        InputMonitoringAccess::Unknown => LiveRead {
            answer: LiveAnswer::NotDetermined,
            api: "IOHIDCheckAccess",
            detail: "This process has never asked, so there is no row in the pane yet.",
        },
    }
}

#[cfg(target_os = "macos")]
fn read_microphone() -> LiveRead {
    use tauri_plugin_voice_transcription::mic_permissions;

    match mic_permissions::check_microphone_permission() {
        mic_permissions::MicrophonePermissionStatus::Granted => LiveRead {
            answer: LiveAnswer::Granted,
            api: "AVCaptureDevice.authorizationStatus, media type audio",
            detail: "macOS grants this process the microphone right now. A granted answer is cached for the life of the process, so a grant revoked since launch still reads as granted until Juno relaunches.",
        },
        mic_permissions::MicrophonePermissionStatus::Denied => LiveRead {
            answer: LiveAnswer::Denied,
            api: "AVCaptureDevice.authorizationStatus, media type audio",
            detail: "macOS has refused this process. A refusal sticks for the life of the process, so a grant made since launch shows here only after a relaunch.",
        },
        mic_permissions::MicrophonePermissionStatus::Undetermined => LiveRead {
            answer: LiveAnswer::NotDetermined,
            api: "AVCaptureDevice.authorizationStatus, media type audio",
            detail: "This process has never asked, so there is no row in the pane yet.",
        },
        mic_permissions::MicrophonePermissionStatus::NotApplicable => LiveRead {
            answer: LiveAnswer::NotApplicable,
            api: "AVCaptureDevice.authorizationStatus, media type audio",
            detail: "There is no microphone permission to ask about on this platform.",
        },
    }
}

#[cfg(not(target_os = "macos"))]
fn read_not_applicable() -> LiveRead {
    LiveRead {
        answer: LiveAnswer::NotApplicable,
        api: "none",
        detail: "TCC is a macOS system, so there is nothing to ask on this platform.",
    }
}

fn read_live(permission_type: &str) -> LiveRead {
    #[cfg(target_os = "macos")]
    {
        match permission_type {
            types::ACCESSIBILITY => read_accessibility(),
            types::SCREEN_RECORDING => read_screen_recording(),
            types::INPUT_MONITORING => read_input_monitoring(),
            types::MICROPHONE => read_microphone(),
            _ => LiveRead {
                answer: LiveAnswer::Unreadable,
                api: "none",
                detail: "Juno does not use this permission.",
            },
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = permission_type;
        read_not_applicable()
    }
}

// ── tccutil ──────────────────────────────────────────────────────────────────

/// Accept a bundle id only if it looks like one.
///
/// This is the guard that makes the dangerous form of the command
/// unreachable. `tccutil reset SERVICE` with no bundle id resets that service
/// for EVERY app on the machine, so nothing in this module builds an argv
/// without a bundle id that passed through here: empty, whitespace, anything
/// that could read as a flag, and anything carrying a shell or argument
/// separator are all refused before a `Command` exists.
fn validate_bundle_id(bundle_id: &str) -> Result<String, String> {
    let trimmed = bundle_id.trim();

    if trimmed.is_empty() {
        return Err("No bundle id, so there is nothing safe to reset.".to_string());
    }
    if trimmed.starts_with('-') {
        return Err("A bundle id cannot start with a dash.".to_string());
    }
    if !trimmed.contains('.') {
        return Err("That is not a bundle id.".to_string());
    }
    if trimmed.starts_with('.') || trimmed.ends_with('.') {
        return Err("That is not a bundle id.".to_string());
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        return Err(
            "A bundle id holds only letters, digits, dots, dashes and underscores.".to_string(),
        );
    }

    Ok(trimmed.to_string())
}

/// Build the exact argv for one reset.
///
/// Three arguments, always: `reset`, the service, the bundle id. The shape is
/// fixed here rather than at the call site so the tests can hold it still.
fn reset_command_args(permission_type: &str, bundle_id: &str) -> Result<[String; 3], String> {
    let service = tcc_service_for(permission_type)
        .ok_or_else(|| format!("Juno does not use a permission called {}.", permission_type))?;
    let bundle_id = validate_bundle_id(bundle_id)?;

    Ok(["reset".to_string(), service.to_string(), bundle_id])
}

/// `tccutil` exits 64 with this when the bundle id is not registered with TCC.
/// It means the command had the authority to look and found nothing, which is
/// a fine outcome for a reset: there was no grant to clear.
const NO_SUCH_BUNDLE: &str = "No such bundle identifier";

struct TccutilRun {
    succeeded: bool,
    message: String,
}

/// Run one reset off the main thread.
///
/// No sudo: `tccutil` resets grants for the running user, which is exactly the
/// scope wanted here.
async fn run_reset(args: [String; 3]) -> Result<TccutilRun, String> {
    tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new(tools::TCCUTIL_PATH)
            .args(args)
            .output()
            .map_err(|e| format!("Could not run tccutil: {}", e))?;

        if output.status.success() {
            return Ok(TccutilRun {
                succeeded: true,
                message: "macOS cleared the grant for this bundle id.".to_string(),
            });
        }

        // tccutil's own output is read, never reported onward: it is mapped to
        // one of our own sentences so nothing outside the permission state can
        // leak into the UI or the log. Observed on macOS 15: this message
        // arrives on stderr, and stdout is checked too so a later version
        // moving it does not turn a known case into an unknown one.
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        if said.contains(NO_SUCH_BUNDLE) {
            return Ok(TccutilRun {
                succeeded: true,
                message: "macOS has no record under this bundle id, so there was nothing to clear."
                    .to_string(),
            });
        }

        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        Ok(TccutilRun {
            succeeded: false,
            message: format!("tccutil refused the reset and exited with code {}.", code),
        })
    })
    .await
    .map_err(|e| format!("The reset task did not finish: {}", e))?
}

#[cfg(target_os = "macos")]
fn tccutil_present() -> bool {
    std::path::Path::new(tools::TCCUTIL_PATH).exists()
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Whether the reset controls are offered, and why not when they are not.
///
/// The gate is the app's existing debug mode, asked through the same command
/// the frontend asks: `get_debug_mode`. There is deliberately no second
/// definition of "is this a debug build" in this module.
async fn reset_gate(state: State<'_, AppState>) -> (bool, Option<String>) {
    let debug_mode = match crate::commands::core::get_debug_mode(state).await {
        Ok(value) => value,
        Err(e) => {
            warn!("Could not read debug mode, so reset stays off: {}", e);
            return (
                false,
                Some("Debug mode could not be read, so reset is off.".to_string()),
            );
        }
    };

    if !debug_mode {
        return (
            false,
            Some(
                "Resetting a grant is a debug-mode tool. Turn on debug mode to use it.".to_string(),
            ),
        );
    }

    #[cfg(target_os = "macos")]
    {
        if !tccutil_present() {
            return (
                false,
                Some(
                    "This machine has no tccutil, so a grant cannot be reset from here."
                        .to_string(),
                ),
            );
        }
        (true, None)
    }

    #[cfg(not(target_os = "macos"))]
    {
        (
            false,
            Some("TCC is a macOS system, so there is nothing to reset here.".to_string()),
        )
    }
}

/// Report what macOS answers about this process right now, per permission.
///
/// Live every time: no cache read, and no prompting call, so looking at this
/// panel can never raise a system dialog or return yesterday's answer.
#[tauri::command]
pub async fn get_permission_diagnostics(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PermissionDiagnosticsReport, String> {
    let (reset_available, reset_unavailable_reason) = reset_gate(state).await;

    // Read from the running app. The demo build ships under a different
    // identifier, and a hardcoded one would describe an app that is not this
    // one.
    let bundle_id = app.config().identifier.clone();
    let package = app.package_info();

    let permissions = REPORTED
        .iter()
        .copied()
        .map(|permission_type| {
            let read = read_live(permission_type);
            PermissionDiagnostic {
                permission_type: permission_type.to_string(),
                label: label_for(permission_type).to_string(),
                required: is_required(permission_type),
                answer: read.answer,
                answer_detail: read.detail.to_string(),
                api: read.api.to_string(),
                tcc_service: tcc_service_for(permission_type)
                    .unwrap_or("none")
                    .to_string(),
                settings_url: settings_url_for(permission_type).to_string(),
                settings_label: settings_label_for(permission_type),
                gates: gates_for(permission_type).to_string(),
                reset_this_launch: was_reset_this_launch(permission_type),
            }
        })
        .collect();

    Ok(PermissionDiagnosticsReport {
        bundle_id,
        app_name: package.name.clone(),
        app_version: package.version.to_string(),
        debug_build: cfg!(debug_assertions),
        platform_supported: cfg!(target_os = "macos"),
        reset_available,
        reset_unavailable_reason,
        // `AppHandle::restart` exists in this Tauri version and is already the
        // mechanism behind `restart_app_after_permissions`.
        relaunch_available: true,
        permissions,
    })
}

/// Revoke one permission's grant for this app, so macOS asks again.
///
/// Destructive on purpose and narrow on purpose: one permission, one bundle
/// id, the running app's own. The caller confirms first; this refuses to run
/// at all unless debug mode is on.
#[tauri::command]
pub async fn reset_permission_grant(
    app: AppHandle,
    state: State<'_, AppState>,
    permission_type: String,
) -> Result<PermissionResetOutcome, String> {
    let (reset_available, reason) = reset_gate(state).await;
    if !reset_available {
        return Err(
            reason.unwrap_or_else(|| "Resetting a grant is not available here.".to_string())
        );
    }

    let bundle_id = app.config().identifier.clone();
    let args = reset_command_args(&permission_type, &bundle_id)?;
    let service = args[1].clone();
    let target = args[2].clone();

    info!("Resetting the {} grant for {}", service, target);

    let run = run_reset(args).await?;

    if run.succeeded {
        note_reset(&permission_type);
    }

    Ok(PermissionResetOutcome {
        permission_type,
        tcc_service: service,
        bundle_id: target,
        succeeded: run.succeeded,
        message: run.message,
        relaunch_required: run.succeeded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_BUNDLE: &str = "com.juno.example";

    #[test]
    fn maps_every_permission_juno_uses_to_its_tcc_service() {
        assert_eq!(tcc_service_for("accessibility"), Some("Accessibility"));
        // Input Monitoring is filed under ListenEvent, which is the single
        // most surprising name in the set.
        assert_eq!(tcc_service_for("input_monitoring"), Some("ListenEvent"));
        assert_eq!(tcc_service_for("microphone"), Some("Microphone"));
        assert_eq!(tcc_service_for("screen_recording"), Some("ScreenCapture"));
    }

    #[test]
    fn refuses_to_map_a_permission_juno_does_not_use() {
        assert_eq!(tcc_service_for(""), None);
        assert_eq!(tcc_service_for("Accessibility"), None);
        assert_eq!(tcc_service_for("SystemPolicyAllFiles"), None);
        assert_eq!(tcc_service_for("everything"), None);
    }

    #[test]
    fn every_reported_permission_has_a_service_and_a_pane() {
        for permission_type in REPORTED {
            assert!(
                tcc_service_for(permission_type).is_some(),
                "{} has no service",
                permission_type
            );
            assert!(
                settings_url_for(permission_type).starts_with("x-apple.systempreferences:"),
                "{} has no pane",
                permission_type
            );
            assert!(!gates_for(permission_type).is_empty());
        }
    }

    #[test]
    fn refuses_an_empty_or_missing_bundle_id() {
        assert!(validate_bundle_id("").is_err());
        assert!(validate_bundle_id("   ").is_err());
        assert!(validate_bundle_id("\t\n").is_err());
        assert!(reset_command_args("accessibility", "").is_err());
        assert!(reset_command_args("accessibility", "   ").is_err());
    }

    #[test]
    fn refuses_a_bundle_id_that_could_read_as_a_flag_or_carry_extras() {
        assert!(validate_bundle_id("-all").is_err());
        assert!(validate_bundle_id("--force").is_err());
        assert!(validate_bundle_id("com.juno.example extra").is_err());
        assert!(validate_bundle_id("com.juno.example;whoami").is_err());
        assert!(validate_bundle_id("com.juno.example\nAccessibility").is_err());
        assert!(validate_bundle_id("notabundleid").is_err());
        assert!(validate_bundle_id(".com.juno").is_err());
        assert!(validate_bundle_id("com.juno.").is_err());
    }

    #[test]
    fn accepts_a_real_bundle_id_and_trims_it() {
        assert_eq!(validate_bundle_id(REAL_BUNDLE), Ok(REAL_BUNDLE.to_string()));
        assert_eq!(
            validate_bundle_id("  com.juno.example.demo  "),
            Ok("com.juno.example.demo".to_string())
        );
    }

    #[test]
    fn the_argv_always_carries_a_bundle_id() {
        // The dangerous form is `tccutil reset SERVICE` with two arguments.
        // Anything this builds has three, and the third is the bundle id.
        let args = reset_command_args("microphone", REAL_BUNDLE)
            .unwrap_or_else(|_| panic!("a real bundle id should be accepted"));
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "reset");
        assert_eq!(args[1], "Microphone");
        assert_eq!(args[2], REAL_BUNDLE);
    }

    #[test]
    fn refuses_to_build_an_argv_for_an_unknown_permission() {
        assert!(reset_command_args("all", REAL_BUNDLE).is_err());
        assert!(reset_command_args("", REAL_BUNDLE).is_err());
    }

    #[test]
    fn the_reset_ledger_only_marks_what_was_reset() {
        assert!(!was_reset_this_launch("a_permission_never_reset_in_tests"));
    }
}
