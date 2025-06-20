// Commands for managing AI providers

use tauri::{AppHandle, State};
use tracing::{error, info, warn};
use serde_json::Value;

use crate::settings::{SettingsManager, ProviderConfig, ProviderInfo};
use crate::state::AppState;

/// Get all providers from centralized settings
#[tauri::command]
pub async fn get_providers(app_handle: AppHandle) -> Result<Vec<ProviderInfo>, String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();
    Ok(settings.providers.providers)
}

/// Get the active provider from centralized settings
#[tauri::command]
pub async fn get_active_provider(app_handle: AppHandle) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();
    Ok(settings.providers.active_provider)
}

/// Set the active provider in centralized settings
#[tauri::command]
pub async fn set_active_provider(app_handle: AppHandle, provider_id: String) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle);

    // Validate provider exists
    let settings = settings_manager.get_settings();
    if !settings.providers.providers.iter().any(|p| p.id == provider_id) {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    // Update active provider
    let mut updated_providers = settings.providers.clone();
    updated_providers.active_provider = provider_id;

    settings_manager.update_section("providers", serde_json::to_value(updated_providers).map_err(|e| format!("Serialization error: {}", e))?)
        .await
        .map_err(|e| format!("Failed to update active provider: {}", e))?;

    info!("Active provider updated successfully");
    Ok(())
}

/// Add a new provider to centralized settings
#[tauri::command]
pub async fn add_provider(
    app: AppHandle,
    provider_info: ProviderInfo,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app);
    let mut settings = settings_manager.get_settings();

    // Check if provider already exists
    if settings.providers.providers.iter().any(|p| p.id == provider_info.id) {
        return Err(format!("Provider '{}' already exists", provider_info.id));
    }

    // For simplified schema, just check if provider info is valid
    if provider_info.id.is_empty() || provider_info.name.is_empty() {
        return Err("Provider ID and name cannot be empty".to_string());
    }

    // Add the provider
    settings.providers.providers.push(provider_info.clone());

    // Save the updated providers
    let providers_value = serde_json::to_value(&settings.providers)
        .map_err(|e| format!("Failed to serialize providers: {}", e))?;

    settings_manager.update_section("providers", providers_value).await?;

    info!("✅ Provider '{}' added successfully", provider_info.id);
    Ok(())
}

/// Update an existing provider in centralized settings
#[tauri::command]
pub async fn update_provider(
    app_handle: AppHandle,
    provider_id: String,
    provider_info: ProviderInfo,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();

    let mut updated_providers = settings.providers.clone();

    // Find and update provider
    if let Some(provider) = updated_providers.providers.iter_mut().find(|p| p.id == provider_id) {
        *provider = provider_info;
    } else {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    settings_manager.update_section("providers", serde_json::to_value(updated_providers).map_err(|e| format!("Serialization error: {}", e))?)
        .await
        .map_err(|e| format!("Failed to update provider: {}", e))?;

    info!("Provider updated successfully");
    Ok(())
}

/// Remove a provider from centralized settings
#[tauri::command]
pub async fn remove_provider(app_handle: AppHandle, provider_id: String) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();

    let mut updated_providers = settings.providers.clone();

    // Remove provider
    let initial_len = updated_providers.providers.len();
    updated_providers.providers.retain(|p| p.id != provider_id);

    if updated_providers.providers.len() == initial_len {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    // If we removed the active provider, set first available as active
    if updated_providers.active_provider == provider_id {
        if let Some(first_provider) = updated_providers.providers.first() {
            updated_providers.active_provider = first_provider.id.clone();
        } else {
            return Err("Cannot remove the last provider".to_string());
        }
    }

    settings_manager.update_section("providers", serde_json::to_value(updated_providers).map_err(|e| format!("Serialization error: {}", e))?)
        .await
        .map_err(|e| format!("Failed to remove provider: {}", e))?;

    info!("Provider removed successfully");
    Ok(())
}

/// Test a provider configuration
#[tauri::command]
pub async fn test_provider(
    app_handle: AppHandle,
    provider_id: String,
) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();

    let provider = settings.providers.providers.iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    // Basic validation - simplified for new schema
    if provider.api_key.is_none() {
        return Ok(false);
    }

    // TODO: Add actual provider connectivity test
    info!("Provider test completed for: {}", provider_id);
    Ok(true)
}

/// Get provider status
#[tauri::command]
pub async fn get_provider_status(
    app_handle: AppHandle,
    provider_id: String,
) -> Result<Value, String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();

    let provider = settings.providers.providers.iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    let status = serde_json::json!({
        "id": provider.id,
        "name": provider.name,
        "enabled": provider.enabled,
        "has_api_key": provider.api_key.is_some(),
        "model": provider.model,
        "is_active": settings.providers.active_provider == provider_id
    });

    Ok(status)
}

/// Validate a provider model combination
#[tauri::command]
pub async fn validate_provider_model(
    app_handle: AppHandle,
    provider_id: String,
    model_id: String,
) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();

    let provider = settings.providers.providers.iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    // TODO: Add actual model validation logic
    // For now, just check if the provider exists and is enabled
    Ok(provider.enabled)
}

/// Get available models for a provider
#[tauri::command]
pub async fn get_provider_models(
    app_handle: AppHandle,
    provider_id: String,
) -> Result<Vec<Value>, String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();

    let provider = settings.providers.providers.iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    // TODO: Return actual models from provider
    // For now, return a basic model list
    let models = vec![
        serde_json::json!({
            "id": provider.model,
            "name": provider.model,
            "description": format!("Model for {}", provider.name),
            "maxTokens": 4096, // Default value since field was removed
            "provider": provider_id
        })
    ];

    Ok(models)
}

// Legacy command wrappers for backward compatibility (can be removed later)

/// Update provider API key using centralized settings
#[tauri::command]
pub async fn update_provider_api_key(
    app_handle: AppHandle,
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();

    let mut updated_providers = settings.providers.clone();

    if let Some(provider) = updated_providers.providers.iter_mut().find(|p| p.id == provider_id) {
        provider.api_key = Some(api_key);
    } else {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    settings_manager.update_section("providers", serde_json::to_value(updated_providers).map_err(|e| format!("Serialization error: {}", e))?)
        .await
        .map_err(|e| format!("Failed to update provider API key: {}", e))?;

    Ok(())
}

/// Update provider model using centralized settings
#[tauri::command]
pub async fn update_provider_model(
    app_handle: AppHandle,
    provider_id: String,
    model: String,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();

    let mut updated_providers = settings.providers.clone();

    if let Some(provider) = updated_providers.providers.iter_mut().find(|p| p.id == provider_id) {
        provider.model = model;
    } else {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    settings_manager.update_section("providers", serde_json::to_value(updated_providers).map_err(|e| format!("Serialization error: {}", e))?)
        .await
        .map_err(|e| format!("Failed to update provider model: {}", e))?;

    Ok(())
}

/// Update provider max tokens using centralized settings
#[tauri::command]
pub async fn update_provider_max_tokens(
    app_handle: AppHandle,
    provider_id: String,
    max_tokens: u32,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();

    let mut updated_providers = settings.providers.clone();

    if let Some(provider) = updated_providers.providers.iter_mut().find(|p| p.id == provider_id) {
        // max_tokens field removed from simplified schema - this is a no-op
    } else {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    settings_manager.update_section("providers", serde_json::to_value(updated_providers).map_err(|e| format!("Serialization error: {}", e))?)
        .await
        .map_err(|e| format!("Failed to update provider max tokens: {}", e))?;

    Ok(())
}

/// Update provider temperature using centralized settings
#[tauri::command]
pub async fn update_provider_temperature(
    app_handle: AppHandle,
    provider_id: String,
    temperature: f32,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();

    let mut updated_providers = settings.providers.clone();

    if let Some(provider) = updated_providers.providers.iter_mut().find(|p| p.id == provider_id) {
        // temperature field removed from simplified schema - this is a no-op
    } else {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    settings_manager.update_section("providers", serde_json::to_value(updated_providers).map_err(|e| format!("Serialization error: {}", e))?)
        .await
        .map_err(|e| format!("Failed to update provider temperature: {}", e))?;

    Ok(())
}

/// Update provider system prompt using centralized settings
#[tauri::command]
pub async fn update_provider_system_prompt(
    app_handle: AppHandle,
    provider_id: String,
    system_prompt: String,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle);
    let settings = settings_manager.get_settings();

    let mut updated_providers = settings.providers.clone();

    if let Some(provider) = updated_providers.providers.iter_mut().find(|p| p.id == provider_id) {
        // system_prompt field removed from simplified schema - this is a no-op
    } else {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    settings_manager.update_section("providers", serde_json::to_value(updated_providers).map_err(|e| format!("Serialization error: {}", e))?)
        .await
        .map_err(|e| format!("Failed to update provider system prompt: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn validate_api_key(
    app: AppHandle,
    provider_id: String,
    api_key: String,
) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app);
    let settings = settings_manager.get_settings();

    // Find the provider
    let _provider = settings.providers.providers.iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    // For simplified schema, just check if API key is not empty
    if api_key.trim().is_empty() {
        return Err("API key cannot be empty".to_string());
    }

    info!("✅ API key validated for provider: {}", provider_id);
    Ok(true)
}

#[tauri::command]
pub async fn get_provider_config(
    app: AppHandle,
    provider_id: String,
) -> Result<serde_json::Value, String> {
    let settings_manager = SettingsManager::new(app);
    let settings = settings_manager.get_settings();

    let provider = settings.providers.providers.iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    Ok(serde_json::json!({
        "id": provider.id,
        "name": provider.name,
        "model": provider.model,
        "enabled": provider.enabled,
        "hasApiKey": provider.api_key.is_some(),
        // Simplified - no maxTokens field
        "maxTokens": 4096,
    }))
}
