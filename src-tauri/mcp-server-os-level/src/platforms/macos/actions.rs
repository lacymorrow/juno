use std::fmt;

// Enum to represent which click method was used - move to module level
pub(crate) enum ClickMethod {
    AXPress,
    AXClick,
    MouseSimulation,
}

impl fmt::Display for ClickMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClickMethod::AXPress => write!(f, "AXPress"),
            ClickMethod::AXClick => write!(f, "AXClick"),
            ClickMethod::MouseSimulation => write!(f, "MouseSimulation"),
        }
    }
}

// Define enum for click method selection
#[derive(Debug)]
pub(crate) enum ClickMethodSelection {
    /// Try all methods in sequence (current behavior)
    Auto,
    /// Use only AXPress action
    AXPress,
    /// Use only AXClick action
    AXClick,
    /// Use only mouse simulation
    MouseSimulation,
}

impl Default for ClickMethodSelection {
    fn default() -> Self {
        ClickMethodSelection::Auto
    }
}
