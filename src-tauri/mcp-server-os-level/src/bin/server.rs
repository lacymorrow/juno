use std::{net::SocketAddr, sync::Arc, io::{self, BufRead, BufReader, Write}};

use axum::{
    routing::post,
    Router,
};
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, level_filters::LevelFilter};
use serde_json::{json, Value};
mod types;
use types::*;
mod handlers;

// Import only the handlers actually used
use handlers::mcp::mcp_handler;
use handlers::click_by_index::click_by_index_handler;
use handlers::type_by_index::type_by_index_handler;
use handlers::press_key_by_index::press_key_by_index_handler;
use handlers::open_application::open_application_handler;
use handlers::open_url::open_url_handler;
use handlers::input_control::input_control_handler;
use handlers::list_elements_and_attributes::list_elements_and_attributes_handler;
use handlers::utils::*;

// Import mcp_handler helpers but we'll call them directly
use handlers::mcp::{handle_initialize, handle_execute_tool_function, mcp_error_response};

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
            .with_ansi(false)  // Disable ANSI color codes
            .with_writer(std::io::stderr)  // Only write logs to stderr
            .init();
    } else {
        // For HTTP mode, use default settings
        tracing_subscriber::fmt()
            .with_max_level(LevelFilter::DEBUG)
            .init();
    }
    
    info!("starting ui automation server");
    
    // Check permissions early - add this line
    check_os_permissions();
    
    // Create app state
    let app_state = Arc::new(AppState {
        element_cache: Arc::new(Mutex::new(None)),
    });

    if use_stdio {
        info!("running in STDIO mode for MCP");
        // run_stdio_mode(app_state).await?;
    } else {
        info!("running in HTTP mode on port 8080");
        run_http_server(app_state).await?;
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
        .route("/api/list-elements-and-attributes", post(list_elements_and_attributes_handler))
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

// Add this function right after main imports but before the types
fn check_os_permissions() {
    // Only check on macOS
    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::platforms::macos::check_accessibility_permissions;
        
        match check_accessibility_permissions(true) {
            Ok(granted) => {
                if !granted {
                    info!("accessibility permissions: prompt shown to user");
                    // Sleep to give user time to respond to the prompt
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    
                    // Check again without prompt
                    match check_accessibility_permissions(false) {
                        Ok(_) => info!("accessibility permissions now granted"),
                        Err(e) => {
                            error!("accessibility permissions check failed: {}", e);
                            info!("**************************************************************");
                            info!("* ACCESSIBILITY PERMISSIONS REQUIRED                          *");
                            info!("* Go to System Preferences > Security & Privacy > Privacy >   *");
                            info!("* Accessibility and add this application.                     *");
                            info!("* Without this permission, UI automation will not function.   *");
                            info!("**************************************************************");
                        }
                    }
                } else {
                    info!("accessibility permissions already granted");
                }
            },
            Err(e) => {
                error!("accessibility permissions check failed: {}", e);
                info!("**************************************************************");
                info!("* ACCESSIBILITY PERMISSIONS REQUIRED                          *");
                info!("* Go to System Preferences > Security & Privacy > Privacy >   *");
                info!("* Accessibility and add this application.                     *");
                info!("* Without this permission, UI automation will not function.   *");
                info!("**************************************************************");
            }
        }
    }
}
