use std::future::Future;
use std::pin::Pin;
/// TODO: ELIMINATE STRING MATCHING
use std::time::Duration;
use tracing::{debug, warn};

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
        tokio::net::lookup_host("1.1.1.1:53"),
    )
    .await
    {
        Ok(Ok(_)) => {
            debug!("DNS connectivity check: SUCCESS");
            true
        }
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
        reqwest::get("https://httpbin.org/status/200"),
    )
    .await
    {
        Ok(Ok(response)) if response.status().is_success() => {
            debug!("HTTP connectivity check: SUCCESS");
            true
        }
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
        client
            .head(crate::constants::api::endpoints::ANTHROPIC_API_ORIGIN)
            .send(),
    )
    .await
    {
        Ok(Ok(_)) => {
            debug!("Cloud API connectivity check: SUCCESS");
            true
        }
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

    error_lower.contains("network")
        || error_lower.contains("connection")
        || error_lower.contains(crate::constants::error_messages::patterns::TIMEOUT)
        || error_lower.contains("unreachable")
        || error_lower.contains("dns")
        || error_lower.contains("http request failed")
        || error_lower.contains("error sending request")
        || error_lower.contains("no route to host")
        || error_lower.contains(crate::constants::error_messages::patterns::CONNECTION_REFUSED)
        || error_lower.contains("connection reset")
        || error_lower.contains("temporary failure in name resolution")
        // reqwest says "operation timed out", not "timeout".
        || error_lower.contains("timed out")
        // A response body cut off mid-stream (Wi-Fi dropped during an answer).
        || error_lower.contains("error decoding response body")
        || error_lower.contains("failed to read stream line")
        || error_lower.contains("incomplete message")
        || error_lower.contains("broken pipe")
        || error_lower.contains("stream stalled")
}

#[cfg(test)]
mod tests {
    use super::is_network_error;

    #[test]
    fn a_connection_lost_mid_answer_reads_as_offline() {
        for message in [
            "LLM error: Failed to read stream line: request or response body error: operation timed out",
            "LLM error: HTTP request failed: error sending request for url (https://api.anthropic.com/v1/messages)",
            "error decoding response body",
            "hyper::Error(IncompleteMessage): connection closed before message completed",
            "LLM error: stream stalled: no data for 90s",
        ] {
            assert!(is_network_error(message), "{message}");
        }
    }

    #[test]
    fn an_ordinary_failure_does_not() {
        for message in [
            "Anthropic API error 400 (invalid_request_error): prompt is too long",
            "Agent reached maximum steps.",
        ] {
            assert!(!is_network_error(message), "{message}");
        }
    }
}
