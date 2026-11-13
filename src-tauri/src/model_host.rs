//! # Can Juno fetch a model from Hugging Face right now?
//!
//! One answer, owned here, for every model download Juno makes: Whisper and
//! Parakeet weights (`commands/stt_models.rs`) and Kokoro's voice
//! (`tts/kokoro.rs`, which fetches through the `any-tts` crate). Before this
//! each of them asked the network for itself, found out the hard way, and
//! asked again on the next utterance.
//!
//! ## Two questions, not one
//!
//! "The download failed" is not a diagnosis. A laptop with its Wi-Fi off and
//! a work laptop whose proxy blocks Hugging Face fail identically at the call
//! site, and telling the first person that Hugging Face is blocked sends them
//! looking for a firewall rule that does not exist. So the probe asks twice:
//!
//! - **Hugging Face**, over the real download path (a one-byte `Range`
//!   request for a real model file, which follows the same redirect to the
//!   same CDN a download does). An API endpoint would not prove that.
//! - **The internet**, only when the first failed, through an endpoint that
//!   answers `204` with no body. That status is the instrument: a captive
//!   portal or an interception proxy has to redirect or serve a page, and
//!   neither can imitate an empty 204. Apple's own reachability URL is HTML,
//!   which is exactly what a block page looks like, so it cannot be used.
//!
//! Hugging Face answered: [`HostStatus::Reachable`]. It did not, but the
//! internet did: [`HostStatus::Blocked`]. Neither answered, or the machine
//! has no route at all: [`HostStatus::Offline`].
//!
//! ## Nothing is on a timer
//!
//! The status is probed when something actually wants a model and nothing is
//! known yet, and re-probed on exactly two events: the machine's route
//! changing (`connectivity.rs`, which runs an `NWPathMonitor`), and the
//! person pressing Retry. There is deliberately no periodic re-probe. A
//! retry loop on a network that blocks Hugging Face is the bug this module
//! exists to remove, and a timer is that bug with a longer period.
//!
//! The answer is not written to disk either. It describes the network the Mac
//! is on, not the Mac, and carrying yesterday's office into today's kitchen
//! would leave someone pressing Retry to undo it.
//!
//! ## Unknown allows
//!
//! [`HostStatus::Unknown`] means nobody has asked yet, and it permits a fetch.
//! Refusing on it would make a machine that has never probed unable to
//! download anything, which is worse than one wasted attempt. Once a probe or
//! a download has found the host unavailable, automatic fetching stops until
//! one of the two events above.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

use crate::commands::model_download::looks_like_a_web_page;

/// The line a person reads when Hugging Face cannot be reached on this
/// network. It names the network, because that is the thing that is
/// different; naming Hugging Face would be true and useless.
pub const NOT_ON_THIS_NETWORK: &str = "Not available on this network.";

/// The real download path for the smallest model in the catalog. A one-byte
/// `Range` request for it goes through the same redirect to the same CDN a
/// real download does, so a network that allows `huggingface.co` but blocks
/// the file host is still caught.
const HOST_PROBE_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin";

/// Answers `204` with an empty body. The standard connectivity probe, and the
/// only shape a captive portal cannot imitate.
const INTERNET_PROBE_URL: &str = "https://www.gstatic.com/generate_204";

/// A probe is a yes-or-no question. If it cannot be answered in this long the
/// answer is no: nothing waits on it, and a slow proxy is a blocked one as
/// far as an 82MB download is concerned.
const PROBE_TIMEOUT: Duration = Duration::from_secs(6);
const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_secs(4);

/// Whether Juno can fetch model weights, and if not, whose fault it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostStatus {
    /// Nobody has asked yet.
    #[default]
    Unknown,
    /// Hugging Face answered.
    Reachable,
    /// The machine has no usable internet. Not Hugging Face's fault, and not
    /// described as though it were.
    Offline,
    /// The internet works; Hugging Face does not reach this machine.
    Blocked,
}

impl HostStatus {
    /// Whether an automatic request to Hugging Face may be made right now.
    ///
    /// Only a settled "no" stops one. See the module note on `Unknown`.
    pub fn may_fetch(self) -> bool {
        matches!(self, Self::Unknown | Self::Reachable)
    }

    /// The one line a person reads about this status, or `None` when there is
    /// nothing worth saying.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::Blocked => Some(NOT_ON_THIS_NETWORK),
            Self::Offline => Some("No internet connection. Downloaded models still work."),
            Self::Unknown | Self::Reachable => None,
        }
    }
}

/// The two probes, turned into the one answer.
///
/// Order matters. No route at all is offline before anything else is asked.
pub fn classify(route_up: bool, host_ok: bool, internet_ok: bool) -> HostStatus {
    if !route_up {
        return HostStatus::Offline;
    }
    if host_ok {
        return HostStatus::Reachable;
    }
    if internet_ok {
        HostStatus::Blocked
    } else {
        HostStatus::Offline
    }
}

/// Whether a reply to the Hugging Face probe came from Hugging Face, or from
/// something standing in front of it.
///
/// The content type is the discriminator, not the status: a proxy block page
/// is usually a perfectly ordinary `200`. A `5xx` counts as reached, because
/// Hugging Face being unwell is not the network blocking anything, and the
/// download's own retry policy is the right handler for that. The probe URL
/// is a public file, so a `401`, `403`, `407` or `451` on it is never Hugging
/// Face enforcing its own access rules.
pub fn host_answered(status: u16, content_type: Option<&str>) -> bool {
    if looks_like_a_web_page(content_type) {
        return false;
    }
    !matches!(status, 401 | 403 | 407 | 451)
}

/// Whether the internet itself works. Only an empty `204` counts; see the
/// module note on why nothing softer does.
pub fn internet_answered(status: u16) -> bool {
    status == 204
}

// --- The live answer ---

static STATUS: LazyLock<Mutex<HostStatus>> = LazyLock::new(|| Mutex::new(HostStatus::Unknown));
static APP: OnceLock<AppHandle> = OnceLock::new();
/// One probe at a time, however many callers ask at once.
static PROBING: AtomicBool = AtomicBool::new(false);

fn set(next: HostStatus) {
    let changed = {
        let Ok(mut slot) = STATUS.lock() else {
            return;
        };
        if *slot == next {
            false
        } else {
            *slot = next;
            true
        }
    };
    if !changed {
        return;
    }
    info!("[ModelHost] huggingface.co: {:?}", next);
    if let Some(app) = APP.get() {
        if let Err(e) = app.emit(crate::constants::events::model_host::CHANGED, next) {
            warn!("[ModelHost] Could not emit the change: {}", e);
        }
    }
}

/// What is known right now. Never does I/O, so it is safe on the speech path.
pub fn status() -> HostStatus {
    STATUS.lock().map(|s| *s).unwrap_or_default()
}

/// Whether an automatic Hugging Face request may be made right now.
pub fn may_fetch() -> bool {
    status().may_fetch()
}

/// Remember the handle so a change can reach the UI. Call once at launch.
/// Deliberately does not probe: nothing may want a model this run, and a
/// request nobody asked for is what this module exists to stop.
pub fn start(app: AppHandle) {
    let _ = APP.set(app);
}

/// Settle the answer if nothing is known, then return it. A second caller
/// while a probe is in flight takes what is known rather than making its own
/// request: a list of rows asking at once is exactly the storm to avoid.
pub async fn ensure() -> HostStatus {
    let known = status();
    if known != HostStatus::Unknown {
        return known;
    }
    probe().await
}

/// Ask again whatever is known. The person pressed Retry, or something that
/// should have worked did not.
pub async fn recheck() -> HostStatus {
    probe().await
}

/// Settle the answer without waiting for it. For a call site that should not
/// stall on a probe: a probe on a network that will not answer takes seconds,
/// and neither launch nor a settings pane may wait that long. When it lands
/// it arrives as `model-host-changed` and the surface redraws.
pub fn ensure_in_background() {
    let _ = tauri::async_runtime::spawn(async {
        ensure().await;
    });
}

/// Run `recheck` without waiting for it. For call sites that have just failed
/// and have nothing to do with the answer except remember it.
pub fn recheck_in_background() {
    let _ = tauri::async_runtime::spawn(async {
        recheck().await;
    });
}

/// The machine gained or lost its route.
///
/// Losing it is an answer in itself. Gaining it is not "still fine": the
/// path that just came up may be a different network, and the one Lacy hits
/// this on is a laptop that moves between a home network and a work one. So
/// the old answer is thrown away rather than trusted. Nothing probes here:
/// the next thing that wants a model does, so a network that flaps costs one
/// probe per actual need and not one per flap.
pub fn on_network_change(up: bool) {
    set(if up {
        HostStatus::Unknown
    } else {
        HostStatus::Offline
    });
}

async fn probe() -> HostStatus {
    if !crate::connectivity::network_up() {
        set(HostStatus::Offline);
        return HostStatus::Offline;
    }
    if PROBING.swap(true, Ordering::SeqCst) {
        return status();
    }
    let outcome = run_probes().await;
    PROBING.store(false, Ordering::SeqCst);
    if let Some(outcome) = outcome {
        set(outcome);
        return outcome;
    }
    status()
}

/// `None` when the probe could not be made at all, which says nothing about
/// the network and must not be recorded as though it did.
async fn run_probes() -> Option<HostStatus> {
    let client = match reqwest::Client::builder()
        .connect_timeout(PROBE_CONNECT_TIMEOUT)
        .timeout(PROBE_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            warn!("[ModelHost] Could not build the probe client: {}", e);
            return None;
        }
    };

    if probe_host(&client).await {
        return Some(classify(true, true, false));
    }
    let internet_ok = probe_internet(&client).await;
    Some(classify(true, false, internet_ok))
}

async fn probe_host(client: &reqwest::Client) -> bool {
    // One byte. Enough to prove the path, small enough to cost nothing.
    let request = client
        .get(HOST_PROBE_URL)
        .header(reqwest::header::RANGE, "bytes=0-0");
    match request.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            let answered = host_answered(status, content_type.as_deref());
            if !answered {
                info!(
                    "[ModelHost] huggingface.co answered {} as {:?}; not the model host",
                    status, content_type
                );
            }
            answered
        }
        Err(e) => {
            info!("[ModelHost] huggingface.co did not answer: {}", e);
            false
        }
    }
}

async fn probe_internet(client: &reqwest::Client) -> bool {
    match client.get(INTERNET_PROBE_URL).send().await {
        Ok(response) => internet_answered(response.status().as_u16()),
        Err(e) => {
            info!("[ModelHost] The internet probe did not answer: {}", e);
            false
        }
    }
}

/// Ask again, and say what the answer is. The Retry button in Models.
#[tauri::command]
pub async fn recheck_model_host() -> HostStatus {
    recheck().await
}

/// What is known, without asking.
#[tauri::command]
pub fn get_model_host_status() -> HostStatus {
    status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_route_is_offline_and_never_blames_hugging_face() {
        // Even if a stale probe said the host answered: no route, no opinion.
        assert_eq!(classify(false, true, true), HostStatus::Offline);
        assert_eq!(classify(false, false, false), HostStatus::Offline);
    }

    #[test]
    fn the_host_answering_is_reachable() {
        assert_eq!(classify(true, true, false), HostStatus::Reachable);
    }

    #[test]
    fn a_working_internet_and_a_silent_host_is_blocked() {
        assert_eq!(classify(true, false, true), HostStatus::Blocked);
    }

    #[test]
    fn neither_answering_is_offline_not_blocked() {
        // A route the OS calls usable is not the same as working internet:
        // a captive portal has one. Blaming Hugging Face here is the bug.
        assert_eq!(classify(true, false, false), HostStatus::Offline);
    }

    #[test]
    fn a_block_page_is_not_the_host_however_ordinary_its_status() {
        assert!(!host_answered(200, Some("text/html")));
        assert!(!host_answered(200, Some("text/html; charset=utf-8")));
        assert!(!host_answered(200, Some("TEXT/HTML")));
        assert!(!host_answered(206, Some("application/xhtml+xml")));
    }

    #[test]
    fn a_proxy_refusing_is_not_the_host_either() {
        for status in [401, 403, 407, 451] {
            assert!(
                !host_answered(status, Some("application/octet-stream")),
                "{status} should not read as Hugging Face answering"
            );
        }
    }

    #[test]
    fn the_host_being_unwell_still_counts_as_reached() {
        // 5xx is Hugging Face answering badly, not the network refusing. The
        // download's retry policy handles it; calling it Blocked would stop
        // every future download until someone pressed Retry.
        assert!(host_answered(500, Some("application/json")));
        assert!(host_answered(503, None));
        assert!(host_answered(429, None));
    }

    #[test]
    fn a_one_byte_range_answer_is_the_host() {
        assert!(host_answered(206, Some("binary/octet-stream")));
        assert!(host_answered(200, Some("application/octet-stream")));
    }

    #[test]
    fn only_an_empty_204_proves_the_internet() {
        assert!(internet_answered(204));
        // A captive portal's redirect or page: both fail, which is correct.
        assert!(!internet_answered(200));
        assert!(!internet_answered(302));
        assert!(!internet_answered(511));
    }

    #[test]
    fn only_a_settled_no_stops_an_automatic_fetch() {
        assert!(HostStatus::Unknown.may_fetch());
        assert!(HostStatus::Reachable.may_fetch());
        assert!(!HostStatus::Blocked.may_fetch());
        assert!(!HostStatus::Offline.may_fetch());
    }

    #[test]
    fn the_blocked_line_names_the_network_and_not_hugging_face() {
        let line = HostStatus::Blocked.note().unwrap_or_default();
        assert_eq!(line, NOT_ON_THIS_NETWORK);
        assert!(!line.to_lowercase().contains("hugging"));
        assert!(!line.contains('\u{2014}'));
        // Offline says something different, so the two are never confused.
        assert_ne!(HostStatus::Offline.note(), HostStatus::Blocked.note());
        assert_eq!(HostStatus::Reachable.note(), None);
        assert_eq!(HostStatus::Unknown.note(), None);
    }

    #[test]
    fn the_status_serializes_as_the_frontend_reads_it() {
        let json = |s: HostStatus| serde_json::to_string(&s).unwrap_or_default();
        assert_eq!(json(HostStatus::Blocked), "\"blocked\"");
        assert_eq!(json(HostStatus::Offline), "\"offline\"");
        assert_eq!(json(HostStatus::Reachable), "\"reachable\"");
        assert_eq!(json(HostStatus::Unknown), "\"unknown\"");
    }
}
