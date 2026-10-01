//! # Automation (TCC `AppleEvents`) consent, asked before macOS asks
//!
//! Juno drives other apps with AppleScript: `System Events` for keystrokes,
//! dark mode and lock screen, Safari for page reads. Every one of those is one
//! Automation grant, keyed to the pair (Juno, that app), and macOS asks for it
//! the first time Juno sends the event. Its dialog arrives with no Juno around
//! it, in the middle of whatever the person asked for, and it arrives once per
//! target app, which on a first run is several in a row.
//!
//! `AEDeterminePermissionToAutomateTarget` is the way out. With
//! `askUserIfNeeded` false it is the only AppleEvents call that answers the
//! question without raising the dialog, so Juno can find out that macOS *would*
//! ask, say why in its own words first, and then let the system ask once, on a
//! button the person pressed. With the flag true it is the same call doing the
//! asking, so there is exactly one code path and no second way for the dialog
//! to appear.
//!
//! The two answers that are not yes or no both matter:
//!
//! * `errAEEventWouldRequireUserConsent` is the whole reason this module
//!   exists: nothing has been decided yet, and asking is still Juno's move.
//! * `procNotFound` means the target app is not running, and AppleEvents
//!   cannot decide anything about an app that is not there. System Events is
//!   launched on demand, so this is ordinary rather than exceptional; it is
//!   reported as its own answer instead of being folded into a denial.
//!
//! This is the single home for the FFI. Everything else goes through the safe
//! wrappers below.

/// What macOS says about Juno automating one target app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationConsent {
    /// Juno may send this app events right now.
    Allowed,
    /// The person said no, or switched Juno off for this app in System
    /// Settings. Flipping it back on takes effect without a restart.
    Refused,
    /// Nothing has been decided. Sending an event now is what raises the
    /// system dialog, so this is the state Juno gets in front of.
    WouldAsk,
    /// The target app is not running, so there is nothing to decide yet.
    TargetNotRunning,
    /// The call failed, or this is not a macOS build. Nothing is known.
    Unknown,
}

impl AutomationConsent {
    /// Map the `OSStatus` from `AEDeterminePermissionToAutomateTarget`.
    ///
    /// Written as a pure function over the raw code so the mapping can be
    /// tested without an AppleEvents target, which a test runner does not
    /// have and must never go looking for.
    pub fn from_status(status: i32) -> Self {
        match status {
            codes::NO_ERR => Self::Allowed,
            codes::ERR_AE_EVENT_NOT_PERMITTED => Self::Refused,
            codes::ERR_AE_EVENT_WOULD_REQUIRE_USER_CONSENT => Self::WouldAsk,
            codes::PROC_NOT_FOUND => Self::TargetNotRunning,
            _ => Self::Unknown,
        }
    }

    /// True only when Juno can go ahead. Every other answer means stopping.
    pub fn is_allowed(self) -> bool {
        self == Self::Allowed
    }
}

/// The documented `OSStatus` values this module reads.
pub mod codes {
    pub const NO_ERR: i32 = 0;
    /// `errAEEventNotPermitted`
    pub const ERR_AE_EVENT_NOT_PERMITTED: i32 = -1743;
    /// `errAEEventWouldRequireUserConsent`
    pub const ERR_AE_EVENT_WOULD_REQUIRE_USER_CONSENT: i32 = -1744;
    /// `procNotFound`
    pub const PROC_NOT_FOUND: i32 = -600;
}

/// An app Juno drives with AppleScript.
///
/// A closed set rather than a string, because the bundle id reaches a system
/// call and because the person sees the name. Both belong in one place where
/// they cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutomationTarget {
    /// `System Events`, which Juno uses for keystrokes, dark mode and lock
    /// screen.
    SystemEvents,
    /// Safari, for reading the page the person is looking at.
    Safari,
}

impl AutomationTarget {
    pub fn bundle_id(self) -> &'static str {
        match self {
            Self::SystemEvents => "com.apple.systemevents",
            Self::Safari => "com.apple.Safari",
        }
    }

    /// The name macOS itself puts in its dialog and in the Automation pane, so
    /// Juno's sentence and the system's sentence use the same word.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::SystemEvents => "System Events",
            Self::Safari => "Safari",
        }
    }

    /// What this target lets Juno do, in a person's words.
    ///
    /// "System Events" is a true name and a useless one: it is the part of
    /// macOS that presses keys and reads windows, and nobody outside Apple
    /// knows that. The dialog will say "System Events", so Juno says it too,
    /// but Juno says what it is for first.
    pub fn what_it_unlocks(self) -> &'static str {
        match self {
            Self::SystemEvents => "press keys and work menus for you",
            Self::Safari => "read the page you are looking at",
        }
    }

    /// Resolve the key the frontend and the gate pass around.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "system_events" => Some(Self::SystemEvents),
            "safari" => Some(Self::Safari),
            _ => None,
        }
    }

    /// The stable key, matching `from_key`.
    pub fn key(self) -> &'static str {
        match self {
            Self::SystemEvents => "system_events",
            Self::Safari => "safari",
        }
    }

    /// True for a faceless helper macOS starts when something asks for it.
    ///
    /// AppleEvents cannot decide anything about an app that is not running, so
    /// a target like this is started before asking. An app the person opens
    /// themselves is not: starting Safari behind their back to answer a
    /// permission question would be Juno taking a liberty.
    pub fn launches_on_demand(self) -> bool {
        match self {
            Self::SystemEvents => true,
            Self::Safari => false,
        }
    }
}

/// Ask macOS what it thinks, without letting it ask the person.
pub fn consent(target: AutomationTarget) -> AutomationConsent {
    imp::determine(target, false)
}

/// Let macOS raise its one dialog, now that Juno has explained why.
///
/// Blocks while the dialog is up, so callers run it off the main thread.
pub fn request_consent(target: AutomationTarget) -> AutomationConsent {
    imp::determine(target, true)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{AutomationConsent, AutomationTarget};
    use std::ffi::c_void;
    use tracing::debug;

    /// `typeApplicationBundleID` ('bund'): address a target by bundle id, so
    /// nothing here has to find a pid or a path.
    const TYPE_APPLICATION_BUNDLE_ID: u32 = 0x62756E64;
    /// `typeWildCard` ('****'): ask about the app rather than one event.
    const TYPE_WILD_CARD: u32 = 0x2A2A2A2A;

    /// `AEDesc`: a four-char type tag and an opaque storage handle.
    ///
    /// Both fields exist so the layout matches what AppleEvents expects. Rust
    /// only ever writes them once and hands the struct over, so they read as
    /// dead to the compiler while being the entire point of the type.
    #[repr(C)]
    #[allow(dead_code)]
    struct AEDesc {
        descriptor_type: u32,
        data_handle: *mut c_void,
    }

    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        /// `OSErr AECreateDesc(DescType, const void*, Size, AEDesc*)`
        fn AECreateDesc(
            type_code: u32,
            data_ptr: *const c_void,
            data_size: isize,
            result: *mut AEDesc,
        ) -> i16;
        /// `OSErr AEDisposeDesc(AEDesc*)`
        fn AEDisposeDesc(desc: *mut AEDesc) -> i16;
        /// `OSStatus AEDeterminePermissionToAutomateTarget(const AEAddressDesc*,
        /// AEEventClass, AEEventID, Boolean)`
        fn AEDeterminePermissionToAutomateTarget(
            target: *const AEDesc,
            event_class: u32,
            event_id: u32,
            ask_user_if_needed: u8,
        ) -> i32;
    }

    pub fn determine(target: AutomationTarget, ask_user_if_needed: bool) -> AutomationConsent {
        let bundle_id = target.bundle_id();

        let mut desc = AEDesc {
            descriptor_type: 0,
            data_handle: std::ptr::null_mut(),
        };

        // SAFETY: the bundle id is a `&'static str` that outlives the call,
        // and its length is passed explicitly, so AECreateDesc copies exactly
        // the bytes that exist. `desc` is a live, correctly shaped AEDesc.
        let create_status = unsafe {
            AECreateDesc(
                TYPE_APPLICATION_BUNDLE_ID,
                bundle_id.as_ptr() as *const c_void,
                bundle_id.len() as isize,
                &mut desc,
            )
        };
        if create_status != 0 {
            debug!(
                "Could not address {} for an Automation check (AECreateDesc {})",
                bundle_id, create_status
            );
            return AutomationConsent::Unknown;
        }

        // SAFETY: `desc` was just created successfully and is disposed below
        // on every path. A wildcard class and id ask about the target app
        // rather than about one specific event.
        let status = unsafe {
            AEDeterminePermissionToAutomateTarget(
                &desc,
                TYPE_WILD_CARD,
                TYPE_WILD_CARD,
                u8::from(ask_user_if_needed),
            )
        };

        // SAFETY: disposing the descriptor this function owns, exactly once.
        unsafe {
            AEDisposeDesc(&mut desc);
        }

        let consent = AutomationConsent::from_status(status);
        debug!(
            "Automation consent for {} (asked user: {}): {:?} (OSStatus {})",
            bundle_id, ask_user_if_needed, consent, status
        );
        consent
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::{AutomationConsent, AutomationTarget};

    pub fn determine(_target: AutomationTarget, _ask_user_if_needed: bool) -> AutomationConsent {
        // There is no AppleEvents outside macOS, so there is nothing to gate.
        AutomationConsent::Allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_documented_statuses_each_mean_something_different() {
        assert_eq!(
            AutomationConsent::from_status(codes::NO_ERR),
            AutomationConsent::Allowed
        );
        assert_eq!(
            AutomationConsent::from_status(codes::ERR_AE_EVENT_NOT_PERMITTED),
            AutomationConsent::Refused
        );
        assert_eq!(
            AutomationConsent::from_status(codes::ERR_AE_EVENT_WOULD_REQUIRE_USER_CONSENT),
            AutomationConsent::WouldAsk
        );
        assert_eq!(
            AutomationConsent::from_status(codes::PROC_NOT_FOUND),
            AutomationConsent::TargetNotRunning
        );
    }

    #[test]
    fn an_undocumented_status_claims_nothing() {
        // Guessing from an unknown OSStatus is how a working setup gets told
        // it is broken.
        assert_eq!(
            AutomationConsent::from_status(-25211),
            AutomationConsent::Unknown
        );
    }

    #[test]
    fn only_allowed_lets_juno_proceed() {
        assert!(AutomationConsent::Allowed.is_allowed());
        for other in [
            AutomationConsent::Refused,
            AutomationConsent::WouldAsk,
            AutomationConsent::TargetNotRunning,
            AutomationConsent::Unknown,
        ] {
            assert!(!other.is_allowed(), "{:?} must not pass the gate", other);
        }
    }

    #[test]
    fn every_target_round_trips_through_its_key() {
        for target in [AutomationTarget::SystemEvents, AutomationTarget::Safari] {
            assert_eq!(AutomationTarget::from_key(target.key()), Some(target));
        }
        assert_eq!(AutomationTarget::from_key("finder"), None);
    }

    #[test]
    fn no_target_hands_the_person_a_bundle_id() {
        // The copy a person reads is built from `display_name` and
        // `what_it_unlocks`. Neither may carry a reverse-domain identifier.
        for target in [AutomationTarget::SystemEvents, AutomationTarget::Safari] {
            assert!(!target.display_name().contains("com."));
            assert!(!target.what_it_unlocks().contains("com."));
            assert!(target.bundle_id().starts_with("com.apple."));
        }
    }
}
