use tauri::App; // placeholder
use juno_lib::commands::display::{validate_kind_and_payload, sanitize_payload, MAX_PAYLOAD_SIZE};
use serde_json::json;

#[test]
fn test_validate_kind_ok() {
    let payload = json!("test");
    assert!(validate_kind_and_payload("image", &payload).is_ok());
}

#[test]
fn test_validate_kind_invalid() {
    let payload = json!("test");
    assert!(validate_kind_and_payload("unknown", &payload).is_err());
}

#[test]
fn test_payload_size_limit() {
    let big_string = "a".repeat(MAX_PAYLOAD_SIZE + 100);
    let payload = json!(big_string);
    assert!(validate_kind_and_payload("html", &payload).is_err());
}

#[test]
fn test_html_sanitization() {
    let payload = json!("<script>alert('x')</script><div>Safe</div>");
    let result = sanitize_payload("html", &payload).unwrap();
    assert!(!result.as_str().unwrap().contains("script"));
}