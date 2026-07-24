pub mod agent_factory;
pub mod base_agent;
pub mod browser_agent;
pub mod desktop_agent;
pub mod orchestrator;
pub mod session;
pub mod system_agent;

// Re-export key types for easier use
pub use agent_factory::*;
pub use base_agent::*;
pub use browser_agent::BrowserAgent;
pub use desktop_agent::DesktopAgent;
pub use orchestrator::{Orchestrator, OrchestratorConfig};
pub use session::{
    broadcast_sessions_updated, color_for_slot, AgentSession, AgentSessionId, AgentSessionInfo,
    AgentSessionRegistry, AgentSessionStatus, SessionHandle, SESSION_COLOR_SLOTS,
};
pub use system_agent::SystemAgent;
