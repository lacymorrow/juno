use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::agent::traits::MemoryManager;
use crate::state::AppState;
use crate::agent::implementations::memory_manager::{AdvancedMemoryManager, VisualContextConfig, VisualContextSummary};

/// DTOs for memory management commands (simplified version)
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryStatus {
    pub total_messages: usize,
    pub estimated_tokens: usize,
    pub memory_efficiency_ratio: f64,
}

/// Get current memory status - enhanced for AdvancedMemoryManager
#[tauri::command]
pub async fn get_memory_status(state: State<'_, AppState>) -> Result<MemoryStatus, String> {
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    // Get enhanced metrics from AdvancedMemoryManager
    let messages = memory_guard.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    let total_messages = messages.len();
    // Enhanced token estimation: ~4 chars per token
    let estimated_tokens = messages.iter()
        .map(|msg| msg.content.len() / 4)
        .sum();

    // Get efficiency ratio from AdvancedMemoryManager metrics
    let metrics = memory_guard.get_memory_metrics().await;
    let efficiency_ratio = metrics.memory_efficiency_ratio;

    Ok(MemoryStatus {
        total_messages,
        estimated_tokens,
        memory_efficiency_ratio: efficiency_ratio,
    })
}

/// Clear conversation memory
#[tauri::command]
pub async fn clear_conversation_memory(state: State<'_, AppState>) -> Result<(), String> {
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    memory_guard.clear_memory().await
        .map_err(|e| format!("Failed to clear memory: {}", e))
}

/// Clean orphaned tool calls
#[tauri::command]
pub async fn clean_orphaned_tool_calls(state: State<'_, AppState>) -> Result<String, String> {
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    memory_guard.clean_orphaned_tool_calls().await
        .map_err(|e| format!("Failed to clean orphaned tool calls: {}", e))?;

    Ok("Orphaned tool calls cleaned successfully".to_string())
}

/// Clean orphaned tool results that have no corresponding tool calls
#[tauri::command]
pub async fn clean_orphaned_tool_results(state: State<'_, AppState>) -> Result<String, String> {
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    let cleaned_count = memory_guard.clean_orphaned_tool_results().await
        .map_err(|e| format!("Failed to clean orphaned tool results: {}", e))?;

    Ok(format!("Cleaned {} orphaned tool results successfully", cleaned_count))
}

/// Get conversation messages (basic implementation)
#[tauri::command]
pub async fn get_conversation_messages(state: State<'_, AppState>) -> Result<Vec<crate::agent::core::Message>, String> {
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    memory_guard.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))
}

/// Get last N messages
#[tauri::command]
pub async fn get_last_n_messages(n: usize, state: State<'_, AppState>) -> Result<Vec<crate::agent::core::Message>, String> {
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    memory_guard.get_last_n_messages(n).await
        .map_err(|e| format!("Failed to get last {} messages: {}", n, e))
}

/// Get visual context summaries - supported by AdvancedMemoryManager
#[tauri::command]
pub async fn get_visual_summaries(
    app_handle: tauri::AppHandle,
) -> Result<Vec<VisualContextSummary>, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    Ok(memory_guard.get_visual_summaries().await)
}

/// Update visual context configuration - supported by AdvancedMemoryManager
#[tauri::command]
pub async fn update_visual_config(
    app_handle: tauri::AppHandle,
    config: VisualContextConfig,
) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    memory_guard.update_visual_config(config).await
        .map_err(|e| format!("Failed to update visual config: {}", e))
}

/// Get current visual context configuration - supported by AdvancedMemoryManager
#[tauri::command]
pub async fn get_visual_config(
    app_handle: tauri::AppHandle,
) -> Result<VisualContextConfig, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    Ok(memory_guard.get_visual_config().await)
}

/// Force compression of all screenshots - supported by AdvancedMemoryManager
#[tauri::command]
pub async fn compress_all_screenshots(
    app_handle: tauri::AppHandle,
) -> Result<usize, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    memory_guard.compress_all_screenshots().await
        .map_err(|e| format!("Failed to compress screenshots: {}", e))
}

/// Enable/disable screenshot compression - supported by AdvancedMemoryManager
#[tauri::command]
pub async fn configure_screenshot_compression(
    app_handle: tauri::AppHandle,
    enable_compression: bool,
    immediate_compression: bool,
    max_base64_screenshots: usize,
    retention_seconds: u64,
) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    let new_config = VisualContextConfig {
        enable_screenshot_compression: enable_compression,
        immediate_compression,
        max_base64_screenshots,
        screenshot_retention_seconds: retention_seconds,
        fallback_to_generic_description: true,
    };

    memory_guard.update_visual_config(new_config).await
        .map_err(|e| format!("Failed to configure screenshot compression: {}", e))
}

/// Get memory statistics - enhanced for AdvancedMemoryManager
#[tauri::command]
pub async fn get_memory_compression_stats(
    app_handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    let metrics = memory_guard.get_memory_metrics().await;
    let visual_summaries = memory_guard.get_visual_summaries().await;

    // Calculate visual compression stats
    let total_screenshots_compressed = visual_summaries.len();
    let total_original_tokens: usize = visual_summaries.iter()
        .map(|s| s.estimated_original_tokens)
        .sum();
    let total_compressed_tokens: usize = visual_summaries.iter()
        .map(|s| s.compressed_tokens)
        .sum();
    let tokens_saved = total_original_tokens.saturating_sub(total_compressed_tokens);
    let average_compression_ratio = if !visual_summaries.is_empty() {
        visual_summaries.iter().map(|s| s.compression_ratio).sum::<f64>() / visual_summaries.len() as f64
    } else {
        0.0
    };

    Ok(serde_json::json!({
        "memory_metrics": {
            "total_messages": metrics.total_messages,
            "estimated_tokens": metrics.estimated_tokens,
            "memory_efficiency_ratio": metrics.memory_efficiency_ratio,
            "pruning_events": metrics.pruning_events,
            "summarization_events": metrics.summarization_events,
            "orphaned_tool_calls_cleaned": metrics.orphaned_tool_calls_cleaned,
            "average_response_time_ms": metrics.average_response_time_ms
        },
        "visual_compression": {
            "total_screenshots_compressed": total_screenshots_compressed,
            "total_original_tokens": total_original_tokens,
            "total_compressed_tokens": total_compressed_tokens,
            "tokens_saved": tokens_saved,
            "average_compression_ratio": average_compression_ratio,
            "latest_summaries": visual_summaries.into_iter().take(5).collect::<Vec<_>>()
        }
    }))
}

/// Emergency function to recover from token overflow - enhanced for AdvancedMemoryManager
#[tauri::command]
pub async fn emergency_memory_recovery(
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    let metrics = memory_guard.get_memory_metrics().await;
    let config = memory_guard.get_config().await;

    // Check if we need emergency recovery
    if metrics.estimated_tokens > config.max_tokens {
        log::warn!("Token count high ({}), performing emergency recovery", metrics.estimated_tokens);

        // Try pruning first (less destructive)
        let pruned_count = memory_guard.prune_memory(Some(config.min_messages_to_keep)).await
            .map_err(|e| format!("Failed to prune memory: {}", e))?;

        let new_metrics = memory_guard.get_memory_metrics().await;

        if new_metrics.estimated_tokens > config.max_tokens {
            // If pruning wasn't enough, clear memory
            memory_guard.clear_memory().await
                .map_err(|e| format!("Failed to clear memory: {}", e))?;

            Ok(format!("Emergency recovery complete: Pruned {} messages, then cleared all memory due to persistent high token count", pruned_count))
        } else {
            Ok(format!("Emergency recovery complete: Pruned {} messages, token count reduced from {} to {}",
                      pruned_count, metrics.estimated_tokens, new_metrics.estimated_tokens))
        }
    } else {
        Ok(format!("Emergency recovery complete: Token count {} within limits (max: {})",
                  metrics.estimated_tokens, config.max_tokens))
    }
}

/// Get conversation summaries - enhanced for AdvancedMemoryManager
#[tauri::command]
pub async fn get_conversation_summaries(
    app_handle: tauri::AppHandle,
) -> Result<Vec<crate::agent::implementations::memory_manager::ConversationSummary>, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    Ok(memory_guard.get_summaries().await)
}

/// Force memory optimization - enhanced for AdvancedMemoryManager
#[tauri::command]
pub async fn optimize_memory(
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    memory_guard.optimize_memory().await
        .map_err(|e| format!("Failed to optimize memory: {}", e))?;

    Ok("Memory optimization completed successfully".to_string())
}

/// Get memory configuration - enhanced for AdvancedMemoryManager
#[tauri::command]
pub async fn get_memory_config(
    app_handle: tauri::AppHandle,
) -> Result<crate::agent::implementations::memory_manager::MemoryConfig, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    Ok(memory_guard.get_config().await)
}

/// Update memory configuration - enhanced for AdvancedMemoryManager
#[tauri::command]
pub async fn update_memory_config(
    app_handle: tauri::AppHandle,
    config: crate::agent::implementations::memory_manager::MemoryConfig,
) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    memory_guard.update_config(config).await
        .map_err(|e| format!("Failed to update memory config: {}", e))
}

/// Get advanced memory metrics with comprehensive analysis - NEW ADVANCED FEATURE
#[tauri::command]
pub async fn get_advanced_memory_metrics(
    app_handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    let metrics = memory_guard.get_memory_metrics().await;
    let config = memory_guard.get_config().await;
    let messages = memory_guard.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))?;
    let visual_summaries = memory_guard.get_visual_summaries().await;

    // Calculate utilization ratios
    let message_utilization = (metrics.total_messages as f64 / config.max_messages as f64) * 100.0;
    let token_utilization = (metrics.estimated_tokens as f64 / config.max_tokens as f64) * 100.0;

    // Analyze message types
    let mut tool_calls = 0;
    let mut tool_results = 0;
    let mut user_messages = 0;
    let mut assistant_messages = 0;

    for message in &messages {
        match message.role {
            crate::agent::core::Role::User => user_messages += 1,
            crate::agent::core::Role::Assistant => {
                assistant_messages += 1;
                if message.tool_calls.is_some() {
                    tool_calls += 1;
                }
            },
            crate::agent::core::Role::Tool => tool_results += 1,
            _ => {}
        }
    }

    // Visual compression analytics
    let total_visual_compression_ratio = if !visual_summaries.is_empty() {
        visual_summaries.iter().map(|s| s.compression_ratio).sum::<f64>() / visual_summaries.len() as f64
    } else {
        0.0
    };

    Ok(serde_json::json!({
        "utilization": {
            "message_utilization_percent": message_utilization,
            "token_utilization_percent": token_utilization,
            "memory_efficiency_ratio": metrics.memory_efficiency_ratio,
            "visual_compression_ratio": total_visual_compression_ratio
        },
        "composition": {
            "user_messages": user_messages,
            "assistant_messages": assistant_messages,
            "tool_calls": tool_calls,
            "tool_results": tool_results,
            "visual_summaries": visual_summaries.len()
        },
        "performance": {
            "pruning_events": metrics.pruning_events,
            "summarization_events": metrics.summarization_events,
            "orphaned_calls_cleaned": metrics.orphaned_tool_calls_cleaned,
            "average_response_time_ms": metrics.average_response_time_ms
        },
        "limits": {
            "max_messages": config.max_messages,
            "max_tokens": config.max_tokens,
            "min_messages_to_keep": config.min_messages_to_keep,
            "summarization_batch_size": config.summarization_batch_size
        }
    }))
}

/// Force memory prune with custom parameters - NEW ADVANCED FEATURE
#[tauri::command]
pub async fn force_memory_prune(
    app_handle: tauri::AppHandle,
    target_messages: Option<usize>,
) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    let before_metrics = memory_guard.get_memory_metrics().await;

    let pruned_count = memory_guard.prune_memory(target_messages).await
        .map_err(|e| format!("Failed to force prune memory: {}", e))?;

    let after_metrics = memory_guard.get_memory_metrics().await;

    Ok(format!(
        "Force pruning complete: {} messages removed, {} messages remaining, tokens reduced from {} to {}",
        pruned_count,
        after_metrics.total_messages,
        before_metrics.estimated_tokens,
        after_metrics.estimated_tokens
    ))
}

/// Get tiered memory context for advanced workflows - NEW ADVANCED FEATURE
#[tauri::command]
pub async fn get_tiered_memory_context(
    app_handle: tauri::AppHandle,
    max_immediate_tokens: usize,
) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    let (immediate_context, background_context) = memory_guard.get_tiered_context(max_immediate_tokens).await
        .map_err(|e| format!("Failed to get tiered context: {}", e))?;

    let immediate_tokens: usize = immediate_context.iter()
        .map(|m| crate::agent::implementations::memory_manager::AdvancedMemoryManager::estimate_message_tokens(m))
        .sum();

    let background_tokens: usize = background_context.iter()
        .map(|m| crate::agent::implementations::memory_manager::AdvancedMemoryManager::estimate_message_tokens(m))
        .sum();

    Ok(serde_json::json!({
        "immediate_context": {
            "messages": immediate_context,
            "message_count": immediate_context.len(),
            "estimated_tokens": immediate_tokens
        },
        "background_context": {
            "messages": background_context,
            "message_count": background_context.len(),
            "estimated_tokens": background_tokens
        },
        "total_tokens": immediate_tokens + background_tokens
    }))
}
