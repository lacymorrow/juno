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
}

fn init_desktop() -> Result<Desktop> {
    Desktop::new(false, true).context("Failed to initialize Desktop engine. Check accessibility permissions.")
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
        eprintln!("{}", serde_json::to_string(&err).unwrap_or_else(|_| format!("{{\"error\":\"{}\"}}", e)));
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Init tracing
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter("warn")
            .init();
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
            let (x, y) = desktop.cursor_position().context("Failed to get cursor position")?;
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
            desktop
                .open_url(&url, None)
                .context("Open URL failed")?;
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

        Commands::UiTree { app } => {
            desktop
                .call_tool("getUiTree", json!({ "application_name": app }))
                .context("Failed to get UI tree")?
        }

        Commands::FindElements { selector } => {
            desktop
                .call_tool("findElementsBySelector", json!({ "selector": selector }))
                .context("Find elements failed")?
        }

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
    };

    output(&cli.format, result);
    Ok(())
}
