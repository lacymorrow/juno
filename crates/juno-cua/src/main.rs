//! # juno-cua — Headless Computer Use Agent CLI
//!
//! Exposes individual CUA tools (screenshot, click, type, scroll, etc.) as CLI
//! subcommands. No GUI, no Tauri runtime — just the Desktop accessibility engine.
//!
//! ## Usage
//!
//! ```sh
//! # Take a screenshot (base64 JSON)
//! juno-cua screenshot
//!
//! # Click at coordinates
//! juno-cua click --x 500 --y 300
//!
//! # Type text
//! juno-cua type-text --text "hello world"
//!
//! # Generic tool call (pass any tool name + JSON args)
//! juno-cua call --tool leftClick --args '{"x":100,"y":200}'
//!
//! # List all available tools
//! juno-cua list-tools
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use computer_use_ai_sdk::Desktop;
use serde_json::{json, Value};

/// Click button type — validated at parse time by clap.
#[derive(Clone, Debug, Copy, clap::ValueEnum, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum ClickType {
    Left,
    Right,
    Middle,
    Double,
    Triple,
}

/// Scroll direction — validated at parse time by clap.
#[derive(Clone, Debug, Copy, clap::ValueEnum, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Modifier key — validated at parse time by clap.
#[derive(Clone, Debug, Copy, clap::ValueEnum, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum ModifierKey {
    Cmd,
    Ctrl,
    Alt,
    Shift,
}

#[derive(Parser, Debug)]
#[command(
    name = "juno-cua",
    version,
    about = "Headless Computer Use Agent — individual CUA tool access without GUI",
    long_about = "Direct access to Juno's computer use tools (screenshot, click, type, scroll, key press, etc.) from the command line. Designed for scripting, automation, and integration with agents like OpenClaw."
)]
struct Cli {
    /// Enable verbose/debug logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Output format
    #[arg(long, global = true, default_value = "json")]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum OutputFormat {
    Json,
    Pretty,
    Quiet,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Take a screenshot (returns base64-encoded PNG)
    Screenshot,

    /// Click at screen coordinates
    Click {
        /// X coordinate
        #[arg(long)]
        x: f64,
        /// Y coordinate
        #[arg(long)]
        y: f64,
        /// Click type: left, right, middle, double, triple
        #[arg(long, value_enum, default_value = "left")]
        button: ClickType,
    },

    /// Move mouse to coordinates
    MouseMove {
        #[arg(long)]
        x: f64,
        #[arg(long)]
        y: f64,
    },

    /// Get current cursor position
    CursorPosition,

    /// Type text (keystroke simulation)
    TypeText {
        /// Text to type
        #[arg(long)]
        text: String,
    },

    /// Press a key with optional modifier
    PressKey {
        /// Key name (e.g., Enter, Tab, a, space)
        #[arg(long)]
        key: String,
        /// Modifier key (cmd, ctrl, alt, shift)
        #[arg(long, value_enum)]
        modifier: Option<ModifierKey>,
    },

    /// Hold a key down
    HoldKey {
        #[arg(long)]
        key: String,
        /// Duration in ms (optional)
        #[arg(long)]
        duration_ms: Option<u64>,
    },

    /// Release a held key
    ReleaseKey {
        #[arg(long)]
        key: String,
    },

    /// Scroll at position
    Scroll {
        /// X coordinate
        #[arg(long)]
        x: f64,
        /// Y coordinate
        #[arg(long)]
        y: f64,
        /// Direction: up, down, left, right
        #[arg(long, value_enum)]
        direction: ScrollDirection,
        /// Scroll amount
        #[arg(long, default_value = "3")]
        amount: f64,
    },

    /// Open an application by name
    OpenApp {
        /// Application name
        #[arg(long)]
        name: String,
    },

    /// Open a URL in the default browser
    OpenUrl {
        /// URL to open
        #[arg(long)]
        url: String,
    },

    /// Get focused element info (accessibility)
    FocusedElement,

    /// Get clipboard content
    GetClipboard,

    /// Set clipboard content
    SetClipboard {
        #[arg(long)]
        content: String,
    },

    /// Get the UI tree for an application
    UiTree {
        /// Application name (optional — omit for all)
        #[arg(long)]
        app: Option<String>,
    },

    /// Find elements by selector
    FindElements {
        /// Selector string
        #[arg(long)]
        selector: String,
    },

    /// Wait for a duration
    Wait {
        /// Duration in milliseconds
        #[arg(long)]
        ms: u64,
    },

    /// List all available tools (JSON schema output)
    ListTools,

    /// Generic tool call — pass any tool name and JSON args
    Call {
        /// Tool name (e.g., leftClick, captureScreenshot, pressKey)
        #[arg(long)]
        tool: String,
        /// JSON arguments for the tool
        #[arg(long, default_value = "{}")]
        args: String,
    },

    /// Print a concise, LLM-readable tool catalog
    Capabilities,

    /// Start MCP (Model Context Protocol) server over stdio
    ServeMcp,
}

fn init_desktop() -> Result<Desktop> {
    Desktop::new(false, true)
        .context("Failed to initialize Desktop engine. Check accessibility permissions.")
}

fn output(format: &OutputFormat, value: Value) {
    match format {
        OutputFormat::Json => {
            let out = serde_json::to_string(&value).unwrap_or_else(|_| "null".into());
            println!("{}", out);
        }
        OutputFormat::Pretty => {
            let out = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "null".into());
            println!("{}", out);
        }
        OutputFormat::Quiet => {
            // Successful results are silent in quiet mode
        }
    }
}

fn main() {
    if let Err(e) = run() {
        let err = json!({ "error": format!("{:#}", e) });
        eprintln!(
            "{}",
            serde_json::to_string(&err).unwrap_or_else(|_| format!("{{\"error\":\"{}\"}}", e))
        );
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Init tracing
    if cli.verbose {
        tracing_subscriber::fmt().with_env_filter("debug").init();
    } else {
        tracing_subscriber::fmt().with_env_filter("warn").init();
    }

    // These commands don't need Desktop initialization
    match &cli.command {
        Commands::Capabilities => {
            print_capabilities();
            return Ok(());
        }
        Commands::ServeMcp => {
            return run_mcp_server();
        }
        _ => {}
    }

    let desktop = init_desktop()?;

    let result: Value = match cli.command {
        Commands::Screenshot => {
            let b64 = desktop
                .capture_screenshot_base64()
                .context("Screenshot failed")?;
            json!({ "screenshot_base64": b64 })
        }

        Commands::Click { x, y, button } => {
            match button {
                ClickType::Left => desktop.left_click(x, y, None),
                ClickType::Right => desktop.right_click(x, y, None),
                ClickType::Middle => desktop.middle_click(x, y, None),
                ClickType::Double => desktop.double_click(x, y, None),
                ClickType::Triple => desktop.triple_click(x, y, None),
            }
            .context("Click failed")?;
            json!({ "status": "success", "action": "click", "x": x, "y": y, "button": button })
        }

        Commands::MouseMove { x, y } => {
            desktop.mouse_move(x, y).context("Mouse move failed")?;
            json!({ "status": "success", "action": "mouse_move", "x": x, "y": y })
        }

        Commands::CursorPosition => {
            let (x, y) = desktop
                .cursor_position()
                .context("Failed to get cursor position")?;
            json!({ "x": x, "y": y })
        }

        Commands::TypeText { text } => {
            desktop.type_text(&text).context("Type text failed")?;
            json!({ "status": "success", "action": "type_text", "length": text.len() })
        }

        Commands::PressKey { key, modifier } => {
            let mod_str = modifier.map(|m| match m {
                ModifierKey::Cmd => "cmd",
                ModifierKey::Ctrl => "ctrl",
                ModifierKey::Alt => "alt",
                ModifierKey::Shift => "shift",
            });
            desktop
                .press_key(&key, mod_str)
                .context("Press key failed")?;
            json!({ "status": "success", "action": "press_key", "key": key, "modifier": modifier })
        }

        Commands::HoldKey { key, duration_ms } => {
            desktop
                .hold_key(&key, duration_ms)
                .context("Hold key failed")?;
            json!({ "status": "success", "action": "hold_key", "key": key })
        }

        Commands::ReleaseKey { key } => {
            desktop.release_key(&key).context("Release key failed")?;
            json!({ "status": "success", "action": "release_key", "key": key })
        }

        Commands::Scroll {
            x,
            y,
            direction,
            amount,
        } => {
            let dir_str = match direction {
                ScrollDirection::Up => "up",
                ScrollDirection::Down => "down",
                ScrollDirection::Left => "left",
                ScrollDirection::Right => "right",
            };
            desktop
                .scroll_at_position(x, y, dir_str, amount)
                .context("Scroll failed")?;
            json!({ "status": "success", "action": "scroll", "x": x, "y": y, "direction": direction, "amount": amount })
        }

        Commands::OpenApp { name } => {
            desktop
                .open_application(&name)
                .context("Open application failed")?;
            json!({ "status": "success", "action": "open_application", "app": name })
        }

        Commands::OpenUrl { url } => {
            desktop.open_url(&url, None).context("Open URL failed")?;
            json!({ "status": "success", "action": "open_url", "url": url })
        }

        Commands::FocusedElement => {
            let element = desktop
                .focused_element()
                .context("Failed to get focused element")?;
            let attrs = element.attributes();
            serde_json::to_value(attrs).context("Failed to serialize element")?
        }

        Commands::GetClipboard => {
            let content = desktop
                .get_clipboard_content()
                .context("Failed to get clipboard")?;
            json!({ "content": content })
        }

        Commands::SetClipboard { content } => {
            desktop
                .set_clipboard_content(&content)
                .context("Failed to set clipboard")?;
            json!({ "status": "success", "action": "set_clipboard" })
        }

        Commands::UiTree { app } => desktop
            .call_tool("getUiTree", json!({ "application_name": app }))
            .context("Failed to get UI tree")?,

        Commands::FindElements { selector } => desktop
            .call_tool("findElementsBySelector", json!({ "selector": selector }))
            .context("Find elements failed")?,

        Commands::Wait { ms } => {
            desktop.wait(ms).context("Wait failed")?;
            json!({ "status": "success", "action": "wait", "ms": ms })
        }

        Commands::ListTools => {
            let tools = desktop.list_tools();
            serde_json::to_value(tools).context("Failed to serialize tools")?
        }

        Commands::Call { tool, args } => {
            let parsed_args: Value =
                serde_json::from_str(&args).context("Invalid JSON in --args")?;
            desktop
                .call_tool(&tool, parsed_args)
                .context(format!("Tool '{}' failed", tool))?
        }

        Commands::Capabilities | Commands::ServeMcp => unreachable!("handled above"),
    };

    output(&cli.format, result);
    Ok(())
}

// ---------------------------------------------------------------------------
// MCP Server (JSON-RPC 2.0 over stdio)
// ---------------------------------------------------------------------------

/// Tools that return screenshot data — their results need MCP image content blocks.
const SCREENSHOT_TOOLS: &[&str] = &["captureScreenshot", "computer"];

fn run_mcp_server() -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to build tokio runtime")?;
    rt.block_on(mcp_server_loop())
}

async fn mcp_server_loop() -> Result<()> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);

    // Lazy-init Desktop on first tool use (initialize/ping respond without permissions)
    let mut desktop: Option<Desktop> = None;

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await.context("stdin read")?;
        if n == 0 {
            break; // EOF
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let err_resp = jsonrpc_error(Value::Null, -32700, &format!("Parse error: {e}"));
                write_jsonrpc(&mut stdout, &err_resp).await?;
                continue;
            }
        };

        // Notifications (no "id" field) — no response required
        if request.get("id").is_none() {
            continue;
        }

        let id = request["id"].clone();
        let method = match request.get("method").and_then(|v| v.as_str()) {
            Some(m) => m,
            None => {
                let err_resp = jsonrpc_error(
                    id,
                    -32600,
                    "Invalid Request: missing or non-string 'method'",
                );
                write_jsonrpc(&mut stdout, &err_resp).await?;
                continue;
            }
        };

        let response = match method {
            "initialize" => jsonrpc_ok(
                id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "juno-cua",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            ),

            "tools/list" => {
                let desktop_ref = get_or_init_desktop(&mut desktop)?;
                let tools = desktop_ref.list_tools();
                let mcp_tools: Vec<Value> = tools
                    .iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description,
                            "inputSchema": t.input_schema
                        })
                    })
                    .collect();
                jsonrpc_ok(id, json!({ "tools": mcp_tools }))
            }

            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                let tool_name = params["name"].as_str().unwrap_or("");
                let tool_args = params.get("arguments").cloned().unwrap_or(json!({}));

                if tool_name.is_empty() {
                    jsonrpc_error(id, -32602, "Missing tool name in params.name")
                } else {
                    let desktop_ref = get_or_init_desktop(&mut desktop)?;
                    match desktop_ref.call_tool(tool_name, tool_args) {
                        Ok(result) => {
                            let content = if is_screenshot_result(tool_name, &result) {
                                extract_image_content(&result)
                            } else {
                                vec![json!({
                                    "type": "text",
                                    "text": serde_json::to_string(&result)
                                        .unwrap_or_else(|_| "null".into())
                                })]
                            };
                            jsonrpc_ok(id, json!({ "content": content }))
                        }
                        Err(e) => jsonrpc_ok(
                            id,
                            json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!("Error: {e}")
                                }],
                                "isError": true
                            }),
                        ),
                    }
                }
            }

            "ping" => jsonrpc_ok(id, json!({})),

            _ => jsonrpc_error(id, -32601, &format!("Method not found: {method}")),
        };

        write_jsonrpc(&mut stdout, &response).await?;
    }

    Ok(())
}

fn get_or_init_desktop(desktop: &mut Option<Desktop>) -> Result<&Desktop> {
    if desktop.is_none() {
        *desktop = Some(init_desktop()?);
    }
    desktop
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Desktop initialization failed"))
}

fn is_screenshot_result(tool_name: &str, result: &Value) -> bool {
    if SCREENSHOT_TOOLS.contains(&tool_name) {
        return true;
    }
    // Also detect screenshot results from generic `call` or `computer` action=screenshot
    result.get("screenshot_base64").is_some() || result.get("base64_image").is_some()
}

fn extract_image_content(result: &Value) -> Vec<Value> {
    // Try known base64 fields
    let b64 = result
        .get("screenshot_base64")
        .or_else(|| result.get("base64_image"))
        .and_then(|v| v.as_str());

    match b64 {
        Some(data) => vec![json!({
            "type": "image",
            "data": data,
            "mimeType": "image/png"
        })],
        None => vec![json!({
            "type": "text",
            "text": serde_json::to_string(result).unwrap_or_else(|_| "null".into())
        })],
    }
}

fn jsonrpc_ok(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn jsonrpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

async fn write_jsonrpc(stdout: &mut tokio::io::Stdout, response: &Value) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let line = serde_json::to_string(response).unwrap_or_else(|_| "{}".into());
    stdout
        .write_all(line.as_bytes())
        .await
        .context("stdout write")?;
    stdout.write_all(b"\n").await.context("stdout newline")?;
    stdout.flush().await.context("stdout flush")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Capabilities output
// ---------------------------------------------------------------------------

fn print_capabilities() {
    println!(
        r#"# juno-cua — Computer Use Agent CLI
# macOS desktop automation. All commands return JSON.

## Screenshot & Vision
  juno-cua screenshot                          Capture screen (base64 PNG JSON)
  juno-cua ui-tree [--app NAME]                Get accessibility tree
  juno-cua find-elements --selector SELECTOR   Find UI elements by AX selector
  juno-cua focused-element                     Get focused element info

## Mouse
  juno-cua click --x X --y Y [--button left|right|middle|double|triple]
  juno-cua mouse-move --x X --y Y
  juno-cua cursor-position                     Get current cursor coordinates
  juno-cua scroll --x X --y Y --direction up|down|left|right [--amount 3]

## Keyboard
  juno-cua type-text --text "..."              Type text via keystroke simulation
  juno-cua press-key --key KEY [--modifier cmd|ctrl|alt|shift]
  juno-cua hold-key --key KEY [--duration-ms MS]
  juno-cua release-key --key KEY

## System
  juno-cua get-clipboard                       Read clipboard contents
  juno-cua set-clipboard --content "..."       Set clipboard contents
  juno-cua open-app --name "App Name"          Launch application
  juno-cua open-url --url "https://..."        Open URL in default browser
  juno-cua wait --ms 1000                      Wait for duration

## Advanced
  juno-cua list-tools                          Full JSON schemas for all tools
  juno-cua call --tool NAME --args '{{...}}'     Generic tool invocation
  juno-cua capabilities                        Print this catalog

## Notes
- Requires macOS accessibility permissions (System Settings → Privacy → Accessibility)
- All output is JSON by default. Use --format pretty for formatted output.
- Use --verbose for debug logging."#
    );
}
