# Agent System

## Overview

The agent system provides AI-driven automation through a structured framework with providers, tools, and execution management.

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  Agent Runner   │    │   Tool Provider │    │   AI Provider   │
│  - Execution    │◄──►│   - Registry    │    │   - Brain       │
│  - Memory       │    │   - Executors   │    │   - Models      │
│  - Iterations   │    │   - Definitions │    │   - API Calls   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Components

### 1. Agent Brain (AI Providers)

**Location**: `src-tauri/src/agent/providers/`

**Supported Providers**:
- **Anthropic** - Primary provider with Claude models
- **OpenAI** - GPT models with tool calling
- **Google Gemini** - Multimodal capabilities
- **Local/Ollama** - Local model support

**Factory Pattern**:
```rust
// BrainFactory creates appropriate provider
let brain = BrainFactory::create_brain()?;

// Provider selection based on configuration
// Falls back to environment variables if config fails
```

### 2. Tool System

**Location**: `src-tauri/src/agent/tools/`

**Tool Categories**:

#### Desktop Tools
- **Screenshot**: `capture_screenshot`, `capture_element_screenshot`
- **Mouse**: Click, drag, move operations
- **Keyboard**: Text input, key presses
- **Window**: Management and focus control
- **Application**: Launch and control

#### Browser Tools  
- **Navigation**: `browser_navigate(url)`
- **Content**: `browser_extract_content()`
- **Interaction**: `browser_interact(selector, action)`
- **State**: `browser_get_current_url()`, `browser_screenshot()`

#### Basic Tools
- **File Operations**: Read, write, list files
- **Shell Commands**: Execute bash commands
- **Text Editor**: File editing with undo support

#### Voice Tools
- **TTS**: Text-to-speech with multiple providers
- **Transcription**: Speech-to-text via plugin

### 3. Execution Flow

**DefaultAgentRunner**:
```rust
// Maximum 15 iterations per execution
const MAX_ITERATIONS: u32 = 15;

// Agent execution loop:
1. Initialize memory and tools
2. Process user query
3. Think (AI reasoning)
4. Act (tool execution)  
5. Repeat until completion or max iterations
6. Return results
```

**Cancellation Support**:
- Escape key triggers cancellation signal
- Graceful shutdown with cleanup
- Dynamic escape key registration only during execution

### 4. Memory Management

**SimpleMemoryManager**:
- Conversation history preservation
- Tool call results integration
- Context window management
- Message role management (user, assistant, tool)

### 5. Tool Provider

**LocalToolProvider**:
- **Registration**: `register_async_tool(definition, executor)`
- **Execution**: Async tool execution with error handling
- **Lazy Loading**: Browser controller initialized on first use
- **Error Propagation**: Tool errors converted to agent errors

## Agent Types

### Single Agent Mode
**Command**: `submit_query(query: String)`
- Direct AI agent execution
- Full tool access
- Straightforward execution flow

### Multi-Agent Orchestration
**Command**: `submit_orchestrated_query(query: String, use_orchestrator: bool)`
- **Orchestrator**: Coordinates multiple specialized agents
- **Specialized Agents**: Desktop, Research, Code, Web agents
- **Task Distribution**: Intelligent task routing
- **Parallel Execution**: Multiple tasks simultaneously

## Tool Development

### Creating New Tools

1. **Define Tool Structure**:
```rust
ToolDefinition {
    name: "tool_name".to_string(),
    description: "Tool description for AI".to_string(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "param": {"type": "string", "description": "Parameter description"}
        },
        "required": ["param"]
    })
}
```

2. **Implement Executor**:
```rust
let executor = move |input: Value| async move {
    // Parse input parameters
    let param = input["param"].as_str().unwrap();
    
    // Execute tool logic
    let result = perform_action(param).await?;
    
    // Return success value
    Ok(serde_json::json!({
        "success": true,
        "result": result
    }))
};
```

3. **Register Tool**:
```rust
tool_provider.register_async_tool(definition, executor).await;
```

### Tool Best Practices

- **Async Execution**: All tools should be async for non-blocking operation
- **Error Handling**: Convert errors to appropriate formats
- **Input Validation**: Validate and sanitize all input parameters
- **Resource Management**: Clean up resources after use
- **Documentation**: Provide clear descriptions for AI understanding

## Configuration

### Provider Settings
```rust
// Provider selection and configuration
BrainFactory::init() // Initialize from config files
// Falls back to environment variables

// Runtime provider switching supported
set_active_provider(provider_name)
```

### Agent Limits
- **Max Iterations**: 15 per execution (configurable)
- **Tool Timeout**: Individual tool execution limits
- **Memory Limits**: Context window management
- **Cancellation**: User-initiated via escape key

## Error Handling

### Agent Errors
```rust
pub enum AgentError {
    Terminated,           // User cancellation
    MaxStepsReached,     // Iteration limit hit
    ToolNotFound,        // Invalid tool request
    ProviderError,       // AI provider failure
    ToolExecutionError,  // Tool execution failure
}
```

### Recovery Strategies
- **Tool Failures**: Retry with error context
- **Provider Failures**: Fallback providers
- **Resource Issues**: Cleanup and reinitialize
- **User Cancellation**: Graceful termination

## Performance Considerations

### Memory Management
- **Context Pruning**: Automatic memory cleanup
- **Lazy Loading**: Resources initialized on demand
- **Resource Pooling**: Reuse expensive resources

### Execution Optimization
- **Parallel Tools**: Multiple tools can execute simultaneously
- **Caching**: Tool results cached when appropriate
- **Batching**: Multiple similar operations combined

### Monitoring
- **Iteration Tracking**: Monitor execution progress
- **Performance Metrics**: Tool execution times
- **Resource Usage**: Memory and CPU monitoring 
