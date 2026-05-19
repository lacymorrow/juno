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
        "bash"
        | "execute_bash"
        | "run_bash_command"
        | "shell_execute"
        | "execute_command"
        | "run_command" => classify_shell_risk(tool_input),

        // Computer use actions (screenshot/cursor are safe; keyboard combos vary)
        "computer" => classify_computer_use_risk(tool_input),

        // File mutations
        "write_file" | "edit_file" | "create_file" | "str_replace_editor" => {
            classify_file_write_risk(tool_input)
        }
        "delete_file" | "remove_file" | "unlink_file" => RiskLevel::High,

        // Browser navigation to sensitive sites
        "browser_navigate" | "navigate_to_url" | "open_url" => {
            classify_browser_nav_risk(tool_input)
        }

        // Form fill — could submit payments/passwords
        "browser_fill" | "fill_form" | "type_in_element" | "browser_type" => {
            classify_form_fill_risk(tool_input)
        }

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
            cmd.split_whitespace()
                .next()
                .map(|s| s.to_string())
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

    // Medium: reads/writes files, network calls, etc. — still worth noting
    RiskLevel::Medium
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
        "/etc/", "/usr/", "/bin/", "/sbin/", "/System/", "/Library/",
        "etc/", "usr/", "bin/", "sbin/",
    ];
    if system_prefixes.iter().any(|prefix| path.starts_with(prefix)) {
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
        let r = classify_risk("browser_navigate", &json!({"url": "https://example.com/checkout"}));
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
