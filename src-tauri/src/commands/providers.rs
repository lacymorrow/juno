// Commands for managing AI providers

use crate::agent::providers::config::{AgentMode, ProviderConfig};
use crate::agent::providers::factory::{BrainFactory, ProviderInfo};
use crate::constants::errors::{actions, components, templates};
use crate::settings::manager::SettingsManager;
use crate::settings::ProviderConfig as CentralizedProviderConfig;
use tracing::info;

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template
        .replacen("{}", context, 1)
        .replacen("{}", &error.to_string(), 1)
}

/// Get the list of available providers
#[tauri::command]
pub(crate) async fn get_providers(
    app_handle: tauri::AppHandle,
) -> Result<Vec<ProviderInfo>, String> {
    Ok(BrainFactory::list_providers_with_app_handle(Some(
        &app_handle,
    )))
}

/// Get the current active provider
#[tauri::command]
pub(crate) async fn get_active_provider(app_handle: tauri::AppHandle) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let config = ProviderConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::PROVIDER_SETTINGS, e))?;

    Ok(config.active_provider)
}

/// Set the active provider and validate/reset model if needed
#[tauri::command]
pub(crate) async fn set_active_provider(
    app_handle: tauri::AppHandle,
    provider_id: String,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::PROVIDER_SETTINGS, e))?;

    // Get the new provider info to validate models
    let providers = BrainFactory::list_providers();
    let new_provider = providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    // Check if current model is valid for new provider
    if let Some(provider_settings) = config.get_provider_settings(&provider_id) {
        if let Some(current_model) = &provider_settings.model {
            let model_valid = new_provider
                .model_info
                .iter()
                .any(|m| m.id == *current_model)
                || new_provider.models.contains(current_model);

            if !model_valid {
                // Reset to default model for this provider - use centralized update
                if let Some(provider) = config.providers.iter_mut().find(|p| p.id == provider_id) {
                    provider.model = Some(new_provider.default_model.clone());
                }
                info!(
                    "Reset model to default '{}' for provider '{}'",
                    new_provider.default_model, provider_id
                );
            }
        }
    }

    // Update active provider. Goes through the setter rather than the field
    // because picking one here is a person's decision, and the setter is what
    // records that — from now on Juno's own default-picking leaves it alone.
    config
        .set_active_provider(provider_id.clone())
        .map_err(|e| e.to_string())?;

    // Save to centralized settings
    config
        .save_to_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, actions::PROVIDER_SETTINGS, e))?;

    info!("Set active provider to: {}", provider_id);
    Ok(())
}

/// Validate if a model is available for the current provider
#[tauri::command]
pub(crate) async fn validate_provider_model(
    provider_id: String,
    model_id: String,
) -> Result<bool, String> {
    let providers = BrainFactory::list_providers();
    let provider = providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    let is_valid =
        provider.model_info.iter().any(|m| m.id == model_id) || provider.models.contains(&model_id);

    Ok(is_valid)
}

/// Get available models for a specific provider
#[tauri::command]
pub(crate) async fn get_provider_models(
    provider_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let providers = BrainFactory::list_providers();
    let provider = providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    // Return enhanced model info if available, otherwise fall back to simple model list
    if !provider.model_info.is_empty() {
        Ok(provider
            .model_info
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "name": m.name,
                    "supports_computer_use": m.supports_computer_use,
                    "is_recommended": m.is_recommended,
                    "is_legacy": m.is_legacy
                })
            })
            .collect())
    } else {
        Ok(provider
            .models
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m,
                    "name": m,
                    "supports_computer_use": false,
                    "is_recommended": false,
                    "is_legacy": false
                })
            })
            .collect())
    }
}

/// Get settings for a specific provider
#[tauri::command]
pub(crate) async fn get_provider_settings(
    app_handle: tauri::AppHandle,
    provider_id: String,
) -> Result<CentralizedProviderConfig, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let config = ProviderConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::PROVIDER_SETTINGS, e))?;

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
    api_key: String,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::PROVIDER_SETTINGS, e))?;

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

    config
        .save_to_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, "API key", e))?;

    info!("Updated API key for provider: {}", provider_id);
    Ok(())
}

/// Whether Juno can already talk to a model without asking for anything.
///
/// Onboarding asks this to decide whether the "Connect Your AI" step is worth
/// showing. A signed-in Claude CLI counts: that person is not missing a
/// credential, they are holding one Juno can use, and putting an API key field
/// in front of them asks them to pay twice for what they have.
///
/// The CLI is asked last, and only once the cheap checks have all said no, so
/// the subprocess runs exactly in the case where the alternative is showing
/// somebody a wall.
#[tauri::command]
pub(crate) async fn check_api_keys_available(app_handle: tauri::AppHandle) -> Result<bool, String> {
    use std::env;

    // Check settings store for saved keys
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let config = ProviderConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::PROVIDER_SETTINGS, e))?;

    // Check if any provider has an API key in the store
    for provider in &config.providers {
        if provider.api_key.as_ref().is_some_and(|k| !k.is_empty()) {
            info!("API key found in store for provider: {}", provider.id);
            return Ok(true);
        }
    }

    // A demo build carries its own key, so the onboarding key step is moot.
    if crate::demo::is_demo_build() {
        info!("Demo build: an Anthropic key is compiled in");
        return Ok(true);
    }

    // Fallback: check environment variables (loaded from .env files at startup)
    let env_keys = ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "GEMINI_API_KEY"];

    for key in &env_keys {
        if env::var(key).is_ok_and(|v| !v.is_empty()) {
            info!("API key found in environment variable: {}", key);
            return Ok(true);
        }
    }

    // Last, and only now: the subscription they may already be paying for.
    // Proof required — this answer decides whether setup hides the key step
    // entirely, and hiding it on a maybe would leave somebody finished and
    // unable to ask Juno anything.
    if crate::agent::providers::claude_cli::cli_status()
        .await
        .is_signed_in()
    {
        info!("No API keys, but the Claude CLI is signed in");
        return Ok(true);
    }
    if crate::agent::providers::codex_cli::cli_status()
        .await
        .is_signed_in()
    {
        info!("No API keys, but the Codex CLI is signed in with ChatGPT");
        return Ok(true);
    }

    info!("No API keys found in store or environment, and no signed-in CLI");
    Ok(false)
}

/// Update model for a provider
#[tauri::command]
pub(crate) async fn update_provider_model(
    app_handle: tauri::AppHandle,
    provider_id: String,
    model: String,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::PROVIDER_SETTINGS, e))?;

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

    config
        .save_to_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, "model", e))?;

    info!("Updated model for provider: {} to {}", provider_id, model);
    Ok(())
}

/// Update max tokens for a provider
#[tauri::command]
pub(crate) async fn update_provider_max_tokens(
    app_handle: tauri::AppHandle,
    provider_id: String,
    max_tokens: u32,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::PROVIDER_SETTINGS, e))?;

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

    config
        .save_to_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, "max_tokens", e))?;

    info!("Updated max_tokens for provider: {}", provider_id);
    Ok(())
}

/// Update temperature for a provider
#[tauri::command]
pub(crate) async fn update_provider_temperature(
    app_handle: tauri::AppHandle,
    provider_id: String,
    temperature: f32,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::PROVIDER_SETTINGS, e))?;

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

    config
        .save_to_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, "temperature", e))?;

    info!("Updated temperature for provider: {}", provider_id);
    Ok(())
}

/// Update system prompt for a provider
#[tauri::command]
pub(crate) async fn update_provider_system_prompt(
    app_handle: tauri::AppHandle,
    provider_id: String,
    system_prompt: String,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::PROVIDER_SETTINGS, e))?;

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

    config
        .save_to_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, "system prompt", e))?;

    info!("Updated system prompt for provider: {}", provider_id);
    Ok(())
}

/// Update whether a provider loads the MCP servers on the person's own
/// Claude account (claude.ai connectors, user-level servers). Claude CLI
/// only today; off makes the CLI run with `--strict-mcp-config` (LAC-4056).
#[tauri::command]
pub(crate) async fn update_provider_load_account_mcp(
    app_handle: tauri::AppHandle,
    provider_id: String,
    load_account_mcp: bool,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let mut config = ProviderConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::PROVIDER_SETTINGS, e))?;

    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;
    provider.load_account_mcp = load_account_mcp;

    config
        .save_to_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, "load_account_mcp", e))?;

    info!(
        "Updated load_account_mcp for provider {}: {}",
        provider_id, load_account_mcp
    );
    Ok(())
}

/// Get current agent mode
#[tauri::command]
pub(crate) async fn get_agent_mode(app_handle: tauri::AppHandle) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let app_settings = settings_manager
        .get_all_settings()
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::SETTINGS, e))?;

    Ok(app_settings.agent.execution_mode)
}

/// Set agent mode
#[tauri::command]
pub(crate) async fn set_agent_mode(
    app_handle: tauri::AppHandle,
    mode: String,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let _agent_mode = AgentMode::from_str(&mode).ok_or_else(|| {
        format!(
            "Invalid agent mode: '{}'. Must be 'single' or 'multi'",
            mode
        )
    })?;

    // Load current agent settings
    let mut app_settings = settings_manager
        .get_all_settings()
        .await
        .map_err(|e| format_error(templates::FAILED_TO_LOAD, actions::SETTINGS, e))?;

    // Update agent execution mode
    app_settings.agent.execution_mode = mode.clone();

    // Save updated settings
    settings_manager
        .save_all_settings(&app_settings)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, actions::AGENT_SETTINGS, e))?;

    info!("Set agent mode to: {}", mode);
    Ok(())
}

/// What one small call with a pasted key says about it.
///
/// Setup asks this the moment a key lands in the box, so a person who was
/// handed a key learns whether it works before they leave the step, in plain
/// words, instead of at their first question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum KeyCheck {
    /// The provider answered with the key.
    Ok,
    /// The provider says this is not a key it knows.
    Rejected,
    /// The key is real and its account has nothing left to spend.
    NoCredit,
    /// No verdict: offline, a timeout, or an answer that is not about the key.
    /// Never shown as a failure.
    Unknown,
}

/// How long the check may take before it gives up and says nothing.
const KEY_CHECK_TIMEOUT_SECS: u64 = 8;

/// Read a provider's answer. Only a definite statement about the key or its
/// balance counts against it; everything else (a retired model id, a rate
/// limit, a server error) is `Unknown`, because telling someone a good key is
/// bad is the worse mistake.
fn classify_key_response(status: u16, body: &str) -> KeyCheck {
    if (200..300).contains(&status) {
        return KeyCheck::Ok;
    }
    let body = body.to_ascii_lowercase();
    if body.contains("credit balance") || body.contains("insufficient_quota") {
        return KeyCheck::NoCredit;
    }
    if status == 401 || body.contains("api key not valid") || body.contains("api_key_invalid") {
        return KeyCheck::Rejected;
    }
    KeyCheck::Unknown
}

/// Try a key against its provider without saving it. The key is sent only to
/// that provider and is never logged.
#[tauri::command]
pub(crate) async fn check_provider_api_key(
    provider_id: String,
    api_key: String,
) -> Result<KeyCheck, String> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Ok(KeyCheck::Unknown);
    }
    let Some(model) = BrainFactory::list_providers()
        .into_iter()
        .find(|p| p.id == provider_id)
        .map(|p| p.default_model)
    else {
        return Ok(KeyCheck::Unknown);
    };
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(KEY_CHECK_TIMEOUT_SECS))
        .build()
    else {
        return Ok(KeyCheck::Unknown);
    };

    // The smallest request that makes each provider look at the key and, where
    // it has one, the balance behind it.
    let request = match provider_id.as_str() {
        "anthropic" => client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": model,
                "max_tokens": 1,
                "messages": [{ "role": "user", "content": "hi" }],
            })),
        "openai" => client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(api_key)
            .json(&serde_json::json!({
                "model": model,
                "max_completion_tokens": 16,
                "messages": [{ "role": "user", "content": "hi" }],
            })),
        "gemini" => client
            .get("https://generativelanguage.googleapis.com/v1beta/models")
            .header("x-goog-api-key", api_key),
        _ => return Ok(KeyCheck::Unknown),
    };

    let verdict = match request.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            classify_key_response(status, &body)
        }
        Err(_) => KeyCheck::Unknown,
    };
    info!("Key check for {}: {:?}", provider_id, verdict);
    Ok(verdict)
}

#[cfg(test)]
mod key_check_tests {
    use super::*;

    #[test]
    fn a_success_is_ok() {
        assert_eq!(classify_key_response(200, "{}"), KeyCheck::Ok);
    }

    #[test]
    fn an_unknown_key_is_rejected() {
        assert_eq!(
            classify_key_response(401, r#"{"error":{"type":"authentication_error"}}"#),
            KeyCheck::Rejected
        );
        assert_eq!(
            classify_key_response(400, "API key not valid. Please pass a valid API key."),
            KeyCheck::Rejected
        );
    }

    #[test]
    fn an_empty_account_is_no_credit() {
        assert_eq!(
            classify_key_response(400, "Your credit balance is too low to access the API."),
            KeyCheck::NoCredit
        );
        assert_eq!(
            classify_key_response(429, r#"{"error":{"type":"insufficient_quota"}}"#),
            KeyCheck::NoCredit
        );
    }

    #[test]
    fn anything_else_is_not_held_against_the_key() {
        assert_eq!(
            classify_key_response(404, "model not found"),
            KeyCheck::Unknown
        );
        assert_eq!(
            classify_key_response(429, "rate limited"),
            KeyCheck::Unknown
        );
        assert_eq!(classify_key_response(500, ""), KeyCheck::Unknown);
    }
}
