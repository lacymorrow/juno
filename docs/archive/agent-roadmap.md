# Agent Architecture Roadmap: Aligning with Manus Patterns

## 1. Introduction

This document outlines specific architectural improvements for the agent loop and related components within `dotdot`, inspired by the analysis of the `Manus` project (`MANUS_COMPETITIVE_ANALYSIS.md`). While the main `ROADMAP.md` covers broader feature parity (like specific tools), this document focuses on refining the core agent execution flow currently implemented in `src-tauri/src/anthropic.rs` to adopt best practices observed in `Manus`.

The goal is to evolve `dotdot`'s current multi-turn request handler into a more robust, flexible, and maintainable agent framework.

## 2. Core Agent Loop and State Management

**Improvement over `dotdot`:** `Manus` uses an explicit state machine (`AgentState`) and a `max_steps` limit, offering more flexible control and termination than `dotdot`'s fixed `MAX_ITERATIONS` loop. It also supports explicit termination via a `Terminate` tool.

*   **2.1. Implement Explicit Agent State:**
    *   **Action:** Define a Rust `enum AgentState { Running, Finished, Failed, Paused, ... }` within the agent module (`src-tauri/src/agent.rs` if created, or a dedicated state module).
    *   **Benefit:** Provides clear, explicit control over the agent's lifecycle.
*   **2.2. Refactor Loop Control:**
    *   **Action:** Modify the main loop in `anthropic.rs` (or its future refactored location) to check `AgentState` instead of relying solely on `MAX_ITERATIONS`. Introduce a configurable `max_steps` similar to `Manus`.
    *   **Benefit:** More flexible loop control, allows indefinite running if needed (within `max_steps`), enables cleaner termination.
*   **2.3. Introduce `TerminateTool` Concept:**
    *   **Action:** Designate a specific tool response (or a dedicated internal tool) that can transition the `AgentState` to `Finished`. This could be triggered by the LLM explicitly calling a "terminate" or "task_complete" tool.
    *   **Benefit:** Allows the LLM to signal task completion gracefully, aligning with the agent paradigm.

## 3. Memory Structure and Tool Result Integration

**Improvement over `dotdot`:** `Manus` uses distinct message roles (`user`, `assistant`, `tool`) for its memory, providing clearer separation of concerns compared to `dotdot` bundling tool results into a subsequent `user` message.

*   **3.1. Refine `AnthropicMessage` or Introduce New Memory Structure:**
    *   **Action:** Modify the `AnthropicMessage` struct (or create a new `AgentMemoryMessage` struct) to include a dedicated `tool` role or variant.
    *   **Benefit:** Better semantic representation of the conversation history, aligning with standard agent memory patterns.
*   **3.2. Modify Tool Result Handling in Loop:**
    *   **Action:** Instead of collecting tool results and adding them as a single `user` message in the *next* iteration, modify the loop to add *each* tool result as a distinct `tool` message (using the refined structure from 3.1) to the `conversation_history` *within the current iteration*, immediately after execution.
    *   **Benefit:** Simplifies the loop logic, provides tool results to the LLM in the subsequent `think` phase more naturally, and aligns with Anthropic's recommended format for providing tool results.

## 4. Think/Act Phase Separation

**Improvement over `dotdot`:** `Manus` has clearer logical separation between the `think` phase (LLM interaction) and the `act` phase (tool execution). `dotdot` interleaves these more tightly within its single loop structure.

*   **4.1. Refactor Loop into Logical Phases:**
    *   **Action:** Restructure the main loop function (e.g., `submit_query` or its successor) into distinct internal functions or blocks corresponding to `think` (prepare prompt, call LLM, parse response, update memory with assistant message) and `act` (iterate tool calls, execute, update memory with tool messages).
    *   **Benefit:** Improves code readability, maintainability, and adherence to the conceptual ReAct (Reasoning and Acting) pattern common in agents.

## 5. Decoupling Agent Logic from UI/Side Effects

**Improvement over `dotdot`:** The `Manus` `run` method primarily returns a final string result, keeping agent logic separate from UI updates or side effects like TTS. `dotdot`'s `submit_query` currently handles TTS and UI event emission directly at the end.

*   **5.1. Isolate Core Agent Logic:**
    *   **Action:** Refactor the agent loop so that its primary output is the final result (e.g., a String or a structured result object) or the final agent state.
    *   **Benefit:** Makes the core agent logic more testable, reusable, and independent of specific UI frameworks or side effects.
*   **5.2. Handle UI Updates Externally:**
    *   **Action:** Move the TTS invocation and the Tauri `emit` call to the *caller* of the agent's main run function, or use a dedicated event/callback mechanism triggered by the agent reaching a `Finished` state.
    *   **Benefit:** Enforces separation of concerns. The agent focuses on task execution; other parts of the application handle presenting the results.

## 6. Formalize Tool Abstraction

**Improvement over `dotdot`:** `Manus` uses a `ToolCollection` and a more formal definition of tools. `dotdot` relies on `handle_tool_call` dispatching based on name.

*   **6.1. Implement `Tool` Trait/Struct (Reinforcement):**
    *   **Action:** Define a common `Tool` trait in Rust (as mentioned in `ROADMAP.md`). Refactor functions currently called by `handle_tool_call` to implement this trait. The trait should define metadata (name, description, parameters) and an `execute` method.
    *   **Benefit:** Provides a unified, type-safe way to define, register, and execute tools, improving code structure and extensibility.
*   **6.2. Create Tool Registry:**
    *   **Action:** Implement a `ToolRegistry` or `ToolCollection` struct within the agent to hold instances of available tools conforming to the `Tool` trait. Use this registry for looking up and executing tools based on the LLM's request.
    *   **Benefit:** Centralizes tool management, simplifies adding/removing tools.

By implementing these architectural changes, `dotdot`'s agent capabilities will become more robust, maintainable, and aligned with established patterns in AI agent development, enhancing its competitiveness. 
