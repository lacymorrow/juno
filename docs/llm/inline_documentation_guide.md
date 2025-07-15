# 📝 Inline Documentation Guide for LLM Comprehension

## Purpose

This guide establishes patterns for inline code documentation that optimizes LLM understanding and navigation.

## 🎯 Documentation Principles

### 1. **Context First**
Always provide context at the beginning of files and functions.

### 2. **Purpose Over Implementation**
Explain WHY, not just WHAT.

### 3. **LLM Navigation Hints**
Add breadcrumbs for related code.

## 📋 File Header Pattern

```rust
//! # Module Name
//! 
//! Purpose: Brief description of what this module does
//! 
//! ## Key Components
//! - `MainStruct` - Primary data structure
//! - `process_data()` - Main processing function
//! 
//! ## Related Files
//! - `agent/core.rs` - Uses this module for X
//! - `commands/registry.rs` - Registers commands from here
//! 
//! ## Usage Example
//! ```rust
//! let result = module::process_data(input)?;
//! ```

use crate::prelude::*;
```

## 🔧 Function Documentation Pattern

```rust
/// Processes user input and delegates to appropriate agent.
/// 
/// # Purpose
/// Central entry point for all AI agent interactions. Handles task
/// delegation based on input type and current system state.
/// 
/// # Arguments
/// * `input` - User query or command
/// * `state` - Current application state
/// 
/// # Returns
/// * `Ok(Response)` - Processed response from agent
/// * `Err(AgentError)` - Various error conditions
/// 
/// # Related Functions
/// - See `delegate_to_browser_agent()` for web tasks
/// - See `delegate_to_desktop_agent()` for UI automation
/// 
/// # Example
/// ```rust
/// let response = process_agent_input("Take a screenshot", &state).await?;
/// ```
pub async fn process_agent_input(
    input: &str,
    state: &AppState
) -> Result<Response, AgentError> {
    // Implementation
}
```

## 🏗️ Struct Documentation Pattern

```rust
/// Manages the lifecycle and state of AI agents.
/// 
/// # Purpose
/// Coordinates multiple specialized agents, maintains conversation
/// history, and handles tool delegation.
/// 
/// # Key Fields
/// - `memory_manager` - Stores conversation history
/// - `active_agents` - Currently running agents
/// - `tool_provider` - Available tools for agents
/// 
/// # Usage Context
/// Created once per application instance in `main.rs`.
/// Accessed via `AppState` throughout the application.
/// 
/// # See Also
/// - `Agent` trait in `traits.rs`
/// - Tool definitions in `tools/mod.rs`
#[derive(Clone)]
pub struct AgentOrchestrator {
    /// Manages conversation memory with token limits
    pub memory_manager: Arc<Mutex<MemoryManager>>,
    
    /// Active agent instances indexed by ID
    pub active_agents: HashMap<String, Box<dyn Agent>>,
    
    /// Provides tools to agents
    pub tool_provider: Arc<ToolProvider>,
}
```

## 🎨 Complex Logic Documentation

```rust
impl AgentOrchestrator {
    /// Determines the best agent for a given task.
    /// 
    /// # Decision Flow
    /// 1. Parse input for keywords and intent
    /// 2. Check for explicit agent selection (e.g., "browser:")
    /// 3. Analyze task requirements:
    ///    - Web URLs → Browser Agent
    ///    - File paths → File Agent  
    ///    - UI elements → Desktop Agent
    /// 4. Default to Orchestrator for ambiguous tasks
    /// 
    /// # Why This Matters
    /// Proper agent selection improves:
    /// - Task success rate by 40%
    /// - Response time by avoiding delegation chains
    /// - Resource usage by using specialized agents
    fn select_agent(&self, input: &str) -> AgentType {
        // Quick keyword check for explicit selection
        if input.starts_with("browser:") {
            return AgentType::Browser;
        }
        
        // URL pattern indicates web task
        if URL_REGEX.is_match(input) {
            return AgentType::Browser;
        }
        
        // File operations
        if input.contains("file") || input.contains("edit") {
            return AgentType::File;
        }
        
        // Default to orchestrator for complex tasks
        AgentType::Orchestrator
    }
}
```

## 📊 Error Handling Documentation

```rust
/// Handles agent execution with comprehensive error recovery.
/// 
/// # Error Handling Strategy
/// 1. **Timeout** (30s) - Prevents infinite loops
/// 2. **Retry Logic** - 3 attempts for transient failures
/// 3. **Fallback** - Degrades to simpler agent if specialized fails
/// 4. **User Feedback** - Clear error messages for common issues
/// 
/// # Common Failure Modes
/// - API rate limits → Wait and retry
/// - Network timeout → Immediate retry with backoff
/// - Invalid tool use → Re-prompt with guidance
/// - Memory overflow → Clear old conversations
/// 
/// # Recovery Actions
/// See `recover_from_error()` for specific strategies
pub async fn execute_with_recovery(
    &mut self,
    task: Task
) -> Result<Response, AgentError> {
    let mut attempts = 0;
    const MAX_ATTEMPTS: u32 = 3;
    
    loop {
        match self.execute_internal(task.clone()).await {
            Ok(response) => return Ok(response),
            Err(e) if attempts < MAX_ATTEMPTS => {
                attempts += 1;
                
                // Log attempt for debugging
                tracing::warn!(
                    attempt = attempts,
                    error = ?e,
                    "Retrying after error"
                );
                
                // Apply recovery strategy
                self.recover_from_error(&e).await?;
                
                // Exponential backoff
                tokio::time::sleep(
                    Duration::from_secs(2_u64.pow(attempts))
                ).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

## 🔄 State Management Documentation

```rust
/// Application-wide state container.
/// 
/// # Thread Safety
/// All fields use `Arc<Mutex<T>>` for safe concurrent access.
/// Always acquire locks in consistent order to prevent deadlocks:
/// 1. config
/// 2. memory_manager  
/// 3. active_agents
/// 
/// # Initialization
/// Created once in `main()` and cloned for each Tauri command.
/// 
/// # Memory Considerations
/// - `memory_manager` can grow large - monitor size
/// - `browser_controller` is expensive - lazy initialize
/// - `active_agents` should be cleaned up after use
pub struct AppState {
    /// User configuration - loaded from store
    pub config: Arc<Mutex<Config>>,
    
    /// Shared memory across all agents
    pub memory_manager: Arc<Mutex<MemoryManager>>,
    
    /// Currently active agents (clean up after use!)
    pub active_agents: Arc<Mutex<Vec<String>>>,
    
    /// Expensive browser instance (lazy init)
    pub browser_controller: Arc<Mutex<Option<BrowserController>>>,
}
```

## 📌 Navigation Comments

Add navigation hints throughout the codebase:

```rust
// ===== AGENT REGISTRATION =====
// New agents must be registered here and in:
// - `agent_factory.rs` for instantiation
// - `registry.rs` for command exposure
// - `types.rs` for AgentType enum

// ===== TOOL IMPLEMENTATION =====
// To add new tools:
// 1. Define in `tools/your_tool.rs`
// 2. Register in agent's `register_tools()`
// 3. Add to `ToolType` enum
// 4. Document in `API.md`

// ===== ERROR HANDLING =====
// This follows the pattern established in `error_handling.rs`
// All errors must eventually map to `AgentError` for consistency
```

## 🎯 Best Practices

### DO ✅
- Start files with module-level documentation
- Explain complex algorithms before implementation
- Link related code with "See also" comments
- Document error conditions and recovery
- Add usage examples for public APIs
- Use consistent terminology across codebase

### DON'T ❌
- Repeat obvious code in comments
- Use vague descriptions like "processes data"
- Document private implementation details
- Write novels - be concise but complete
- Assume context - provide it explicitly

## 📝 Documentation Templates

### Quick Function Template
```rust
/// Brief description of function purpose.
/// 
/// # Related
/// - See `related_function()` for similar functionality
pub fn function_name(param: Type) -> Result<Output, Error> {
    // Implementation
}
```

### Complex System Template
```rust
/// System/Component name and primary purpose.
/// 
/// # Architecture
/// Brief overview of how this fits in the system.
/// 
/// # Key Concepts
/// - Concept 1: Brief explanation
/// - Concept 2: Brief explanation
/// 
/// # Usage Flow
/// 1. Step one
/// 2. Step two
/// 3. Step three
/// 
/// # Performance Considerations
/// Any important performance notes.
/// 
/// # See Also
/// - Related module 1
/// - Related module 2
```

## 🔍 LLM-Friendly Patterns

1. **Use Descriptive Names**
   ```rust
   // Good
   pub fn validate_and_sanitize_user_input()
   
   // Bad  
   pub fn proc_input()
   ```

2. **Group Related Code**
   ```rust
   // ===== INITIALIZATION =====
   
   // ===== PROCESSING =====
   
   // ===== CLEANUP =====
   ```

3. **Explain Magic Numbers**
   ```rust
   // Maximum attempts before considering agent unresponsive
   const MAX_RETRY_ATTEMPTS: u32 = 3;
   
   // Token limit for Claude 3.5 Sonnet
   const MAX_CONTEXT_TOKENS: usize = 200_000;
   ```

4. **Document Invariants**
   ```rust
   /// INVARIANT: This vec must never be empty.
   /// Initial element added in new(), panic if violated.
   agents: Vec<Agent>,
   ```

---

*Following these patterns will significantly improve LLM comprehension and navigation of the codebase.*