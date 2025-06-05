use std::{
    net::SocketAddr,
    sync::Arc,
    io::ErrorKind,
};

use axum::{routing::post, Router};
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, level_filters::LevelFilter};
use computer_use_ai_sdk::Desktop;
use server::handlers::click_by_index::click_by_index_handler;
use server::handlers::input_control::input_control_handler;
use server::handlers::list_elements_and_attributes::list_elements_and_attributes_handler;
use server::handlers::mcp::{
    handle_execute_tool_function, handle_initialize, mcp_error_response,
};
use server::handlers::open_application::open_application_handler;
use server::handlers::open_url::open_url_handler;
use server::handlers::press_key_by_index::press_key_by_index_handler;
use server::handlers::type_by_index::type_by_index_handler;
use server::types::{AppState, MCPRequest};
use serde_json::{self, json, Value};
use tokio::io::{stdin, stdout, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};

// Declare the server module
mod server;

// ================ Main ================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Check if we should use STDIO mode
    let use_stdio = std::env::args().any(|arg| arg == "--stdio");

    // initialize tracing with different settings based on mode
    if use_stdio {
        // For STDIO mode, disable colors and only log to stderr
        tracing_subscriber::fmt()
            .with_max_level(LevelFilter::DEBUG)
            .with_ansi(false) // Disable ANSI color codes
            .with_writer(std::io::stderr) // Only write logs to stderr
            .init();
    } else {
        // For HTTP mode, use default settings
        tracing_subscriber::fmt()
            .with_max_level(LevelFilter::DEBUG)
            .init();
    }

    info!("starting ui automation server");

    // Check permissions early
    check_os_permissions();

    // Create app state using the type from the server module
    // Initialize the Desktop engine here with auto-redirect for better UX
    let desktop_engine = Desktop::new_with_auto_redirect(false, true, true)
        .map_err(|e| anyhow::anyhow!("Failed to initialize desktop engine: {}", e))?;

    let app_state = Arc::new(AppState {
        element_cache: Arc::new(Mutex::new(None)),
        desktop: Arc::new(desktop_engine), // Store the initialized desktop engine
    });

    if use_stdio {
        info!("running in STDIO mode for MCP");
        // Call the new STDIO mode function
        run_stdio_mode(app_state).await?;
    } else {
        info!("running in HTTP mode on port 8080");
        run_http_server(app_state).await?;
    }

    Ok(())
}

// New function to handle STDIO mode
async fn run_stdio_mode(state: Arc<AppState>) -> anyhow::Result<()> {
    let stdin = stdin();
    let mut reader = BufReader::new(stdin);
    let stdout = stdout();
    let mut writer = BufWriter::new(stdout);
    let mut line_buffer = String::new();

    info!("STDIO mode ready. Waiting for JSON MCP requests on stdin...");

    loop {
        line_buffer.clear();
        match reader.read_line(&mut line_buffer).await {
            Ok(0) => {
                info!("stdin closed, exiting STDIO mode.");
                break; // EOF
            }
            Ok(_) => {
                // Attempt to parse the line as an MCPRequest
                let response_json = match serde_json::from_str::<MCPRequest>(&line_buffer) {
                    Ok(request) => {
                        let request_id = request.id.clone(); // Clone id for error handling
                        match request.method.as_str() {
                            "initialize" => handle_initialize(request.id).into_inner(),
                            "executeToolFunction" => {
                                if let Some(params) = request.params {
                                    // Need to await the async function here
                                    handle_execute_tool_function(state.clone(), request.id, params).await.into_inner()
                                } else {
                                    mcp_error_response(request_id, -32602, "invalid params".to_string(), None).into_inner()
                                }
                            }
                            _ => mcp_error_response(
                                request_id,
                                -32601,
                                "method not found".to_string(),
                                None,
                            ).into_inner(),
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse stdin line as MCPRequest JSON: {}. Line: {}", e, line_buffer.trim());
                        // Construct a basic MCP error response if possible, otherwise skip
                        // Try to extract ID if it was partially valid JSON
                        let id_val = serde_json::from_str::<Value>(&line_buffer)
                            .ok()
                            .and_then(|v| v.get("id").cloned())
                            .unwrap_or(Value::Null);
                        mcp_error_response(id_val, -32700, "parse error".to_string(), Some(json!({ "error_details": e.to_string() }))).into_inner()
                    }
                };

                // Serialize the response back to JSON string
                match serde_json::to_string(&response_json) {
                    Ok(response_str) => {
                        if let Err(e) = writer.write_all(response_str.as_bytes()).await {
                            error!("Failed to write response to stdout: {}", e);
                            break; // Exit if we can't write
                        }
                        if let Err(e) = writer.write_all(b"\n").await { // Add newline separator
                            error!("Failed to write newline to stdout: {}", e);
                            break;
                        }
                        if let Err(e) = writer.flush().await {
                            error!("Failed to flush stdout: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize MCP response to JSON: {}", e);
                        // Cannot send error back if serialization fails
                    }
                }
            }
            Err(ref e) if e.kind() == ErrorKind::BrokenPipe => {
                info!("stdin pipe broken, exiting STDIO mode.");
                break;
            }
            Err(e) => {
                error!("Error reading from stdin: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn run_http_server(app_state: Arc<AppState>) -> anyhow::Result<()> {
    // Create CORS layer
    let cors = CorsLayer::very_permissive();

    // Create router with both existing and MCP endpoints plus new endpoints
    let app = Router::new()
        .route("/mcp", post(mcp_handler))
        .route("/api/click-by-index", post(click_by_index_handler))
        .route("/api/type-by-index", post(type_by_index_handler))
        .route("/api/press-key-by-index", post(press_key_by_index_handler))
        .route("/api/open-application", post(open_application_handler))
        .route("/api/open-url", post(open_url_handler))
        .route("/api/input-control", post(input_control_handler))
        .route(
            "/api/list-elements-and-attributes",
            post(list_elements_and_attributes_handler),
        )
        .with_state(app_state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Get the address to bind to
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("listening on {}", addr);

    // Start the server
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

fn check_os_permissions() {
    // Only check on macOS
    #[cfg(target_os = "macos")]
    {
        // Use enhanced permission checking with auto-redirect
        use computer_use_ai_sdk::platforms::macos::permissions::check_accessibility_permissions_with_auto_redirect;

        match check_accessibility_permissions_with_auto_redirect(true, true) {
            Ok(granted) => {
                if !granted {
                    info!("accessibility permissions: prompt shown to user with auto-redirect to System Settings");
                    // Sleep to give user time to respond to the prompt and open settings
                    std::thread::sleep(std::time::Duration::from_secs(3));

                    // Check again without prompt or auto-redirect
                    match check_accessibility_permissions_with_auto_redirect(false, false) {
                        Ok(now_granted) => {
                            if now_granted {
                                info!("accessibility permissions now granted");
                            } else {
                                info!("**************************************************************");
                                info!("* ACCESSIBILITY PERMISSIONS STILL REQUIRED                   *");
                                info!("* System Settings has been opened for you automatically.     *");
                                info!("* Please grant accessibility permissions to this app.        *");
                                info!("* Without this permission, UI automation will not function.   *");
                                info!("**************************************************************");
                            }
                        },
                        Err(e) => {
                            error!("accessibility permissions check failed after auto-redirect: {}", e);
                            info!("**************************************************************");
                            info!("* ACCESSIBILITY PERMISSIONS REQUIRED                          *");
                            info!("* System Settings should have opened automatically.           *");
                            info!("* Please grant accessibility permissions to this app.        *");
                            info!("* Without this permission, UI automation will not function.   *");
                            info!("**************************************************************");
                        }
                    }
                } else {
                    info!("accessibility permissions already granted");
                }
            }
            Err(e) => {
                error!("accessibility permissions check failed: {}", e);
                info!("**************************************************************");
                info!("* ACCESSIBILITY PERMISSIONS REQUIRED                          *");
                info!("* System Settings should have opened automatically.           *");
                info!("* Please grant accessibility permissions to this app.        *");
                info!("* If System Settings didn't open, go to:                     *");
                info!("* System Settings > Privacy & Security > Accessibility       *");
                info!("* Without this permission, UI automation will not function.   *");
                info!("**************************************************************");
            }
        }
    }
}
