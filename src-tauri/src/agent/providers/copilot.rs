//! # GitHub Copilot as an inference provider
//!
//! Somebody with a Copilot plan already pays for Claude. Copilot proxies
//! Anthropic's Messages API at `{endpoints.api}/v1/messages`, so Juno drives
//! the same Claude models it drives with an API key, through the same
//! [`super::anthropic::AnthropicBrain`], with the person's GitHub sign-in as
//! the credential. No key to paste, no second bill.
//!
//! ## Where the credential comes from
//!
//! In order, first found wins:
//!
//! 1. A token saved in Juno's settings for the `copilot` provider (pasted, or
//!    imported from the Copilot CLI with [`import_copilot_cli_token`]).
//! 2. `COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, `GITHUB_TOKEN`, the variables the
//!    Copilot CLI itself honours, in its order.
//! 3. The GitHub CLI: `gh auth token`, then `gh`'s `hosts.yml` directly when
//!    the binary is not on a path Juno can see.
//!
//! A `gho_` token from `gh` is accepted by the Copilot API as a bearer token
//! as-is (verified against the live API 2026-11-12). The Copilot CLI keeps its
//! own token in the macOS Keychain, where reading it shows a permission
//! prompt, so that is only ever done when the person asks for it.
//!
//! Finding a token is not proof of a Copilot plan. [`status`] asks GitHub
//! (`copilot_internal/user`) before anything calls the provider usable, and
//! that answer also says which API host the account lives on (individual,
//! business and enterprise plans are served from different hosts).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::claude_cli::SignIn;
use crate::agent::core::AgentError;

/// The public Copilot API host. Used when GitHub does not name one.
pub const DEFAULT_API_BASE: &str = "https://api.githubcopilot.com";

/// Answers who the token belongs to, whether they have Copilot, and which API
/// host serves their plan.
const USER_ENDPOINT: &str = "https://api.github.com/copilot_internal/user";

/// Sent as `Copilot-Integration-Id`. Optional on the wire today, but it is how
/// Copilot attributes traffic from a GitHub CLI token, so send the one that
/// matches the token's origin.
pub const INTEGRATION_ID: &str = "copilot-developer-cli";

/// The variables the Copilot CLI reads, in its order of precedence.
pub const TOKEN_ENV_VARS: [&str; 3] = ["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"];

/// `gh auth token` is a file or Keychain read; anything slower is stuck.
const GH_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a validated credential is trusted before GitHub is asked again.
/// Requests that come back 401 drop it immediately regardless.
const VALIDATION_TTL: Duration = Duration::from_secs(30 * 60);

/// Long enough for a person to read the Keychain prompt and click Allow.
#[cfg(target_os = "macos")]
const KEYCHAIN_TIMEOUT: Duration = Duration::from_secs(120);

/// Where a token was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    /// Saved in Juno's settings for the `copilot` provider.
    Settings,
    /// One of [`TOKEN_ENV_VARS`].
    Env(&'static str),
    /// `gh auth token`, or `gh`'s `hosts.yml`.
    GhCli,
}

impl TokenSource {
    /// Short, human words for Settings: "from the GitHub CLI".
    pub fn describe(&self) -> String {
        match self {
            TokenSource::Settings => "saved in Juno".to_string(),
            TokenSource::Env(name) => format!("from ${name}"),
            TokenSource::GhCli => "from the GitHub CLI".to_string(),
        }
    }
}

/// A token and where it came from. `Debug` never prints the token.
#[derive(Clone, PartialEq, Eq)]
pub struct FoundToken {
    pub token: String,
    pub source: TokenSource,
}

impl std::fmt::Debug for FoundToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FoundToken")
            .field("token", &"<redacted>")
            .field("source", &self.source)
            .finish()
    }
}

/// What a request needs: who to call, and with what.
#[derive(Clone)]
pub struct Session {
    pub token: String,
    pub api_base: String,
}

impl Session {
    /// The Anthropic Messages endpoint on this account's Copilot API host.
    pub fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.api_base.trim_end_matches('/'))
    }
}

/// What Settings and onboarding show about Copilot.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct CopilotStatus {
    /// A token was found and GitHub confirmed it has Copilot chat.
    pub signed_in: bool,
    /// Some token was found, whether or not it turned out to work.
    pub token_found: bool,
    /// The GitHub login the plan belongs to.
    pub login: Option<String>,
    /// `individual`, `business`, `enterprise`, ... as GitHub reports it.
    pub plan: Option<String>,
    /// Where the token came from, in words.
    pub source: Option<String>,
    /// The Copilot CLI is signed in as this login. When nothing else is
    /// found, the person can import that sign-in (one Keychain prompt).
    pub copilot_cli_login: Option<String>,
    /// Why a found token cannot be used, in words a person can act on.
    pub problem: Option<String>,
}

/// The parts of `copilot_internal/user` Juno reads.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CopilotUser {
    pub login: Option<String>,
    pub copilot_plan: Option<String>,
    #[serde(default)]
    pub chat_enabled: Option<bool>,
    #[serde(default)]
    pub endpoints: Option<CopilotEndpoints>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CopilotEndpoints {
    pub api: Option<String>,
}

impl CopilotUser {
    /// The API host to call. Only an `https` URL is trusted; anything else
    /// falls back to the public host rather than sending a token somewhere odd.
    pub fn api_base(&self) -> String {
        self.endpoints
            .as_ref()
            .and_then(|e| e.api.as_deref())
            .map(|api| api.trim().trim_end_matches('/'))
            .filter(|api| api.starts_with("https://") && api.len() > "https://".len())
            .unwrap_or(DEFAULT_API_BASE)
            .to_string()
    }
}

/// How asking GitHub about a token went.
#[derive(Debug)]
enum Validation {
    /// The token has Copilot chat.
    Ok(CopilotUser),
    /// GitHub rejected the token outright (401).
    BadToken,
    /// The token is fine but the account cannot use Copilot chat here.
    NoCopilot(String),
    /// GitHub could not be reached; nothing is known.
    Unreachable(String),
}

struct Validated {
    token: String,
    source: TokenSource,
    user: CopilotUser,
    checked_at: std::time::Instant,
}

/// The last credential GitHub confirmed, reused by requests until it expires
/// or the API rejects it.
static VALIDATED: Mutex<Option<Validated>> = Mutex::new(None);

/// What the last [`status`] concluded, for synchronous callers (the provider
/// listing runs on every Settings render and cannot make a network call).
static LAST_SIGN_IN: AtomicU8 = AtomicU8::new(0);

fn remember_sign_in(sign_in: SignIn) {
    let encoded = match sign_in {
        SignIn::Unknown => 0,
        SignIn::SignedIn => 1,
        SignIn::SignedOut => 2,
    };
    LAST_SIGN_IN.store(encoded, Ordering::Relaxed);
}

/// What the last check said. `Unknown` until something has asked.
pub fn last_known_sign_in() -> SignIn {
    match LAST_SIGN_IN.load(Ordering::Relaxed) {
        1 => SignIn::SignedIn,
        2 => SignIn::SignedOut,
        _ => SignIn::Unknown,
    }
}

/// Forget the validated credential, so the next request finds and checks one
/// again. Called when the API answers 401.
pub fn invalidate() {
    if let Ok(mut guard) = VALIDATED.lock() {
        *guard = None;
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// The first token in the Copilot CLI's environment-variable order.
pub fn token_from_env(get: impl Fn(&str) -> Option<String>) -> Option<FoundToken> {
    TOKEN_ENV_VARS.iter().copied().find_map(|name| {
        non_empty(get(name)).map(|token| FoundToken {
            token,
            source: TokenSource::Env(name),
        })
    })
}

/// `gh auth token` prints the token and a newline. Anything else (an error
/// message, several words) is not a token.
pub fn parse_gh_token_output(stdout: &str) -> Option<String> {
    let token = stdout.trim();
    if token.is_empty() || token.chars().any(char::is_whitespace) {
        return None;
    }
    Some(token.to_string())
}

/// The github.com token in `gh`'s `hosts.yml`, for when `gh` stored it in
/// plain text (no Keychain) and the binary is not somewhere Juno can see.
pub fn parse_gh_hosts_yml(contents: &str) -> Option<String> {
    let doc: serde_yaml::Value = serde_yaml::from_str(contents).ok()?;
    let host = doc.get("github.com")?;
    if let Some(token) = host.get("oauth_token").and_then(|t| t.as_str()) {
        return parse_gh_token_output(token);
    }
    // Multi-account layout: the active user's token lives under `users`.
    let user = host.get("user").and_then(|u| u.as_str())?;
    host.get("users")?
        .get(user)?
        .get("oauth_token")?
        .as_str()
        .and_then(parse_gh_token_output)
}

/// The login the Copilot CLI last signed in as, from `~/.copilot/config.json`.
///
/// That file allows `//` comment lines, which `serde_json` does not, so they
/// are dropped first. Only github.com logins count: the import reads a
/// Keychain item keyed on the host.
pub fn parse_copilot_cli_login(contents: &str) -> Option<String> {
    let without_comments: String = contents
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let value: serde_json::Value = serde_json::from_str(&without_comments).ok()?;
    let user = value.get("lastLoggedInUser")?;
    let host = user.get("host").and_then(|h| h.as_str())?;
    if host.trim_end_matches('/') != "https://github.com" {
        return None;
    }
    user.get("login")
        .and_then(|l| l.as_str())
        .map(str::to_string)
        .filter(|l| !l.is_empty())
}

fn copilot_cli_login() -> Option<String> {
    let path = dirs::home_dir()?.join(".copilot").join("config.json");
    let contents = std::fs::read_to_string(path).ok()?;
    parse_copilot_cli_login(&contents)
}

fn detect_gh() -> Option<PathBuf> {
    let candidates = [
        Some(PathBuf::from("/opt/homebrew/bin/gh")),
        Some(PathBuf::from("/usr/local/bin/gh")),
        dirs::home_dir().map(|h| h.join(".local/bin/gh")),
        Some(PathBuf::from("/usr/bin/gh")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|c| c.is_file())
        .or_else(|| crate::user_env::find_on_user_path("gh"))
}

fn gh_hosts_path() -> Option<PathBuf> {
    if let Some(dir) = non_empty(std::env::var("GH_CONFIG_DIR").ok()) {
        return Some(PathBuf::from(dir).join("hosts.yml"));
    }
    if let Some(dir) = non_empty(std::env::var("XDG_CONFIG_HOME").ok()) {
        return Some(PathBuf::from(dir).join("gh").join("hosts.yml"));
    }
    dirs::home_dir().map(|h| h.join(".config").join("gh").join("hosts.yml"))
}

async fn token_from_gh() -> Option<String> {
    if let Some(gh) = detect_gh() {
        let mut cmd = tokio::process::Command::new(&gh);
        cmd.args(["auth", "token", "--hostname", "github.com"])
            .env("GH_PROMPT_DISABLED", "1")
            .env("NO_COLOR", "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        match tokio::time::timeout(GH_TIMEOUT, cmd.output()).await {
            Ok(Ok(output)) if output.status.success() => {
                if let Some(token) = parse_gh_token_output(&String::from_utf8_lossy(&output.stdout))
                {
                    return Some(token);
                }
            }
            Ok(Ok(_)) => debug!("[Copilot] `gh auth token` found no github.com sign-in"),
            Ok(Err(e)) => debug!("[Copilot] Could not run gh: {e}"),
            Err(_) => warn!("[Copilot] `gh auth token` timed out"),
        }
    }
    let path = gh_hosts_path()?;
    let contents = tokio::fs::read_to_string(path).await.ok()?;
    parse_gh_hosts_yml(&contents)
}

/// Every token Juno can use without asking, in precedence order.
async fn discover(configured: Option<&str>) -> Vec<FoundToken> {
    let mut found = Vec::new();
    if let Some(token) = non_empty(configured.map(str::to_string)) {
        found.push(FoundToken {
            token,
            source: TokenSource::Settings,
        });
    }
    if let Some(token) = token_from_env(|name| std::env::var(name).ok()) {
        found.push(token);
    }
    if let Some(token) = token_from_gh().await {
        found.push(FoundToken {
            token,
            source: TokenSource::GhCli,
        });
    }
    // The same token reached by two routes only needs asking about once.
    let mut seen = std::collections::HashSet::new();
    found.retain(|t| seen.insert(t.token.clone()));
    found
}

fn http_client() -> Result<reqwest::Client, AgentError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(
            crate::constants::timeouts::HTTP_CONNECT_TIMEOUT_SECONDS,
        ))
        .build()
        .map_err(|e| AgentError::LlmError(format!("Failed to create HTTP client: {e}")))
}

fn user_agent() -> String {
    format!("Juno/{}", env!("CARGO_PKG_VERSION"))
}

async fn validate(client: &reqwest::Client, token: &str) -> Validation {
    let response = client
        .get(USER_ENDPOINT)
        .header("Authorization", format!("token {token}"))
        .header("Accept", "application/json")
        .header("User-Agent", user_agent())
        .send()
        .await;
    let response = match response {
        Ok(r) => r,
        Err(e) => return Validation::Unreachable(e.to_string()),
    };
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Validation::BadToken;
    }
    if status.is_success() {
        return match response.json::<CopilotUser>().await {
            Ok(user) if user.chat_enabled == Some(false) => Validation::NoCopilot(
                "Copilot Chat is turned off for this GitHub account.".to_string(),
            ),
            Ok(user) => Validation::Ok(user),
            Err(e) => Validation::Unreachable(format!("Unreadable answer from GitHub: {e}")),
        };
    }
    if status.is_server_error() {
        return Validation::Unreachable(format!("GitHub answered {status}"));
    }

    // A token type `copilot_internal/user` does not accept (a fine-grained
    // PAT with only "Copilot Requests") can still call the API. Ask the API.
    match probe_models(client, token).await {
        Ok(true) => Validation::Ok(CopilotUser {
            login: None,
            copilot_plan: None,
            chat_enabled: Some(true),
            endpoints: None,
        }),
        Ok(false) => Validation::NoCopilot(
            "This GitHub account does not have access to GitHub Copilot.".to_string(),
        ),
        Err(e) => Validation::Unreachable(e),
    }
}

async fn probe_models(client: &reqwest::Client, token: &str) -> Result<bool, String> {
    let response = client
        .get(format!("{DEFAULT_API_BASE}/models"))
        .bearer_auth(token)
        .header("Copilot-Integration-Id", INTEGRATION_ID)
        .header("User-Agent", user_agent())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if response.status().is_server_error() {
        return Err(format!("Copilot answered {}", response.status()));
    }
    Ok(response.status().is_success())
}

fn store(found: &FoundToken, user: &CopilotUser) {
    if let Ok(mut guard) = VALIDATED.lock() {
        *guard = Some(Validated {
            token: found.token.clone(),
            source: found.source.clone(),
            user: user.clone(),
            checked_at: std::time::Instant::now(),
        });
    }
}

fn status_from(found: &FoundToken, user: &CopilotUser) -> CopilotStatus {
    CopilotStatus {
        signed_in: true,
        token_found: true,
        login: user.login.clone(),
        plan: user.copilot_plan.clone(),
        source: Some(found.source.describe()),
        copilot_cli_login: None,
        problem: None,
    }
}

/// Find a token, ask GitHub about it, and remember the answer.
///
/// `configured` is the token saved in Juno's settings for `copilot`, if any.
pub async fn status(configured: Option<&str>) -> CopilotStatus {
    let candidates = discover(configured).await;
    let cli_login = copilot_cli_login();
    if candidates.is_empty() {
        remember_sign_in(SignIn::SignedOut);
        invalidate();
        return CopilotStatus {
            copilot_cli_login: cli_login,
            ..CopilotStatus::default()
        };
    }

    let client = match http_client() {
        Ok(c) => c,
        Err(e) => {
            return CopilotStatus {
                token_found: true,
                problem: Some(e.to_string()),
                ..CopilotStatus::default()
            }
        }
    };

    let mut problem = None;
    let mut unreachable = false;
    for found in &candidates {
        match validate(&client, &found.token).await {
            Validation::Ok(user) => {
                store(found, &user);
                remember_sign_in(SignIn::SignedIn);
                info!(
                    "[Copilot] Signed in as {} ({} plan), token {}",
                    user.login.as_deref().unwrap_or("unknown"),
                    user.copilot_plan.as_deref().unwrap_or("unknown"),
                    found.source.describe()
                );
                return status_from(found, &user);
            }
            Validation::BadToken => {
                debug!(
                    "[Copilot] GitHub rejected the token {}",
                    found.source.describe()
                );
                if problem.is_none() {
                    problem = Some(
                        "GitHub rejected the saved sign-in. Run `gh auth login` and try again."
                            .to_string(),
                    );
                }
            }
            Validation::NoCopilot(why) => {
                problem = Some(why);
            }
            Validation::Unreachable(why) => {
                warn!("[Copilot] Could not check the sign-in: {why}");
                unreachable = true;
            }
        }
    }

    // Offline is not signed out: leave a provider that worked an hour ago
    // alone rather than greying it out because the Wi-Fi blinked.
    remember_sign_in(if unreachable {
        SignIn::Unknown
    } else {
        SignIn::SignedOut
    });
    if !unreachable {
        invalidate();
    }
    CopilotStatus {
        token_found: true,
        copilot_cli_login: cli_login,
        problem: problem.or_else(|| {
            unreachable.then(|| "Could not reach GitHub to check the sign-in.".to_string())
        }),
        ..CopilotStatus::default()
    }
}

/// The credential a request should use, validated within [`VALIDATION_TTL`].
pub async fn session(configured: Option<&str>) -> Result<Session, AgentError> {
    let configured_token = non_empty(configured.map(str::to_string));
    if let Ok(guard) = VALIDATED.lock() {
        if let Some(v) = guard.as_ref() {
            // A token saved in settings outranks a discovered one, so a cached
            // `gh` token must not shadow one the person has since pasted.
            let shadowed = configured_token
                .as_ref()
                .is_some_and(|t| *t != v.token && v.source != TokenSource::Settings);
            if !shadowed && v.checked_at.elapsed() < VALIDATION_TTL {
                return Ok(Session {
                    token: v.token.clone(),
                    api_base: v.user.api_base(),
                });
            }
        }
    }

    let status = status(configured_token.as_deref()).await;
    if let Ok(guard) = VALIDATED.lock() {
        if let Some(v) = guard.as_ref() {
            return Ok(Session {
                token: v.token.clone(),
                api_base: v.user.api_base(),
            });
        }
    }
    Err(AgentError::ConfigurationError(not_signed_in_message(
        &status,
    )))
}

/// Why Copilot cannot run, and what to do about it.
pub fn not_signed_in_message(status: &CopilotStatus) -> String {
    if let Some(problem) = &status.problem {
        return format!("GitHub Copilot is not available: {problem}");
    }
    if let Some(login) = &status.copilot_cli_login {
        return format!(
            "GitHub Copilot is signed in as {login} in the Copilot CLI, but Juno has not been \
             given that sign-in yet. Open Settings > AI Provider and choose \"Use Copilot CLI \
             sign-in\", or run `gh auth login`."
        );
    }
    "GitHub Copilot is not signed in. Run `gh auth login` in Terminal, or set \
     COPILOT_GITHUB_TOKEN, then try again."
        .to_string()
}

/// Read the Copilot CLI's own GitHub token from the macOS Keychain.
///
/// macOS asks the person first: this is only ever called from a button they
/// pressed, never during detection.
pub async fn import_copilot_cli_token() -> Result<String, String> {
    let login = copilot_cli_login()
        .ok_or_else(|| "The Copilot CLI is not signed in to github.com.".to_string())?;
    read_keychain_token(&login).await
}

#[cfg(target_os = "macos")]
async fn read_keychain_token(login: &str) -> Result<String, String> {
    let account = format!("https://github.com:{login}");
    let mut cmd = tokio::process::Command::new("/usr/bin/security");
    cmd.args([
        "find-generic-password",
        "-s",
        "copilot-cli",
        "-a",
        account.as_str(),
        "-w",
    ])
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .kill_on_drop(true);
    match tokio::time::timeout(KEYCHAIN_TIMEOUT, cmd.output()).await {
        Ok(Ok(output)) if output.status.success() => {
            parse_gh_token_output(&String::from_utf8_lossy(&output.stdout))
                .ok_or_else(|| "The Copilot CLI's Keychain entry was empty.".to_string())
        }
        Ok(Ok(_)) => Err("macOS did not allow access to the Copilot CLI sign-in.".to_string()),
        Ok(Err(e)) => Err(format!("Could not read the Keychain: {e}")),
        Err(_) => Err("Timed out waiting for Keychain access.".to_string()),
    }
}

#[cfg(not(target_os = "macos"))]
async fn read_keychain_token(_login: &str) -> Result<String, String> {
    Err("Importing the Copilot CLI sign-in is only supported on macOS.".to_string())
}

/// `X-Initiator` for a request whose last message is `last_is_tool_result`.
///
/// Copilot counts a premium request when a person asks something, not for
/// every step of the agent loop that follows; the Copilot CLI marks the
/// follow-ups `agent`. Juno's follow-ups are exactly the turns that carry
/// tool results back.
pub fn initiator(last_is_tool_result: bool) -> &'static str {
    if last_is_tool_result {
        "agent"
    } else {
        "user"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_vars_follow_the_copilot_cli_order() {
        let env = |name: &str| match name {
            "GH_TOKEN" => Some("gh-token".to_string()),
            "GITHUB_TOKEN" => Some("github-token".to_string()),
            _ => None,
        };
        let found = token_from_env(env).expect("a token");
        assert_eq!(found.token, "gh-token");
        assert_eq!(found.source, TokenSource::Env("GH_TOKEN"));

        let all = |name: &str| Some(format!("{name}-value"));
        assert_eq!(
            token_from_env(all).map(|t| t.source),
            Some(TokenSource::Env("COPILOT_GITHUB_TOKEN"))
        );
    }

    #[test]
    fn an_empty_variable_is_not_a_token() {
        let env = |name: &str| match name {
            "COPILOT_GITHUB_TOKEN" => Some("  ".to_string()),
            "GITHUB_TOKEN" => Some("real".to_string()),
            _ => None,
        };
        assert_eq!(
            token_from_env(env).map(|t| t.source),
            Some(TokenSource::Env("GITHUB_TOKEN"))
        );
    }

    #[test]
    fn gh_output_is_one_token_or_nothing() {
        assert_eq!(
            parse_gh_token_output("gho_abc123\n"),
            Some("gho_abc123".to_string())
        );
        assert_eq!(parse_gh_token_output("\n"), None);
        assert_eq!(
            parse_gh_token_output("no oauth token found for github.com"),
            None
        );
    }

    #[test]
    fn hosts_yml_single_and_multi_account() {
        let single = "github.com:\n    oauth_token: gho_single\n    user: lacy\n";
        assert_eq!(parse_gh_hosts_yml(single), Some("gho_single".to_string()));

        let multi = "github.com:\n    users:\n        lacy:\n            oauth_token: gho_multi\n        other:\n            oauth_token: gho_other\n    user: lacy\n";
        assert_eq!(parse_gh_hosts_yml(multi), Some("gho_multi".to_string()));

        // Keychain-backed gh keeps no token in the file.
        let keyring = "github.com:\n    users:\n        lacy: {}\n    user: lacy\n";
        assert_eq!(parse_gh_hosts_yml(keyring), None);
        assert_eq!(
            parse_gh_hosts_yml("ghe.example.com:\n  oauth_token: x\n"),
            None
        );
    }

    #[test]
    fn copilot_cli_config_tolerates_comments() {
        let config = r#"// Copilot CLI settings
{
  // who is signed in
  "lastLoggedInUser": {"host": "https://github.com", "login": "lacy"},
  "loggedInUsers": []
}"#;
        assert_eq!(parse_copilot_cli_login(config), Some("lacy".to_string()));
        let ghe = r#"{"lastLoggedInUser": {"host": "https://ghe.example.com", "login": "lacy"}}"#;
        assert_eq!(parse_copilot_cli_login(ghe), None);
        assert_eq!(parse_copilot_cli_login("{}"), None);
    }

    #[test]
    fn the_api_host_comes_from_github_and_must_be_https() {
        let user: CopilotUser = serde_json::from_str(
            r#"{"login":"lacy","copilot_plan":"enterprise","chat_enabled":true,
                "endpoints":{"api":"https://api.enterprise.githubcopilot.com/"},"extra":1}"#,
        )
        .expect("parses");
        assert_eq!(user.api_base(), "https://api.enterprise.githubcopilot.com");

        let plain: CopilotUser =
            serde_json::from_str(r#"{"endpoints":{"api":"http://evil.example"}}"#).expect("parses");
        assert_eq!(plain.api_base(), DEFAULT_API_BASE);

        let none: CopilotUser = serde_json::from_str("{}").expect("parses");
        assert_eq!(none.api_base(), DEFAULT_API_BASE);
    }

    #[test]
    fn messages_url_joins_cleanly() {
        let session = Session {
            token: "t".into(),
            api_base: "https://api.githubcopilot.com/".into(),
        };
        assert_eq!(
            session.messages_url(),
            "https://api.githubcopilot.com/v1/messages"
        );
    }

    #[test]
    fn tool_result_turns_are_agent_initiated() {
        assert_eq!(initiator(false), "user");
        assert_eq!(initiator(true), "agent");
    }

    #[test]
    fn debug_never_prints_the_token() {
        let found = FoundToken {
            token: "gho_secret".into(),
            source: TokenSource::GhCli,
        };
        assert!(!format!("{found:?}").contains("gho_secret"));
    }

    #[test]
    fn the_message_points_at_the_fix() {
        let none = not_signed_in_message(&CopilotStatus::default());
        assert!(none.contains("gh auth login"));
        let cli = not_signed_in_message(&CopilotStatus {
            copilot_cli_login: Some("lacy".into()),
            ..CopilotStatus::default()
        });
        assert!(cli.contains("lacy"));
    }
}
