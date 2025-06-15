#!/bin/bash

# Fix get_tool_count method calls that don't exist
sed -i 's/provider\.get_tool_count()/"[tools registered]"/g' src-tauri/src/agent/providers/factory.rs

# Fix the import error for AgentError
sed -i 's/use crate::errors::AgentError;/use crate::agent::errors::AgentError;/g' src-tauri/src/anthropic.rs

# Fix the pattern matching issue in lib.rs
sed -i 's/if let Ok(state) = app_handle_clone\.try_state::<crate::state::AppState>()/if let Some(state) = app_handle_clone.try_state::<crate::state::AppState>()/g' src-tauri/src/lib.rs

echo "Fixed compilation errors"