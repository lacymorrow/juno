# Roadmap: Enhancing DotDot with Agent Orchestration and Tools

## Introduction

This document outlines a roadmap for enhancing the `dotdot` Tauri application. The goal is to retain its strong foundation in macOS-specific, granular desktop automation while incorporating the agent orchestration patterns and advanced tool capabilities observed in the `Manus` Python project.

`dotdot` currently excels at providing low-level desktop control primitives via Tauri commands. `Manus` demonstrates a higher-level agent architecture using tool calling, specialized tools (like browser interaction and code execution), and more structured prompt/memory management. This roadmap aims to bridge that gap.

## Phase 1: Implement Core Agent Framework

**Status: Partially Complete.** A core loop exists (`anthropic::submit_query`), but lacks a formal `Agent` structure, dedicated memory roles, and a trait-based `ToolCollection`.

**Improvement over `dotdot`:** `Manus` has a dedicated `ToolCallAgent` class managing the execution loop, state, memory, and tool invocation. `dotdot` currently lacks a central orchestrating agent.

*   **1.1. Define Agent Structure:**
    *   Create a central `Agent` struct or trait within the Rust backend (`src-tauri/src/agent.rs`?).
    *   This agent should manage its state (e.g., current goal, status), potentially integrating with or utilizing the existing `AppState`.
*   **1.2. Implement Execution Loop:**
    *   Design a `run` or `think` loop within the `Agent` struct.
    *   This loop will fetch prompts/goals, interact with an LLM (likely via the existing `anthropic::submit_query` logic, but orchestrated by the agent), parse responses (including tool calls), execute tools, and manage state updates.
*   **1.3. Introduce Agent Memory:**
    *   Implement a mechanism for storing conversation history and potentially intermediate results or observations, similar to `Manus.memory`. This could be part of the `Agent`'s state.
*   **1.4. Formalize "Tools":**
    *   Define a `Tool` trait or struct.
    *   Existing commands in `commands.rs` can be refactored or wrapped to conform to this `Tool` interface.
    *   Create a `ToolCollection` or similar registry within the `Agent` to manage available tools.
*   **1.5. Integrate Agent with Tauri:**
    *   Expose agent control commands via Tauri (e.g., `start_agent(prompt)`, `get_agent_status()`).
    *   The agent will internally call the necessary low-level commands (now refactored as Tools).

## Phase 2: Integrate Key Manus-Inspired Tools

**Status: Partially Complete.** Basic file/process execution exists but lacks the robustness and dedicated interfaces of Manus tools. `TextEditorTool` and `PythonExecute` (corresponding to Anthropic's `bash` tool) are **not exposed** via Tauri commands.

**Improvement over `dotdot`:** `Manus` features specialized, high-level tools like `BrowserUseTool`, `PythonExecute`, and `StrReplaceEditor`.

*   **2.1. Develop `BrowserUseTool`:**
    *   **Goal:** Provide dedicated, robust browser automation capabilities beyond simple URL opening and generic clicks.
    *   **Implementation:**
        *   Research and select a suitable Rust library for browser automation (e.g., `headless_chrome`, `fantoccini`, playwright-rust bindings if mature, or potentially deeper integration with macOS WebKit APIs).
        *   Implement core browser actions as methods within this tool (navigate, find elements by CSS/XPath, click, type, scrape content, manage cookies/state).
        *   Create a `BrowserContextManager` to handle browser instances and state, similar to `Manus.browser_context_helper`.
*   **2.2. Implement `PythonExecute` Tool (Anthropic `bash` equivalent):**
    *   **Goal:** Allow the agent to execute shell commands (like Python scripts) for tasks not easily covered by other tools.
    *   **Implementation:**
        *   **Security:** This is critical. Execution must be sandboxed or carefully controlled.
        *   Use `std::process::Command` to invoke interpreters safely (Python, bash, etc.).
        *   Define clear input/output mechanisms.
        *   Implement strict timeouts and resource limits (partially done via `wait-timeout`).
        *   **Expose via Tauri command.**
*   **2.3. Create `TextEditorTool` (Anthropic `text_editor` equivalent):**
    *   **Goal:** Provide capabilities for file manipulation, similar to `Manus.StrReplaceEditor` and Anthropic's `text_editor` tool.
    *   **Implementation:**
        *   Leverage Rust's standard library (`std::fs`) for file reading and writing.
        *   Implement text manipulation functions (e.g., view, create, search-and-replace, insertion, undo).
        *   **Expose via Tauri commands.**
*   **2.4. Implement `TerminateTool`:**
    *   **Goal:** Allow the agent (or LLM) to signal the end of a task.
    *   **Implementation:** A simple tool that sets a flag or state within the agent to halt its execution loop gracefully.

## Phase 3: Refinements and Advanced Features

*   **3.1. Enhance Prompt Engineering:**
    *   Develop more sophisticated system and step prompts within the agent framework, potentially allowing dynamic context injection (like browser state) similar to `Manus`.
*   **3.2. Improve State Management:**
    *   Refine how state is managed across agent steps and tool executions.
*   **3.3. Robust Error Handling:**
    *   Implement comprehensive error handling within the agent loop and tool executions, providing informative feedback.
*   **3.4. Tool Selection Logic:**
    *   While primarily relying on the LLM for tool calling, explore if any internal heuristics or pre-processing could optimize tool selection.

## Long-Term Considerations

*   **Cross-Platform Compatibility:** While initially Mac-focused, design the agent and tools with potential future cross-platform support in mind where feasible.
*   **Security Hardening:** Continuously review and enhance the security aspects, especially around code execution and file system access.
*   **Performance Optimization:** Profile and optimize agent loop and tool performance.
*   **Expanded Toolset:** Consider adding more specialized tools (e.g., calendar access, email interaction, specific application APIs).

This roadmap provides a structured approach to evolving `dotdot` into a more powerful, agent-driven application, leveraging the architectural strengths of projects like `Manus` while building upon our existing desktop automation capabilities. 
