//! Which machinery actually carried out an input action.
//!
//! The agent has three ways to reach an app, in descending order of politeness.
//! Only the last one takes the pointer the user is holding, so it is the one the
//! app has to ask about, and the reason this is a type rather than a log string.

use serde::{Deserialize, Serialize};

/// How an input action reached its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputTier {
    /// An accessibility action on the element itself. No cursor, no focus change.
    Accessibility,
    /// A CGEvent posted straight to the target process. The system cursor and the
    /// frontmost application are both untouched.
    ProcessTargeted,
    /// The shared physical cursor. Visible to the user and disruptive, so it is
    /// only ever reached with consent.
    PhysicalCursor,
}

impl InputTier {
    /// Stable identifier for logs and event payloads.
    pub fn as_str(&self) -> &'static str {
        match self {
            InputTier::Accessibility => "accessibility",
            InputTier::ProcessTargeted => "process_targeted",
            InputTier::PhysicalCursor => "physical_cursor",
        }
    }

    /// True when running this tier moves the pointer the user shares with Juno.
    pub fn takes_physical_cursor(&self) -> bool {
        matches!(self, InputTier::PhysicalCursor)
    }
}

/// A completed input action: the tier that carried it plus the concrete
/// mechanism, which differs within a tier (SkyLight vs CGEventPostToPid) and is
/// worth keeping for diagnosis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputOutcome {
    pub tier: InputTier,
    pub method: &'static str,
}

impl InputOutcome {
    pub fn accessibility(method: &'static str) -> Self {
        Self {
            tier: InputTier::Accessibility,
            method,
        }
    }

    pub fn process_targeted(method: &'static str) -> Self {
        Self {
            tier: InputTier::ProcessTargeted,
            method,
        }
    }

    pub fn physical_cursor(method: &'static str) -> Self {
        Self {
            tier: InputTier::PhysicalCursor,
            method,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_physical_tier_takes_the_cursor() {
        assert!(!InputTier::Accessibility.takes_physical_cursor());
        assert!(!InputTier::ProcessTargeted.takes_physical_cursor());
        assert!(InputTier::PhysicalCursor.takes_physical_cursor());
    }

    #[test]
    fn constructors_carry_their_tier() {
        assert_eq!(
            InputOutcome::process_targeted("SkyLight").tier,
            InputTier::ProcessTargeted
        );
        assert_eq!(
            InputOutcome::physical_cursor("HID-with-restore").tier,
            InputTier::PhysicalCursor
        );
        assert_eq!(InputOutcome::accessibility("AXPress").method, "AXPress");
    }

    #[test]
    fn tier_names_are_stable() {
        assert_eq!(InputTier::Accessibility.as_str(), "accessibility");
        assert_eq!(InputTier::ProcessTargeted.as_str(), "process_targeted");
        assert_eq!(InputTier::PhysicalCursor.as_str(), "physical_cursor");
    }
}
