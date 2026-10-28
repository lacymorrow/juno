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
//! # Why it never interrupts, and never swaps the bundle under a live process
//!
//! During the session an update is only *downloaded*; the verified bytes wait
//! in process state ([`PENDING`]). They are installed when Juno quits
//! ([`install_pending`], called from the exit path in `lib.rs`) or when the
//! person presses "Restart to update". Installing mid-session replaced
//! `/Applications/Juno.app` on disk under the running process, after which
//! macOS TCC judged the process against a bundle it no longer matched and
//! every permission (Music automation, screen capture, microphone) failed
//! until relaunch. Nothing restarts on its own: Juno is a dictation tool, and
//! a relaunch it chose itself would land mid-sentence.
//!
//! A build that is not a published release (a CI branch build installed with
//! `juno-build <branch>`) is never replaced by the schedule at all; see
//! [`is_branch_build`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};
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

/// The longest the feed read may take. A connection that hangs (a network that
/// dropped mid-request, a Mac waking onto a dead socket) never errors on its
/// own, and the check would sit in `Checking` for good.
const FEED_TIMEOUT: Duration = Duration::from_secs(60);

/// The longest a whole check, download included, may take before it is given
/// up as failed. Generous because the download is the bulk of it.
const CHECK_DEADLINE: Duration = Duration::from_secs(30 * 60);

/// The UI hears about download progress at most this often, in bytes. Chunks
/// are small; one event per chunk is thousands of events for one download.
const PROGRESS_STEP: u64 = 512 * 1024;

/// Which builds this install accepts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    /// Only releases a human has promoted. What a user should be on.
    Stable,
    /// Every build, including the one from the merge ten minutes ago.
    ///
    /// The default, because the people running Juno today are the people
    /// writing it, and a dogfood build that arrives a week late is not
    /// dogfooding. Move `#[default]` to `Stable` before there are users who
    /// did not sign up to find the bugs.
    #[default]
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
            Self::Stable => crate::constants::api::endpoints::UPDATE_FEED_STABLE,
            Self::Prerelease => crate::constants::api::endpoints::UPDATE_FEED_PRERELEASE,
        }
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
    /// Downloaded and verified, waiting in memory. It installs when Juno
    /// quits; the running bundle on disk is untouched until then.
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
    /// Why the last attempt failed, when `stage` is `Failed`: one plain line
    /// a person may read (see [`failure_line`]). The detail is in the log.
    pub error: Option<String>,
    /// One plain sentence for the UI to show for the current stage, when the
    /// backend has something specific to say (the UI renders it verbatim).
    pub detail: Option<String>,
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
            detail: None,
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

/// Holds the one-check-at-a-time flag and gives it back when dropped, on every
/// path out including a panic. Clearing it by hand at the end of the check
/// left it set for good if the check ever unwound, and every later check,
/// scheduled or pressed, then returned "already running" without doing
/// anything.
struct InFlightClaim<'a>(&'a AtomicBool);

impl<'a> InFlightClaim<'a> {
    fn take(flag: &'a AtomicBool) -> Option<Self> {
        if flag.swap(true, Ordering::SeqCst) {
            None
        } else {
            Some(Self(flag))
        }
    }
}

impl Drop for InFlightClaim<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// A downloaded, signature-verified update waiting for Juno to quit.
struct PendingInstall {
    update: Update,
    bytes: Vec<u8>,
}

static PENDING: Mutex<Option<PendingInstall>> = Mutex::new(None);

fn pending_slot() -> std::sync::MutexGuard<'static, Option<PendingInstall>> {
    match PENDING.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// The compile-time build kind: `"branch"` for a CI branch build
/// (`build-branch.yml` sets `JUNO_BUILD_KIND=branch`), unset for releases.
const BUILD_KIND: Option<&str> = option_env!("JUNO_BUILD_KIND");

/// Whether `kind` names a branch build. Pure, so it can be tested.
pub fn is_branch_kind(kind: Option<&str>) -> bool {
    matches!(kind.map(str::trim), Some(k) if k.eq_ignore_ascii_case("branch"))
}

/// True for a build that is not a published release. The schedule must not
/// replace it: a tester's branch build would be overwritten within seconds.
pub fn is_branch_build() -> bool {
    is_branch_kind(BUILD_KIND)
}

/// What Settings says once an update is downloaded and waiting.
pub fn ready_line(version: Option<&str>) -> String {
    match version.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => format!("Juno {v} is downloaded. It installs when Juno quits."),
        None => "An update is downloaded. It installs when Juno quits.".to_string(),
    }
}

/// Install the waiting update, if any. Called when Juno quits, and before a
/// relaunch. Synchronous: the exit path cannot await. Takes the update out of
/// the slot first, so it can never install twice. Returns whether it installed.
pub fn install_pending() -> bool {
    let Some(PendingInstall { update, bytes }) = pending_slot().take() else {
        return false;
    };
    let version = update.version.clone();
    match update.install(bytes) {
        Ok(()) => {
            info!("[Updater] {version} installed on quit; it runs on the next launch");
            true
        }
        Err(e) => {
            warn!("[Updater] could not install {version} on quit: {e}");
            false
        }
    }
}

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

/// Apply `mutate` to the status without telling anyone. Returns the result.
fn mutate_status<F: FnOnce(&mut UpdateStatus)>(mutate: F) -> UpdateStatus {
    let mut guard = match status_cell().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    mutate(&mut guard);
    guard.clone()
}

fn announce_status(app: &AppHandle, snapshot: &UpdateStatus) {
    if let Err(e) = app.emit(events::updates::STATUS, snapshot) {
        debug!("[Updater] no listener for the status event: {e}");
    }
}

/// Apply `mutate` to the status and tell the UI, in that order.
fn update_status<F: FnOnce(&mut UpdateStatus)>(app: &AppHandle, mutate: F) {
    let snapshot = mutate_status(mutate);
    announce_status(app, &snapshot);
}

/// Whether enough of the download has arrived since the UI last heard to be
/// worth another event. The last bytes always are.
fn progress_worth_announcing(seen: u64, announced: u64, total: Option<u64>) -> bool {
    seen.saturating_sub(announced) >= PROGRESS_STEP || total.is_some_and(|total| seen >= total)
}

/// Look for a newer build, and download it if there is one. It installs when
/// Juno quits ([`install_pending`]).
///
/// Safe to call from anywhere: a second call while one is running returns
/// immediately rather than queueing. Errors land in the status rather than
/// propagating, because most callers are timers with nobody to tell.
pub async fn check_and_install(app: &AppHandle, channel: UpdateChannel) {
    let Some(_claim) = InFlightClaim::take(&IN_FLIGHT) else {
        debug!("[Updater] a check is already running; skipping this one");
        return;
    };

    // A check that never answers must not hold the slot: it ends as a failure
    // and the next one, scheduled or pressed, gets to run.
    let outcome = match tokio::time::timeout(CHECK_DEADLINE, run_check(app, channel)).await {
        Ok(outcome) => outcome,
        Err(_) => Err(format!(
            "the {} check did not finish in {} minutes",
            channel.as_str(),
            CHECK_DEADLINE.as_secs() / 60
        )),
    };

    if let Err(message) = outcome {
        warn!("[Updater] {message}");
        update_status(app, |status| {
            status.stage = UpdateStage::Failed;
            status.error = Some(failure_line(&message).to_string());
        });
    }
}

/// What Settings shows after a check that did not finish. The raw reason
/// names a feed URL and an error chain, which tells a person nothing they can
/// act on; it goes to the log. A lost connection is worth saying plainly,
/// because it explains itself and fixes itself.
pub fn failure_line(message: &str) -> &'static str {
    if crate::utils::network::is_network_error(message) {
        "No internet connection. Juno will check again later."
    } else {
        "The last check didn't finish. Juno will check again later."
    }
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

    let found = tokio::time::timeout(FEED_TIMEOUT, updater.check())
        .await
        .map_err(|_| {
            format!(
                "the {} feed did not answer in {}s",
                channel.as_str(),
                FEED_TIMEOUT.as_secs()
            )
        })?
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
            status.detail = None;
        });
        return Ok(());
    };

    let version = update.version.clone();
    let notes = update.body.clone();

    let already_waiting = pending_slot()
        .as_ref()
        .is_some_and(|p| p.update.version == version);
    if already_waiting {
        debug!("[Updater] {version} is already downloaded and waiting for quit");
        update_status(app, |status| {
            status.stage = UpdateStage::ReadyToRestart;
            status.available_version = Some(version.clone());
            status.notes = notes;
            status.detail = Some(ready_line(Some(&version)));
        });
        return Ok(());
    }

    info!("[Updater] {version} is available; downloading");
    update_status(app, |status| {
        status.stage = UpdateStage::Downloading;
        status.available_version = Some(version.clone());
        status.notes = notes;
        status.detail = None;
    });

    // `on_chunk` is called per chunk with the chunk's length, so the running
    // total is ours to keep. The closure is synchronous and must stay cheap:
    // it runs between reads of the socket. The status always holds the true
    // count; the UI is told every `PROGRESS_STEP` bytes.
    let progress_app = app.clone();
    let mut seen: u64 = 0;
    let mut announced: u64 = 0;
    let bytes = update
        .download(
            move |chunk, total| {
                seen += chunk as u64;
                let snapshot = mutate_status(|status| {
                    status.downloaded_bytes = seen;
                    status.total_bytes = total;
                });
                if progress_worth_announcing(seen, announced, total) {
                    announced = seen;
                    announce_status(&progress_app, &snapshot);
                }
            },
            || debug!("[Updater] download finished and verified"),
        )
        .await
        .map_err(|e| format!("could not download {version}: {e}"))?;

    *pending_slot() = Some(PendingInstall { update, bytes });

    info!("[Updater] {version} is downloaded; it installs when Juno quits");
    update_status(app, |status| {
        status.stage = UpdateStage::ReadyToRestart;
        status.detail = Some(ready_line(Some(&version)));
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
                Ok((true, _)) if is_branch_build() => info!(
                    "[Updater] this is a branch build, not a release; \
                     scheduled updates would overwrite it, so none will run"
                ),
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
    #[test]
    fn only_the_branch_kind_marks_a_branch_build() {
        assert!(is_branch_kind(Some("branch")));
        assert!(is_branch_kind(Some(" Branch ")));
        assert!(!is_branch_kind(None));
        assert!(!is_branch_kind(Some("")));
        assert!(!is_branch_kind(Some("release")));
    }

    #[test]
    fn the_ready_line_says_it_installs_on_quit() {
        let line = ready_line(Some("0.8.216"));
        assert!(line.contains("0.8.216") && line.contains("installs when Juno quits"));
        assert_eq!(
            ready_line(None),
            "An update is downloaded. It installs when Juno quits."
        );
        assert_eq!(ready_line(Some("  ")), ready_line(None));
        assert!(!line.contains('\u{2014}'));
    }

    #[test]
    fn a_failed_check_never_shows_a_url_or_an_error_chain() {
        let offline = "could not read the stable feed: error sending request for url \
                       (https://github.com/x/latest.json): dns error";
        assert_eq!(
            failure_line(offline),
            "No internet connection. Juno will check again later."
        );
        let other = "could not read the stable feed: the signature did not verify";
        let line = failure_line(other);
        assert!(
            !line.contains("signature") && !line.contains("http"),
            "{line}"
        );
    }

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

    /// tauri.conf.json names the stable feed too (the updater plugin needs an
    /// endpoint at build time). It must be the same URL the stable channel
    /// reads, or a build that ignores the runtime choice looks elsewhere.
    #[test]
    fn the_stable_feed_matches_tauri_conf() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap_or_default();
        let endpoints = &conf["plugins"]["updater"]["endpoints"];
        assert_eq!(
            endpoints[0].as_str(),
            Some(UpdateChannel::Stable.endpoint()),
            "{endpoints}"
        );
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
    fn one_check_at_a_time_and_the_slot_comes_back_when_it_ends() {
        let flag = AtomicBool::new(false);
        let first = InFlightClaim::take(&flag);
        assert!(first.is_some());
        assert!(InFlightClaim::take(&flag).is_none(), "a second check waits");
        drop(first);
        assert!(
            InFlightClaim::take(&flag).is_some(),
            "the slot is free again"
        );
    }

    #[test]
    fn the_slot_comes_back_when_a_check_panics() {
        let flag = AtomicBool::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _claim = InFlightClaim::take(&flag);
            panic!("the check blew up");
        }));
        assert!(result.is_err());
        assert!(
            InFlightClaim::take(&flag).is_some(),
            "a panic must not strand every later check as already running"
        );
    }

    #[test]
    fn download_progress_is_announced_in_steps_and_at_the_end() {
        assert!(!progress_worth_announcing(16 * 1024, 0, Some(50_000_000)));
        assert!(progress_worth_announcing(
            PROGRESS_STEP,
            0,
            Some(50_000_000)
        ));
        assert!(!progress_worth_announcing(
            PROGRESS_STEP + 10,
            PROGRESS_STEP,
            None
        ));
        // The last chunk is announced however small.
        assert!(progress_worth_announcing(1_000, 900, Some(1_000)));
    }

    #[test]
    fn a_hung_check_is_bounded() {
        assert!(FEED_TIMEOUT < CHECK_DEADLINE);
        assert!(CHECK_DEADLINE < CHECK_INTERVAL);
    }

    #[test]
    fn the_first_check_waits_for_startup_to_finish() {
        assert!(FIRST_CHECK_DELAY >= Duration::from_secs(10));
        assert!(CHECK_INTERVAL > FIRST_CHECK_DELAY);
    }
}
