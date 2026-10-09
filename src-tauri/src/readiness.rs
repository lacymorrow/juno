//! # Launch readiness: what is still warming up
//!
//! The bar is put on screen before anything heavy has loaded. The speech
//! engine (Whisper or Parakeet) loads in the background and can take from a
//! tenth of a second to several seconds, depending on the model and how cold
//! the disk is. While it does, the bar is already there, and its status dot
//! breathes a slow system blue so the person can see Juno is getting ready
//! rather than ignoring them.
//!
//! This module owns that answer. It keeps the set of resources still loading,
//! emits `startup-readiness-changed` when the set changes, and answers
//! `get_startup_readiness` for a page that mounts after the fact. The page
//! renders it and decides nothing.
//!
//! Nothing here can leave the dot pulsing forever: a resource whose load ends
//! in failure counts as done (there is nothing left to wait for), and anything
//! still pending after [`GIVE_UP_AFTER`] is dropped from the set.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Listener};
use tracing::{info, warn};

use crate::constants::events;

/// Past this, a resource still loading is no longer shown as loading. A
/// pulse that never ends is a lie about a load that is not coming.
pub const GIVE_UP_AFTER: Duration = Duration::from_secs(30);

/// Something the first interaction needs that loads after the bar is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    /// The speech-to-text engine dictation and spoken queries transcribe with.
    Voice,
}

impl Resource {
    pub const ALL: [Resource; 1] = [Resource::Voice];

    fn milestone(self) -> &'static str {
        match self {
            Resource::Voice => "voice engine ready",
        }
    }
}

/// What the bar renders: is anything still loading, and what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub loading: bool,
    pub pending: Vec<Resource>,
}

/// The resources still loading. Pure, so the rules are testable without an
/// app.
#[derive(Debug, Default)]
pub struct Pending(Vec<Resource>);

impl Pending {
    pub fn all() -> Self {
        Self(Resource::ALL.to_vec())
    }

    /// Mark one resource done. True when that changed anything.
    pub fn ready(&mut self, resource: Resource) -> bool {
        let before = self.0.len();
        self.0.retain(|r| *r != resource);
        self.0.len() != before
    }

    /// Mark everything done. True when anything was still pending.
    pub fn clear(&mut self) -> bool {
        let had_any = !self.0.is_empty();
        self.0.clear();
        had_any
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            loading: !self.0.is_empty(),
            pending: self.0.clone(),
        }
    }
}

static PENDING: Mutex<Pending> = Mutex::new(Pending(Vec::new()));

fn with_pending<T>(f: impl FnOnce(&mut Pending) -> T) -> T {
    match PENDING.lock() {
        Ok(mut pending) => f(&mut pending),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

fn emit(app: &AppHandle) {
    let snapshot = with_pending(|p| p.snapshot());
    if let Err(e) = app.emit(events::system::STARTUP_READINESS_CHANGED, &snapshot) {
        warn!("[Readiness] Could not announce readiness: {}", e);
    }
}

/// One resource finished loading (or gave up). Logged once as a `[Startup]`
/// milestone, then announced.
pub fn mark_ready(app: &AppHandle, resource: Resource) {
    if with_pending(|p| p.ready(resource)) {
        crate::startup_timing::mark(resource.milestone());
        emit(app);
    }
}

/// Start tracking. Called at the top of setup, before any window exists, so
/// the bar's first read already sees the truth.
pub fn start(app: &AppHandle) {
    with_pending(|p| *p = Pending::all());

    // Voice: the plugin says when its background load ends, either way. The
    // listeners go up before the check, so a load that ends in between is
    // still heard.
    let ready_app = app.clone();
    app.listen_any(
        tauri_plugin_voice_transcription::constants::engine::READY,
        move |_| mark_ready(&ready_app, Resource::Voice),
    );
    let failed_app = app.clone();
    app.listen_any(
        tauri_plugin_voice_transcription::constants::engine::FAILED,
        move |_| mark_ready(&failed_app, Resource::Voice),
    );
    if !tauri_plugin_voice_transcription::engine_loading() {
        mark_ready(app, Resource::Voice);
    }

    let give_up_app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(GIVE_UP_AFTER).await;
        if with_pending(|p| p.clear()) {
            info!(
                "[Readiness] Still loading after {:?}; no longer showing it",
                GIVE_UP_AFTER
            );
            emit(&give_up_app);
        }
    });
}

/// What is still loading, for a page that mounts after the last change.
#[tauri::command]
pub fn get_startup_readiness() -> Snapshot {
    with_pending(|p| p.snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_starts_pending_and_loading() {
        let p = Pending::all();
        let s = p.snapshot();
        assert!(s.loading);
        assert_eq!(s.pending, Resource::ALL.to_vec());
    }

    #[test]
    fn a_ready_resource_leaves_the_set_once() {
        let mut p = Pending::all();
        assert!(p.ready(Resource::Voice));
        assert!(!p.ready(Resource::Voice), "a second ready changes nothing");
        assert!(!p.snapshot().loading);
    }

    #[test]
    fn nothing_tracked_means_nothing_loading() {
        let p = Pending::default();
        assert_eq!(
            p.snapshot(),
            Snapshot {
                loading: false,
                pending: vec![]
            }
        );
    }

    #[test]
    fn giving_up_clears_whatever_is_left() {
        let mut p = Pending::all();
        assert!(p.clear());
        assert!(!p.clear());
        assert!(!p.snapshot().loading);
    }

    #[test]
    fn the_snapshot_serializes_the_way_the_page_reads_it() {
        let json = serde_json::to_value(Pending::all().snapshot()).expect("serializes");
        assert_eq!(json["loading"], true);
        assert_eq!(json["pending"][0], "voice");
    }
}
