//! # Connectivity: can Juno answer right now?
//!
//! One answer, owned here, for every surface that needs it: the bar's status
//! dot, `submit_query` deciding whether to call a model at all, and speech
//! choosing a voice that works offline.
//!
//! Two inputs feed it:
//!
//! - **The network.** On macOS, an `NWPathMonitor` tells us when the machine
//!   gains or loses a usable route. It is event-driven: nothing is polled and
//!   no outside URL is fetched to find out.
//! - **The provider.** Whether the active provider has a credential (an API
//!   key, or a signed-in CLI), and whether its last call reached it. A call
//!   that dies on a connection error marks the provider unreachable until the
//!   next call that gets an answer, or until the network comes back.
//!
//! A change to the combined status is emitted as `connectivity-changed`; the
//! `get_connectivity` command returns the current one. The frontend renders
//! it and decides nothing.
//!
//! A lost connection is not a bug, so nothing here produces an error for a
//! person to read. When Juno cannot answer, it says so in a plain sentence.

use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, warn};

use crate::agent::local_intents::Reply;
use crate::agent::providers::types::Provider;

/// Who answers, in the words a person would use for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Answerer {
    #[default]
    Claude,
    OpenAi,
    Gemini,
    Codex,
}

impl Answerer {
    pub fn for_provider(provider: &Provider) -> Self {
        match provider {
            Provider::Anthropic | Provider::ClaudeCli => Self::Claude,
            Provider::OpenAI | Provider::Rig => Self::OpenAi,
            Provider::Gemini => Self::Gemini,
            Provider::CodexCli => Self::Codex,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::OpenAi => "OpenAI",
            Self::Gemini => "Gemini",
            Self::Codex => "Codex",
        }
    }

    /// The host a reachability probe connects to.
    fn host(self) -> &'static str {
        match self {
            Self::Claude => "api.anthropic.com",
            Self::OpenAi | Self::Codex => "api.openai.com",
            Self::Gemini => "generativelanguage.googleapis.com",
        }
    }
}

/// The one status a surface shows, worst first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Connected,
    Offline,
    SignedOut,
    ProviderUnreachable,
}

/// What `connectivity-changed` carries and `get_connectivity` returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub status: Status,
    /// A short description of the status, for an accessible label or tooltip.
    pub label: String,
    /// The provider's everyday name ("Claude").
    pub provider: String,
}

/// Why a query cannot reach a model right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    Offline,
    ProviderUnreachable(Answerer),
}

/// The health state. Pure: every transition returns whether the snapshot a
/// surface would show has changed, so the caller knows when to emit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Health {
    /// Optimistic until the monitor says otherwise.
    network_up: bool,
    has_credential: bool,
    provider_reachable: bool,
    answerer: Answerer,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            network_up: true,
            has_credential: true,
            provider_reachable: true,
            answerer: Answerer::default(),
        }
    }
}

impl Health {
    pub fn status(&self) -> Status {
        if !self.network_up {
            Status::Offline
        } else if !self.has_credential {
            Status::SignedOut
        } else if !self.provider_reachable {
            Status::ProviderUnreachable
        } else {
            Status::Connected
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let name = self.answerer.name();
        let label = match self.status() {
            Status::Connected => format!("Connected to {name}"),
            Status::Offline => "No internet connection".to_string(),
            Status::SignedOut => format!("Not signed in to {name}"),
            Status::ProviderUnreachable => format!("Can't reach {name}"),
        };
        Snapshot {
            status: self.status(),
            label,
            provider: name.to_string(),
        }
    }

    pub fn network_up(&self) -> bool {
        self.network_up
    }

    /// The machine gained or lost its route. Coming back online also forgets
    /// that the provider was unreachable: the likeliest reason it was is the
    /// network that just returned, and the next call will say otherwise.
    pub fn network_changed(&mut self, up: bool) -> bool {
        let before = self.snapshot();
        self.network_up = up;
        if up {
            self.provider_reachable = true;
        }
        before != self.snapshot()
    }

    /// The active provider, and whether it has a credential. A different
    /// provider starts out reachable: the old one's failure says nothing about it.
    pub fn provider_changed(&mut self, answerer: Answerer, has_credential: bool) -> bool {
        let before = self.snapshot();
        if answerer != self.answerer {
            self.provider_reachable = true;
        }
        self.answerer = answerer;
        self.has_credential = has_credential;
        before != self.snapshot()
    }

    /// A call to the provider died on a connection error.
    pub fn provider_failed(&mut self) -> bool {
        let before = self.snapshot();
        self.provider_reachable = false;
        before != self.snapshot()
    }

    /// A call to the provider got an answer (any answer, a refusal included).
    pub fn provider_answered(&mut self) -> bool {
        let before = self.snapshot();
        self.provider_reachable = true;
        before != self.snapshot()
    }

    /// Whether a query should skip the model. Signed out is not blocked here:
    /// the existing sign-in path explains that better than a canned line.
    pub fn blocked(&self) -> Option<Blocked> {
        if !self.network_up {
            Some(Blocked::Offline)
        } else if !self.provider_reachable {
            Some(Blocked::ProviderUnreachable(self.answerer))
        } else {
            None
        }
    }
}

// --- What Juno says ---

pub const OFFLINE_REPLY: &str =
    "I can't reach the internet right now, so I can't answer that. Check your Wi-Fi and ask again.";

pub const LOST_MID_TURN_OFFLINE_REPLY: &str =
    "I lost the internet connection partway through, so I stopped there. Check your Wi-Fi and ask again.";

pub fn provider_down_reply(answerer: Answerer) -> String {
    let name = answerer.name();
    format!(
        "I can't reach {name} right now, so I can't answer that. Your internet is working, so it's likely on {name}'s end. Try again in a minute."
    )
}

pub fn lost_mid_turn_provider_reply(answerer: Answerer) -> String {
    format!(
        "I lost the connection to {} partway through, so I stopped there. Try again in a minute.",
        answerer.name()
    )
}

/// Text inside a double-quoted JSX attribute, with the characters that would
/// end or confuse it written as entities the JSX parser decodes back.
pub fn jsx_attr(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The reply for a query that could not be answered: the line, spoken as-is,
/// and a "Send again" button that resends exactly what was asked. Nothing is
/// ever resent on its own; a request sent minutes later could act on a
/// situation that has moved on. A query that carried images gets no button,
/// since only its words could be sent again.
pub fn unanswered_reply(line: &str, query: &str, had_images: bool) -> Reply {
    let query = query.trim();
    if had_images || query.is_empty() {
        return Reply::text(line);
    }
    Reply::card(
        format!(
            "{line}\n\n<QueryButton query=\"{}\" label=\"Send again\" />",
            jsx_attr(query)
        ),
        line,
    )
}

/// The reply for a query blocked before it reached a model.
pub fn blocked_reply(blocked: Blocked, query: &str, had_images: bool) -> Reply {
    match blocked {
        Blocked::Offline => unanswered_reply(OFFLINE_REPLY, query, had_images),
        Blocked::ProviderUnreachable(answerer) => {
            unanswered_reply(&provider_down_reply(answerer), query, had_images)
        }
    }
}

/// The reply for a turn a lost connection ended partway.
pub fn lost_mid_turn_reply(health: &Health, query: &str, had_images: bool) -> Reply {
    if health.network_up() {
        unanswered_reply(
            &lost_mid_turn_provider_reply(health.answerer),
            query,
            had_images,
        )
    } else {
        unanswered_reply(LOST_MID_TURN_OFFLINE_REPLY, query, had_images)
    }
}

// --- The live state ---

static HEALTH: LazyLock<Mutex<Health>> = LazyLock::new(|| Mutex::new(Health::default()));
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Apply a transition and tell every surface when what it shows changed.
fn update(transition: impl FnOnce(&mut Health) -> bool) {
    let changed_to = {
        let Ok(mut health) = HEALTH.lock() else {
            return;
        };
        transition(&mut *health).then(|| health.snapshot())
    };
    let Some(snapshot) = changed_to else {
        return;
    };
    info!("[Connectivity] {}", snapshot.label);
    if let Some(app) = APP.get() {
        if let Err(e) = app.emit(
            crate::constants::events::system::CONNECTIVITY_CHANGED,
            &snapshot,
        ) {
            warn!("[Connectivity] Could not emit the change: {}", e);
        }
    }
}

pub fn current() -> Health {
    HEALTH.lock().map(|h| h.clone()).unwrap_or_default()
}

/// False only when the network monitor has said the machine has no route.
pub fn network_up() -> bool {
    current().network_up()
}

pub fn record_network(up: bool) {
    update(|h| h.network_changed(up));
}

pub fn record_provider_failure() {
    update(Health::provider_failed);
}

pub fn record_provider_answer() {
    update(Health::provider_answered);
}

/// How long a reachability probe waits for a TCP connection.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Whether a query must skip the model, and why.
///
/// No network: blocked, no question. A provider marked unreachable gets one
/// quick TCP probe first, because only a call can clear that mark and a
/// query that is never sent would leave it set forever.
pub async fn check_before_query() -> Option<Blocked> {
    let health = current();
    match health.blocked()? {
        Blocked::Offline => Some(Blocked::Offline),
        Blocked::ProviderUnreachable(answerer) => {
            let probe = tokio::time::timeout(
                PROBE_TIMEOUT,
                tokio::net::TcpStream::connect((answerer.host(), 443)),
            )
            .await;
            if matches!(probe, Ok(Ok(_))) {
                record_provider_answer();
                None
            } else {
                Some(Blocked::ProviderUnreachable(answerer))
            }
        }
    }
}

/// Read the active provider and whether it can run, and record it.
async fn refresh_provider(app: &AppHandle) {
    use crate::agent::providers::claude_cli::{self, SignIn};
    use crate::agent::providers::config::ProviderConfig;

    let Some(manager) = app.try_state::<crate::settings::manager::SettingsManager>() else {
        return;
    };
    let Ok(config) = ProviderConfig::load_from_centralized_settings(&manager).await else {
        return;
    };
    let Some(provider) = Provider::from_str(&config.active_provider) else {
        return;
    };
    let has_credential = match provider {
        Provider::ClaudeCli => {
            claude_cli::is_claude_cli_available()
                && claude_cli::last_known_sign_in() != SignIn::SignedOut
        }
        Provider::CodexCli => crate::agent::providers::codex_cli::is_codex_cli_available(),
        _ => crate::agent::providers::startup_default::active_has_credential(&config),
    };
    let answerer = Answerer::for_provider(&provider);
    update(|h| h.provider_changed(answerer, has_credential));
}

/// Start watching. Call once the provider has been settled at launch; later
/// calls do nothing.
pub fn start(app: AppHandle) {
    use tauri::Listener;

    if APP.set(app.clone()).is_err() {
        return;
    }

    #[cfg(target_os = "macos")]
    if !path_monitor::start(record_network) {
        warn!("[Connectivity] Network monitor unavailable; assuming online");
    }

    let listener_app = app.clone();
    app.listen_any(
        crate::constants::settings::events::PROVIDER_SETTINGS_CHANGED,
        move |_| {
            let app = listener_app.clone();
            tauri::async_runtime::spawn(async move { refresh_provider(&app).await });
        },
    );
    tauri::async_runtime::spawn(async move { refresh_provider(&app).await });
}

/// The status the bar's dot shows.
#[tauri::command]
pub fn get_connectivity() -> Snapshot {
    current().snapshot()
}

/// `NWPathMonitor` through Network.framework's C API. The update handler runs
/// on a private serial queue at start and again whenever the path changes.
#[cfg(target_os = "macos")]
mod path_monitor {
    use block2::{Block, RcBlock};
    use std::ffi::{c_char, c_void};

    /// `nw_path_status_satisfied`, and `satisfiable` (a route that comes up
    /// on demand, such as a VPN), both of which can carry a request.
    const NW_PATH_STATUS_SATISFIED: i32 = 1;
    const NW_PATH_STATUS_SATISFIABLE: i32 = 3;
    const NW_PATH_STATUS_INVALID: i32 = 0;

    #[link(name = "Network", kind = "framework")]
    extern "C" {
        fn nw_path_monitor_create() -> *mut c_void;
        fn nw_path_monitor_set_queue(monitor: *mut c_void, queue: *mut c_void);
        fn nw_path_monitor_set_update_handler(
            monitor: *mut c_void,
            handler: &Block<dyn Fn(*mut c_void) + 'static>,
        );
        fn nw_path_monitor_start(monitor: *mut c_void);
        fn nw_path_get_status(path: *mut c_void) -> i32;
    }

    extern "C" {
        fn dispatch_queue_create(label: *const c_char, attr: *mut c_void) -> *mut c_void;
    }

    /// Start the monitor for the life of the app. Returns false if macOS
    /// would not create one.
    pub fn start(on_change: fn(bool)) -> bool {
        // SAFETY: plain C calls with no preconditions beyond valid arguments.
        // The monitor and queue are retained for the life of the process on
        // purpose; Network.framework copies the handler block.
        unsafe {
            let monitor = nw_path_monitor_create();
            if monitor.is_null() {
                return false;
            }
            let queue =
                dispatch_queue_create(c"ai.junebug.connectivity".as_ptr(), std::ptr::null_mut());
            if queue.is_null() {
                return false;
            }
            let handler = RcBlock::new(move |path: *mut c_void| {
                if path.is_null() {
                    return;
                }
                // SAFETY: macOS hands the handler a live path object.
                let status = nw_path_get_status(path);
                if status == NW_PATH_STATUS_INVALID {
                    return;
                }
                on_change(
                    status == NW_PATH_STATUS_SATISFIED || status == NW_PATH_STATUS_SATISFIABLE,
                );
            });
            nw_path_monitor_set_update_handler(monitor, &handler);
            nw_path_monitor_set_queue(monitor, queue);
            nw_path_monitor_start(monitor);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_connected() {
        let health = Health::default();
        assert_eq!(health.status(), Status::Connected);
        assert_eq!(health.snapshot().label, "Connected to Claude");
        assert_eq!(health.blocked(), None);
    }

    #[test]
    fn losing_the_network_goes_offline_and_blocks() {
        let mut health = Health::default();
        assert!(health.network_changed(false));
        assert_eq!(health.status(), Status::Offline);
        assert_eq!(health.snapshot().label, "No internet connection");
        assert_eq!(health.blocked(), Some(Blocked::Offline));
        // Same news twice is not a change.
        assert!(!health.network_changed(false));
    }

    #[test]
    fn a_connection_failure_marks_the_provider_down_until_it_answers() {
        let mut health = Health::default();
        assert!(health.provider_failed());
        assert_eq!(health.status(), Status::ProviderUnreachable);
        assert_eq!(health.snapshot().label, "Can't reach Claude");
        assert_eq!(
            health.blocked(),
            Some(Blocked::ProviderUnreachable(Answerer::Claude))
        );
        assert!(health.provider_answered());
        assert_eq!(health.status(), Status::Connected);
    }

    #[test]
    fn the_network_coming_back_clears_a_provider_failure() {
        let mut health = Health::default();
        health.network_changed(false);
        health.provider_failed();
        assert!(health.network_changed(true));
        assert_eq!(health.status(), Status::Connected);
    }

    #[test]
    fn offline_outranks_everything() {
        let mut health = Health::default();
        health.provider_changed(Answerer::Claude, false);
        health.provider_failed();
        health.network_changed(false);
        assert_eq!(health.status(), Status::Offline);
    }

    #[test]
    fn signed_out_shows_but_does_not_block() {
        let mut health = Health::default();
        assert!(health.provider_changed(Answerer::OpenAi, false));
        assert_eq!(health.status(), Status::SignedOut);
        assert_eq!(health.snapshot().label, "Not signed in to OpenAI");
        assert_eq!(health.blocked(), None);
    }

    #[test]
    fn switching_provider_forgets_the_old_ones_failure() {
        let mut health = Health::default();
        health.provider_failed();
        assert!(health.provider_changed(Answerer::Gemini, true));
        assert_eq!(health.status(), Status::Connected);
        // Re-reading the same provider keeps a real failure.
        health.provider_failed();
        assert!(!health.provider_changed(Answerer::Gemini, true));
        assert_eq!(health.status(), Status::ProviderUnreachable);
    }

    #[test]
    fn offline_short_circuit_says_so_plainly_and_offers_send_again() {
        let reply = blocked_reply(Blocked::Offline, "what's the weather", false);
        assert_eq!(reply.spoken, OFFLINE_REPLY);
        assert!(!reply.failed);
        assert!(reply.display.starts_with(OFFLINE_REPLY));
        assert!(reply
            .display
            .contains("<QueryButton query=\"what's the weather\" label=\"Send again\" />"));
    }

    #[test]
    fn provider_down_short_circuit_names_the_provider() {
        let reply = blocked_reply(
            Blocked::ProviderUnreachable(Answerer::Claude),
            "hello",
            false,
        );
        assert!(reply.spoken.starts_with("I can't reach Claude right now"));
    }

    #[test]
    fn a_query_with_images_gets_no_send_again() {
        let reply = blocked_reply(Blocked::Offline, "what is this", true);
        assert_eq!(reply.display, OFFLINE_REPLY);
    }

    #[test]
    fn mid_turn_loss_names_what_was_lost() {
        let mut health = Health::default();
        health.provider_failed();
        let reply = lost_mid_turn_reply(&health, "q", true);
        assert!(reply.spoken.starts_with("I lost the connection to Claude"));
        health.network_changed(false);
        let reply = lost_mid_turn_reply(&health, "q", true);
        assert_eq!(reply.spoken, LOST_MID_TURN_OFFLINE_REPLY);
    }

    #[test]
    fn the_resent_query_survives_attribute_quoting() {
        assert_eq!(
            jsx_attr(r#"say "hi" & <b>"#),
            "say &quot;hi&quot; &amp; &lt;b&gt;"
        );
    }

    #[test]
    fn no_reply_carries_an_em_dash() {
        for line in [
            OFFLINE_REPLY.to_string(),
            LOST_MID_TURN_OFFLINE_REPLY.to_string(),
            provider_down_reply(Answerer::Claude),
            lost_mid_turn_provider_reply(Answerer::Claude),
        ] {
            assert!(!line.contains('\u{2014}'), "{line}");
        }
    }
}
