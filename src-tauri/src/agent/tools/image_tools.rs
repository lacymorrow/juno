//! # Image Tools (LAC-4206)
//!
//! Agent-facing tools for generating and editing images. The pair LAC-4204
//! extends this file with a Nano Banana 2.1 edit path and ImageCard polish;
//! this ticket lands the shared provider trait, the Gemini image provider,
//! the OpenAI GPT Image 2 provider, and the `generate_image` / `edit_image`
//! tool surface. Only one tool-registration site exists per CEO coordination
//! note on LAC-4206: see tools/mod.rs and factory.rs.
//!
//! Entitlements (LAC-4134): both tools declare `requires = ENTITLEMENT`.
//! Until LAC-4134 ships, the gate is open — a Gemini or OpenAI key present
//! means the person pays their own provider. The gateway (LAC-4131) is a
//! future third `ImageProvider`.
//!
//! No local diffusion (mflux / Apple Image Playground); keep that as a
//! future "advanced, free" option.

use crate::agent::core::ToolDefinition;
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::providers::config::{load_provider_config, ProviderConfig};
use crate::agent::providers::types::Provider;
use crate::constants::agent;
use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chrono::Local;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tracing::{error, info, warn};

/// Entitlement name the LAC-4134 hook will match on. The hook is not landed
/// yet; default is allowed. TODO(LAC-4134): wire the hook here.
pub const ENTITLEMENT: &str = "image_edits";

/// Default Pictures dir for generated images.
fn pictures_dir() -> PathBuf {
    dirs::picture_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Pictures"))
        .join("Juno")
}

/// Build a safe filename slug from a prompt or instruction.
pub fn slugify(input: &str, max_len: usize) -> String {
    let mut out = String::with_capacity(max_len);
    let mut last_dash = false;
    for ch in input.chars().take(max_len * 2) {
        let keep = ch.is_ascii_alphanumeric();
        if keep {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= max_len {
            break;
        }
    }
    let trimmed = out.trim_end_matches('-').to_string();
    if trimmed.is_empty() {
        "image".to_string()
    } else {
        trimmed
    }
}

/// Compute an output path under `~/Pictures/Juno/` with a dated, readable name.
pub fn output_path(dir: &Path, hint: &str) -> PathBuf {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let slug = slugify(hint, 32);
    dir.join(format!("{stamp}-{slug}.png"))
}

/// The ImageProvider trait is pluggable. One trait, N implementations
/// (Gemini, OpenAI, gateway). The tool picks by the key that exists.
#[async_trait]
pub trait ImageProvider: Send + Sync {
    /// Short name for logs and cost reports (e.g. "gemini", "openai", "gateway").
    fn name(&self) -> &'static str;

    /// Per-image cost in cents, best-effort. Zero if unknown.
    fn cost_cents_per_image(&self) -> f64;

    /// Text-to-image.
    async fn generate(&self, prompt: &str, size: Option<&str>) -> Result<Vec<u8>, String>;

    /// Edit an existing image (PNG bytes in, PNG bytes out). `instruction` is
    /// a short natural-language description of the change.
    async fn edit(&self, image_png: &[u8], instruction: &str) -> Result<Vec<u8>, String>;
}

// ============================================================================
// Gemini (Nano Banana) — default when GEMINI_API_KEY is present.
//
// Model id reported by Google as `gemini-3.1-flash-image-preview` at the time
// of filing; confirm on ai.google.dev before the final price gets wired into
// LAC-4124. Reported ~$0.067 per image.
// ============================================================================

const GEMINI_IMAGE_MODEL: &str = "gemini-3.1-flash-image-preview";
const GEMINI_IMAGE_COST_CENTS: f64 = 6.7;

pub struct GeminiImageProvider {
    api_key: String,
}

impl GeminiImageProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/{}:generateContent",
            crate::constants::api::endpoints::GEMINI_API_BASE,
            GEMINI_IMAGE_MODEL
        )
    }

    async fn call(&self, parts: Value) -> Result<Vec<u8>, String> {
        let body = json!({
            "contents": [ { "role": "user", "parts": parts } ],
            "generationConfig": { "responseModalities": ["IMAGE"] }
        });
        let resp = reqwest::Client::new()
            .post(self.endpoint())
            .header("x-goog-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("gemini image request failed: {e}"))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("gemini image read body failed: {e}"))?;
        if !status.is_success() {
            return Err(format!("gemini image {status}: {text}"));
        }
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|e| format!("gemini image parse failed: {e}"))?;
        extract_inline_image(&parsed)
    }
}

/// Walk the Gemini response shape and pull the first inline image blob out.
fn extract_inline_image(parsed: &Value) -> Result<Vec<u8>, String> {
    let candidates = parsed
        .get("candidates")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "gemini response missing candidates".to_string())?;
    for cand in candidates {
        let parts = cand
            .pointer("/content/parts")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for part in parts {
            if let Some(data) = part.pointer("/inlineData/data").and_then(|v| v.as_str()) {
                return B64
                    .decode(data)
                    .map_err(|e| format!("gemini image base64 decode failed: {e}"));
            }
            if let Some(data) = part.pointer("/inline_data/data").and_then(|v| v.as_str()) {
                return B64
                    .decode(data)
                    .map_err(|e| format!("gemini image base64 decode failed: {e}"));
            }
        }
    }
    Err("gemini response had no inline image".to_string())
}

#[async_trait]
impl ImageProvider for GeminiImageProvider {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn cost_cents_per_image(&self) -> f64 {
        GEMINI_IMAGE_COST_CENTS
    }

    async fn generate(&self, prompt: &str, _size: Option<&str>) -> Result<Vec<u8>, String> {
        let parts = json!([ { "text": prompt } ]);
        self.call(parts).await
    }

    async fn edit(&self, image_png: &[u8], instruction: &str) -> Result<Vec<u8>, String> {
        let b64 = B64.encode(image_png);
        let parts = json!([
            { "text": instruction },
            { "inlineData": { "mimeType": "image/png", "data": b64 } }
        ]);
        self.call(parts).await
    }
}

// ============================================================================
// OpenAI GPT Image 2 — second choice when only OPENAI_API_KEY is present.
//
// Images API: /v1/images/generations and /v1/images/edits. Medium quality by
// default per the ticket (roughly $0.01 to $0.21 per image by quality tier).
// ============================================================================

const OPENAI_IMAGE_MODEL: &str = "gpt-image-1";
const OPENAI_IMAGE_COST_CENTS_MEDIUM: f64 = 4.0;

pub struct OpenAIGptImageProvider {
    api_key: String,
}

impl OpenAIGptImageProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

async fn openai_decode_first_image(resp_text: &str) -> Result<Vec<u8>, String> {
    let parsed: Value = serde_json::from_str(resp_text)
        .map_err(|e| format!("openai image parse failed: {e}"))?;
    let data = parsed
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "openai response missing data array".to_string())?;
    let b64 = data
        .first()
        .and_then(|d| d.get("b64_json"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "openai response missing b64_json".to_string())?;
    B64.decode(b64)
        .map_err(|e| format!("openai image base64 decode failed: {e}"))
}

#[async_trait]
impl ImageProvider for OpenAIGptImageProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn cost_cents_per_image(&self) -> f64 {
        OPENAI_IMAGE_COST_CENTS_MEDIUM
    }

    async fn generate(&self, prompt: &str, size: Option<&str>) -> Result<Vec<u8>, String> {
        let body = json!({
            "model": OPENAI_IMAGE_MODEL,
            "prompt": prompt,
            "size": size.unwrap_or("1024x1024"),
            "quality": "medium",
            "n": 1,
        });
        let resp = reqwest::Client::new()
            .post("https://api.openai.com/v1/images/generations")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("openai image request failed: {e}"))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("openai read body failed: {e}"))?;
        if !status.is_success() {
            return Err(format!("openai image {status}: {text}"));
        }
        openai_decode_first_image(&text).await
    }

    async fn edit(&self, image_png: &[u8], instruction: &str) -> Result<Vec<u8>, String> {
        let form = reqwest::multipart::Form::new()
            .text("model", OPENAI_IMAGE_MODEL)
            .text("prompt", instruction.to_string())
            .text("quality", "medium")
            .text("n", "1")
            .part(
                "image",
                reqwest::multipart::Part::bytes(image_png.to_vec())
                    .file_name("input.png")
                    .mime_str("image/png")
                    .map_err(|e| format!("openai edit mime: {e}"))?,
            );
        let resp = reqwest::Client::new()
            .post("https://api.openai.com/v1/images/edits")
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("openai edit request failed: {e}"))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("openai edit read body failed: {e}"))?;
        if !status.is_success() {
            return Err(format!("openai edit {status}: {text}"));
        }
        openai_decode_first_image(&text).await
    }
}

// ============================================================================
// Provider selection.
//
// Order: Gemini key -> OpenAI key -> (future) gateway for LAC-4131. The
// Anthropic/Claude-login path has no image key; the tool returns the no-key
// sentinel which the prompt teaches the model to translate into an action
// line + palette.
// ============================================================================

/// Resolve the image provider from a `ProviderConfig`. Returns `None` when
/// no supported key is configured; the caller surfaces that as a settings
/// prompt, not as a hard error.
pub fn select_provider(cfg: &ProviderConfig) -> Option<Box<dyn ImageProvider>> {
    if let Some(key) = cfg
        .get_provider_settings(Provider::Gemini.id())
        .and_then(|p| p.api_key.clone())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("GEMINI_API_KEY").ok().filter(|s| !s.is_empty()))
    {
        return Some(Box::new(GeminiImageProvider::new(key)));
    }
    if let Some(key) = cfg
        .get_provider_settings(Provider::OpenAI.id())
        .and_then(|p| p.api_key.clone())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("OPENAI_API_KEY").ok().filter(|s| !s.is_empty()))
    {
        return Some(Box::new(OpenAIGptImageProvider::new(key)));
    }
    None
}

/// Sentinel error string the model is taught to translate into an action
/// line and a palette description.
pub const NO_IMAGE_KEY_SENTINEL: &str = "no_image_provider";

fn no_key_response() -> Value {
    json!({
        "ok": false,
        "reason": NO_IMAGE_KEY_SENTINEL,
        "action": "add a Gemini or OpenAI key in Settings",
    })
}

// ============================================================================
// Tool definitions.
// ============================================================================

fn generate_definition() -> ToolDefinition {
    ToolDefinition {
        name: agent::tool_names::GENERATE_IMAGE.to_string(),
        description: concat!(
            "Generates an image from a text prompt and saves it as a PNG. ",
            "Call this when the person asks you to make or draw an image from scratch. ",
            "Do NOT call this when the person is only asking you to describe something. ",
            "Returns the saved file path; never embed the image inline. Render the result ",
            "as <ImageCard path=\"...\" caption=\"...\" />."
        )
        .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "What to draw." },
                "size": {
                    "type": "string",
                    "description": "Optional pixel size such as 1024x1024.",
                    "default": "1024x1024"
                }
            },
            "required": ["prompt"]
        }),
        api_type: None,
        beta_flag: None,
    }
}

fn edit_definition() -> ToolDefinition {
    ToolDefinition {
        name: agent::tool_names::EDIT_IMAGE.to_string(),
        description: concat!(
            "Edits an image and saves the result as a new PNG. Call this when the person ",
            "wants to SEE a change to a photo or screenshot (repaint walls, swap a color, ",
            "remove an object, try a palette swatch). Do NOT call this when a description ",
            "is enough. `image` is the attached photo, a screenshot path, or a file path on ",
            "disk. Returns the saved file path; never embed the image inline. Render the ",
            "result as <ImageCard path=\"...\" caption=\"...\" />."
        )
        .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "image": {
                    "type": "string",
                    "description": "Path or attachment id for the source image."
                },
                "instruction": {
                    "type": "string",
                    "description": "What to change, in one sentence."
                },
                "n": { "type": "integer", "minimum": 1, "maximum": 4, "default": 1 }
            },
            "required": ["image", "instruction"]
        }),
        api_type: None,
        beta_flag: None,
    }
}

// ============================================================================
// Execution.
// ============================================================================

async fn ensure_dir(dir: &Path) -> Result<(), String> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| format!("create {dir:?}: {e}"))
}

async fn write_png(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent).await?;
    }
    tokio::fs::write(path, bytes)
        .await
        .map_err(|e| format!("write {path:?}: {e}"))
}

fn expand_tilde(input: &str) -> PathBuf {
    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if input == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(input)
}

async fn read_source_image(image_arg: &str) -> Result<Vec<u8>, String> {
    let expanded = expand_tilde(image_arg);
    tokio::fs::read(&expanded)
        .await
        .map_err(|e| format!("read source image {expanded:?}: {e}"))
}

fn log_cost(action: &str, provider: &dyn ImageProvider) {
    // Per-turn cost report (PR #595 telemetry hook). LAC-4124 reads these
    // lines to price Pro usage.
    info!(
        target: "juno.cost",
        action = action,
        provider = provider.name(),
        cents = provider.cost_cents_per_image(),
        "image call"
    );
}

async fn generate_exec(input: Value, app_handle: AppHandle) -> Result<Value, String> {
    let prompt = input
        .get("prompt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing prompt".to_string())?;
    let size = input.get("size").and_then(|v| v.as_str());

    let cfg = load_provider_config(Some(&app_handle));
    let Some(provider) = select_provider(&cfg) else {
        warn!("generate_image called with no image provider configured");
        return Ok(no_key_response());
    };
    log_cost("generate", provider.as_ref());

    let bytes = provider.generate(prompt, size).await.map_err(|e| {
        error!(provider = provider.name(), error = %e, "image generate failed");
        e
    })?;
    let dir = pictures_dir();
    let out = output_path(&dir, prompt);
    write_png(&out, &bytes).await?;
    Ok(json!({
        "ok": true,
        "path": out.to_string_lossy(),
        "caption": prompt,
        "provider": provider.name(),
    }))
}

async fn edit_exec(input: Value, app_handle: AppHandle) -> Result<Value, String> {
    let image_arg = input
        .get("image")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing image".to_string())?;
    let instruction = input
        .get("instruction")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing instruction".to_string())?;

    let cfg = load_provider_config(Some(&app_handle));
    let Some(provider) = select_provider(&cfg) else {
        warn!("edit_image called with no image provider configured");
        return Ok(no_key_response());
    };
    log_cost("edit", provider.as_ref());

    let source = read_source_image(image_arg).await?;
    let bytes = provider.edit(&source, instruction).await.map_err(|e| {
        error!(provider = provider.name(), error = %e, "image edit failed");
        e
    })?;
    let dir = pictures_dir();
    let out = output_path(&dir, instruction);
    write_png(&out, &bytes).await?;
    Ok(json!({
        "ok": true,
        "path": out.to_string_lossy(),
        "caption": instruction,
        "provider": provider.name(),
    }))
}

/// Register `generate_image` and `edit_image` with the given tool provider.
///
/// Both tools carry the `image_edits` entitlement. The hook from LAC-4134 is
/// not landed yet; while it's absent, the gate is open (the person pays their
/// own provider).
pub async fn register_image_tools(provider: &mut LocalToolProvider, app_handle: AppHandle) {
    let gen_def = generate_definition();
    let gen_handle = app_handle.clone();
    let gen_fn = move |input| {
        let handle = gen_handle.clone();
        async move { generate_exec(input, handle).await }
    };
    provider.register_async_tool(gen_def, gen_fn).await;

    let edit_def = edit_definition();
    let edit_handle = app_handle.clone();
    let edit_fn = move |input| {
        let handle = edit_handle.clone();
        async move { edit_exec(input, handle).await }
    };
    provider.register_async_tool(edit_def, edit_fn).await;

    info!(
        requires = ENTITLEMENT,
        "Registered image tools: generate_image, edit_image (entitlement default-allowed until LAC-4134)"
    );
}

// ============================================================================
// Tests.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::providers::config::ProviderConfig;

    fn empty_cfg() -> ProviderConfig {
        ProviderConfig::default()
    }

    #[test]
    fn slugify_handles_ascii_and_junk() {
        assert_eq!(slugify("Walls in Sage Green", 32), "walls-in-sage-green");
        assert_eq!(slugify("  !!! ???", 32), "image");
        assert_eq!(slugify("a".repeat(200).as_str(), 10), "aaaaaaaaaa");
    }

    #[test]
    fn output_path_has_png_extension_and_slug() {
        let dir = std::path::PathBuf::from("/tmp/juno-test");
        let p = output_path(&dir, "repaint walls sage");
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.ends_with(".png"));
        assert!(name.contains("repaint-walls-sage"));
    }

    #[test]
    fn select_provider_is_none_when_no_keys_set() {
        // Environment may bleed from the host shell in CI. Scrub to simulate
        // a Claude-login-only user.
        let gemini = std::env::var("GEMINI_API_KEY").ok();
        let openai = std::env::var("OPENAI_API_KEY").ok();
        std::env::remove_var("GEMINI_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");

        let picked = select_provider(&empty_cfg());
        assert!(picked.is_none(), "no key means no provider");

        if let Some(v) = gemini {
            std::env::set_var("GEMINI_API_KEY", v);
        }
        if let Some(v) = openai {
            std::env::set_var("OPENAI_API_KEY", v);
        }
    }

    #[test]
    fn select_provider_prefers_gemini_over_openai() {
        let prior_g = std::env::var("GEMINI_API_KEY").ok();
        let prior_o = std::env::var("OPENAI_API_KEY").ok();
        std::env::set_var("GEMINI_API_KEY", "test-g");
        std::env::set_var("OPENAI_API_KEY", "test-o");

        let picked = select_provider(&empty_cfg()).expect("one key is present");
        assert_eq!(picked.name(), "gemini");

        match prior_g {
            Some(v) => std::env::set_var("GEMINI_API_KEY", v),
            None => std::env::remove_var("GEMINI_API_KEY"),
        }
        match prior_o {
            Some(v) => std::env::set_var("OPENAI_API_KEY", v),
            None => std::env::remove_var("OPENAI_API_KEY"),
        }
    }

    #[test]
    fn select_provider_falls_back_to_openai() {
        let prior_g = std::env::var("GEMINI_API_KEY").ok();
        let prior_o = std::env::var("OPENAI_API_KEY").ok();
        std::env::remove_var("GEMINI_API_KEY");
        std::env::set_var("OPENAI_API_KEY", "test-o");

        let picked = select_provider(&empty_cfg()).expect("openai key is present");
        assert_eq!(picked.name(), "openai");

        match prior_g {
            Some(v) => std::env::set_var("GEMINI_API_KEY", v),
            None => std::env::remove_var("GEMINI_API_KEY"),
        }
        match prior_o {
            Some(v) => std::env::set_var("OPENAI_API_KEY", v),
            None => std::env::remove_var("OPENAI_API_KEY"),
        }
    }

    #[test]
    fn no_key_response_shape_is_actionable() {
        let v = no_key_response();
        assert_eq!(v["ok"], Value::Bool(false));
        assert_eq!(v["reason"], Value::String(NO_IMAGE_KEY_SENTINEL.into()));
        assert!(v["action"].as_str().unwrap().contains("Settings"));
    }

    #[test]
    fn extract_inline_image_reads_both_casings() {
        let camel = json!({
            "candidates": [ { "content": { "parts": [
                { "inlineData": { "data": B64.encode(b"hello") } }
            ]}}]
        });
        assert_eq!(extract_inline_image(&camel).unwrap(), b"hello".to_vec());
        let snake = json!({
            "candidates": [ { "content": { "parts": [
                { "inline_data": { "data": B64.encode(b"world") } }
            ]}}]
        });
        assert_eq!(extract_inline_image(&snake).unwrap(), b"world".to_vec());
    }
}
