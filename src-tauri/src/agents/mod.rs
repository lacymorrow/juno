pub mod base_agent;
pub mod agent_factory;
pub mod browser_agent;
pub mod desktop_agent;
pub mod system_agent;
pub mod orchestrator;
pub mod session;

// Re-export key types for easier use
pub use base_agent::*;
pub use agent_factory::*;
pub use browser_agent::BrowserAgent;
pub use desktop_agent::DesktopAgent;
pub use system_agent::SystemAgent;
pub use orchestrator::{Orchestrator, OrchestratorConfig};
pub use session::{
    AgentSession, AgentSessionId, AgentSessionInfo, AgentSessionRegistry, AgentSessionStatus,
};
