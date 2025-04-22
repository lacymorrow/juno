# Agent Architecture Roadmap

## 1. Introduction

This document outlines the roadmap for developing and refining the core agent architecture within `dotdot`. It draws inspiration from best practices observed in projects like `Manus` and aims to evolve `dotdot`'s current capabilities into a robust, flexible, and maintainable agent framework.

The goal is to implement a higher-level agent structure that orchestrates tasks using tool calling, manages state and memory effectively, and integrates seamlessly with the existing low-level macOS automation primitives provided by `dotdot`.

## 2. Core Agent Framework (Phase 1)

**Status: Partially Complete.** A basic multi-turn loop exists (`anthropic::submit_query`), but lacks a formal agent structure, explicit state management, dedicated memory roles, and a formal tool abstraction layer.

**Goal:** Implement a central `Agent` entity responsible for managing the execution lifecycle.

*   **2.1. Define Agent Structure & State:**
    *   **Action:** Create a central `Agent` struct (`src-tauri/src/agent.rs`?). Define a Rust `enum AgentState { Idle, Running, Thinking, Acting, Finished, Failed, Paused }` to manage its lifecycle explicitly.
    *   **Benefit:** Provides clear control, enables features like pausing/resuming, and improves observability.
    *   **(Ref: Manus `ToolCallAgent`, `AgentState`; ROADMAP.md 1.1)**
*   **2.2. Implement Execution Loop (`run`/`think`/`act`):**
    *   **Action:** Refactor the existing loop (`anthropic.rs`) into a state-driven loop within the `Agent` struct. Logically separate the loop into `think` (LLM interaction, planning) and `act` (tool execution) phases. Control the loop based on `AgentState` and a configurable `max_steps` limit.
    *   **Benefit:** Improves code structure, aligns with ReAct patterns, allows flexible control, and enables cleaner termination.
    *   **(Ref: Manus `run`/`think`/`act` methods; agent-roadmap.md 2.2, 4.1)**
*   **2.3. Implement Agent Memory:**
    *   **Action:** Refine `AnthropicMessage` or create a new `AgentMemoryMessage` struct with distinct roles (`user`, `assistant`, `tool`). Modify the loop to add `assistant` messages after `think` and `tool` messages immediately after `act`.
    *   **Benefit:** Provides better semantic history, aligns with standard agent patterns (e.g., Anthropic's tool use format), simplifies loop logic.
    *   **(Ref: Manus `memory`, `Message` roles; agent-roadmap.md 3.1, 3.2; ROADMAP.md 1.3)**
*   **2.4. Introduce `TerminateTool` Concept:**
    *   **Action:** Designate a specific tool (e.g., `task_complete`) or internal mechanism that allows the LLM or agent logic to transition the `AgentState` to `Finished` gracefully.
    *   **Benefit:** Enables the agent to recognize task completion and stop cleanly.
    *   **(Ref: Manus `Terminate` tool; agent-roadmap.md 2.3; ROADMAP.md 2.4)**
*   **2.5. Decouple Agent Logic from UI/Side Effects:**
    *   **Action:** Refactor the agent's core loop to return results/state rather than directly triggering side effects like TTS or Tauri events. Handle these effects in the calling code (e.g., Tauri command handlers) based on the agent's final state or events.
    *   **Benefit:** Improves testability, reusability, and separation of concerns.
    *   **(Ref: agent-roadmap.md 5.1, 5.2)**
*   **2.6. Integrate Agent with Tauri:**
    *   **Action:** Expose high-level agent control commands via Tauri (e.g., `start_task(prompt)`, `get_agent_status()`, `stop_task()`). These commands will interact with the central `Agent` instance.
    *   **Benefit:** Provides the user interface layer with control over the agent.
    *   **(Ref: ROADMAP.md 1.5)**

## 3. Formal Tool Abstraction & Integration (Phase 1/2)

**Status: Partially Complete.** Low-level functions exist and are exposed as Tauri commands, but lack a unified, agent-aware abstraction.

**Goal:** Create a formal system for defining, registering, and executing tools within the agent framework.

*   **3.1. Implement `Tool` Trait/Struct:**
    *   **Action:** Define a common `Tool` trait in Rust. The trait should define metadata (name, description, input/output schema) and an `execute` method.
    *   **Benefit:** Provides a unified, type-safe way to define tools.
    *   **(Ref: Manus `Tool` concept; agent-roadmap.md 6.1; ROADMAP.md 1.4)**
*   **3.2. Create Tool Registry/Collection:**
    *   **Action:** Implement a `ToolRegistry` or `ToolCollection` struct within the agent to hold available `Tool` instances. Refactor the tool execution logic (`act` phase) to use this registry for lookup and invocation based on LLM requests.
    *   **Benefit:** Centralizes tool management, simplifies adding/removing tools, decouples agent logic from specific tool implementations.
    *   **(Ref: Manus `available_tools`; agent-roadmap.md 6.2; ROADMAP.md 1.4)**
*   **3.3. Refactor Existing Commands as Tools:**
    *   **Action:** Gradually wrap or refactor the logic within existing `dev_...` Tauri commands (`commands.rs`) to implement the `Tool` trait.
    *   **Benefit:** Leverages existing functionality within the new agent framework.

## 4. Agent Refinements and Advanced Features (Phase 3+)

**Goal:** Enhance the agent's capabilities, intelligence, and robustness.

*   **4.1. Enhance Prompt Engineering:**
    *   Develop more sophisticated system prompts and step prompts for the `think` phase.
    *   Explore dynamic context injection (e.g., current application state, relevant file contents, previous tool outputs) into prompts.
    *   **(Ref: ROADMAP.md 3.1)**
*   **4.2. Improve State Management:**
    *   Refine how task-specific state is managed across multiple steps and tool executions within the agent.
    *   **(Ref: ROADMAP.md 3.2)**
*   **4.3. Robust Error Handling & Recovery:**
    *   Implement comprehensive error handling within the agent loop and tool executions.
    *   Develop strategies for the agent to recover from tool failures or unexpected situations (e.g., retry, alternative tool, ask user).
    *   **(Ref: ROADMAP.md 3.3)**
*   **4.4. Advanced Tool Selection/Orchestration:**
    *   While primarily LLM-driven, explore potential for internal heuristics or pre-processing to aid tool selection or sequence planning.
    *   **(Ref: ROADMAP.md 3.4)**
*   **4.5. Screen Understanding Strategy:**
    *   Continuously evaluate the balance between visual input (screenshots sent to multimodal models) and structural understanding (Accessibility API calls).
    *   Optimize how these methods are combined for different tasks and UI types.
    *   **(Ref: self-operating-computer.md Arch. Diff. 1)**

## 5. Long-Term Considerations

*   **Cross-Platform Compatibility:** Design agent and tool abstractions with potential future Windows/Linux support in mind.
*   **Security Hardening:** Continuously review and enhance security, especially for `bash` execution, file system access, and potentially `BrowserUseTool`.
*   **Performance Optimization:** Profile and optimize the agent loop and critical tool performance.
*   **Expanded Toolset:** Consider adding more specialized tools (e.g., calendar, email, specific app integrations).

---
*This roadmap will be updated as phases are completed and new requirements emerge.* 
