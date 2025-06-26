// Commands for managing AI providers

use crate::agent::providers::config::{ProviderConfig, AgentMode};
use crate::agent::providers::factory::{ProviderInfo, BrainFactory};
use crate::settings::manager::SettingsManager;
use crate::settings::ProviderConfig as CentralizedProviderConfig;
use tauri::State;
use tracing::info;
use crate::constants::errors::templates::FAILED_TO_INITIALIZE;
use crate::constants::errors::components::SETTINGS_MANAGER;

/// Get the list of available providers
#[tauri::command]
pub(crate) async fn get_providers() -> Result<Vec<ProviderInfo>, String> {
    Ok(BrainFactory::list_providers())
}

/// Get the current active provider
#[tauri::command]
pub(crate) async fn get_active_provider(app_handle: tauri::AppHandle) -> Result<String, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!("Failed to initialize settings manager", e))?;

    let config = ProviderConfig::load_from_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to load config: {}", e))?;

    Ok(config.active_provider)
}

/// Set the active provider and validate/reset model if needed
#[tauri::command]
pub(crate) async fn set_active_provider(
    app_handle: tauri::AppHandle,
    provider_id: String
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!("Failed to initialize settings manager", e))?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to load config: {}", e))?;

    // Get the new provider info to validate models
    let providers = BrainFactory::list_providers();
    let new_provider = providers.iter().find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    // Check if current model is valid for new provider
    if let Some(provider_settings) = config.get_provider_settings(&provider_id) {
        if let Some(current_model) = &provider_settings.model {
            let model_valid = new_provider.model_info.iter().any(|m| m.id == *current_model) ||
                              new_provider.models.contains(current_model);

            if !model_valid {
                // Reset to default model for this provider - use centralized update
                if let Some(provider) = config.providers.iter_mut().find(|p| p.id == provider_id) {
                    provider.model = Some(new_provider.default_model.clone());
                }
                info!("Reset model to default '{}' for provider '{}'", new_provider.default_model, provider_id);
            }
        }
    }

    // Update active provider
    config.active_provider = provider_id.clone();

    // Save to centralized settings
    config.save_to_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to save active provider: {}", e))?;

    info!("Set active provider to: {}", provider_id);
    Ok(())
}

/// Validate if a model is available for the current provider
#[tauri::command]
pub(crate) async fn validate_provider_model(provider_id: String, model_id: String) -> Result<bool, String> {
    let providers = BrainFactory::list_providers();
    let provider = providers.iter().find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    let is_valid = provider.model_info.iter().any(|m| m.id == model_id) ||
                   provider.models.contains(&model_id);

    Ok(is_valid)
}

/// Get available models for a specific provider
#[tauri::command]
pub(crate) async fn get_provider_models(provider_id: String) -> Result<Vec<serde_json::Value>, String> {
    let providers = BrainFactory::list_providers();
    let provider = providers.iter().find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    // Return enhanced model info if available, otherwise fall back to simple model list
    if !provider.model_info.is_empty() {
        Ok(provider.model_info.iter().map(|m| {
            serde_json::json!({
                "id": m.id,
                "name": m.name,
                "supports_computer_use": m.supports_computer_use,
                "is_recommended": m.is_recommended
            })
        }).collect())
    } else {
        Ok(provider.models.iter().map(|m| {
            serde_json::json!({
                "id": m,
                "name": m,
                "supports_computer_use": false,
                "is_recommended": false
            })
        }).collect())
    }
}

/// Get settings for a specific provider
#[tauri::command]
pub(crate) async fn get_provider_settings(
    app_handle: tauri::AppHandle,
    provider_id: String
) -> Result<CentralizedProviderConfig, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!("Failed to initialize settings manager", e))?;

    let config = ProviderConfig::load_from_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to load config: {}", e))?;

    match config.get_provider_settings(&provider_id) {
        Some(settings) => Ok(settings.clone()),
        None => Err(format!("Provider '{}' not found", provider_id)),
    }
}

/// Update API key for a provider
#[tauri::command]
pub(crate) async fn update_provider_api_key(
    app_handle: tauri::AppHandle,
    provider_id: String,
    api_key: String
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!("Failed to initialize settings manager", e))?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to load config: {}", e))?;

    // Find the provider and update its API key
    let mut found = false;
    for provider in &mut config.providers {
        if provider.id == provider_id {
            provider.api_key = Some(api_key);
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    config.save_to_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to save API key: {}", e))?;

    info!("Updated API key for provider: {}", provider_id);
    Ok(())
}

/// Update model for a provider
#[tauri::command]
pub(crate) async fn update_provider_model(
    app_handle: tauri::AppHandle,
    provider_id: String,
    model: String
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!("Failed to initialize settings manager", e))?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to load config: {}", e))?;

    // Find the provider and update its model
    let mut found = false;
    for provider in &mut config.providers {
        if provider.id == provider_id {
            provider.model = Some(model.clone());
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    config.save_to_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to save model: {}", e))?;

    info!("Updated model for provider: {} to {}", provider_id, model);
    Ok(())
}

/// Update max tokens for a provider
#[tauri::command]
pub(crate) async fn update_provider_max_tokens(
    app_handle: tauri::AppHandle,
    provider_id: String,
    max_tokens: u32
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!("Failed to initialize settings manager", e))?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to load config: {}", e))?;

    // Find the provider and update its max_tokens
    let mut found = false;
    for provider in &mut config.providers {
        if provider.id == provider_id {
            provider.max_tokens = Some(max_tokens);
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    config.save_to_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to save max_tokens: {}", e))?;

    info!("Updated max_tokens for provider: {}", provider_id);
    Ok(())
}

/// Update temperature for a provider
#[tauri::command]
pub(crate) async fn update_provider_temperature(
    app_handle: tauri::AppHandle,
    provider_id: String,
    temperature: f32
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!("Failed to initialize settings manager", e))?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to load config: {}", e))?;

    // Find the provider and update its temperature
    let mut found = false;
    for provider in &mut config.providers {
        if provider.id == provider_id {
            provider.temperature = Some(temperature);
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    config.save_to_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to save temperature: {}", e))?;

    info!("Updated temperature for provider: {}", provider_id);
    Ok(())
}

/// Update system prompt for a provider
#[tauri::command]
pub(crate) async fn update_provider_system_prompt(
    app_handle: tauri::AppHandle,
    provider_id: String,
    system_prompt: String
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!("Failed to initialize settings manager", e))?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to load config: {}", e))?;

    // Find the provider and update its system prompt
    let mut found = false;
    for provider in &mut config.providers {
        if provider.id == provider_id {
            provider.system_prompt = Some(system_prompt);
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    config.save_to_centralized_settings(&settings_manager).await
        .map_err(|e| format!("Failed to save system prompt: {}", e))?;

    info!("Updated system prompt for provider: {}", provider_id);
    Ok(())
}

/// Get current agent mode
#[tauri::command]
pub(crate) async fn get_agent_mode(app_handle: tauri::AppHandle) -> Result<String, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!("Failed to initialize settings manager", e))?;

    let app_settings = settings_manager.get_all_settings().await
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    Ok(app_settings.agent.execution_mode)
}

/// Set agent mode
#[tauri::command]
pub(crate) async fn set_agent_mode(
    app_handle: tauri::AppHandle,
    mode: String
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!("Failed to initialize settings manager", e))?;

    let _agent_mode = AgentMode::from_str(&mode)
        .ok_or_else(|| format!("Invalid agent mode: '{}'. Must be 'single' or 'multi'", mode))?;

    // Load current agent settings
    let mut app_settings = settings_manager.get_all_settings().await
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // Update agent execution mode
    app_settings.agent.execution_mode = mode.clone();

    // Save updated settings
    settings_manager.save_all_settings(&app_settings).await
        .map_err(|e| format!("Failed to save agent mode: {}", e))?;

    info!("Set agent mode to: {}", mode);
    Ok(())
}
