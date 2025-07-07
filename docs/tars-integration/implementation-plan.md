# Implementation Plan: TARS → Juno Integration

## Overview

This document outlines the complete 12-week phased implementation plan for integrating TARS's enterprise-grade patterns into Juno's production-ready multimodal AI agent system.

## Implementation Philosophy

### Core Principles
1. **Backward Compatibility**: All existing Juno features must continue working
2. **Incremental Enhancement**: Each phase adds value while maintaining stability
3. **Risk Mitigation**: Feature flags and rollback capabilities at every step
4. **Validation-Driven**: Comprehensive testing before phase progression

### Success Metrics
- **Performance**: Response times within 5% of baseline
- **Reliability**: Error rates remain below current levels
- **Functionality**: All existing features working
- **Extensibility**: New capabilities demonstrably working

## Phase 1: Event-Driven Architecture Foundation (Weeks 1-2)

### Objectives
- Replace direct state mutations with structured event emissions
- Enable real-time UI updates and better debugging capabilities
- Create foundation for all subsequent improvements

### Week 1: Event System Core

#### Day 1-2: Event Type Definitions and Basic Processor
**Files to Create:**
- `src-tauri/src/agent/events/mod.rs`
- `src-tauri/src/agent/events/event_types.rs`
- `src-tauri/src/agent/events/event_processor.rs`

**Key Implementation:**
```rust
// src-tauri/src/agent/events/event_types.rs
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum JunoAgentEvent {
    // Core conversation events
    UserMessage { content: String, timestamp: u64 },
    AssistantMessage { content: String, timestamp: u64 },
    AssistantStreamingMessage { content: String, is_partial: bool },
    
    // Tool execution events
    ToolCall { tool_name: String, args: Value, id: String, timestamp: u64 },
    ToolResult { tool_call_id: String, result: Value, timestamp: u64 },
    
    // Agent lifecycle events
    AgentRunStart { session_id: String, agent_type: String, max_iterations: u32 },
    AgentRunEnd { session_id: String, status: String, iterations: u32, elapsed_ms: u64 },
    
    // Voice system events
    VoiceTranscriptionStart { session_id: String },
    VoiceTranscriptionChunk { content: String, is_final: bool },
    VoiceTranscriptionEnd { session_id: String, final_text: String },
    
    // System events
    SystemMessage { level: String, message: String, timestamp: u64 },
    PermissionRequest { permission_type: String, status: String },
    ErrorOccurred { error_type: String, message: String, recoverable: bool },
}

// src-tauri/src/agent/events/event_processor.rs
pub struct JunoEventStreamProcessor {
    events: Arc<RwLock<Vec<JunoAgentEvent>>>,
    subscribers: Arc<RwLock<Vec<Box<dyn EventSubscriber + Send + Sync>>>>,
    app_handle: AppHandle,
}

impl JunoEventStreamProcessor {
    pub async fn send_event(&self, event: JunoAgentEvent) -> Result<(), String> {
        // Add to event log
        {
            let mut events = self.events.write().await;
            events.push(event.clone());
        }
        
        // Notify subscribers
        {
            let subscribers = self.subscribers.read().await;
            for subscriber in subscribers.iter() {
                if let Err(e) = subscriber.on_event(&event).await {
                    tracing::warn!("Event subscriber error: {}", e);
                }
            }
        }
        
        // Emit to frontend
        self.app_handle.emit("agent-event", &event)
            .map_err(|e| format!("Failed to emit event: {}", e))?;
        
        Ok(())
    }
}
```

#### Day 3-4: AppState Integration
**Files to Modify:**
- `src-tauri/src/state.rs`

**Key Changes:**
```rust
// Add to AppState
pub struct AppState {
    // Existing fields...
    pub event_processor: Arc<TokioMutex<JunoEventStreamProcessor>>,
}

impl AppState {
    pub async fn emit_agent_event(&self, event: JunoAgentEvent) -> Result<(), String> {
        let processor = self.event_processor.lock().await;
        processor.send_event(event).await
    }
    
    pub async fn get_event_processor(&self) -> Arc<TokioMutex<JunoEventStreamProcessor>> {
        self.event_processor.clone()
    }
}
```

#### Day 5: Update Main Orchestrator
**Files to Modify:**
- `src-tauri/src/anthropic.rs`

**Key Changes:**
```rust
// Replace direct streaming with event emission in execute_agent_internal
async fn execute_agent_internal(
    query: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let execution_id = uuid::Uuid::new_v4().to_string();
    
    // Emit agent run start event
    state.emit_agent_event(JunoAgentEvent::AgentRunStart {
        session_id: execution_id.clone(),
        agent_type: "orchestrator".to_string(),
        max_iterations: agent::config::MAX_ITERATIONS,
    }).await?;
    
    // ... existing logic ...
    
    // Replace final response handling with events
    match agent_result {
        Ok(message) => {
            state.emit_agent_event(JunoAgentEvent::AssistantMessage {
                content: message.clone(),
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
            }).await?;
            
            state.emit_agent_event(JunoAgentEvent::AgentRunEnd {
                session_id: execution_id,
                status: "completed".to_string(),
                iterations: current_iteration,
                elapsed_ms: start_time.elapsed().as_millis() as u64,
            }).await?;
        }
        Err(e) => {
            state.emit_agent_event(JunoAgentEvent::ErrorOccurred {
                error_type: "agent_execution".to_string(),
                message: e.to_string(),
                recoverable: matches!(e, AgentError::Terminated),
            }).await?;
        }
    }
    
    Ok(())
}
```

#### Day 6-7: Frontend Integration and Testing
**Files to Modify:**
- Frontend event handling components

### Week 2: Event Stream Integration

#### Day 1-3: Voice System Event Integration
**Files to Modify:**
- Voice transcription components
- TTS system

#### Day 4-5: Tool Execution Event Integration
**Files to Modify:**
- `src-tauri/src/agent/implementations/tool_provider.rs`

#### Day 6-7: Comprehensive Testing
- Event flow validation
- Performance impact assessment
- Regression testing

### Phase 1 Deliverables
- ✅ Real-time event-driven UI updates
- ✅ Comprehensive event logging for debugging
- ✅ Foundation for streaming improvements
- ✅ Backward compatibility maintained

## Phase 2: Tool System Modernization (Weeks 3-4)

### Objectives
- Implement multiple tool call strategies for different LLM providers
- Enhance existing reliability patterns with strategic flexibility
- Maintain Juno's error recovery while adding TARS's modularity

### Week 3: Tool Call Engines

#### Day 1-2: Engine Abstraction and Native Engine
**Files to Create:**
- `src-tauri/src/agent/tools/engines/mod.rs`
- `src-tauri/src/agent/tools/engines/native.rs`
- `src-tauri/src/agent/tools/engines/prompt_engineering.rs`
- `src-tauri/src/agent/tools/engines/structured_outputs.rs`

**Key Implementation:**
```rust
// src-tauri/src/agent/tools/engines/mod.rs
pub trait ToolCallEngine: Send + Sync {
    async fn prepare_tools_for_llm(&self, tools: &[ToolDefinition]) -> Result<Value, String>;
    async fn extract_tool_calls(&self, response: &str) -> Result<Vec<ToolCall>, String>;
    fn get_engine_type(&self) -> ToolCallEngineType;
    fn supports_provider(&self, provider: &str) -> bool;
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolCallEngineType {
    Native,           // OpenAI-style function calling
    PromptEngineering, // JSON-based tool execution via prompts
    StructuredOutputs, // Anthropic-style structured outputs
}

pub fn get_engine_for_provider(provider: &str) -> Box<dyn ToolCallEngine> {
    match provider.to_lowercase().as_str() {
        "anthropic" => Box::new(StructuredOutputsEngine::new()),
        "openai" | "azure-openai" => Box::new(NativeToolCallEngine::new()),
        _ => Box::new(PromptEngineeringEngine::new()),
    }
}
```

#### Day 3-4: Engine Implementations
**Individual engine implementations with provider-specific optimizations**

#### Day 5: Engine Selection and Provider Mapping
**Integration with model resolution system**

#### Day 6-7: LocalToolProvider Integration
**Files to Modify:**
- `src-tauri/src/agent/implementations/tool_provider.rs`

**Key Changes:**
```rust
impl LocalToolProvider {
    pub async fn execute_with_engine(
        &self,
        tool_call: ToolCall,
        engine: &dyn ToolCallEngine,
    ) -> Result<ToolResult, AgentError> {
        // Emit tool call start event
        let state = self.app_handle.state::<AppState>();
        state.emit_agent_event(JunoAgentEvent::ToolCall {
            tool_name: tool_call.name.clone(),
            args: tool_call.arguments.clone(),
            id: tool_call.id.clone(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }).await.map_err(|e| AgentError::ToolExecutionError(e))?;
        
        // Execute with existing reliability patterns + new engine support
        let result = self.execute_tool_with_recovery_and_engine(tool_call.clone(), engine).await?;
        
        // Emit tool result event
        state.emit_agent_event(JunoAgentEvent::ToolResult {
            tool_call_id: tool_call.id,
            result: result.output.clone(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }).await.map_err(|e| AgentError::ToolExecutionError(e))?;
        
        Ok(result)
    }
    
    async fn execute_tool_with_recovery_and_engine(
        &self,
        tool_call: ToolCall,
        engine: &dyn ToolCallEngine,
    ) -> Result<ToolResult, AgentError> {
        // Combine TARS engine strategy with Juno reliability patterns
        let mut retries = 0;
        let max_retries = self.config.max_retries;
        
        while retries <= max_retries {
            // Circuit breaker check (Juno pattern)
            if self.circuit_breaker.is_open() {
                return Err(AgentError::CircuitBreakerOpen);
            }
            
            // Execute with engine-specific handling (TARS pattern)
            match self.execute_tool_with_engine_direct(tool_call.clone(), engine).await {
                Ok(tool_result) => {
                    self.circuit_breaker.record_success();
                    return Ok(tool_result);
                }
                Err(error) => {
                    let error_type = self.classify_error(&error);
                    
                    if error_type.should_retry() && retries < max_retries {
                        let delay = error_type.retry_delay(retries);
                        tokio::time::sleep(delay).await;
                        retries += 1;
                        
                        self.recovery_stats.lock().await.record_retry(error_type);
                    } else {
                        self.circuit_breaker.record_failure();
                        return Err(error);
                    }
                }
            }
        }
        
        Err(AgentError::MaxRetriesExceeded)
    }
}
```

### Week 4: Enhanced Tool Execution

#### Day 1-3: Agent Integration with Tool Engines
**Files to Modify:**
- `src-tauri/src/agent/implementations/agent_runner.rs`
- Model provider integration

#### Day 4-5: Performance Optimization
- Tool execution batching
- Engine-specific optimizations

#### Day 6-7: Testing and Validation
- Cross-provider testing
- Performance benchmarking

### Phase 2 Deliverables
- ✅ Multiple tool call strategies working
- ✅ Provider-specific optimizations
- ✅ Maintained reliability patterns
- ✅ Improved tool execution performance

## Phase 3: Memory System Integration (Weeks 5-6)

### Objectives
- Create hybrid memory management combining TARS's event streams with Juno's token optimization
- Maintain conversation continuity while adding event-driven capabilities
- Enhance debugging and introspection capabilities

### Week 5: Hybrid Memory Architecture

#### Day 1-3: HybridMemoryManager Implementation
**Files to Create:**
- `src-tauri/src/agent/memory/hybrid_manager.rs`
- `src-tauri/src/agent/memory/event_converter.rs`

**Key Implementation:**
```rust
// src-tauri/src/agent/memory/hybrid_manager.rs
pub struct HybridMemoryManager {
    // Event stream processing (TARS-inspired)
    event_processor: Arc<TokioMutex<JunoEventStreamProcessor>>,
    
    // Advanced features from Juno
    token_estimator: TokenEstimator,
    visual_compressor: VisualContextCompressor,
    prune_config: PruneConfig,
    
    // Compatibility with existing interface
    messages: Arc<RwLock<Vec<Message>>>,
    pending_tool_calls: Arc<RwLock<HashSet<String>>>,
}

impl MemoryManager for HybridMemoryManager {
    async fn add_message(&mut self, message: Message) -> Result<(), AgentError> {
        // Convert message to event
        let event = self.message_to_event(&message)?;
        
        // Add to event stream (TARS pattern)
        let mut processor = self.event_processor.lock().await;
        processor.send_event(event).await
            .map_err(|e| AgentError::MemoryError(e))?;
        
        // Also maintain legacy message format for compatibility
        {
            let mut messages = self.messages.write().await;
            messages.push(message);
        }
        
        // Apply Juno's token-aware pruning
        self.prune_if_over_limit().await?;
        
        Ok(())
    }
    
    async fn get_conversation_history(&self) -> Result<Vec<Message>, AgentError> {
        // Option 1: Return from legacy messages (fast)
        if self.should_use_legacy_path() {
            let messages = self.messages.read().await;
            return Ok(messages.clone());
        }
        
        // Option 2: Convert from event stream (flexible)
        let processor = self.event_processor.lock().await;
        let events = processor.get_events().await;
        
        let mut messages = Vec::new();
        for event in events {
            if let Some(message) = self.event_to_message(&event).await? {
                messages.push(message);
            }
        }
        
        Ok(messages)
    }
    
    async fn prune_if_over_limit(&self) -> Result<bool, AgentError> {
        // Use Juno's advanced token estimation
        let estimated_tokens = self.estimate_total_tokens().await?;
        
        if estimated_tokens >= self.prune_config.emergency_threshold {
            // Emergency pruning with visual compression
            let compressed_count = self.compress_visual_context().await?;
            info!("Compressed {} visual elements before pruning", compressed_count);
            
            // Prune both event stream and legacy messages
            self.prune_event_stream().await?;
            self.prune_legacy_messages().await?;
            
            return Ok(true);
        }
        
        Ok(false)
    }
}
```

#### Day 4-5: Event-to-Message Conversion
**Bidirectional conversion between event streams and message formats**

#### Day 6-7: Token Estimation Integration
**Combine Juno's token awareness with event processing**

### Week 6: Memory System Integration

#### Day 1-3: Replace Memory Manager Usage
**Files to Modify:**
- Agent implementations
- State management
- Tool execution contexts

#### Day 4-5: Conversation History Persistence
**Enhanced persistence with event replay capabilities**

#### Day 6-7: Performance Optimization
- Memory access patterns
- Event stream pruning strategies

### Phase 3 Deliverables
- ✅ Hybrid memory management working
- ✅ Event-driven conversation history
- ✅ Maintained token optimization
- ✅ Enhanced debugging capabilities

## Phase 4: Agent Architecture Enhancement (Weeks 7-8)

### Objectives
- Refactor agent execution to use specialized processors
- Implement dynamic model resolution with agent-specific preferences
- Enhance multi-agent coordination

### Week 7: Agent Processors

#### Day 1-3: Specialized Processor Components
**Files to Create:**
- `src-tauri/src/agent/processors/mod.rs`
- `src-tauri/src/agent/processors/llm_processor.rs`
- `src-tauri/src/agent/processors/tool_processor.rs`
- `src-tauri/src/agent/processors/loop_executor.rs`

#### Day 4-5: Agent Execution Refactoring
**Files to Modify:**
- `src-tauri/src/agent/implementations/agent_runner.rs`

#### Day 6-7: Multi-Agent Integration
**Enhanced orchestrator-specialist coordination**

### Week 8: Model Provider System

#### Day 1-3: Dynamic Model Resolution
**Files to Create:**
- `src-tauri/src/agent/providers/model_resolver.rs`

**Key Implementation:**
```rust
pub struct ModelResolver {
    providers: HashMap<String, ModelProvider>,
    default_selection: ModelSelection,
    agent_specific_preferences: HashMap<AgentType, ModelSelection>,
}

impl ModelResolver {
    pub fn resolve_for_agent(
        &self,
        agent_type: AgentType,
        runtime_model: Option<&str>,
        runtime_provider: Option<&str>,
    ) -> Result<ResolvedModel, String> {
        // Priority: runtime > agent-specific > default
        let provider = runtime_provider
            .or_else(|| {
                self.agent_specific_preferences
                    .get(&agent_type)?
                    .provider
                    .as_deref()
            })
            .unwrap_or(&self.default_selection.provider);
            
        let model = runtime_model
            .or_else(|| {
                self.agent_specific_preferences
                    .get(&agent_type)?
                    .model
                    .as_deref()
            })
            .unwrap_or(&self.default_selection.model);
            
        Ok(ResolvedModel {
            provider: provider.to_string(),
            model: model.to_string(),
            agent_type: Some(agent_type),
        })
    }
}
```

#### Day 4-5: Agent-Specific Model Preferences
**Optimize model selection for different agent types**

#### Day 6-7: Provider Optimization and Testing
**Performance testing across different model providers**

### Phase 4 Deliverables
- ✅ Specialized agent processors working
- ✅ Dynamic model resolution
- ✅ Agent-specific optimizations
- ✅ Enhanced multi-agent coordination

## Phase 5: MCP Integration (Weeks 9-10)

### Objectives
- Implement MCP protocol support alongside existing tools
- Enable third-party tool ecosystem
- Maintain compatibility with existing LocalToolProvider

### Week 9: MCP Foundation

#### Day 1-3: MCP Client and Transport Abstractions
**Files to Create:**
- `src-tauri/src/agent/mcp/mod.rs`
- `src-tauri/src/agent/mcp/client.rs`
- `src-tauri/src/agent/mcp/transport.rs`

**Key Implementation:**
```rust
// src-tauri/src/agent/mcp/client.rs
pub struct McpClient {
    transport: Box<dyn McpTransport>,
    server_config: McpServerConfig,
    tools: Vec<McpTool>,
}

impl McpClient {
    pub async fn mount_server(&mut self, config: McpServerConfig) -> Result<(), String> {
        // Choose transport based on configuration
        self.transport = if config.is_builtin {
            Box::new(InMemoryTransport::new(config.handler))
        } else {
            Box::new(StdioTransport::new(config.command, config.args))
        };
        
        // Initialize connection
        self.transport.initialize().await?;
        
        // Discover available tools
        self.tools = self.transport.discover_tools().await?;
        
        info!("Mounted MCP server '{}' with {} tools", config.name, self.tools.len());
        Ok(())
    }
    
    pub async fn execute_tool(&self, tool_name: &str, args: Value) -> Result<Value, String> {
        let tool = self.tools.iter()
            .find(|t| t.name == tool_name)
            .ok_or_else(|| format!("Tool '{}' not found", tool_name))?;
            
        self.transport.call_tool(tool, args).await
    }
}
```

#### Day 4-5: In-Memory Transport for Built-in Services
**Efficient transport for internal MCP servers**

#### Day 6-7: Stdio Transport for External Servers
**Process management for external MCP servers**

### Week 10: MCP Integration

#### Day 1-3: Bridge MCP Tools to LocalToolProvider
**Files to Modify:**
- `src-tauri/src/agent/implementations/tool_provider.rs`

**Key Changes:**
```rust
impl LocalToolProvider {
    pub async fn mount_mcp_server(&mut self, config: McpServerConfig) -> Result<(), String> {
        let mut mcp_client = McpClient::new();
        mcp_client.mount_server(config.clone()).await?;
        
        // Bridge MCP tools to our existing tool system
        for mcp_tool in mcp_client.get_tools() {
            let tool_def = ToolDefinition {
                name: mcp_tool.name.clone(),
                description: mcp_tool.description.clone(),
                input_schema: mcp_tool.input_schema.clone(),
                api_type: Some("mcp".to_string()),
                beta_flag: None,
            };
            
            let mcp_client_arc = Arc::new(mcp_client.clone());
            let tool_name = mcp_tool.name.clone();
            
            let executor = move |input: Value| {
                let client = mcp_client_arc.clone();
                let name = tool_name.clone();
                async move {
                    client.execute_tool(&name, input).await
                }
            };
            
            self.register_async_tool(tool_def, executor).await;
            info!("Bridged MCP tool to LocalToolProvider: {}", mcp_tool.name);
        }
        
        Ok(())
    }
}
```

#### Day 4-5: Dynamic Server Mounting and Tool Discovery
**Runtime MCP server management**

#### Day 6-7: Documentation and Examples
**MCP integration guides and examples**

### Phase 5 Deliverables
- ✅ MCP protocol support working
- ✅ External MCP servers integrated
- ✅ Existing tool compatibility maintained
- ✅ Third-party tool ecosystem enabled

## Phase 6: Cross-Platform Foundation (Weeks 11-12)

### Objectives
- Create platform abstraction layer
- Extract macOS-specific code to provider pattern
- Establish foundation for Windows/Linux support

### Week 11: Platform Abstraction

#### Day 1-3: Platform Provider Interfaces
**Files to Create:**
- `src-tauri/src/platforms/mod.rs`
- `src-tauri/src/platforms/provider.rs`
- `src-tauri/src/platforms/macos/mod.rs`

**Key Implementation:**
```rust
// src-tauri/src/platforms/provider.rs
#[async_trait]
pub trait PlatformProvider: Send + Sync {
    async fn capture_screenshot(&self) -> Result<Vec<u8>, String>;
    async fn click_at_position(&self, x: i32, y: i32) -> Result<(), String>;
    async fn type_text(&self, text: &str) -> Result<(), String>;
    async fn get_window_list(&self) -> Result<Vec<Window>, String>;
    async fn get_accessibility_permissions(&self) -> Result<PermissionStatus, String>;
    
    // Platform-specific capabilities
    fn get_platform_name(&self) -> &'static str;
    fn supports_accessibility_api(&self) -> bool;
    fn supports_screen_recording(&self) -> bool;
}

// Platform factory
pub fn create_platform_provider() -> Result<Box<dyn PlatformProvider>, String> {
    #[cfg(target_os = "macos")]
    return Ok(Box::new(MacOSProvider::new()?));
    
    #[cfg(target_os = "windows")]
    return Ok(Box::new(WindowsProvider::new()?));
    
    #[cfg(target_os = "linux")]
    return Ok(Box::new(LinuxProvider::new()?));
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    return Err("Unsupported platform".to_string());
}
```

#### Day 4-5: Extract macOS Code to Provider
**Refactor existing macOS implementations**

#### Day 6-7: Windows Provider Skeleton
**Basic Windows provider structure**

### Week 12: Platform Integration

#### Day 1-3: Platform-Specific Tool Implementations
**Update desktop tools to use platform providers**

#### Day 4-5: Cross-Platform Testing Infrastructure
**Testing framework for multiple platforms**

#### Day 6-7: Documentation and Deployment Guides
**Platform-specific setup and deployment**

### Phase 6 Deliverables
- ✅ Platform abstraction layer working
- ✅ macOS code properly abstracted
- ✅ Windows provider foundation
- ✅ Cross-platform tool execution

## Risk Mitigation and Rollback Strategies

### Feature Flags
Every phase introduces functionality behind feature flags:
```rust
// Feature flag configuration
pub struct FeatureFlags {
    pub event_driven_architecture: bool,
    pub multiple_tool_engines: bool,
    pub hybrid_memory_management: bool,
    pub agent_processors: bool,
    pub mcp_integration: bool,
    pub cross_platform_providers: bool,
}
```

### Rollback Capabilities
Each phase maintains backward compatibility:
- **Phase 1**: Event system can be disabled, falling back to direct UI updates
- **Phase 2**: Tool engines can fall back to original LocalToolProvider
- **Phase 3**: Memory system can use legacy message storage
- **Phase 4**: Agent processors can fall back to original agent runner
- **Phase 5**: MCP integration is purely additive
- **Phase 6**: Platform providers fall back to existing macOS code

### Validation Checkpoints
Each week includes comprehensive validation:
- **Functionality**: All existing features working
- **Performance**: Response times within acceptable range
- **Reliability**: Error rates below baseline
- **Integration**: New features working as designed

### Success Metrics
- **Phase 1**: Event system working without performance regression
- **Phase 2**: Tool system performance matches or exceeds baseline
- **Phase 3**: Memory system maintains conversation quality
- **Phase 4**: Agent responses equivalent to current quality
- **Phase 5**: MCP integration doesn't impact existing tools
- **Phase 6**: Cross-platform foundation ready for expansion

## Final Validation and Deployment

### Pre-Production Testing (Week 13)
- Comprehensive integration testing
- Performance benchmarking
- User acceptance testing
- Security validation

### Production Rollout Strategy
1. **Alpha Release**: Internal testing with all features enabled
2. **Beta Release**: Limited user group with feature flags
3. **Gradual Rollout**: Phased activation of new features
4. **Full Production**: All features enabled for all users

This implementation plan ensures a smooth, risk-mitigated transition that enhances Juno's capabilities while maintaining its production-ready reliability and performance characteristics.