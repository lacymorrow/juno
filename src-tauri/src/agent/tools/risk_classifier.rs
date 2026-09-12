//! Risk classification for tool actions.
//!
//! Classifies tool calls by risk level so the agent runner can gate High/Critical
//! actions behind a human confirmation prompt automatically, without requiring the
//! global `tool_approval_required` flag to be set.

use serde_json::Value;

use crate::state::RiskLevel;

/// Classify the risk level of a tool call based on its name and input.
/// Returns the highest applicable risk level found.
pub fn classify_risk(tool_name: &str, tool_input: &Value) -> RiskLevel {
    match tool_name {
        // Shell execution — highest-variance category
        "bash" | "execute_bash" | "run_bash_command" | "shell_execute" | "execute_command"
        | "run_command" => classify_shell_risk(tool_input),

        // Computer use actions (screenshot/cursor are safe; keyboard combos vary)
        "computer" => classify_computer_use_risk(tool_input),

        // File mutations
        "write_file" | "edit_file" | "create_file" | "str_replace_editor" => {
            classify_file_write_risk(tool_input)
        }
        "delete_file" | "remove_file" | "unlink_file" => RiskLevel::High,

        // Arbitrary JavaScript injected into the user's real Safari session.
        // The substring blocklist in safari_tools.rs (validate_javascript_safety)
        // is advisory and bypassable — e.g. `window["ev"+"al"]` assembles the
        // identifier at runtime and no string filter catches that. This High
        // classification is the actual control: the agent runner routes
        // High/Critical through the human approval flow, exactly like agent
        // bash (security audit 2026-02-08, items #15/#20; same pattern as #3).
        // Parameterized Safari tools (click by cached numeric id, DOM
        // extraction with a fixed script) stay Low because their injected JS
        // is compiled from typed inputs, not caller-supplied code.
        "safari_execute_javascript" => RiskLevel::High,

        // Browser navigation to sensitive sites
        "browser_navigate" | "navigate_to_url" | "open_url" | "safari_navigate" => {
            classify_browser_nav_risk(tool_input)
        }

        // Form fill — could submit payments/passwords. safari_type_text
        // injects an escaped string literal into a fixed script (not
        // caller-supplied code), so it is parameterized as JS, but the
        // *content* can still be sensitive form data.
        "browser_fill" | "fill_form" | "type_in_element" | "browser_type" | "safari_type_text" => {
            classify_form_fill_risk(tool_input)
        }

        // Agent self-scheduling — creates a persistent automation that
        // re-executes unattended with full tool access after this session
        // ends, so creation requires human confirmation
        "create_scheduled_automation" => RiskLevel::High,
        "delete_scheduled_automation" => RiskLevel::Medium,

        // Everything else is low risk by default
        _ => RiskLevel::Low,
    }
}

/// Returns true when the risk level is high enough to require human confirmation.
pub fn needs_approval(risk_level: &RiskLevel) -> bool {
    matches!(risk_level, RiskLevel::High | RiskLevel::Critical)
}

/// Extract a human-readable target app hint from tool input, if present.
pub fn extract_target_app(tool_name: &str, tool_input: &Value) -> Option<String> {
    match tool_name {
        "bash" | "execute_bash" | "run_bash_command" | "shell_execute" | "execute_command"
        | "run_command" => {
            let cmd = tool_input
                .get("command")
                .or_else(|| tool_input.get("cmd"))
                .or_else(|| tool_input.get("bash"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // Extract the binary name from the command
            cmd.split_whitespace().next().map(|s| s.to_string())
        }
        "browser_navigate" | "navigate_to_url" | "open_url" => tool_input
            .get("url")
            .and_then(|v| v.as_str())
            .and_then(|url| {
                url.split('/')
                    .nth(2) // third segment = hostname
                    .map(|h| h.to_string())
            }),
        _ => None,
    }
}

// --- private helpers ---

fn classify_shell_risk(input: &Value) -> RiskLevel {
    let cmd = input
        .get("command")
        .or_else(|| input.get("cmd"))
        .or_else(|| input.get("bash"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Critical: irreversible or privilege-escalating patterns
    if cmd.contains("sudo")
        || cmd.contains("rm -rf")
        || cmd.contains("mkfs")
        || cmd.contains("> /dev/")
        || cmd.contains("dd if=")
        || cmd.contains("chmod 777 /")
        || cmd.contains(":(){:|:&};:") // fork bomb
        || cmd.contains("shred ")
        || cmd.contains("wipefs")
    {
        return RiskLevel::Critical;
    }

    // High: potentially destructive or installs software
    if cmd.contains("rm ")
        || (cmd.contains("mv ") && cmd.contains(" /"))
        || (cmd.contains("curl") && (cmd.contains("| sh") || cmd.contains("| bash")))
        || (cmd.contains("wget") && (cmd.contains("| sh") || cmd.contains("| bash")))
        || cmd.contains("pip install")
        || cmd.contains("pip3 install")
        || cmd.contains("npm install")
        || cmd.contains("yarn add")
        || cmd.contains("brew install")
        || cmd.contains("apt install")
        || cmd.contains("apt-get install")
    {
        return RiskLevel::High;
    }

    // Default: High. Arbitrary shell execution runs with full user
    // privileges, so agent bash requires human approval by default; the
    // agent runner routes High/Critical through the tool-approval flow
    // (security audit 2026-02-08, item #3). The global
    // `tool_approval_required` setting continues to apply on top of this.
    RiskLevel::High
}

fn classify_computer_use_risk(input: &Value) -> RiskLevel {
    let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("");
    match action {
        "screenshot" | "cursor_position" => RiskLevel::Low,
        "key" => {
            let text = input
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            // Destructive key combos
            if text.contains("ctrl+alt+delete")
                || text.contains("cmd+q")
                || text.contains("super+delete")
            {
                RiskLevel::High
            } else {
                RiskLevel::Low
            }
        }
        _ => RiskLevel::Low,
    }
}

fn classify_file_write_risk(input: &Value) -> RiskLevel {
    let path = input
        .get("path")
        .or_else(|| input.get("file_path"))
        .or_else(|| input.get("filename"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Directory traversal is always critical — could target any system path
    if path.contains("..") {
        return RiskLevel::Critical;
    }

    // Writing to system directories is critical (absolute or relative prefixes)
    let system_prefixes: &[&str] = &[
        "/etc/",
        "/usr/",
        "/bin/",
        "/sbin/",
        "/System/",
        "/Library/",
        "etc/",
        "usr/",
        "bin/",
        "sbin/",
    ];
    if system_prefixes
        .iter()
        .any(|prefix| path.starts_with(prefix))
    {
        RiskLevel::Critical
    } else {
        RiskLevel::Low
    }
}

fn classify_browser_nav_risk(input: &Value) -> RiskLevel {
    let url = input
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    if url.contains("payment")
        || url.contains("checkout")
        || url.contains("billing")
        || url.contains("bank")
        || url.contains("transfer")
        || url.contains("paypal.com")
        || url.contains("stripe.com")
        || url.contains("venmo.com")
        || url.contains("zelle")
    {
        RiskLevel::High
    } else {
        RiskLevel::Low
    }
}

fn classify_form_fill_risk(input: &Value) -> RiskLevel {
    let serialized = input.to_string().to_lowercase();

    // Payment card / identity data in form fields
    if serialized.contains("credit_card")
        || serialized.contains("card_number")
        || serialized.contains("card-number")
        || serialized.contains("cvv")
        || serialized.contains("ssn")
        || serialized.contains("social_security")
        || serialized.contains("password")
        || serialized.contains("passwd")
    {
        RiskLevel::Critical
    } else {
        RiskLevel::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sudo_is_critical() {
        let r = classify_risk("bash", &json!({"command": "sudo rm /etc/hosts"}));
        assert_eq!(r, RiskLevel::Critical);
    }

    #[test]
    fn rm_is_high() {
        let r = classify_risk("bash", &json!({"command": "rm old_file.txt"}));
        assert_eq!(r, RiskLevel::High);
    }

    #[test]
    fn plain_bash_requires_approval_by_default() {
        // Any agent shell execution defaults to High risk, so the runner's
        // approval gate fires even without the global approval flag.
        let r = classify_risk("bash", &json!({"command": "ls -la"}));
        assert_eq!(r, RiskLevel::High);
        assert!(needs_approval(&r));
    }

    #[test]
    fn screenshot_is_low() {
        let r = classify_risk("computer", &json!({"action": "screenshot"}));
        assert_eq!(r, RiskLevel::Low);
    }

    #[test]
    fn system_file_write_is_critical() {
        let r = classify_risk("write_file", &json!({"path": "/etc/sudoers"}));
        assert_eq!(r, RiskLevel::Critical);
    }

    #[test]
    fn payment_url_is_high() {
        let r = classify_risk(
            "browser_navigate",
            &json!({"url": "https://example.com/checkout"}),
        );
        assert_eq!(r, RiskLevel::High);
    }

    #[test]
    fn needs_approval_critical() {
        assert!(needs_approval(&RiskLevel::Critical));
    }

    #[test]
    fn no_approval_for_low() {
        assert!(!needs_approval(&RiskLevel::Low));
    }

    #[test]
    fn safari_execute_javascript_is_high_and_gated() {
        // Arbitrary Safari JS defaults to High so the runner's approval gate
        // fires even without the global approval flag (audit #15/#20). The
        // input content does not matter — the capability itself is the risk.
        let r = classify_risk(
            "safari_execute_javascript",
            &json!({"javascript": "document.title"}),
        );
        assert_eq!(r, RiskLevel::High);
        assert!(needs_approval(&r));

        // A blocklist bypass payload is still High for the same reason.
        let r = classify_risk(
            "safari_execute_javascript",
            &json!({"javascript": "window[\"ev\"+\"al\"]('x')"}),
        );
        assert_eq!(r, RiskLevel::High);
    }

    #[test]
    fn parameterized_safari_tools_stay_low() {
        // These compile fixed scripts from typed inputs (numeric ids), so
        // they carry no arbitrary-JS risk and stay unprompted.
        let r = classify_risk("safari_extract_dom", &json!({}));
        assert_eq!(r, RiskLevel::Low);
        let r = classify_risk("safari_click_element", &json!({"element_id": 7}));
        assert_eq!(r, RiskLevel::Low);
        let r = classify_risk("safari_list_clickable_elements", &json!({}));
        assert_eq!(r, RiskLevel::Low);
    }

    #[test]
    fn safari_navigate_uses_browser_nav_risk() {
        let r = classify_risk(
            "safari_navigate",
            &json!({"url": "https://bank.example.com/transfer"}),
        );
        assert_eq!(r, RiskLevel::High);
        let r = classify_risk("safari_navigate", &json!({"url": "https://example.com"}));
        assert_eq!(r, RiskLevel::Low);
    }

    #[test]
    fn safari_type_text_uses_form_fill_risk() {
        let r = classify_risk(
            "safari_type_text",
            &json!({"element_id": 3, "text": "hunter2", "field": "password"}),
        );
        assert_eq!(r, RiskLevel::Critical);
        let r = classify_risk(
            "safari_type_text",
            &json!({"element_id": 3, "text": "hello world"}),
        );
        assert_eq!(r, RiskLevel::Low);
    }

    #[test]
    fn path_traversal_is_critical() {
        let r = classify_risk("write_file", &json!({"path": "/tmp/../../etc/passwd"}));
        assert_eq!(r, RiskLevel::Critical);
    }

    #[test]
    fn relative_system_path_is_critical() {
        let r = classify_risk("write_file", &json!({"path": "etc/passwd"}));
        assert_eq!(r, RiskLevel::Critical);
    }
}
