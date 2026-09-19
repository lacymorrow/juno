//! # Voice session identity
//!
//! One microphone, one open session, one record that says whose it is.
//!
//! Every way of stopping a voice session used to infer ownership from a
//! different signal: a liveness flag, the input monitor's hold state, the bar's
//! UI state, the name of the command that was invoked. Four shipped bugs came
//! out of that, each one a stop path acting on a session it did not own:
//! dictation transcripts submitted to the agent, a cancelled session
//! resurrected by a late key release, "cancel" typing the sentence it was
//! asked to throw away, and the bar's X doing nothing at all for a session
//! started from the keyboard.
//!
//! This module is the answer to all four: a session gets an identity when it
//! starts, and every stop path asks the registry who it is stopping instead of
//! guessing. A claim that does not match the current session is refused, so a
//! stale stop is a no-op rather than a wrong action.
//!
//! A fifth came from the other end. Stops had to claim; starts did not, so a
//! second start replaced the standing session in place. The microphone did not
//! change hands, because the audio engine refuses a second recording, so the
//! second start failed and its failure path tore down the state of the session
//! it had just taken the name of: capture flag down, tray back to idle, input
//! monitor reset, while the first session's microphone was still recording. So
//! [`VoiceSessionRegistry::begin`] refuses too, and a start with nowhere to go
//! is a no-op like a stale stop.
//!
//! The logic here is pure and unit tested. [`crate::state::AppState`] owns one
//! registry behind a mutex and exposes thin wrappers.

use serde::{Deserialize, Serialize};

/// Which side of the app a voice session belongs to.
///
/// This is the whole routing decision for a transcript: a dictation session's
/// text is typed, an agent session's text is submitted. It is recorded when the
/// session starts, so it cannot be clobbered by the stop that produces the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceTarget {
    /// Juno acts on what was said.
    Agent,
    /// What was said is typed.
    Dictation,
}

impl VoiceTarget {
    /// Short label for logs.
    pub fn label(self) -> &'static str {
        match self {
            VoiceTarget::Agent => "agent",
            VoiceTarget::Dictation => "dictation",
        }
    }
}

impl From<crate::triggers::TriggerTarget> for VoiceTarget {
    fn from(target: crate::triggers::TriggerTarget) -> Self {
        match target {
            crate::triggers::TriggerTarget::Agent => VoiceTarget::Agent,
            crate::triggers::TriggerTarget::Dictation => VoiceTarget::Dictation,
        }
    }
}

/// How a session was started.
///
/// Carried for diagnosis rather than routing: when a stop is refused, the log
/// line says which method opened the session that is still standing, which is
/// the one thing the four historical reports never had.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceStartMethod {
    /// Held a key or button for as long as the session should stay open.
    PushToTalk,
    /// Tapped a key or button on, tapped it off again.
    Toggle,
    /// A bare modifier such as Fn, watched by the modifier monitor.
    ModifierKey,
    /// A mouse button, including the floating bar's mic.
    Mouse,
    /// Spoke a wake phrase.
    WakePhrase,
    /// The path that started this session did not say how it was triggered.
    ///
    /// Deliberately not a guess. The start events carry the method in their
    /// payload; an emitter that has not been taught to say it lands here, and
    /// the value is visible in logs rather than being quietly invented.
    Unstated,
}

impl VoiceStartMethod {
    /// Read the method out of a start event's payload value.
    ///
    /// Anything unrecognised, absent, or malformed is [`Unstated`], never a
    /// substitute method. Guessing here is how the original defect worked.
    ///
    /// [`Unstated`]: VoiceStartMethod::Unstated
    pub fn from_wire(raw: Option<&str>) -> Self {
        match raw {
            Some("push_to_talk") => VoiceStartMethod::PushToTalk,
            Some("toggle") => VoiceStartMethod::Toggle,
            Some("modifier_key") => VoiceStartMethod::ModifierKey,
            Some("mouse") => VoiceStartMethod::Mouse,
            Some("wake_phrase") => VoiceStartMethod::WakePhrase,
            _ => VoiceStartMethod::Unstated,
        }
    }

    /// Read the method out of a start event's raw JSON payload.
    ///
    /// Start events are emitted from several places, some of which predate
    /// this and send `()`. Those land on [`Unstated`] instead of being
    /// interpreted, so an emitter that has not been taught to say how it fired
    /// is visible rather than silently mislabelled.
    ///
    /// [`Unstated`]: VoiceStartMethod::Unstated
    pub fn from_event_payload(payload: &str) -> Self {
        let parsed = serde_json::from_str::<serde_json::Value>(payload).ok();
        let raw = parsed
            .as_ref()
            .and_then(|value| value.get("method"))
            .and_then(|value| value.as_str());
        Self::from_wire(raw)
    }

    /// The string an emitter puts in a start event's `method` field.
    pub fn as_wire(self) -> &'static str {
        match self {
            VoiceStartMethod::PushToTalk => "push_to_talk",
            VoiceStartMethod::Toggle => "toggle",
            VoiceStartMethod::ModifierKey => "modifier_key",
            VoiceStartMethod::Mouse => "mouse",
            VoiceStartMethod::WakePhrase => "wake_phrase",
            VoiceStartMethod::Unstated => "unstated",
        }
    }
}

impl From<crate::triggers::TriggerMethod> for VoiceStartMethod {
    fn from(method: crate::triggers::TriggerMethod) -> Self {
        match method {
            crate::triggers::TriggerMethod::PushToTalk => VoiceStartMethod::PushToTalk,
            crate::triggers::TriggerMethod::Toggle => VoiceStartMethod::Toggle,
            crate::triggers::TriggerMethod::Voice => VoiceStartMethod::WakePhrase,
        }
    }
}

/// Where a session is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoicePhase {
    /// The microphone is open.
    Live,
    /// A commit has been claimed and the transcript has not arrived yet.
    ///
    /// The session is still the owner of what is coming, which is why it stays
    /// in the registry: the flag that used to answer "is this a dictation" went
    /// false at exactly this moment, and the transcript is produced by the stop
    /// that cleared it.
    Finishing,
}

/// A session's identity. Unique for the life of the process.
pub type VoiceSessionId = u64;

/// One live voice session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct VoiceSession {
    pub id: VoiceSessionId,
    pub target: VoiceTarget,
    pub method: VoiceStartMethod,
    pub phase: VoicePhase,
}

impl VoiceSession {
    pub fn is_live(self) -> bool {
        self.phase == VoicePhase::Live
    }

    /// What a log line calls this session.
    pub fn describe(self) -> String {
        format!(
            "#{} {} via {} ({:?})",
            self.id,
            self.target.label(),
            self.method.as_wire(),
            self.phase
        )
    }
}

/// Who a stop is speaking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionClaim {
    /// A specific session. Anything else open is somebody else's, so the claim
    /// is refused. This is what makes a late key release harmless.
    Id(VoiceSessionId),
    /// Whatever session is open right now.
    ///
    /// For the controls that genuinely mean "end what I can see": the bar's
    /// buttons, the stop key, the force paths. They cannot name an id because
    /// the person pressing them is looking at the session, not holding it.
    Current,
}

/// Why a claim was refused. Each variant is a distinct log line, because
/// "nothing was open" and "something else is open" are different stories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimRejection {
    /// No session is open at all.
    Nothing,
    /// A different session is open. The claimant is late.
    Stale,
    /// This session was already committed and is waiting for its transcript.
    AlreadyFinishing,
}

impl ClaimRejection {
    pub fn reason(self) -> &'static str {
        match self {
            ClaimRejection::Nothing => "no voice session is open",
            ClaimRejection::Stale => "a different voice session is open",
            ClaimRejection::AlreadyFinishing => "that session is already finishing",
        }
    }
}

/// Why [`VoiceSessionRegistry::begin`] refused to open a session.
///
/// There is one microphone, so a start that arrives while a session is standing
/// is not a start. The refusal carries the session that still owns the
/// microphone, because the only useful thing to say about a refused start is
/// whose session it was refused in favour of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartRefused {
    /// The session that still owns the microphone.
    pub standing: VoiceSession,
}

impl StartRefused {
    /// What a log line calls this refusal.
    pub fn reason(self) -> String {
        format!("{} already owns the microphone", self.standing.describe())
    }
}

/// The single owner of voice session identity.
///
/// Holds at most one session, because there is one microphone. Two paths cannot
/// both believe they own it: whatever is in `current` is the session, and a
/// claim either matches it or is refused.
#[derive(Debug, Default)]
pub struct VoiceSessionRegistry {
    last_id: VoiceSessionId,
    current: Option<VoiceSession>,
}

impl VoiceSessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a session and hand back its identity, unless one is already open.
    ///
    /// Stops have always had to claim. Starts did not: a second start replaced
    /// the standing session in place and reported it as "superseded" in a
    /// warning nobody could act on. The microphone never actually changed
    /// hands, because the audio engine refuses a second recording, so the
    /// second start then failed and its failure path tore down the state of the
    /// session it had just taken the name of. That is how a live microphone
    /// ended up behind a bar that said nothing was happening.
    ///
    /// Refusing is the honest answer. The microphone is already open, and a
    /// second press of the same control means "I am already talking", not
    /// "throw away what I said and start again"; ending the standing session
    /// here would discard audio nobody asked to discard. Anything that really
    /// does mean to replace a session stops or cancels it first, and those
    /// paths already exist and already claim.
    ///
    /// A refused start mints nothing, so ids stay in step with sessions that
    /// really opened. Ids are never reused, so an id from an earlier session
    /// can only ever be refused, never mistaken for the current one.
    pub fn begin(
        &mut self,
        target: VoiceTarget,
        method: VoiceStartMethod,
    ) -> Result<VoiceSession, StartRefused> {
        if let Some(standing) = self.current {
            return Err(StartRefused { standing });
        }
        self.last_id += 1;
        let session = VoiceSession {
            id: self.last_id,
            target,
            method,
            phase: VoicePhase::Live,
        };
        self.current = Some(session);
        Ok(session)
    }

    /// The session that is open now, if there is one.
    pub fn current(&self) -> Option<VoiceSession> {
        self.current
    }

    /// Claim a session in order to finalise it: the audio is kept, the
    /// transcript is coming, and it will be delivered to this session's target.
    ///
    /// The session stays registered in [`VoicePhase::Finishing`] because it
    /// still owns a transcript that has not arrived. A second commit for the
    /// same session is refused, so a doubled stop cannot finalise twice.
    pub fn claim_commit(&mut self, claim: SessionClaim) -> Result<VoiceSession, ClaimRejection> {
        let current = self.matching(claim)?;
        if current.phase == VoicePhase::Finishing {
            return Err(ClaimRejection::AlreadyFinishing);
        }
        let committed = VoiceSession {
            phase: VoicePhase::Finishing,
            ..current
        };
        self.current = Some(committed);
        Ok(committed)
    }

    /// Claim a session in order to throw it away: the audio is discarded and
    /// nothing is typed or submitted.
    ///
    /// The session leaves the registry immediately, which is what makes a
    /// cancelled session unresurrectable. A commit that arrives afterwards
    /// finds nothing to claim.
    ///
    /// A session that is already finishing can still be discarded, so a cancel
    /// that lands while speech to text is running drops the transcript instead
    /// of letting it through.
    pub fn claim_discard(&mut self, claim: SessionClaim) -> Result<VoiceSession, ClaimRejection> {
        let current = self.matching(claim)?;
        self.current = None;
        Ok(current)
    }

    /// Hand the arriving transcript to the session that owns it, and close the
    /// session out.
    ///
    /// Deliberately indifferent to phase. Most transcripts arrive because a
    /// commit asked for them, but the speech engine can also finalise on its
    /// own, and a session that is still live owns its text just as much. What
    /// matters is that a discarded session is no longer here, so cancelled
    /// audio can never be routed anywhere.
    pub fn take_transcript_owner(&mut self) -> Option<VoiceSession> {
        self.current.take()
    }

    fn matching(&self, claim: SessionClaim) -> Result<VoiceSession, ClaimRejection> {
        let Some(current) = self.current else {
            return Err(ClaimRejection::Nothing);
        };
        match claim {
            SessionClaim::Current => Ok(current),
            SessionClaim::Id(id) if id == current.id => Ok(current),
            SessionClaim::Id(_) => Err(ClaimRejection::Stale),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> VoiceSessionRegistry {
        VoiceSessionRegistry::new()
    }

    /// Open a session in a test that is not about the start guard.
    fn open(
        reg: &mut VoiceSessionRegistry,
        target: VoiceTarget,
        method: VoiceStartMethod,
    ) -> VoiceSession {
        match reg.begin(target, method) {
            Ok(session) => session,
            Err(refused) => panic!("nothing should have been open, but {}", refused.reason()),
        }
    }

    #[test]
    fn a_session_carries_its_identity_from_the_start() {
        let mut reg = registry();
        let session = open(
            &mut reg,
            VoiceTarget::Dictation,
            VoiceStartMethod::PushToTalk,
        );
        assert_eq!(session.target, VoiceTarget::Dictation);
        assert_eq!(session.method, VoiceStartMethod::PushToTalk);
        assert!(session.is_live());
        assert_eq!(reg.current(), Some(session));
    }

    #[test]
    fn ids_are_never_reused() {
        let mut reg = registry();
        let first = open(&mut reg, VoiceTarget::Agent, VoiceStartMethod::Toggle);
        reg.claim_discard(SessionClaim::Current).expect("claimed");
        let second = open(&mut reg, VoiceTarget::Agent, VoiceStartMethod::Toggle);
        assert_ne!(first.id, second.id);
    }

    #[test]
    fn a_stale_commit_is_refused() {
        // The push-to-talk resurrection, as a unit test. A session is cancelled
        // and a new one opens; the key that was still held from the first one
        // finally comes up and tries to commit it.
        let mut reg = registry();
        let cancelled = open(&mut reg, VoiceTarget::Agent, VoiceStartMethod::PushToTalk);
        reg.claim_discard(SessionClaim::Id(cancelled.id))
            .expect("the cancel owns it");
        let fresh = open(&mut reg, VoiceTarget::Agent, VoiceStartMethod::PushToTalk);

        assert_eq!(
            reg.claim_commit(SessionClaim::Id(cancelled.id)),
            Err(ClaimRejection::Stale),
            "a release carrying an old id must do nothing"
        );
        assert_eq!(
            reg.current(),
            Some(fresh),
            "and must not have touched the session that is actually open"
        );
    }

    #[test]
    fn a_commit_after_a_cancel_finds_nothing() {
        // The same race with no second session: the cancel emptied the
        // registry, so the late stop has nothing to finalise and nothing is
        // submitted.
        let mut reg = registry();
        let session = open(&mut reg, VoiceTarget::Agent, VoiceStartMethod::PushToTalk);
        reg.claim_discard(SessionClaim::Id(session.id))
            .expect("claimed");
        assert_eq!(
            reg.claim_commit(SessionClaim::Id(session.id)),
            Err(ClaimRejection::Nothing)
        );
        assert_eq!(
            reg.claim_commit(SessionClaim::Current),
            Err(ClaimRejection::Nothing),
            "not even a control that means \"end whatever is open\""
        );
    }

    #[test]
    fn a_stale_cancel_is_refused_too() {
        let mut reg = registry();
        let first = open(&mut reg, VoiceTarget::Dictation, VoiceStartMethod::Toggle);
        reg.claim_discard(SessionClaim::Id(first.id))
            .expect("claimed");
        let second = open(&mut reg, VoiceTarget::Dictation, VoiceStartMethod::Toggle);
        assert_eq!(
            reg.claim_discard(SessionClaim::Id(first.id)),
            Err(ClaimRejection::Stale)
        );
        assert_eq!(reg.current(), Some(second));
    }

    #[test]
    fn committing_twice_is_refused() {
        // Two stop paths firing for one release would otherwise finalise the
        // audio twice.
        let mut reg = registry();
        let session = open(
            &mut reg,
            VoiceTarget::Dictation,
            VoiceStartMethod::PushToTalk,
        );
        assert!(reg.claim_commit(SessionClaim::Id(session.id)).is_ok());
        assert_eq!(
            reg.claim_commit(SessionClaim::Id(session.id)),
            Err(ClaimRejection::AlreadyFinishing)
        );
        assert_eq!(
            reg.claim_commit(SessionClaim::Current),
            Err(ClaimRejection::AlreadyFinishing)
        );
    }

    #[test]
    fn the_transcript_goes_to_the_session_that_was_started_not_the_one_running() {
        // The original defect: routing asked whether a session was live, and
        // the transcript only exists because it stopped being live.
        let mut reg = registry();
        let session = open(
            &mut reg,
            VoiceTarget::Dictation,
            VoiceStartMethod::PushToTalk,
        );
        let committed = reg
            .claim_commit(SessionClaim::Id(session.id))
            .expect("claimed");
        assert!(!committed.is_live(), "the audio is no longer open");

        let owner = reg
            .take_transcript_owner()
            .expect("the session still owns it");
        assert_eq!(owner.target, VoiceTarget::Dictation);
        assert_eq!(owner.id, session.id);
    }

    #[test]
    fn a_transcript_is_owned_once() {
        // Otherwise a leftover owner would capture the next session's text.
        let mut reg = registry();
        open(&mut reg, VoiceTarget::Dictation, VoiceStartMethod::Toggle);
        reg.claim_commit(SessionClaim::Current).expect("claimed");
        assert!(reg.take_transcript_owner().is_some());
        assert!(reg.take_transcript_owner().is_none());
    }

    #[test]
    fn a_cancelled_session_owns_no_transcript() {
        // Cancel means cancel: if the engine emits a final result anyway, there
        // is nobody left to type it or send it.
        let mut reg = registry();
        open(
            &mut reg,
            VoiceTarget::Dictation,
            VoiceStartMethod::PushToTalk,
        );
        reg.claim_discard(SessionClaim::Current).expect("claimed");
        assert!(reg.take_transcript_owner().is_none());
    }

    #[test]
    fn a_cancel_during_finalisation_still_drops_the_text() {
        let mut reg = registry();
        let session = open(
            &mut reg,
            VoiceTarget::Dictation,
            VoiceStartMethod::PushToTalk,
        );
        reg.claim_commit(SessionClaim::Id(session.id))
            .expect("claimed");
        reg.claim_discard(SessionClaim::Current)
            .expect("a finishing session can still be thrown away");
        assert!(reg.take_transcript_owner().is_none());
    }

    #[test]
    fn a_live_session_owns_its_transcript_without_a_commit() {
        // The speech engine can finalise on its own (silence detection). The
        // session is the owner either way.
        let mut reg = registry();
        open(&mut reg, VoiceTarget::Agent, VoiceStartMethod::WakePhrase);
        let owner = reg.take_transcript_owner().expect("owned");
        assert_eq!(owner.target, VoiceTarget::Agent);
    }

    #[test]
    fn a_start_while_a_session_is_live_is_refused() {
        // The bug this guard exists for: the bar's mic pressed twice. The
        // second press used to open a session, take ownership from the first,
        // and then fail at the audio engine with "Already dictating".
        let mut reg = registry();
        let live = open(&mut reg, VoiceTarget::Agent, VoiceStartMethod::Mouse);
        assert_eq!(
            reg.begin(VoiceTarget::Agent, VoiceStartMethod::Mouse),
            Err(StartRefused { standing: live })
        );
    }

    #[test]
    fn a_refused_start_leaves_the_existing_session_untouched() {
        // The whole point. A refused start must not be able to affect the
        // session that is actually recording: not its identity, not its phase,
        // and not its right to be stopped by the control the person is looking
        // at.
        let mut reg = registry();
        let live = open(&mut reg, VoiceTarget::Agent, VoiceStartMethod::Mouse);

        assert!(reg
            .begin(VoiceTarget::Dictation, VoiceStartMethod::Toggle)
            .is_err());

        assert_eq!(
            reg.current(),
            Some(live),
            "the standing session is still the open one, unchanged"
        );
        assert_eq!(
            reg.claim_commit(SessionClaim::Id(live.id)),
            Ok(VoiceSession {
                phase: VoicePhase::Finishing,
                ..live
            }),
            "and its own stop still reaches it"
        );
        assert_eq!(
            reg.take_transcript_owner().map(|owner| owner.id),
            Some(live.id),
            "so its transcript still goes where it was started to go"
        );
    }

    #[test]
    fn a_refused_start_mints_no_identity() {
        // Otherwise a refused start burns an id, and the ids in two log lines
        // stop being a count of the sessions that really opened.
        let mut reg = registry();
        let live = open(&mut reg, VoiceTarget::Agent, VoiceStartMethod::Mouse);
        assert!(reg
            .begin(VoiceTarget::Agent, VoiceStartMethod::Mouse)
            .is_err());
        reg.claim_discard(SessionClaim::Current).expect("claimed");

        let next = open(&mut reg, VoiceTarget::Agent, VoiceStartMethod::Mouse);
        assert_eq!(
            next.id,
            live.id + 1,
            "the next session that really opens is the next id"
        );
    }

    #[test]
    fn a_start_is_refused_while_the_standing_session_is_finishing() {
        // A committed session still owns a transcript that has not arrived.
        // Starting over it would hand that text to the new session.
        let mut reg = registry();
        let live = open(&mut reg, VoiceTarget::Dictation, VoiceStartMethod::Toggle);
        let finishing = reg
            .claim_commit(SessionClaim::Id(live.id))
            .expect("the stop owns it");
        assert_eq!(
            reg.begin(VoiceTarget::Agent, VoiceStartMethod::Mouse),
            Err(StartRefused {
                standing: finishing
            })
        );
    }

    #[test]
    fn a_start_is_allowed_once_the_session_before_it_has_ended() {
        // The guard refuses a second microphone, not a second sentence.
        let mut reg = registry();
        open(&mut reg, VoiceTarget::Agent, VoiceStartMethod::Mouse);
        reg.claim_discard(SessionClaim::Current).expect("claimed");
        assert!(reg
            .begin(VoiceTarget::Agent, VoiceStartMethod::Mouse)
            .is_ok());
    }

    #[test]
    fn claims_on_an_empty_registry_do_nothing() {
        let mut reg = registry();
        assert_eq!(
            reg.claim_commit(SessionClaim::Current),
            Err(ClaimRejection::Nothing)
        );
        assert_eq!(
            reg.claim_discard(SessionClaim::Id(7)),
            Err(ClaimRejection::Nothing)
        );
        assert!(reg.take_transcript_owner().is_none());
    }

    #[test]
    fn the_identity_says_which_session_a_stop_reached_not_which_verb_to_use() {
        // The bar's X used to speak only for the agent, so a dictation started
        // from the keyboard was not its to cancel and the button did nothing.
        // The verb is still "cancel"; the registry is what says whose.
        let mut reg = registry();
        open(
            &mut reg,
            VoiceTarget::Dictation,
            VoiceStartMethod::PushToTalk,
        );
        let claimed = reg
            .claim_discard(SessionClaim::Current)
            .expect("the open session is the one the X means");
        assert_eq!(claimed.target, VoiceTarget::Dictation);
    }

    #[test]
    fn an_unstated_method_is_not_invented() {
        assert_eq!(
            VoiceStartMethod::from_wire(None),
            VoiceStartMethod::Unstated
        );
        assert_eq!(
            VoiceStartMethod::from_wire(Some("nonsense")),
            VoiceStartMethod::Unstated
        );
        assert_eq!(
            VoiceStartMethod::from_wire(Some("push_to_talk")),
            VoiceStartMethod::PushToTalk
        );
    }

    #[test]
    fn a_start_event_payload_carries_the_method() {
        assert_eq!(
            VoiceStartMethod::from_event_payload("{\"method\":\"toggle\"}"),
            VoiceStartMethod::Toggle
        );
        // An emitter that still sends `()` produces "null", not a method.
        assert_eq!(
            VoiceStartMethod::from_event_payload("null"),
            VoiceStartMethod::Unstated
        );
        assert_eq!(
            VoiceStartMethod::from_event_payload("not json at all"),
            VoiceStartMethod::Unstated
        );
    }

    #[test]
    fn methods_survive_a_round_trip_through_the_wire() {
        for method in [
            VoiceStartMethod::PushToTalk,
            VoiceStartMethod::Toggle,
            VoiceStartMethod::ModifierKey,
            VoiceStartMethod::Mouse,
            VoiceStartMethod::WakePhrase,
        ] {
            assert_eq!(VoiceStartMethod::from_wire(Some(method.as_wire())), method);
        }
    }

    #[test]
    fn trigger_methods_and_targets_map_onto_session_identity() {
        use crate::triggers::{TriggerMethod, TriggerTarget};
        assert_eq!(
            VoiceStartMethod::from(TriggerMethod::PushToTalk),
            VoiceStartMethod::PushToTalk
        );
        assert_eq!(
            VoiceStartMethod::from(TriggerMethod::Voice),
            VoiceStartMethod::WakePhrase
        );
        assert_eq!(
            VoiceTarget::from(TriggerTarget::Dictation),
            VoiceTarget::Dictation
        );
    }
}
