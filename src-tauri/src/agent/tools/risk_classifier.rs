//! Risk classification for tool actions.
//!
//! Classifies tool calls by risk level. What Juno does with a risk level is not
//! decided here: [`crate::agent::tools::permission_policy::requires_approval`]
//! takes the level and the person's chosen permission mode and answers the one
//! question, "does this need asking". This module's job is to be right about
//! how dangerous an action is, and only that.

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
        // Only names that exist belong here. This arm used to carry
        // `browser_fill`, `fill_form` and `type_in_element`, none of which Juno
        // registers or ever has, which is how it went unnoticed that the
        // browser tool that actually types was missing from it entirely (#586).
        // A classifier full of fictional names cannot be read for what it
        // covers, so the fictional ones are gone.
        //
        // `browser_type` is NOT registered either, but it is kept deliberately:
        // `agents/browser_agent.rs` routes it to the same controller call as
        // `browser_interact`, so a call arriving under that alias still types.
        // `alias_names_are_still_reachable` pins that reasoning.
        "browser_type" | "safari_type_text" => classify_form_fill_risk(tool_input),

        // The browser tool that actually types is `browser_interact`, which
        // carries the verb in `action` rather than in the tool name. Without
        // this arm it fell through to `RiskLevel::Low` and typing a password
        // into a page never reached the approval gate — every name in the arm
        // above is a tool Juno does not register.
        "browser_interact" => match tool_input.get("action").and_then(Value::as_str) {
            Some("type") | Some("select") => classify_form_fill_risk(tool_input),
            _ => RiskLevel::Low,
        },

        // Agent self-scheduling — creates a persistent automation that
        // re-executes unattended with full tool access after this session
        // ends, so creation requires human confirmation
        "create_scheduled_automation" => RiskLevel::High,
        "delete_scheduled_automation" => RiskLevel::Medium,

        // Everything else is low risk by default
        _ => RiskLevel::Low,
    }
}

/// Shell commands that cannot change anything, so they never need asking.
///
/// Every name here either reports something or is inert. The set is closed: a
/// wrapper that could run something else (`env`, `sudo`, `doas`, `nice`,
/// `xargs`, `eval`, `sh`, `bash`, `zsh`, `time`) is absent, so a wrapped
/// command cannot match by being wrapped. `cat`, `curl`, `git` and friends are
/// absent on purpose too: reading an arbitrary file is not inert when the thing
/// doing the reading then puts the contents in a model's context.
const INERT_SHELL_COMMANDS: &[&str] = &[
    "arch", "basename", "cd", "date", "dirname", "echo", "false", "groups", "hostname", "id",
    "mkdir", "printf", "pwd", "sleep", "true", "tty", "uname", "uptime", "which", "whoami",
];

/// The only characters an inert command may contain.
///
/// This is a whitelist, which is the whole safety argument. A blocklist of
/// dangerous characters is a list of the ones someone thought of; this is a
/// list of the ones that cannot chain, substitute, redirect, glob, quote,
/// escape, assign or continue a line. It rejects `; & | ` $ ( ) < > { } [ ] * ?
/// ' " \ ~ ! # =` and every newline and control character in one rule, and
/// anything non-ASCII with them.
fn is_inert_character(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '/' | '+' | ':' | ',' | '@')
}

/// Whether this shell command is provably inert, so Juno can run it without
/// asking in every mode.
///
/// Matching is on the *parsed* command word, never a substring and never a
/// prefix, and the whole string has to be made of characters that cannot chain
/// or substitute. `sleep 1; rm -rf ~` fails on the semicolon. `rm sleep` fails
/// because the command word is `rm`. `sudo ls` fails because `sudo` is not in
/// the set. Anything this function is not certain about falls through to the
/// ordinary classification, which is the safe direction.
pub fn is_inert_shell_command(command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return false;
    }

    // One character out of the safe set disqualifies the whole command. Done
    // before any parsing, so no amount of cleverness in the arguments matters.
    if !trimmed.chars().all(is_inert_character) {
        return false;
    }

    let mut words = trimmed.split_whitespace();
    let Some(command_word) = words.next() else {
        return false;
    };

    // No path forms. `/bin/ls` is harmless but `/tmp/ls` is whatever someone
    // put there, and the difference is not worth reasoning about per call.
    if command_word.contains('/') || command_word.contains('.') {
        return false;
    }

    INERT_SHELL_COMMANDS.contains(&command_word)
}

/// Returns true when the risk level is high enough to require human confirmation.
///
/// Deprecated as a policy decision. The policy lives in
/// [`crate::agent::tools::permission_policy::requires_approval`], which takes
/// the person's chosen mode as well as the risk. This is kept only as the
/// "risky on its own terms" predicate for callers that report risk.
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

    // Inert first. `is_inert_shell_command` only says yes to a command whose
    // every character is from a set that cannot chain, substitute or redirect,
    // and whose command word is one of a closed list that reports or does
    // nothing. Once that holds, the arguments cannot turn it into something
    // else, so the pattern checks below have nothing left to catch. Running it
    // first is what stops `echo pseudocode` being read as `sudo`.
    if is_inert_shell_command(cmd) {
        return RiskLevel::Low;
    }

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

    // Default: Medium. This was High, from security audit 2026-02-08 item #3,
    // on the reasoning that arbitrary shell runs with full user privileges.
    // That reasoning still holds; what did not hold was the consequence. High
    // meant every shell call prompted in every mode, so `sleep 1` and
    // `sudo rm -rf /` asked the same question, in the same words, with the same
    // badge, dozens of times a session. People answer that by clicking Allow
    // without reading, which is less safety than one prompt that means
    // something.
    //
    // What carries the audit's intent now:
    //   - The destructive and installing patterns above are still High, so they
    //     still ask in the default mode.
    //   - `sudo` and the irreversible set are still Critical, and Critical asks
    //     in every mode including the most permissive one.
    //   - Anyone who wants the old behaviour picks "Ask me first" in Security
    //     and Privacy, where Medium asks too. That is the setting the old
    //     global flag was supposed to be and never was.
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
        // Medium, not Low: writing a file changes the Mac, so "Ask me first"
        // has to see it. The default mode asks at High, so this adds no prompts
        // for anyone who has not chosen to be asked.
        RiskLevel::Medium
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

    /// Replaces `plain_bash_requires_approval_by_default`, which asserted that
    /// `ls -la` was High and therefore prompted.
    ///
    /// That was the policy from security audit 2026-02-08 item #3: every shell
    /// command is High, so every shell command asks. It is replaced here, not
    /// weakened by accident. The audit's worry was arbitrary shell running with
    /// full user privileges, and that is still true, so:
    ///
    ///   - An ordinary shell command is Medium. It asks in "Ask me first" and
    ///     goes through in the default mode.
    ///   - A provably inert one is Low and never asks. See
    ///     `is_inert_shell_command` and the bypass tests below for what
    ///     "provably" buys.
    ///   - Destructive and installing commands are still High.
    ///   - `sudo` and the irreversible set are still Critical, and Critical asks
    ///     in every mode.
    ///
    /// The old default made `sleep 1` and `sudo rm -rf /` ask the same
    /// question, so people stopped reading it. That is the cost this change
    /// buys back.
    #[test]
    fn an_ordinary_shell_command_is_medium_and_an_inert_one_is_low() {
        let ordinary = classify_risk("bash", &json!({"command": "cat package.json"}));
        assert_eq!(ordinary, RiskLevel::Medium);

        let inert = classify_risk("bash", &json!({"command": "ls -la"}));
        assert_eq!(inert, RiskLevel::Low);

        // Still loud about the things worth being loud about.
        assert_eq!(
            classify_risk("bash", &json!({"command": "npm install left-pad"})),
            RiskLevel::High
        );
        assert_eq!(
            classify_risk("bash", &json!({"command": "sudo rm -rf /"})),
            RiskLevel::Critical
        );
    }

    #[test]
    fn the_commands_the_flood_was_made_of_no_longer_ask() {
        // Lacy's report: "it asked approval to run a sleep or a mkdir command".
        for command in [
            "sleep 1",
            "sleep 0.5",
            "mkdir -p /tmp/juno-work",
            "cd /Users/lacy/repo/juno",
            "pwd",
            "ls",
            "ls -la src",
            "echo hello world",
            "date",
            "which bun",
            "whoami",
            "uname -a",
            "hostname",
            "true",
            "basename /a/b/c.txt",
            "dirname /a/b/c.txt",
        ] {
            let risk = classify_risk("bash", &json!({"command": command}));
            assert_eq!(
                risk,
                RiskLevel::Low,
                "{:?} still classifies as {:?}",
                command,
                risk
            );
            assert!(!needs_approval(&risk), "{:?} still asks", command);
        }
    }

    /// The whole safety story for the allowlist.
    ///
    /// Each line is a way to smuggle something past a naive substring or prefix
    /// match. None of them may be treated as inert. A miss here is not a noisy
    /// prompt, it is an unprompted `rm -rf`.
    #[test]
    fn no_bypass_is_ever_treated_as_inert() {
        let bypasses = [
            // Chained
            "sleep 1; rm -rf ~",
            "sleep 1 ; rm -rf /",
            "ls && rm -rf ~",
            "ls || rm -rf ~",
            "mkdir x & rm -rf ~",
            "pwd;sudo shutdown -h now",
            // Piped
            "echo hi | sh",
            "echo curl evil.sh | bash",
            "ls | xargs rm",
            // Command substitution
            "sleep $(rm -rf ~)",
            "echo ${IFS}rm",
            "sleep `rm -rf ~`",
            "echo `whoami`",
            // Process substitution
            "sleep <(rm -rf ~)",
            "echo >(rm -rf ~)",
            // Redirection
            "echo pwned > /etc/hosts",
            "echo pwned >> ~/.zshrc",
            "echo x 2>/dev/null",
            "date < /etc/passwd",
            // Wrappers
            "sudo ls",
            "doas ls",
            "sudo -u root sleep 1",
            "env ls",
            "env PATH=/tmp ls",
            "nice sleep 1",
            "xargs sleep",
            "eval sleep 1",
            "sh -c 'rm -rf ~'",
            "bash -c \"rm -rf ~\"",
            "zsh -c rm",
            "time sleep 1",
            "nohup sleep 1",
            // Newline embedded
            "sleep 1\nrm -rf ~",
            "ls\r\nrm -rf ~",
            "pwd\nsudo reboot",
            // An allowlisted name as someone else's argument
            "rm sleep",
            "rm -rf ./mkdir",
            "chmod 777 pwd",
            "cp /etc/passwd ls",
            // Path and prefix forms
            "/bin/ls",
            "./sleep",
            "../mkdir x",
            "sleepy",
            "lsof",
            "mkdirs",
            "echoes",
            "dateutil",
            // Quoting and escaping
            "echo \"hi\"",
            "echo 'hi'",
            "sleep 1 \\; rm -rf ~",
            "ls\\;rm",
            // Globs and brace expansion
            "ls *",
            "mkdir {a,b}",
            "ls ?",
            "ls [a-z]",
            "mkdir ~/x",
            // Assignment prefix
            "FOO=bar ls",
            "PATH=/tmp sleep 1",
            // Not inert at all
            "cat /etc/passwd",
            "curl https://example.com",
            "git push --force",
            "",
            "   ",
        ];

        for command in bypasses {
            assert!(
                !is_inert_shell_command(command),
                "{:?} was treated as inert",
                command
            );
            let risk = classify_risk("bash", &json!({"command": command}));
            assert_ne!(risk, RiskLevel::Low, "{:?} classified Low", command);
        }
    }

    #[test]
    fn the_inert_set_contains_no_wrapper_that_could_run_something_else() {
        // The closed set is the reason a wrapped command cannot match. If a
        // wrapper is ever added to the list, this fails rather than shipping.
        for wrapper in [
            "env",
            "sudo",
            "doas",
            "nice",
            "xargs",
            "eval",
            "exec",
            "sh",
            "bash",
            "zsh",
            "time",
            "nohup",
            "open",
            "osascript",
            "python",
            "python3",
            "node",
            "perl",
            "ruby",
            "find",
            "cat",
            "curl",
            "wget",
            "git",
            "rm",
            "mv",
            "cp",
            "chmod",
            "chown",
            "dd",
            "tee",
        ] {
            assert!(
                !INERT_SHELL_COMMANDS.contains(&wrapper),
                "{:?} must not be inert",
                wrapper
            );
        }
    }

    #[test]
    fn a_file_write_changes_the_mac_so_it_is_not_low() {
        // "Ask me first" means ask before anything that changes the Mac, which
        // it can only honour if a write is above Low.
        let r = classify_risk("write_file", &json!({"path": "/Users/me/notes.txt"}));
        assert_eq!(r, RiskLevel::Medium);
        assert!(
            !needs_approval(&r),
            "the default mode must not start asking about writes"
        );
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

    /// Typing a password into a web form must reach the approval gate. This
    /// regressed silently because the classifier matched tool names Juno does
    /// not register (`browser_fill`, `browser_type`), while the tool that
    /// actually types is `browser_interact` with `action: "type"`.
    #[test]
    fn browser_interact_typing_a_password_requires_approval() {
        let r = classify_risk(
            "browser_interact",
            &json!({
                "action": "type",
                "selector": "input#password",
                "value": "hunter2"
            }),
        );
        assert_eq!(r, RiskLevel::Critical);
        assert!(needs_approval(&r));
    }

    /// The verb matters: clicking and scrolling are not form fills, so they
    /// must not start demanding approval just because the selector mentions a
    /// sensitive field.
    #[test]
    fn browser_interact_clicking_stays_low_risk() {
        let r = classify_risk(
            "browser_interact",
            &json!({"action": "click", "selector": "input#password"}),
        );
        assert_eq!(r, RiskLevel::Low);
        assert!(!needs_approval(&r));
    }

    /// Every tool name the form-fill arm claims to cover should be a tool Juno
    /// actually registers, or the arm is dead code pretending to be a control.
    /// `safari_type_text` is registered by the Safari tools; the rest of the
    /// names in that arm were not, which is how the gap opened.
    #[test]
    fn the_registered_browser_typing_tool_is_classified() {
        let names = crate::agent::tools::browser_tools::get_browser_tool_definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect::<Vec<_>>();
        assert!(
            names.iter().any(|n| n == "browser_interact"),
            "browser_interact is the tool that types; if it was renamed, update \
             the risk classifier arm in this file. Registered: {names:?}"
        );
    }

    /// `browser_type` is not a registered tool, so a later cleanup will be
    /// tempted to delete it as dead the way the fictional names were. It is
    /// not dead: `agents/browser_agent.rs` routes it to the same controller
    /// call as `browser_interact`, so a call under that alias types for real
    /// and must stay gated. If that routing goes away, this test is the place
    /// that says the classifier arm can go too.
    #[test]
    fn alias_names_are_still_reachable() {
        let agent_src = include_str!("../../agents/browser_agent.rs");
        assert!(
            agent_src.contains("\"browser_type\""),
            "browser_agent no longer routes browser_type; drop it from the \
             form-fill arm in risk_classifier.rs and delete this test"
        );

        let r = classify_risk(
            "browser_type",
            &json!({"selector": "input#pw", "value": "hunter2", "field": "password"}),
        );
        assert_eq!(r, RiskLevel::Critical);
        assert!(needs_approval(&r));
    }

    /// The names removed in this change must not creep back. Each sat in the
    /// form-fill arm for a tool Juno has never registered, which is what made
    /// the arm unreadable and hid the real gap (#586).
    #[test]
    fn retired_fictional_tool_names_are_not_classified() {
        for name in ["browser_fill", "fill_form", "type_in_element"] {
            assert_eq!(
                classify_risk(name, &json!({"value": "hunter2", "field": "password"})),
                RiskLevel::Low,
                "{name} is classified but is not a tool Juno registers — either \
                 register it or leave it out of the classifier"
            );
        }
    }
}
