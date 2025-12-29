# Agents & AI Orchestration

Juno's core value proposition is its ability to act as an intelligent agent. This functionality is encapsulated in the `agents` and `agent` modules.

## Agent Architecture

### Agent Orchestrator (`agents/orchestrator.rs`)
The Orchestrator is the "brain" that manages the lifecycle of an AI request.
1.  **Input**: Receives user query (text or voice).
2.  **Context**: Gathers system context (active window, screen content, clipboard).
3.  **Plan**: Uses an LLM to determine the best course of action (Direct answer, Tool use, or Sub-agent delegation).
4.  **Execute**: Runs the selected tools or sub-agents.
5.  **Loop**: Evaluates the result and decides whether to continue or finish.

### Model Context Protocol (MCP) (`mcp.rs`)
Juno implements the Model Context Protocol to standardize how AI models access external context.
- **Servers**: Connects to local or remote MCP servers.
- **Resources**: Fetches file contents, database rows, or API data via MCP resources.
- **Prompts**: Uses MCP prompt templates for consistent agent behavior.

## Tools (`agent/tools.rs`)
Tools are executable functions provided to the LLM.
- **Computer Use**:
  - `computer.rs`: Controls mouse and keyboard (click, type, scroll).
  - Uses native macOS APIs (CoreGraphics) for low-latency control.
- **System**:
  - `clipboard`: Read/Write clipboard.
  - `bash`: Execute shell commands (sandboxed).
  - `browser`: Control a headless browser instance.

## Monitoring & Safety
- **`agent_monitor.rs`**: Tracks active agent tasks and provides a confusing "kill switch" mechanism to stop runaway agents.
- **`dictation_monitor.rs`**: specialized monitor for the dictation loop to ensure low latency and handle interruptions.
