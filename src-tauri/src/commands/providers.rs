// Commands for managing AI providers

use crate::agent::providers::config::{ProviderConfig, ProviderSettings};
use crate::agent::providers::factory::{ProviderInfo, BrainFactory};
use tracing::info;

/// Get the list of available providers
#[tauri::command]
pub(crate) async fn get_providers() -> Result<Vec<ProviderInfo>, String> {
    Ok(BrainFactory::list_providers())
}

/// Get the current active provider
#[tauri::command]
pub(crate) async fn get_active_provider() -> Result<String, String> {
    let provider = BrainFactory::get_current_provider();
    Ok(provider.id().to_string())
}

/// Set the active provider
#[tauri::command]
pub(crate) async fn set_active_provider(provider_id: String) -> Result<(), String> {
    let mut config = ProviderConfig::load()
        .map_err(|e| format!("Failed to load config: {}", e))?;

    config.set_active_provider(&provider_id)
        .map_err(|e| format!("Failed to set active provider: {}", e))?;

    info!("Set active provider to: {}", provider_id);
    Ok(())
}

/// Get settings for a specific provider
#[tauri::command]
pub(crate) async fn get_provider_settings(provider_id: String) -> Result<ProviderSettings, String> {
    let config = ProviderConfig::load()
        .map_err(|e| format!("Failed to load config: {}", e))?;

    match config.get_provider_settings(&provider_id) {
        Some(settings) => Ok(settings.clone()),
        None => Err(format!("Provider '{}' not found", provider_id)),
    }
}

/// Update API key for a provider
#[tauri::command]
pub(crate) async fn update_provider_api_key(provider_id: String, api_key: String) -> Result<(), String> {
    let mut config = ProviderConfig::load()
        .map_err(|e| format!("Failed to load config: {}", e))?;

    config.update_api_key(&provider_id, api_key)
        .map_err(|e| format!("Failed to update API key: {}", e))?;

    info!("Updated API key for provider: {}", provider_id);
    Ok(())
}

/// Update model for a provider
#[tauri::command]
pub(crate) async fn update_provider_model(provider_id: String, model: String) -> Result<(), String> {
    let mut config = ProviderConfig::load()
        .map_err(|e| format!("Failed to load config: {}", e))?;

    config.update_model(&provider_id, model)
        .map_err(|e| format!("Failed to update model: {}", e))?;

    info!("Updated model for provider: {}", provider_id);
    Ok(())
}

/// Update max tokens for a provider
#[tauri::command]
pub(crate) async fn update_provider_max_tokens(provider_id: String, max_tokens: u32) -> Result<(), String> {
    let mut config = ProviderConfig::load()
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

    config.save()
        .map_err(|e| format!("Failed to save config: {}", e))?;

    info!("Updated max_tokens for provider: {}", provider_id);
    Ok(())
}

/// Update temperature for a provider
#[tauri::command]
pub(crate) async fn update_provider_temperature(provider_id: String, temperature: f32) -> Result<(), String> {
    let mut config = ProviderConfig::load()
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

    config.save()
        .map_err(|e| format!("Failed to save config: {}", e))?;

    info!("Updated temperature for provider: {}", provider_id);
    Ok(())
}

/// Update system prompt for a provider
#[tauri::command]
pub(crate) async fn update_provider_system_prompt(provider_id: String, system_prompt: String) -> Result<(), String> {
    let mut config = ProviderConfig::load()
        .map_err(|e| format!("Failed to load config: {}", e))?;

    // Find the provider and update its system_prompt
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

    config.save()
        .map_err(|e| format!("Failed to save config: {}", e))?;

    info!("Updated system prompt for provider: {}", provider_id);
    Ok(())
}
