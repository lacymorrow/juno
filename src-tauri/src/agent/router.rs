//! Smart routing (beta, off by default).
//!
//! Before a query reaches the agent, one small classifier call reads it and
//! picks a route:
//!
//! - `chat`: a conversational answer. Runs single-agent on the table's chat
//!   model, which is faster than the configured model and can still drive the
//!   computer, so a misroute loses speed, never capability.
//! - `tools`: needs files, shell, browser or the desktop. Runs single-agent on
//!   the configured model, with direct tool access. This replaces the old
//!   keyword check in `anthropic.rs`.
//! - `escalate`: long multi-step work or hard reasoning. Runs on the configured
//!   model in the configured agent mode.
//!
//! The router never blocks or fails a query. It only runs on the Anthropic API
//! provider with a key, and anything that goes wrong (setting off, other
//! provider, no key, timeout, HTTP error, unexpected output) returns `None`,
//! which means "behave exactly as if the router did not exist".
//!
//! Which model classifies and which model answers chat both come from
//! `Provider::model_definitions()` (`ModelDefinition::router_role`), never from
//! a list here.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::agent::providers::factory::BrainFactory;
use crate::agent::providers::types::{Availability, Provider, RouterRole};
use crate::constants::settings::{store_keys, SETTINGS_STORE_FILE};
use crate::constants::timeouts::{SMART_ROUTER_CONNECT_TIMEOUT_MS, SMART_ROUTER_TIMEOUT_MS};

const ANTHROPIC_API_URL: &str = crate::constants::api::endpoints::ANTHROPIC_API_URL;
const ROUTE_TOOL_NAME: &str = "route_request";
/// The classifier answers with one short tool call; this is plenty.
const CLASSIFIER_MAX_TOKENS: u32 = 200;
/// Long pasted text says nothing more about the route than its opening does.
const MAX_QUERY_CHARS: usize = 2000;
const MAX_REASON_CHARS: usize = 200;

const CLASSIFIER_SYSTEM_PROMPT: &str = "You route requests for Juno, a Mac assistant that can use the person's computer: files, folders, apps, documents, the shell, a browser and the screen. Call route_request exactly once.
chat: can be answered from general knowledge alone. Nothing on this Mac or the web has to be looked at, opened, created or changed.
tools: needs anything on this Mac or the web: files, apps, documents, spreadsheets, settings, the screen, a website, current information, or sending or scheduling something. Short follow-ups that refer to earlier work (\"do it\", \"try again\") are tools.
escalate: a long multi-step job, or hard reasoning or planning, where a stronger model is worth the wait.
If unsure between chat and tools, pick tools.";

/// Where a request goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Chat,
    Tools,
    Escalate,
}

impl Route {
    pub fn from_label(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "chat" => Some(Route::Chat),
            "tools" => Some(Route::Tools),
            "escalate" => Some(Route::Escalate),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Route::Chat => "chat",
            Route::Tools => "tools",
            Route::Escalate => "escalate",
        }
    }

    /// Whether this route runs the single agent with direct tools, whatever
    /// agent mode is configured. Chat needs no delegation; tools needs direct
    /// access to the computer (what the old keyword check forced). Escalate
    /// keeps the configured mode.
    pub fn forces_single_agent(&self) -> bool {
        matches!(self, Route::Chat | Route::Tools)
    }
}

/// What the router decided for one query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePlan {
    pub route: Route,
    /// The classifier's one-line reason, for the log only.
    pub reason: String,
    /// Model to run this query on instead of the configured one. `None` keeps
    /// the configured model. Never written back to settings.
    pub model_override: Option<String>,
}

/// Why the classifier gave no usable answer. Every variant means "fall back".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouterError {
    Client(String),
    Timeout,
    Http(String),
    Status(u16),
    Body(String),
    NoRouteCall,
    UnknownRoute(String),
}

impl std::fmt::Display for RouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouterError::Client(e) => write!(f, "could not build the HTTP client: {e}"),
            RouterError::Timeout => write!(f, "classifier timed out"),
            RouterError::Http(e) => write!(f, "request failed: {e}"),
            RouterError::Status(code) => write!(f, "classifier returned HTTP {code}"),
            RouterError::Body(e) => write!(f, "unreadable response: {e}"),
            RouterError::NoRouteCall => write!(f, "classifier did not call {ROUTE_TOOL_NAME}"),
            RouterError::UnknownRoute(r) => write!(f, "unknown route '{r}'"),
        }
    }
}

// --- The table lookups ---

/// The model that classifies requests, from the table.
pub fn classifier_model(provider: &Provider) -> Option<&'static str> {
    provider
        .router_model(RouterRole::Classifier)
        .map(|def| def.id)
}

/// The model that answers `chat` requests, from the table. Only a current
/// model that can drive the computer qualifies: tools stay registered on the
/// chat route, so a misrouted request must still be able to use them.
pub fn chat_model(provider: &Provider) -> Option<&'static str> {
    provider
        .router_model(RouterRole::Chat)
        .filter(|def| def.availability == Availability::Current && def.supports_computer_use())
        .map(|def| def.id)
}

/// The model a route runs on, or `None` for the configured model.
pub fn model_for_route(
    route: Route,
    provider: &Provider,
    configured_model: &str,
) -> Option<String> {
    match route {
        Route::Chat => chat_model(provider)
            .filter(|model| *model != configured_model)
            .map(str::to_string),
        // The person chose their model for real work; honour it.
        Route::Tools | Route::Escalate => None,
    }
}

/// Turn a classifier outcome into a plan. An error is `None`: the query runs
/// as though routing were off.
pub fn plan_from_classification(
    outcome: Result<(Route, String), RouterError>,
    provider: &Provider,
    configured_model: &str,
) -> Option<RoutePlan> {
    match outcome {
        Ok((route, reason)) => Some(RoutePlan {
            route,
            reason,
            model_override: model_for_route(route, provider, configured_model),
        }),
        Err(e) => {
            warn!("Smart routing skipped, using the standard path: {}", e);
            None
        }
    }
}

// --- The classifier call ---

/// The request body for one classification. The route comes back as a forced
/// call to a strict tool, so the answer is schema-checked JSON, not prose.
pub fn build_classifier_request(model: &str, query: &str) -> Value {
    let query: String = query.chars().take(MAX_QUERY_CHARS).collect();
    json!({
        "model": model,
        "max_tokens": CLASSIFIER_MAX_TOKENS,
        "system": CLASSIFIER_SYSTEM_PROMPT,
        "tools": [{
            "name": ROUTE_TOOL_NAME,
            "description": "Record where this request should go.",
            "strict": true,
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "route": {
                        "type": "string",
                        "enum": ["chat", "tools", "escalate"]
                    },
                    "reason": {
                        "type": "string",
                        "description": "Why, in under 15 words."
                    }
                },
                "required": ["route", "reason"]
            }
        }],
        "tool_choice": { "type": "tool", "name": ROUTE_TOOL_NAME },
        "messages": [{ "role": "user", "content": query }]
    })
}

/// Read the route out of a Messages API response body.
pub fn parse_classifier_response(body: &Value) -> Result<(Route, String), RouterError> {
    let call = body["content"]
        .as_array()
        .and_then(|blocks| {
            blocks.iter().find(|block| {
                block["type"].as_str() == Some("tool_use")
                    && block["name"].as_str() == Some(ROUTE_TOOL_NAME)
            })
        })
        .ok_or(RouterError::NoRouteCall)?;

    let label = call["input"]["route"]
        .as_str()
        .ok_or(RouterError::NoRouteCall)?;
    let route =
        Route::from_label(label).ok_or_else(|| RouterError::UnknownRoute(label.to_string()))?;
    let reason = call["input"]["reason"]
        .as_str()
        .unwrap_or("")
        .trim()
        .chars()
        .take(MAX_REASON_CHARS)
        .collect();
    Ok((route, reason))
}

/// One shared client, so each query after the first reuses the connection
/// instead of paying for a TLS handshake inside the latency budget.
fn http_client() -> Result<&'static reqwest::Client, RouterError> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(SMART_ROUTER_TIMEOUT_MS))
        .connect_timeout(Duration::from_millis(SMART_ROUTER_CONNECT_TIMEOUT_MS))
        .build()
        .map_err(|e| RouterError::Client(e.to_string()))?;
    Ok(CLIENT.get_or_init(|| client))
}

async fn classify(api_key: &str, model: &str, query: &str) -> Result<(Route, String), RouterError> {
    let response = http_client()?
        .post(ANTHROPIC_API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&build_classifier_request(model, query))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                RouterError::Timeout
            } else {
                RouterError::Http(e.to_string())
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(RouterError::Status(status.as_u16()));
    }
    let body: Value = response.json().await.map_err(|e| {
        if e.is_timeout() {
            RouterError::Timeout
        } else {
            RouterError::Body(e.to_string())
        }
    })?;
    parse_classifier_response(&body)
}

// --- Entry point ---

/// Decide how to run `query`, or `None` to run it exactly as before.
pub async fn plan_route(app_handle: &tauri::AppHandle, query: &str) -> Option<RoutePlan> {
    if !is_enabled(app_handle) {
        return None;
    }

    let (provider, configured_model) = BrainFactory::active_provider_and_model(Some(app_handle))?;
    if provider != Provider::Anthropic {
        debug!(
            "Smart routing is on but the provider is {}; skipping",
            provider.id()
        );
        return None;
    }

    let config = crate::agent::providers::config::load_provider_config(Some(app_handle));
    let api_key = crate::demo::resolve_api_key(
        config
            .resolve_provider(Provider::Anthropic)
            .and_then(|c| c.api_key),
        std::env::var("ANTHROPIC_API_KEY").ok(),
        crate::demo::api_key(),
    )?;
    let model = classifier_model(&provider)?;

    let started = Instant::now();
    let outcome = classify(&api_key, model, query).await;
    let elapsed_ms = started.elapsed().as_millis();

    let plan = plan_from_classification(outcome, &provider, &configured_model)?;
    info!(
        "Smart routing: route={} model={} in {}ms, reason: {}",
        plan.route.label(),
        plan.model_override.as_deref().unwrap_or(&configured_model),
        elapsed_ms,
        plan.reason
    );
    Some(plan)
}

// --- Setting ---

/// Whether smart routing is on. Off unless the person turned it on.
pub fn is_enabled(app: &tauri::AppHandle) -> bool {
    use tauri_plugin_store::StoreExt;
    app.store(SETTINGS_STORE_FILE)
        .ok()
        .and_then(|store| store.get(store_keys::SMART_ROUTING_ENABLED))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

/// Read the smart-routing beta flag.
#[tauri::command]
pub async fn get_smart_routing_enabled(app_handle: tauri::AppHandle) -> Result<bool, String> {
    Ok(is_enabled(&app_handle))
}

/// Turn the smart-routing beta on or off. Applies from the next query.
#[tauri::command]
pub async fn set_smart_routing_enabled(
    app_handle: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app_handle
        .store(SETTINGS_STORE_FILE)
        .map_err(|e| format!("Failed to access settings store: {e}"))?;
    store.set(store_keys::SMART_ROUTING_ENABLED, Value::Bool(enabled));
    store
        .save()
        .map_err(|e| format!("Failed to save settings store: {e}"))?;
    info!(
        "Smart routing {}",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::providers::types::model_ids;

    fn response_with(input: Value) -> Value {
        json!({
            "content": [{
                "type": "tool_use",
                "id": "toolu_1",
                "name": ROUTE_TOOL_NAME,
                "input": input
            }],
            "stop_reason": "tool_use"
        })
    }

    #[test]
    fn parses_each_route() {
        for (label, route) in [
            ("chat", Route::Chat),
            ("tools", Route::Tools),
            ("escalate", Route::Escalate),
        ] {
            let body = response_with(json!({ "route": label, "reason": "because" }));
            assert_eq!(
                parse_classifier_response(&body),
                Ok((route, "because".to_string()))
            );
        }
    }

    #[test]
    fn a_missing_reason_is_not_an_error() {
        let body = response_with(json!({ "route": "tools" }));
        assert_eq!(
            parse_classifier_response(&body),
            Ok((Route::Tools, String::new()))
        );
    }

    #[test]
    fn a_long_reason_is_cut_on_a_character_boundary() {
        let long = "é".repeat(MAX_REASON_CHARS + 50);
        let body = response_with(json!({ "route": "chat", "reason": long }));
        let (_, reason) = parse_classifier_response(&body).unwrap();
        assert_eq!(reason.chars().count(), MAX_REASON_CHARS);
    }

    #[test]
    fn an_unknown_route_is_an_error() {
        let body = response_with(json!({ "route": "desktop", "reason": "x" }));
        assert_eq!(
            parse_classifier_response(&body),
            Err(RouterError::UnknownRoute("desktop".to_string()))
        );
    }

    #[test]
    fn prose_instead_of_a_tool_call_is_an_error() {
        let body = json!({ "content": [{ "type": "text", "text": "tools" }] });
        assert_eq!(
            parse_classifier_response(&body),
            Err(RouterError::NoRouteCall)
        );
        assert_eq!(
            parse_classifier_response(&json!({})),
            Err(RouterError::NoRouteCall)
        );
    }

    #[test]
    fn the_request_forces_the_strict_route_tool() {
        let request = build_classifier_request("m", "make a spreadsheet of my desktop files");
        assert_eq!(request["tool_choice"]["type"], "tool");
        assert_eq!(request["tool_choice"]["name"], ROUTE_TOOL_NAME);
        assert_eq!(request["tools"][0]["name"], ROUTE_TOOL_NAME);
        assert_eq!(request["tools"][0]["strict"], true);
        assert_eq!(
            request["tools"][0]["input_schema"]["additionalProperties"],
            false
        );
        let routes = request["tools"][0]["input_schema"]["properties"]["route"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .map(|label| Route::from_label(label).is_some())
            .collect::<Vec<_>>();
        assert_eq!(routes, vec![true, true, true]);
    }

    #[test]
    fn a_huge_query_is_shortened_before_it_is_sent() {
        let query = "ü".repeat(MAX_QUERY_CHARS * 3);
        let request = build_classifier_request("m", &query);
        let sent = request["messages"][0]["content"].as_str().unwrap();
        assert_eq!(sent.chars().count(), MAX_QUERY_CHARS);
    }

    #[test]
    fn the_table_names_a_classifier_and_a_chat_model_for_anthropic() {
        let provider = Provider::Anthropic;
        let classifier = classifier_model(&provider).expect("a classifier model");
        let chat = chat_model(&provider).expect("a chat model");
        assert!(provider.knows_model(classifier));
        assert!(provider.knows_model(chat));
        // Verified against the table, not memory. Chat moved from Sonnet 5 to
        // Sonnet 5.5 when Sonnet 5 went legacy (the role requires a current
        // model) — 2026-10-05 weekly model check, LAC-4142.
        assert_eq!(classifier, model_ids::CLAUDE_HAIKU_4_5);
        assert_eq!(chat, model_ids::CLAUDE_SONNET_5_5);
    }

    #[test]
    fn each_role_is_held_by_at_most_one_model() {
        for provider in [
            Provider::Anthropic,
            Provider::OpenAI,
            Provider::Rig,
            Provider::Gemini,
            Provider::ClaudeCli,
            Provider::CodexCli,
        ] {
            for role in [RouterRole::Classifier, RouterRole::Chat] {
                let holders = provider
                    .model_definitions()
                    .iter()
                    .filter(|def| def.router_role == Some(role))
                    .count();
                assert!(
                    holders <= 1,
                    "{:?} has {holders} {:?} models",
                    provider,
                    role
                );
            }
        }
    }

    #[test]
    fn the_chat_model_can_still_use_the_computer() {
        let provider = Provider::Anthropic;
        let chat = chat_model(&provider).expect("a chat model");
        assert!(provider.model_supports_computer_use(chat));
        let def = provider
            .model_definitions()
            .iter()
            .find(|def| def.id == chat)
            .unwrap();
        assert_eq!(def.availability, Availability::Current);
    }

    #[test]
    fn the_classifier_never_answers_a_request() {
        let provider = Provider::Anthropic;
        let classifier = classifier_model(&provider).unwrap();
        for route in [Route::Chat, Route::Tools, Route::Escalate] {
            for configured in provider.models() {
                assert_ne!(
                    model_for_route(route, &provider, &configured).as_deref(),
                    Some(classifier),
                    "{:?} with {configured} ran on the classifier",
                    route
                );
            }
        }
    }

    #[test]
    fn chat_runs_on_the_chat_model_and_work_on_the_configured_one() {
        let provider = Provider::Anthropic;
        let configured = provider.default_model();
        assert_eq!(
            model_for_route(Route::Chat, &provider, configured),
            chat_model(&provider).map(str::to_string)
        );
        assert_eq!(model_for_route(Route::Tools, &provider, configured), None);
        assert_eq!(
            model_for_route(Route::Escalate, &provider, configured),
            None
        );
    }

    #[test]
    fn chat_on_the_chat_model_already_needs_no_override() {
        let provider = Provider::Anthropic;
        let chat = chat_model(&provider).unwrap();
        assert_eq!(model_for_route(Route::Chat, &provider, chat), None);
    }

    #[test]
    fn providers_without_router_roles_never_override() {
        for provider in [
            Provider::OpenAI,
            Provider::Gemini,
            Provider::ClaudeCli,
            Provider::CodexCli,
        ] {
            let configured = provider.default_model();
            for route in [Route::Chat, Route::Tools, Route::Escalate] {
                assert_eq!(model_for_route(route, &provider, configured), None);
            }
        }
    }

    #[test]
    fn a_classifier_failure_falls_back_to_the_standard_path() {
        let provider = Provider::Anthropic;
        let configured = provider.default_model();
        for error in [
            RouterError::Timeout,
            RouterError::Status(529),
            RouterError::Http("connection reset".into()),
            RouterError::NoRouteCall,
            RouterError::UnknownRoute("x".into()),
        ] {
            assert_eq!(
                plan_from_classification(Err(error), &provider, configured),
                None
            );
        }
    }

    #[test]
    fn a_classification_becomes_a_plan() {
        let provider = Provider::Anthropic;
        let configured = provider.default_model();
        let plan = plan_from_classification(
            Ok((Route::Chat, "general knowledge".into())),
            &provider,
            configured,
        )
        .unwrap();
        assert_eq!(plan.route, Route::Chat);
        assert_eq!(plan.reason, "general knowledge");
        assert_eq!(
            plan.model_override,
            chat_model(&provider).map(str::to_string)
        );
    }

    #[test]
    fn only_escalate_keeps_the_configured_agent_mode() {
        assert!(Route::Chat.forces_single_agent());
        assert!(Route::Tools.forces_single_agent());
        assert!(!Route::Escalate.forces_single_agent());
    }
}
