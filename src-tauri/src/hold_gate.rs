//! # A release that arrives before its start has landed
//!
//! A held key opens a voice session in two steps that do not share a task: the
//! input monitor emits a start event on its tick, and a listener spawned for
//! that event registers the session and opens the microphone. Opening the
//! microphone is async and slow (permission check, pausing the wake-phrase
//! engine, the audio engine itself), so a short hold can let go of the key
//! while the start is still in flight.
//!
//! The release used to emit its stop or cancel straight away, and that event
//! raced the start it was meant to end. When it won, it found no session (or
//! one with no microphone behind it yet) and did nothing, and then the start
//! finished and opened the microphone with no release ever coming. The bar sat
//! in listening, and every later hold was refused because a session was
//! already standing.
//!
//! The gate makes the order explicit. The monitor opens an entry when it emits
//! the start and puts its id in the event. A release that finds the entry still
//! open records what it meant instead of acting. When the start handler
//! returns, whichever way it returns, it settles the entry and performs the
//! recorded release, so the stop or cancel always lands after the session it
//! ends.
//!
//! Each target has its own gate behind a plain `std` mutex: nothing awaits
//! while holding it, and settling from a `Drop` needs it to be synchronous.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::state::VoiceTarget;

/// Identifies one held start. Unique per target for the life of the process.
pub type HoldId = u64;

/// What a release meant, when it had to wait for its start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldEnd {
    /// Held past the threshold: finalise the session.
    Commit,
    /// A short hold: throw the session away.
    Cancel,
}

/// An entry whose start handler never reported back is forgotten after this
/// long. Nothing in the start path takes anywhere near it; it only bounds the
/// bookkeeping if a handler was never registered to settle it.
const STALE_AFTER: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct InFlight {
    id: HoldId,
    opened_at: Instant,
    release: Option<HoldEnd>,
    /// The start was refused because the other trigger's session holds the
    /// microphone. See [`StartGate::disown`].
    disowned: bool,
}

/// How many disowned holds are remembered while their key is still down.
/// One per target is the real number; the rest is slack.
const DISOWNED_KEPT: usize = 8;

/// The starts of one target that have been emitted and not yet settled.
#[derive(Debug, Default)]
pub struct StartGate {
    last_id: HoldId,
    in_flight: Vec<InFlight>,
    /// Settled holds whose start was disowned and whose key is still down.
    /// Their release is swallowed.
    disowned: Vec<HoldId>,
}

impl StartGate {
    /// A start is about to be emitted. Returns the id the event carries.
    pub fn open(&mut self) -> HoldId {
        self.in_flight
            .retain(|entry| entry.opened_at.elapsed() < STALE_AFTER);
        self.last_id += 1;
        self.in_flight.push(InFlight {
            id: self.last_id,
            opened_at: Instant::now(),
            release: None,
            disowned: false,
        });
        self.last_id
    }

    /// The key came up. Answers `true` when the caller must not act on the
    /// release now: either the start has not landed yet, in which case the
    /// release is recorded and [`StartGate::settle`] hands it back once the
    /// start is done, or the start was disowned and the release is swallowed.
    pub fn defer(&mut self, id: HoldId, end: HoldEnd) -> bool {
        if let Some(entry) = self.in_flight.iter_mut().find(|entry| entry.id == id) {
            entry.release = Some(end);
            return true;
        }
        if let Some(index) = self.disowned.iter().position(|&held| held == id) {
            self.disowned.remove(index);
            return true;
        }
        false
    }

    /// The start was refused because the *other* trigger's session holds the
    /// microphone: this hold opened nothing, so its release ends nothing.
    ///
    /// Without this the release went out as an ordinary stop or cancel, and
    /// the stop paths hand a verb aimed at the wrong side to the session that
    /// is open. So tapping the agent key while dictating cancelled the
    /// dictation, and letting go of it committed the dictation early while
    /// its own key was still held.
    pub fn disown(&mut self, id: HoldId) {
        if let Some(entry) = self.in_flight.iter_mut().find(|entry| entry.id == id) {
            entry.disowned = true;
        }
    }

    /// The start handler is done, whether it opened the microphone, was
    /// refused, or failed. Returns the release that arrived while it ran, if
    /// any, for the caller to perform now.
    pub fn settle(&mut self, id: HoldId) -> Option<HoldEnd> {
        let index = self.in_flight.iter().position(|entry| entry.id == id)?;
        let entry = self.in_flight.remove(index);
        if !entry.disowned {
            return entry.release;
        }
        if entry.release.is_none() {
            // The key is still down: remember to swallow its release.
            self.disowned.push(id);
            if self.disowned.len() > DISOWNED_KEPT {
                self.disowned.remove(0);
            }
        }
        None
    }

    /// Whether a start is still in flight. Tests only.
    #[cfg(test)]
    pub fn is_in_flight(&self, id: HoldId) -> bool {
        self.in_flight.iter().any(|entry| entry.id == id)
    }
}

static AGENT: Mutex<StartGate> = Mutex::new(StartGate {
    last_id: 0,
    in_flight: Vec::new(),
    disowned: Vec::new(),
});
static DICTATION: Mutex<StartGate> = Mutex::new(StartGate {
    last_id: 0,
    in_flight: Vec::new(),
    disowned: Vec::new(),
});

fn with<R>(target: VoiceTarget, f: impl FnOnce(&mut StartGate) -> R) -> R {
    let gate = match target {
        VoiceTarget::Agent => &AGENT,
        VoiceTarget::Dictation => &DICTATION,
    };
    let mut guard = match gate.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    f(&mut guard)
}

/// See [`StartGate::open`].
pub fn open(target: VoiceTarget) -> HoldId {
    with(target, StartGate::open)
}

/// See [`StartGate::defer`].
pub fn defer(target: VoiceTarget, id: HoldId, end: HoldEnd) -> bool {
    with(target, |gate| gate.defer(id, end))
}

/// See [`StartGate::settle`].
pub fn settle(target: VoiceTarget, id: HoldId) -> Option<HoldEnd> {
    with(target, |gate| gate.settle(id))
}

/// A start for `target` was refused because a session for `standing` holds
/// the microphone. When that is the other trigger's session, the hold is
/// disowned (see [`StartGate::disown`]). A refused start of the same target
/// is a second press of a control already talking, and its release still ends
/// that session, as it always has.
pub fn disown_if_crossed(target: VoiceTarget, standing: VoiceTarget, hold: Option<HoldId>) {
    if let Some(id) = hold {
        if crossed(target, standing) {
            with(target, |gate| gate.disown(id));
        }
    }
}

/// A refused start whose release must not reach the standing session.
pub fn crossed(target: VoiceTarget, standing: VoiceTarget) -> bool {
    target != standing
}

/// The event a release emits for a target. One table, so the immediate path
/// in the monitors and the deferred path here cannot disagree.
pub fn release_event(target: VoiceTarget, end: HoldEnd) -> &'static str {
    use crate::constants::events;
    match (target, end) {
        (VoiceTarget::Agent, HoldEnd::Commit) => events::agent::TRANSCRIPTION_STOP,
        (VoiceTarget::Agent, HoldEnd::Cancel) => events::agent::CANCEL,
        (VoiceTarget::Dictation, HoldEnd::Commit) => events::dictation::STOP,
        (VoiceTarget::Dictation, HoldEnd::Cancel) => events::dictation::TRANSCRIPTION_CANCEL,
    }
}

/// The hold id a start event carries, if it came from a held key.
pub fn hold_from_payload(payload: &str) -> Option<HoldId> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()?
        .get("hold")?
        .as_u64()
}

/// Settles a held start when the start handler is finished with it, on every
/// way out of the handler, an unwinding panic included. Whatever release
/// arrived meanwhile is emitted then, so it reaches the session the start
/// registered (or, if the start was refused or failed, the same no-op or
/// cleanup it would always have reached).
pub struct SettleOnDrop {
    app: tauri::AppHandle,
    target: VoiceTarget,
    hold: Option<HoldId>,
}

impl SettleOnDrop {
    pub fn new(app: tauri::AppHandle, target: VoiceTarget, hold: Option<HoldId>) -> Self {
        Self { app, target, hold }
    }
}

impl Drop for SettleOnDrop {
    fn drop(&mut self) {
        let Some(id) = self.hold.take() else {
            return;
        };
        let Some(end) = settle(self.target, id) else {
            return;
        };
        tracing::info!(
            "[HoldGate] {} hold #{} was let go before its start landed; applying the release ({:?}) now",
            self.target.label(),
            id,
            end
        );
        if let Err(e) = tauri::Emitter::emit(&self.app, release_event(self.target, end), ()) {
            tracing::error!("[HoldGate] Failed to emit the deferred release: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agent_key_tapped_during_a_dictation_does_not_cancel_it() {
        // Agent key down while dictating: its start is refused (the dictation
        // holds the microphone) and disowned. The tap's release lands after.
        let mut gate = StartGate::default();
        let id = gate.open();
        gate.disown(id);
        assert_eq!(gate.settle(id), None);
        assert!(
            gate.defer(id, HoldEnd::Cancel),
            "the release is swallowed, not sent on to the dictation"
        );
        // Swallowed once; the next hold is an ordinary one.
        let next = gate.open();
        assert_eq!(gate.settle(next), None);
        assert!(!gate.defer(next, HoldEnd::Commit));
    }

    #[test]
    fn a_disowned_release_that_arrived_early_is_dropped_too() {
        let mut gate = StartGate::default();
        let id = gate.open();
        assert!(gate.defer(id, HoldEnd::Commit), "start still in flight");
        gate.disown(id);
        assert_eq!(
            gate.settle(id),
            None,
            "the recorded commit must not be performed on the other session"
        );
        assert!(!gate.defer(id, HoldEnd::Commit), "nothing left to swallow");
    }

    #[test]
    fn only_the_other_triggers_session_disowns() {
        assert!(crossed(VoiceTarget::Agent, VoiceTarget::Dictation));
        assert!(crossed(VoiceTarget::Dictation, VoiceTarget::Agent));
        assert!(
            !crossed(VoiceTarget::Agent, VoiceTarget::Agent),
            "a second press of the same control still ends its session"
        );
    }

    #[test]
    fn a_release_after_the_start_landed_acts_at_once() {
        let mut gate = StartGate::default();
        let id = gate.open();
        assert_eq!(gate.settle(id), None, "nothing was let go while it ran");
        assert!(
            !gate.defer(id, HoldEnd::Commit),
            "the start has landed, so the release is performed immediately"
        );
    }

    #[test]
    fn a_release_before_the_start_landed_waits_for_it() {
        // The ~150ms press: the start event is out, the microphone is still
        // opening, and the key is already up.
        let mut gate = StartGate::default();
        let id = gate.open();
        assert!(gate.defer(id, HoldEnd::Cancel), "the release must wait");
        assert!(gate.is_in_flight(id));

        // The start lands (or fails) and hands the release back, so the
        // session it registered is cancelled and the microphone released.
        assert_eq!(gate.settle(id), Some(HoldEnd::Cancel));
        assert!(!gate.is_in_flight(id));
        assert_eq!(gate.settle(id), None, "a release is performed once");
    }

    #[test]
    fn a_committed_release_waits_the_same_way() {
        let mut gate = StartGate::default();
        let id = gate.open();
        assert!(gate.defer(id, HoldEnd::Commit));
        assert_eq!(gate.settle(id), Some(HoldEnd::Commit));
    }

    #[test]
    fn the_next_hold_after_a_deferred_release_is_ordinary() {
        let mut gate = StartGate::default();
        let first = gate.open();
        assert!(gate.defer(first, HoldEnd::Cancel));
        assert_eq!(gate.settle(first), Some(HoldEnd::Cancel));

        let second = gate.open();
        assert_ne!(first, second);
        assert_eq!(gate.settle(second), None);
        assert!(!gate.defer(second, HoldEnd::Commit));
    }

    #[test]
    fn overlapping_starts_keep_their_own_releases() {
        let mut gate = StartGate::default();
        let a = gate.open();
        let b = gate.open();
        assert!(gate.defer(a, HoldEnd::Cancel));
        assert_eq!(gate.settle(b), None);
        assert_eq!(gate.settle(a), Some(HoldEnd::Cancel));
    }

    #[test]
    fn a_release_for_an_unknown_hold_is_not_deferred() {
        let mut gate = StartGate::default();
        assert!(!gate.defer(42, HoldEnd::Cancel));
        assert_eq!(gate.settle(42), None);
    }

    #[test]
    fn the_hold_id_rides_in_the_start_payload() {
        assert_eq!(
            hold_from_payload(r#"{"method":"push_to_talk","hold":7}"#),
            Some(7)
        );
        assert_eq!(hold_from_payload(r#"{"method":"toggle"}"#), None);
        assert_eq!(hold_from_payload("null"), None);
        assert_eq!(hold_from_payload(""), None);
    }

    #[test]
    fn deferred_releases_emit_the_same_events_as_immediate_ones() {
        use crate::constants::events;
        assert_eq!(
            release_event(VoiceTarget::Agent, HoldEnd::Commit),
            events::agent::TRANSCRIPTION_STOP
        );
        assert_eq!(
            release_event(VoiceTarget::Agent, HoldEnd::Cancel),
            events::agent::CANCEL
        );
        assert_eq!(
            release_event(VoiceTarget::Dictation, HoldEnd::Commit),
            events::dictation::STOP
        );
        assert_eq!(
            release_event(VoiceTarget::Dictation, HoldEnd::Cancel),
            events::dictation::TRANSCRIPTION_CANCEL
        );
    }
}
