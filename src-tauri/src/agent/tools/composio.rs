//! The one place that knows Composio (LAC-4210).
//!
//! Composio Connect is attached as a single remote MCP server
//! (`connect.composio.dev/mcp`, streamable HTTP, MCP OAuth). Everything else
//! in Juno stays generic: the transport is `mcp_integration` and the OAuth
//! client is `mcp_oauth`. What lives here is only what is Composio-shaped —
//! the server config, the two blocked remote-shell tools, reading a "this app
//! is not connected" signal out of a tool result, and driving the per-app
//! connect flow against the `COMPOSIO_MANAGE_CONNECTIONS` meta-tool.
//!
//! The approval gate is deliberately NOT here, and not anywhere, for Phase 1
//! (Lacy 2026-10-08): Composio tools fall through to the risk classifier's
//! existing Low default, which means a send can run unasked in the default
//! mode. Known and accepted, contained by the Advanced beta toggle; the gate
//! is a follow-up before any public release.
//!
//! The agent never runs OAuth and never drives a connect flow. On missing
//! auth it gets one sentence telling it a Connect button is on screen; the
//! button calls a Tauri command, the command lands here.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::mcp_integration::MCPServerConfig;

/// The server name, which also fixes the registered tool prefix
/// (`composio_*`, from `mcp_integration::parse_tool_definition`).
pub const SERVER_NAME: &str = "composio";
/// The one endpoint. Verified streamable HTTP with MCP OAuth on 2026-10-08.
pub const MCP_URL: &str = "https://connect.composio.dev/mcp";

/// Cut by scope: Juno already has a local shell, and a second shell in
/// someone else's cloud is a seam and a billing line.
pub const BLOCKED_TOOLS: &[&str] = &["COMPOSIO_REMOTE_WORKBENCH", "COMPOSIO_REMOTE_BASH_TOOL"];

/// The registered (prefixed) name of a Composio meta-tool.
pub fn registered_tool_name(tool: &str) -> String {
    format!("{SERVER_NAME}_{tool}")
}

/// The MCP server config the Advanced toggle installs. `approved` is true
/// because the HTTP transport spawns nothing; the OAuth consent in the
/// person's browser is the approval that matters.
pub fn server_config() -> MCPServerConfig {
    let mut config = MCPServerConfig::new(
        SERVER_NAME.to_string(),
        "https".to_string(),
        vec![MCP_URL.to_string()],
    );
    config.description = Some("Connected apps (internal beta)".to_string());
    config.oauth = true;
    config.approved = true;
    config.timeout_seconds = 60;
    config.blocked_tools = BLOCKED_TOOLS.iter().map(|t| t.to_string()).collect();
    config
}

/// One connected app, as the Tools screen renders it: app, account,
/// Disconnect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedApp {
    /// Composio's toolkit slug, lowercase (`gmail`, `googlecalendar`).
    pub toolkit_slug: String,
    /// The name a person uses for the app ("Google Calendar").
    pub app_name: String,
    /// The connected account, when the connect flow reported one.
    pub account: Option<String>,
    /// Unix seconds when the connection was recorded.
    pub connected_at: u64,
}

/// The toolkit a slug belongs to: `GMAIL_SEND_EMAIL` is `gmail`.
pub fn toolkit_from_slug(slug: &str) -> Option<String> {
    slug.split('_').next().map(|t| t.to_lowercase())
}

/// The display name for a lowercase toolkit slug, in the words a person uses
/// for the app. An unknown toolkit still gets a readable word (title-cased)
/// rather than its slug.
pub fn toolkit_display(toolkit_slug: &str) -> String {
    match toolkit_slug {
        "gmail" => "Gmail".to_string(),
        "googlecalendar" => "Google Calendar".to_string(),
        "googledrive" => "Google Drive".to_string(),
        "googledocs" => "Google Docs".to_string(),
        "googlesheets" => "Google Sheets".to_string(),
        "slack" => "Slack".to_string(),
        "notion" => "Notion".to_string(),
        "linear" => "Linear".to_string(),
        "github" => "GitHub".to_string(),
        "stripe" => "Stripe".to_string(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        }
    }
}

/// Whether a string is a Composio action slug: `TOOLKIT_VERB_OBJECT`, all
/// uppercase, at least two `_`-separated tokens (`GMAIL_SEND_EMAIL`).
fn is_action_slug(candidate: &str) -> bool {
    let tokens: Vec<&str> = candidate.split('_').collect();
    tokens.len() >= 2
        && tokens.iter().all(|token| {
            !token.is_empty()
                && token
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        })
        && candidate.chars().any(|c| c.is_ascii_uppercase())
}

/// Every action slug in a meta-tool call's arguments. Schema-free walk over
/// slug-valued fields, because the meta-tool's argument shape is Composio's
/// to change: today it is `{"tools": [{"tool_slug": ...}]}`, and a drift
/// should degrade to "no slug found", never to a wrong app on the card.
fn attempted_slugs(input: &Value) -> Vec<String> {
    const SLUG_KEYS: &[&str] = &["tool_slug", "slug", "tool", "tool_name", "action", "name"];

    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for key in SLUG_KEYS {
                    if let Some(Value::String(candidate)) = map.get(*key) {
                        if is_action_slug(candidate) {
                            out.push(candidate.clone());
                            break;
                        }
                    }
                }
                for child in map.values() {
                    walk(child, out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, out);
                }
            }
            _ => {}
        }
    }

    let mut slugs = Vec::new();
    walk(input, &mut slugs);
    slugs
}

/// Whether a Composio tool result says an app connection is missing or
/// expired, and for which toolkit.
///
/// This reads vendor payloads, so there is no structured error type to lean
/// on; the compensations are that the patterns are few and specific, that it
/// only ever runs on results from the Composio server, and that the attempted
/// inner slugs give the toolkit deterministically rather than parsing it out
/// of prose. A false negative costs one extra model turn (the model relays
/// the error); a false positive shows a connect card for an app that is
/// already connected, whose button is then a no-op re-consent. Both fail
/// soft.
pub fn connection_needed(tool_input: &Value, output: &Value) -> Option<ConnectRequired> {
    let text = output.to_string().to_lowercase();
    const SIGNALS: &[&str] = &[
        "no connected account",
        "not connected",
        "connection required",
        "please connect",
        "connect your account",
        "reconnect",
        "could not find a connection",
    ];
    if !SIGNALS.iter().any(|signal| text.contains(signal)) {
        return None;
    }

    // The toolkit comes from what was attempted, not from the error prose.
    let toolkit_slug = attempted_slugs(tool_input)
        .into_iter()
        .find_map(|slug| toolkit_from_slug(&slug).filter(|t| t != SERVER_NAME))?;
    let app_name = toolkit_display(&toolkit_slug);
    Some(ConnectRequired {
        toolkit_slug,
        app_name,
    })
}

/// Payload for the `integration-connect-required` event.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectRequired {
    pub toolkit_slug: String,
    pub app_name: String,
}

/// The sentence the model gets in place of the raw vendor error, so its reply
/// is one line pointing at the button rather than a JSON recital.
pub fn connect_guidance(app_name: &str) -> String {
    format!(
        "{app_name} is not connected. A Connect button is now showing in the \
         conversation; tell the person in one sentence to press it, and do not \
         try to connect the app yourself."
    )
}

/// Build arguments for `COMPOSIO_MANAGE_CONNECTIONS` from its *discovered*
/// input schema rather than a hardcoded shape.
///
/// The meta-tool's schema is Composio's to change and has drifted before, so
/// the mapping is by property name and enum value: a property whose name
/// mentions toolkit/app gets the toolkit slug, and an operation-shaped
/// property gets whichever of its enum values matches the intent. Anything
/// unrecognised is simply left out, which the server answers with a
/// validation error a person can read.
pub fn manage_connections_args(schema: &Value, toolkit_slug: &str, intent: ConnectIntent) -> Value {
    let mut args = serde_json::Map::new();
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        // No readable schema: fall back to the simplest plausible shape.
        return json!({ "toolkit_slug": toolkit_slug, "mode": intent.keywords()[0] });
    };

    for (name, property) in properties {
        let lower = name.to_lowercase();
        if lower.contains("toolkit") || lower.contains("app") {
            if property.get("type").and_then(Value::as_str) == Some("array") {
                args.insert(name.clone(), json!([toolkit_slug]));
            } else {
                args.insert(name.clone(), json!(toolkit_slug));
            }
            continue;
        }
        if lower == "mode" || lower.contains("operation") || lower == "action" {
            let choice = property
                .get("enum")
                .and_then(Value::as_array)
                .and_then(|options| {
                    options.iter().filter_map(Value::as_str).find(|option| {
                        let option = option.to_lowercase();
                        intent
                            .keywords()
                            .iter()
                            .any(|keyword| option.contains(keyword))
                    })
                })
                .map(|s| s.to_string())
                .unwrap_or_else(|| intent.keywords()[0].to_string());
            args.insert(name.clone(), json!(choice));
        }
    }

    if args.is_empty() {
        return json!({ "toolkit_slug": toolkit_slug, "mode": intent.keywords()[0] });
    }
    Value::Object(args)
}

/// What the person asked the connections meta-tool to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectIntent {
    Initiate,
    List,
    Disconnect,
}

impl ConnectIntent {
    /// Lowercase keywords an operation enum value might contain, most
    /// specific first; the first one is also the bare fallback value.
    fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::Initiate => &["initiate", "create", "connect", "enable", "add"],
            Self::List => &["list", "get", "status", "check"],
            Self::Disconnect => &["delete", "remove", "disconnect", "disable", "revoke"],
        }
    }
}

/// The first consent URL in a tool result: the link the person's browser
/// opens to approve the app on the provider's own consent screen.
pub fn first_url(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            // Prose sometimes wraps the link; take from the scheme to the
            // next whitespace and shed trailing punctuation.
            let start = s.find("https://")?;
            let tail = s.get(start..)?;
            let url = tail
                .split_whitespace()
                .next()
                .unwrap_or(tail)
                .trim_end_matches([')', ']', '.', ',']);
            (!url.is_empty()).then(|| url.to_string())
        }
        Value::Object(map) => map.values().find_map(first_url),
        Value::Array(items) => items.iter().find_map(first_url),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blocked_tools_are_the_two_remote_shells() {
        let config = server_config();
        assert!(config.oauth);
        assert_eq!(config.blocked_tools.len(), 2);
        assert!(config
            .blocked_tools
            .iter()
            .all(|t| t.contains("REMOTE_WORKBENCH") || t.contains("REMOTE_BASH")));
    }

    #[test]
    fn a_missing_connection_is_detected_and_named_after_the_attempted_app() {
        let input = json!({
            "tools": [{"tool_slug": "GOOGLECALENDAR_EVENTS_LIST", "arguments": {}}]
        });
        let output = json!({
            "content": [{"type": "text", "text":
                "Error: No connected account found for this toolkit. Please connect your account."}]
        });
        let required = connection_needed(&input, &output).expect("detects");
        assert_eq!(required.toolkit_slug, "googlecalendar");
        assert_eq!(required.app_name, "Google Calendar");

        // An ordinary result raises nothing.
        let fine = json!({"content": [{"type": "text", "text": "3 events today"}]});
        assert!(connection_needed(&input, &fine).is_none());

        // A failure with no attempted app slug raises nothing either: there
        // is no app to put on the button.
        let no_slug_input = json!({"query": "calendar"});
        assert!(connection_needed(&no_slug_input, &output).is_none());
    }

    #[test]
    fn manage_connections_args_follow_the_discovered_schema() {
        // Today's plausible shape.
        let schema = json!({
            "type": "object",
            "properties": {
                "mode": {"type": "string", "enum": ["LIST", "INITIATE", "DELETE"]},
                "toolkit_slug": {"type": "string"}
            }
        });
        let args = manage_connections_args(&schema, "gmail", ConnectIntent::Initiate);
        assert_eq!(args["toolkit_slug"], json!("gmail"));
        assert_eq!(args["mode"], json!("INITIATE"));

        // A drifted shape: array of apps, differently named operation.
        let schema = json!({
            "type": "object",
            "properties": {
                "operation": {"type": "string", "enum": ["create_connection", "remove_connection"]},
                "apps": {"type": "array", "items": {"type": "string"}}
            }
        });
        let args = manage_connections_args(&schema, "gmail", ConnectIntent::Disconnect);
        assert_eq!(args["apps"], json!(["gmail"]));
        assert_eq!(args["operation"], json!("remove_connection"));

        // No schema at all still produces something a server can reject
        // legibly rather than a panic or an empty object.
        let args = manage_connections_args(&json!(null), "gmail", ConnectIntent::Initiate);
        assert_eq!(args["toolkit_slug"], json!("gmail"));
    }

    #[test]
    fn the_consent_url_is_found_wherever_the_result_put_it() {
        let result = json!({
            "content": [{"type": "text", "text":
                "Approve access here: https://accounts.google.com/o/oauth2/consent?x=1, then return."}]
        });
        assert_eq!(
            first_url(&result).as_deref(),
            Some("https://accounts.google.com/o/oauth2/consent?x=1")
        );
        assert_eq!(first_url(&json!({"ok": true})), None);
    }
}
