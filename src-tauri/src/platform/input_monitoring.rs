//! # Input Monitoring (TCC `ListenEvent`) via IOKit
//!
//! The only honest way to ask macOS whether this process may observe keyboard
//! and mouse events system-wide is `IOHIDCheckAccess`, and the only way to make
//! macOS *list* the app in System Settings > Privacy & Security > Input
//! Monitoring is `IOHIDRequestAccess`. Until an app has called the request
//! function (or created an event tap), it has no row in that pane at all, so
//! nothing the user or an automation can toggle exists yet.
//!
//! Everything else that used to stand in for this check was wrong:
//! * reading the user TCC database matches stale rows from old builds and the
//!   `ListenEvent` grant lives in the system database anyway, and
//! * `tell application "System Events"` tests the Automation permission, not
//!   Input Monitoring.
//!
//! Both IOKit calls are available from macOS 10.15; the app's minimum system
//! version is 12.3, so no availability guard is needed. This is the single
//! home for the FFI; everything else goes through the safe wrappers below.

/// What macOS says about this process's Input Monitoring grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMonitoringAccess {
    /// The user switched Juno on in Input Monitoring.
    Granted,
    /// Juno is listed and switched off.
    Denied,
    /// Juno has never asked, so there is no row yet.
    Unknown,
}

impl InputMonitoringAccess {
    /// Map the raw `IOHIDAccessType` value. Anything outside the documented
    /// range is treated as `Unknown`, which callers already handle as "not
    /// granted".
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            imp::K_IOHID_ACCESS_TYPE_GRANTED => Self::Granted,
            imp::K_IOHID_ACCESS_TYPE_DENIED => Self::Denied,
            _ => Self::Unknown,
        }
    }

    pub fn is_granted(self) -> bool {
        self == Self::Granted
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::InputMonitoringAccess;
    use tracing::debug;

    /// `kIOHIDRequestTypeListenEvent`: observe events (Input Monitoring).
    const K_IOHID_REQUEST_TYPE_LISTEN_EVENT: u32 = 1;
    /// `kIOHIDAccessTypeGranted`
    pub const K_IOHID_ACCESS_TYPE_GRANTED: u32 = 0;
    /// `kIOHIDAccessTypeDenied`
    pub const K_IOHID_ACCESS_TYPE_DENIED: u32 = 1;

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        /// `IOHIDAccessType IOHIDCheckAccess(IOHIDRequestType requestType)`
        fn IOHIDCheckAccess(request_type: u32) -> u32;
        /// `bool IOHIDRequestAccess(IOHIDRequestType requestType)`
        fn IOHIDRequestAccess(request_type: u32) -> bool;
    }

    pub fn check() -> InputMonitoringAccess {
        // SAFETY: plain C call with a documented integer argument and no
        // pointers; it has no preconditions beyond running on macOS 10.15+.
        let raw = unsafe { IOHIDCheckAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT) };
        let access = InputMonitoringAccess::from_raw(raw);
        debug!("Input Monitoring access (IOHIDCheckAccess): {:?}", access);
        access
    }

    pub fn request() -> bool {
        // SAFETY: as above. The call may show the system consent alert and
        // registers this app's row in the Input Monitoring pane; it returns
        // whether access is currently granted.
        unsafe { IOHIDRequestAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT) }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::InputMonitoringAccess;

    pub const K_IOHID_ACCESS_TYPE_GRANTED: u32 = 0;
    pub const K_IOHID_ACCESS_TYPE_DENIED: u32 = 1;

    pub fn check() -> InputMonitoringAccess {
        InputMonitoringAccess::Granted
    }

    pub fn request() -> bool {
        true
    }
}

/// Ask macOS whether this process may observe input system-wide. Never
/// prompts and never creates a row in System Settings.
pub fn check_input_monitoring_access() -> InputMonitoringAccess {
    imp::check()
}

/// Register this app in the Input Monitoring pane and, on first call, show
/// the system consent alert. Returns whether access is granted right now.
/// Call this before driving the pane: without it there is no row to switch.
pub fn request_input_monitoring_access() -> bool {
    imp::request()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_access_values_map_to_documented_variants() {
        assert_eq!(
            InputMonitoringAccess::from_raw(0),
            InputMonitoringAccess::Granted
        );
        assert_eq!(
            InputMonitoringAccess::from_raw(1),
            InputMonitoringAccess::Denied
        );
        assert_eq!(
            InputMonitoringAccess::from_raw(2),
            InputMonitoringAccess::Unknown
        );
    }

    #[test]
    fn out_of_range_values_are_unknown_not_granted() {
        for raw in [3u32, 7, u32::MAX] {
            let access = InputMonitoringAccess::from_raw(raw);
            assert_eq!(access, InputMonitoringAccess::Unknown);
            assert!(!access.is_granted());
        }
    }

    #[test]
    fn only_granted_counts_as_granted() {
        assert!(InputMonitoringAccess::Granted.is_granted());
        assert!(!InputMonitoringAccess::Denied.is_granted());
        assert!(!InputMonitoringAccess::Unknown.is_granted());
    }
}
