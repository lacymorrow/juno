use std::time::Duration;
use tracing::{info, warn, debug};
use std::future::Future;
use std::pin::Pin;

/// Check if the device has internet connectivity
/// Returns true if online, false if offline
pub async fn is_online() -> bool {
    // Try multiple quick connectivity checks
    let connectivity_checks: Vec<Pin<Box<dyn Future<Output = bool> + Send>>> = vec![
        Box::pin(check_dns_resolution()),
        Box::pin(check_http_connectivity()),
        Box::pin(check_cloud_api_connectivity()),
    ];

    // Run checks in parallel with timeout
    let results = futures::future::join_all(connectivity_checks).await;

    // If any check succeeds, we're online
    let online = results.iter().any(|&result| result);

    if online {
        debug!("Network connectivity check: ONLINE");
    } else {
        warn!("Network connectivity check: OFFLINE");
    }

    online
}

/// Quick DNS resolution check
async fn check_dns_resolution() -> bool {
    match tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::lookup_host("1.1.1.1:53")
    ).await {
        Ok(Ok(_)) => {
            debug!("DNS connectivity check: SUCCESS");
            true
        },
        _ => {
            debug!("DNS connectivity check: FAILED");
            false
        }
    }
}

/// Quick HTTP connectivity check
async fn check_http_connectivity() -> bool {
    match tokio::time::timeout(
        Duration::from_secs(3),
        reqwest::get("https://httpbin.org/status/200")
    ).await {
        Ok(Ok(response)) if response.status().is_success() => {
            debug!("HTTP connectivity check: SUCCESS");
            true
        },
        _ => {
            debug!("HTTP connectivity check: FAILED");
            false
        }
    }
}

/// Check connectivity to Anthropic API
async fn check_cloud_api_connectivity() -> bool {
    // Quick HEAD request to Anthropic API endpoint
    let client = reqwest::Client::new();
    match tokio::time::timeout(
        Duration::from_secs(3),
        client.head("https://api.anthropic.com/").send()
    ).await {
        Ok(Ok(_)) => {
            debug!("Cloud API connectivity check: SUCCESS");
            true
        },
        _ => {
            debug!("Cloud API connectivity check: FAILED");
            false
        }
    }
}

/// Get a user-friendly offline message
pub fn get_offline_message() -> String {
    "Looks like I'm offline, you'll need to connect to internet.".to_string()
}

/// Check if an error indicates a network connectivity issue
pub fn is_network_error(error_msg: &str) -> bool {
    let error_lower = error_msg.to_lowercase();

    error_lower.contains("network") ||
    error_lower.contains("connection") ||
    error_lower.contains("timeout") ||
    error_lower.contains("unreachable") ||
    error_lower.contains("dns") ||
    error_lower.contains("http request failed") ||
    error_lower.contains("error sending request") ||
    error_lower.contains("no route to host") ||
    error_lower.contains("connection refused") ||
    error_lower.contains("connection reset") ||
    error_lower.contains("temporary failure in name resolution")
}