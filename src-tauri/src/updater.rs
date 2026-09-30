//! Keeping Juno current, without anyone having to think about it.
//!
//! The whole policy lives here: which feed to read, when to look, what to do
//! with what it finds, and what the person is told while it happens. The UI
//! renders [`UpdateStatus`] and calls three commands. It decides nothing.
//!
//! # Why there are two feeds
//!
//! `release-every-merge.yml` cuts a version on every merge and publishes it as
//! a GitHub *prerelease*. GitHub's `/releases/latest` excludes prereleases, so
//! the stable feed only moves when someone runs `juno-build promote`, which
//! asks "ship to every user?" first. That gate is right for users and wrong
//! for us: dogfooding needs the build from ten minutes ago.
//!
//! So there are two, and they are the same mechanism pointed at different
//! files:
//!
//! - **Stable** reads `releases/latest/download/latest.json`, which GitHub
//!   resolves to the newest promoted release.
//! - **Prerelease** reads a fixed `canary` tag whose `latest.json` every
//!   release build overwrites, so it always names the newest build of all.
//!
//! The endpoint is chosen at runtime through
//! [`UpdaterExt::updater_builder`], not from `tauri.conf.json`. The `pubkey`
//! there still applies to both: the channel decides which manifest is read,
//! never whether the signature is checked.
//!
//! # Why it never interrupts
//!
//! An update is downloaded and installed in the background, which on macOS
//! swaps the bundle on disk while the running process carries on with the old
//! code. Nothing restarts on its own. Quitting normally and reopening is
//! enough; the Restart button is a shortcut, not a requirement. Juno is a
//! dictation tool, and a relaunch it chose itself would land mid-sentence.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;
use tracing::{debug, info, warn};

use crate::constants::events;

/// How long after launch the first check waits.
///
/// Startup is already contended: window creation, the model load, the tray,
/// the provider probe. A network round trip that nobody is waiting for goes
/// last.
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(20);

/// How often to look after that.
///
/// Six hours is a compromise between a laptop that is closed most of the day
/// and a release cadence of several per day. A check costs one small GET.
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Which builds this install accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    /// Only releases a human has promoted. What a user should be on.
    Stable,
    /// Every build, including the one from the merge ten minutes ago.
    Prerelease,
}

impl UpdateChannel {
    /// Parse a stored value. Anything unrecognised reads as the default rather
    /// than failing: a typo in the store must not cost someone their updates.
    pub fn from_stored(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "stable" => Self::Stable,
            "prerelease" | "canary" => Self::Prerelease,
            other => {
                warn!("[Updater] unknown channel {other:?}; using the default");
                Self::default()
            }
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Prerelease => "prerelease",
        }
    }

    /// The manifest this channel reads.
    pub fn endpoint(self) -> &'static str {
        match self {
            Self::Stable => {
                "https://github.com/lacymorrow/juno/releases/latest/download/latest.json"
            }
            Self::Prerelease => {
                "https://github.com/lacymorrow/juno/releases/download/canary/latest.json"
            }
        }
    }
}

impl Default for UpdateChannel {
    fn default() -> Self {
        Self::Prerelease
    }
}

/// Where the update flow currently is. One value, so the UI never has to
/// combine flags to work out what to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateStage {
    /// Nothing has happened yet this run.
    Idle,
    /// A check is in flight.
    Checking,
    /// Checked, and this is the newest build on the channel.
    UpToDate,
    /// A newer build exists and is coming down.
    Downloading,
    /// Installed on disk. The running process is still the old one.
    ReadyToRestart,
    /// The last attempt failed. `error` says how.
    Failed,
}

/// Everything the UI shows, in one payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub stage: UpdateStage,
    /// The version running right now.
    pub current_version: String,
    /// The version found, once one has been.
    pub available_version: Option<String>,
    /// Release notes for `available_version`.
    pub notes: Option<String>,
    /// Why the last attempt failed, when `stage` is `Failed`.
    pub error: Option<String>,
    /// Which feed the last check read.
    pub channel: UpdateChannel,
    /// Downloaded bytes, while `stage` is `Downloading`.
    pub downloaded_bytes: u64,
    /// Total bytes, when the server said.
    pub total_bytes: Option<u64>,
}

impl UpdateStatus {
    fn new(channel: UpdateChannel) -> Self {
        Self {
            stage: UpdateStage::Idle,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            available_version: None,
            notes: None,
            error: None,
            channel,
            downloaded_bytes: 0,
            total_bytes: None,
        }
    }
}

fn status_cell() -> &'static Mutex<UpdateStatus> {
    static STATUS: OnceLock<Mutex<UpdateStatus>> = OnceLock::new();
    STATUS.get_or_init(|| Mutex::new(UpdateStatus::new(UpdateChannel::default())))
}

/// One check at a time. A manual press during the scheduled check must not
/// start a second download of the same bytes.
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Read the current status. Never blocks on the network.
pub fn current_status() -> UpdateStatus {
    match status_cell().lock() {
        Ok(status) => status.clone(),
        // A poisoned lock means a previous holder panicked mid-update. The
        // status is display state, so recovering the value is strictly better
        // than propagating the panic into the UI.
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

/// Apply `mutate` to the status and tell the UI, in that order.
fn update_status<F: FnOnce(&mut UpdateStatus)>(app: &AppHandle, mutate: F) {
    let snapshot = {
        let mut guard = match status_cell().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        mutate(&mut guard);
        guard.clone()
    };
    if let Err(e) = app.emit(events::updates::STATUS, &snapshot) {
        debug!("[Updater] no listener for the status event: {e}");
    }
}

/// Look for a newer build, and take it if there is one.
///
/// Safe to call from anywhere: a second call while one is running returns
/// immediately rather than queueing. Errors land in the status rather than
/// propagating, because most callers are timers with nobody to tell.
pub async fn check_and_install(app: &AppHandle, channel: UpdateChannel) {
    if IN_FLIGHT.swap(true, Ordering::SeqCst) {
        debug!("[Updater] a check is already running; skipping this one");
        return;
    }

    let outcome = run_check(app, channel).await;

    if let Err(message) = outcome {
        warn!("[Updater] {message}");
        update_status(app, |status| {
            status.stage = UpdateStage::Failed;
            status.error = Some(message);
        });
    }

    IN_FLIGHT.store(false, Ordering::SeqCst);
}

/// The body of a check. Split out so [`check_and_install`] owns the in-flight
/// flag on every exit path, including the error ones.
async fn run_check(app: &AppHandle, channel: UpdateChannel) -> Result<(), String> {
    update_status(app, |status| {
        status.stage = UpdateStage::Checking;
        status.error = None;
        status.channel = channel;
        status.downloaded_bytes = 0;
        status.total_bytes = None;
    });

    let endpoint = channel
        .endpoint()
        .parse()
        .map_err(|e| format!("the {} feed URL is not a URL: {e}", channel.as_str()))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| {
            format!(
                "could not point the updater at the {} feed: {e}",
                channel.as_str()
            )
        })?
        .build()
        .map_err(|e| format!("could not build the updater: {e}"))?;

    let found = updater
        .check()
        .await
        .map_err(|e| format!("could not read the {} feed: {e}", channel.as_str()))?;

    let Some(update) = found else {
        info!(
            "[Updater] {} is the newest build on the {} channel",
            env!("CARGO_PKG_VERSION"),
            channel.as_str()
        );
        update_status(app, |status| {
            status.stage = UpdateStage::UpToDate;
            status.available_version = None;
            status.notes = None;
        });
        return Ok(());
    };

    let version = update.version.clone();
    let notes = update.body.clone();
    info!("[Updater] {version} is available; downloading");
    update_status(app, |status| {
        status.stage = UpdateStage::Downloading;
        status.available_version = Some(version.clone());
        status.notes = notes;
    });

    // `on_chunk` is called per chunk with the chunk's length, so the running
    // total is ours to keep. The closure is synchronous and must stay cheap:
    // it runs between reads of the socket.
    let progress_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                update_status(&progress_app, |status| {
                    status.downloaded_bytes += chunk as u64;
                    status.total_bytes = total;
                });
            },
            || debug!("[Updater] download finished; installing"),
        )
        .await
        .map_err(|e| format!("could not install {version}: {e}"))?;

    info!("[Updater] {version} is installed; it runs on the next launch");
    update_status(app, |status| {
        status.stage = UpdateStage::ReadyToRestart;
    });
    Ok(())
}

/// Start the background schedule: one check shortly after launch, then every
/// [`CHECK_INTERVAL`].
///
/// Reads the channel and the enabled flag from the store on every tick rather
/// than capturing them, so changing either in Settings takes effect at the
/// next check instead of the next launch.
pub fn spawn_schedule(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            match settings_for(&app).await {
                Ok((true, channel)) => check_and_install(&app, channel).await,
                Ok((false, _)) => {
                    debug!("[Updater] automatic checks are off; skipping this tick")
                }
                Err(e) => warn!("[Updater] could not read the update settings: {e}"),
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// `(automatic checks on, channel)` from the settings store.
pub async fn settings_for(app: &AppHandle) -> Result<(bool, UpdateChannel), String> {
    let manager = crate::settings::manager::SettingsManager::new(app.clone())?;
    let settings = manager.get_update_settings().await?;
    Ok((
        settings.auto_check_enabled,
        UpdateChannel::from_stored(&settings.channel),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_channel_is_prerelease_because_we_are_the_testers() {
        assert_eq!(UpdateChannel::default(), UpdateChannel::Prerelease);
    }

    #[test]
    fn a_stored_channel_round_trips() {
        for channel in [UpdateChannel::Stable, UpdateChannel::Prerelease] {
            assert_eq!(UpdateChannel::from_stored(channel.as_str()), channel);
        }
    }

    #[test]
    fn a_stored_channel_is_read_loosely() {
        assert_eq!(
            UpdateChannel::from_stored("  Prerelease "),
            UpdateChannel::Prerelease
        );
        assert_eq!(UpdateChannel::from_stored("STABLE"), UpdateChannel::Stable);
        // The tag is called `canary`; accept the name someone would type.
        assert_eq!(
            UpdateChannel::from_stored("canary"),
            UpdateChannel::Prerelease
        );
    }

    #[test]
    fn an_unreadable_channel_falls_back_rather_than_failing() {
        assert_eq!(
            UpdateChannel::from_stored("beta-2"),
            UpdateChannel::default()
        );
        assert_eq!(UpdateChannel::from_stored(""), UpdateChannel::default());
    }

    /// The two channels must not read the same file, or picking one would be
    /// decoration. Both must be HTTPS: `validate_endpoints` rejects plain HTTP
    /// unless the app opts into insecure transport, and a rejected endpoint
    /// would surface as a failed check rather than a build error.
    #[test]
    fn the_channels_read_different_https_feeds() {
        let stable = UpdateChannel::Stable.endpoint();
        let prerelease = UpdateChannel::Prerelease.endpoint();
        assert_ne!(stable, prerelease);
        for endpoint in [stable, prerelease] {
            assert!(endpoint.starts_with("https://"), "{endpoint}");
            assert!(endpoint.ends_with("latest.json"), "{endpoint}");
            assert!(url::Url::parse(endpoint).is_ok(), "{endpoint}");
        }
    }

    /// The prerelease feed has to be a fixed URL. If it ever resolved through
    /// `/releases/latest` it would inherit exactly the prerelease blindness
    /// this channel exists to route around.
    #[test]
    fn the_prerelease_feed_does_not_go_through_releases_latest() {
        let endpoint = UpdateChannel::Prerelease.endpoint();
        assert!(
            endpoint.contains("/releases/download/canary/"),
            "{endpoint}"
        );
        assert!(!endpoint.contains("/releases/latest/"), "{endpoint}");
    }

    #[test]
    fn the_first_check_waits_for_startup_to_finish() {
        assert!(FIRST_CHECK_DELAY >= Duration::from_secs(10));
        assert!(CHECK_INTERVAL > FIRST_CHECK_DELAY);
    }
}
