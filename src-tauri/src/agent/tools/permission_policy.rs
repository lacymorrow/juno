//! The one place that decides whether Juno stops to ask.
//!
//! Before this module the answer came from two places at once: a global
//! `tool_approval_required` flag and a risk threshold inside the agent runner.
//! Both had to agree to let a tool through, so turning the flag off changed
//! nothing a person could see and every shell command prompted anyway. A
//! control that does not control what it names is the defect; one function is
//! the fix.
//!
//! Three things live here:
//!
//! 1. [`PermissionMode`], the choice a person makes in Security and Privacy.
//! 2. [`requires_approval`], the only answer to "does this need asking".
//! 3. [`describe_action`], the sentence the prompt shows, written here so the
//!    frontend never has to turn a tool name into English.

use serde_json::Value;

use crate::agent::tools::risk_classifier;
use crate::state::RiskLevel;

/// How much Juno interrupts.
///
/// Persisted as a string in `AgentSettings::permission_mode` so an unknown
/// value from a newer build degrades to the default instead of failing to
/// deserialize the whole settings file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionMode {
    /// Ask before anything that changes the Mac. Loud on purpose.
    AskFirst,
    /// Ask before risky things. The default.
    #[default]
    AskWhenRisky,
    /// Do not ask, with one floor: anything irreversible still asks.
    DontAsk,
}

// The stored values. They are declared in `constants::settings::defaults`
// because the TypeScript codegen reads that file, and aliased here because
// this is the module that gives them meaning.

/// Stored value for [`PermissionMode::AskFirst`].
pub const MODE_ASK_FIRST: &str = crate::constants::settings::defaults::PERMISSION_MODE_ASK_FIRST;
/// Stored value for [`PermissionMode::AskWhenRisky`].
pub const MODE_ASK_WHEN_RISKY: &str =
    crate::constants::settings::defaults::PERMISSION_MODE_ASK_WHEN_RISKY;
/// Stored value for [`PermissionMode::DontAsk`].
pub const MODE_DONT_ASK: &str = crate::constants::settings::defaults::PERMISSION_MODE_DONT_ASK;

impl PermissionMode {
    /// Read a stored setting. Anything unrecognised becomes the default, so a
    /// hand-edited or newer settings file cannot land someone in a mode that
    /// asks less than they chose.
    pub fn from_setting(value: &str) -> Self {
        match value {
            MODE_ASK_FIRST => Self::AskFirst,
            MODE_DONT_ASK => Self::DontAsk,
            _ => Self::AskWhenRisky,
        }
    }

    /// The value written to settings.
    pub fn as_setting(self) -> &'static str {
        match self {
            Self::AskFirst => MODE_ASK_FIRST,
            Self::AskWhenRisky => MODE_ASK_WHEN_RISKY,
            Self::DontAsk => MODE_DONT_ASK,
        }
    }
}

/// Whether this action has to stop and ask the person.
///
/// `granted` is a "do not ask again" the person already gave for this tool in
/// this conversation. It is checked after the floor, never before it.
///
/// The floor: [`RiskLevel::Critical`] always asks. Critical is the irreversible
/// and privilege-escalating set (sudo, `rm -rf`, writing into system
/// directories, typing a password or a card number into a page). No mode and no
/// standing grant waives it, which is why there is no full-autonomy mode: a
/// mode that waived this would have to say so out loud, and nobody asked for
/// one. Widening what Critical means to make a mode look tame is the other way
/// to break this, so Critical is defined once in `risk_classifier` and this
/// module does not get a vote.
pub fn requires_approval(mode: PermissionMode, risk: &RiskLevel, granted: bool) -> bool {
    if matches!(risk, RiskLevel::Critical) {
        return true;
    }
    if granted {
        return false;
    }
    match mode {
        // Ask about everything the classifier puts above Low.
        //
        // This comment used to say "everything that changes the Mac is Medium
        // or above, so Low is the only thing that goes through". That was not
        // true, and stating an invariant the code does not hold is what stopped
        // the next reader checking: Anthropic's text editor
        // (`str_replace_based_edit_tool`), the `text_editor_*` family and the
        // two file tools in `desktop_tools` all matched no arm in
        // `risk_classifier`, came through here as Low, and rewrote files on
        // disk with no prompt in the mode whose name is a promise to ask. They
        // are classified now, and `risk_classifier::gate_name_truth` fails the
        // build if a tool Juno can execute goes unclassified again.
        //
        // What is true: Low is the classifier's judgement that an action
        // leaves nothing behind worth interrupting for. Reading, looking, a
        // screenshot, an inert shell command, and desktop or page input the
        // person is watching happen (a click, a keystroke, a scroll) are all
        // Low. The last of those is deliberate and is the one case where "ask
        // first" does not: a prompt per click is a prompt nobody reads, which
        // is less safety than one prompt that means something. Everything that
        // writes a file, deletes one, runs a command, runs a script, types
        // into a page or grants a standing capability is Medium or above and
        // asks here. `risk_classifier::DELIBERATELY_UNGATED_TOOLS` is the list
        // of what Low covers and why, rather than a sentence like this one.
        PermissionMode::AskFirst => !matches!(risk, RiskLevel::Low),
        // Destructive, installing, or capability-granting actions are High.
        PermissionMode::AskWhenRisky => matches!(risk, RiskLevel::High),
        PermissionMode::DontAsk => false,
    }
}

/// Why an approval wait ended.
///
/// Every variant resolves to `approved` or `denied`. There is deliberately no
/// `Pending` here: the row on screen keeps its Allow and Don't allow buttons
/// while it believes the answer is pending, and the bug this type exists to
/// prevent was a timeout that denied the tool in the backend and told the
/// frontend nothing, leaving two live buttons that did nothing at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalOutcome {
    /// The person pressed Allow.
    Allowed,
    /// The person pressed Don't allow.
    Denied,
    /// Nobody answered inside the window.
    TimedOut,
    /// The run was cancelled while the question was on screen.
    Cancelled,
}

impl ApprovalOutcome {
    /// Every outcome, so a test can prove each one settles the row.
    pub const ALL: [ApprovalOutcome; 4] = [
        ApprovalOutcome::Allowed,
        ApprovalOutcome::Denied,
        ApprovalOutcome::TimedOut,
        ApprovalOutcome::Cancelled,
    ];

    /// The state the chat row must end up in. Matches the frontend's
    /// `approval_state`, which only knows `pending`, `approved` and `denied`.
    pub fn resolution(self) -> &'static str {
        match self {
            Self::Allowed => "approved",
            Self::Denied | Self::TimedOut | Self::Cancelled => "denied",
        }
    }

    /// What goes in the transcript the model reads back.
    pub fn reason(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Denied => "you did not allow it",
            Self::TimedOut => "nobody answered in time",
            Self::Cancelled => "the run was stopped",
        }
    }
}

// --- the sentence on the prompt ---

/// One plain sentence for what Juno is about to do.
///
/// The prompt used to read "Run bash", an em dash, then "sleep 1", which names
/// the tool and the implementation and asks a person to audit it. This writes what a person can
/// picture, and it never leaks a raw tool name: the fallback runs a tool name
/// through [`friendly_tool_name`] rather than printing `safari_extract_dom`.
pub fn describe_action(tool_name: &str, tool_input: &Value) -> String {
    let field = |key: &str| tool_input.get(key).and_then(Value::as_str);
    let shell = || {
        field("command")
            .or_else(|| field("cmd"))
            .or_else(|| field("bash"))
            .unwrap_or("")
    };
    let path = || field("path").or_else(|| field("file_path")).unwrap_or("");

    match tool_name {
        "bash" | "execute_bash" | "run_bash_command" | "shell_execute" | "execute_command"
        | "run_command" => {
            let cmd = clip(shell(), 160);
            if cmd.is_empty() {
                "Run a command in the terminal".to_string()
            } else {
                format!("Run this in the terminal: {}", cmd)
            }
        }

        // Every name the risk classifier's file arms guard needs a sentence
        // here, or a person gets asked to approve "Use text editor str
        // replace", which is the tool name with the underscores taken out.
        // `risk_classifier::gate_name_truth::everything_the_gate_stops_reads_as_a_sentence`
        // fails when one of them falls through to the generic fallback.
        "write_file" | "create_file" | "text_editor_create" => match short_path(path()) {
            Some(name) => format!("Write to {}", name),
            None => "Write to a file".to_string(),
        },
        "edit_file" | "text_editor_insert" | "text_editor_str_replace" | "open_file_and_type" => {
            match short_path(path()) {
                Some(name) => format!("Change {}", name),
                None => "Change a file".to_string(),
            }
        }
        // Anthropic's editor carries the verb in `command`, the same way the
        // risk classifier reads it, so the sentence says which of the two
        // things it is about to do.
        "str_replace_based_edit_tool" => {
            let verb = if field("command") == Some("create") {
                "Write to"
            } else {
                "Change"
            };
            match short_path(path()) {
                Some(name) => format!("{} {}", verb, name),
                None => format!("{} a file", verb),
            }
        }
        // Takes no arguments: it restores whatever Juno last edited, from one
        // snapshot that the undo itself spends. Naming a path here would be a
        // guess, so it does not.
        "text_editor_undo_edit" => "Undo the last file change Juno made".to_string(),
        "save_and_close_file" => "Save the file you have open, and close it".to_string(),
        "run_applescript" => "Run a script on your Mac".to_string(),
        "delete_file" | "remove_file" | "unlink_file" => match short_path(path()) {
            Some(name) => format!("Delete {}", name),
            None => "Delete a file".to_string(),
        },

        "browser_navigate" | "navigate_to_url" | "open_url" | "safari_navigate" => {
            match host_of(field("url").unwrap_or("")) {
                Some(host) => format!("Open {}", host),
                None => "Open a page in the browser".to_string(),
            }
        }

        "safari_execute_javascript" => "Run a script on the page you have open".to_string(),

        "browser_type" | "safari_type_text" => "Type into the page".to_string(),
        "browser_interact" => match field("action") {
            Some("type") => "Type into the page".to_string(),
            Some("select") => "Choose an option on the page".to_string(),
            _ => "Use the page".to_string(),
        },

        "create_scheduled_automation" => {
            "Set up a task that runs on its own later, without you here".to_string()
        }
        "delete_scheduled_automation" => "Remove one of your scheduled tasks".to_string(),

        "computer" => match field("action") {
            Some("key") => match field("text") {
                Some(keys) => format!("Press {}", clip(keys, 40)),
                None => "Press a key combination".to_string(),
            },
            Some("type") => "Type on your keyboard".to_string(),
            Some(other) => format!("Use your mouse and keyboard to {}", other.replace('_', " ")),
            None => "Use your mouse and keyboard".to_string(),
        },

        // Connected-app actions. The tool name is a slug
        // (`mcp_composio_GMAIL_SEND_EMAIL`) or Composio's multi-execute with
        // the real slugs in its arguments; either way the person has to read
        // what is about to happen ("Send an email to katie@example.com in
        // Gmail"), never a slug with the underscores taken out.
        name if risk_classifier::is_external_mcp_tool(name)
            || risk_classifier::integration_action_slug(name).is_some() =>
        {
            describe_integration_action(name, tool_input)
        }

        other => format!("Use {}", friendly_tool_name(other)),
    }
}

/// Slug tokens that read as the verb of the sentence. The risk verbs plus the
/// ordinary write verbs connectors use; a slug with none of these still gets
/// a readable sentence, just a less specific one.
const INTEGRATION_SENTENCE_VERBS: &[&str] = &[
    // The gate's own verb lists, so the sentence can always name what the
    // gate stopped.
    "SEND",
    "DELETE",
    "ARCHIVE",
    "REFUND",
    "PAY",
    "TRANSFER",
    "CHARGE",
    "REMOVE",
    "REVOKE",
    "SHARE",
    "GET",
    "LIST",
    "SEARCH",
    "FETCH",
    "READ",
    // Ordinary write verbs.
    "CREATE",
    "UPDATE",
    "ADD",
    "MOVE",
    "REPLY",
    "FORWARD",
    "MANAGE",
    "EXECUTE",
    "CHECK",
    "WAIT",
    "UPLOAD",
    "DOWNLOAD",
    "INSERT",
    "COPY",
    "MARK",
    "PATCH",
    "SYNC",
    "DUPLICATE",
];

/// The sentence for a connected-app action.
fn describe_integration_action(tool_name: &str, tool_input: &Value) -> String {
    use crate::agent::tools::risk_classifier as rc;

    let Some(slug) = rc::integration_action_slug(tool_name) else {
        return "Use a tool from a connected app".to_string();
    };

    if slug == rc::COMPOSIO_MULTI_EXECUTE_TOOL {
        let actions = rc::multi_execute_inner_actions(tool_input);
        let Some((riskiest, args)) = actions
            .iter()
            .max_by_key(|(inner, _)| rc::classify_integration_slug(inner))
            .cloned()
        else {
            return "Run actions in your connected apps".to_string();
        };
        let sentence = integration_sentence(&riskiest, &args);
        return match actions.len() {
            0 | 1 => sentence,
            2 => format!("{}, and one more app action", sentence),
            n => format!("{}, and {} more app actions", sentence, n - 1),
        };
    }

    integration_sentence(slug, tool_input)
}

/// `GMAIL_SEND_EMAIL` with `{"to": "katie@example.com"}` reads
/// "Send an email to katie@example.com in Gmail".
fn integration_sentence(slug: &str, args: &Value) -> String {
    use crate::agent::tools::risk_classifier as rc;

    let tokens: Vec<&str> = slug.split('_').collect();
    let toolkit = tokens.first().and_then(|t| rc::toolkit_display_name(t));

    // The verb is wherever the slug put it: `GMAIL_SEND_EMAIL` leads with it,
    // `GOOGLECALENDAR_EVENTS_LIST` trails with it.
    let verb_index =
        (1..tokens.len()).find(|&index| INTEGRATION_SENTENCE_VERBS.contains(&tokens[index]));

    let mut sentence = match verb_index {
        Some(verb_index) => {
            let verb = capitalize(&tokens[verb_index].to_lowercase());
            let object = tokens
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(index, _)| *index != verb_index)
                .map(|(_, token)| token.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            if object.is_empty() {
                verb
            } else {
                format!("{} {}", verb, with_article(&object))
            }
        }
        None => {
            let object = tokens
                .iter()
                .skip(1)
                .map(|token| token.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            if object.is_empty() {
                "Use an app action".to_string()
            } else {
                format!("Run {}", object)
            }
        }
    };

    if let Some(target) = integration_target(args) {
        sentence.push_str(&format!(" to {}", clip(&target, 60)));
    }
    if let Some(app) = toolkit {
        sentence.push_str(&format!(" in {}", app));
    }
    sentence
}

/// Who or what the action is aimed at, read from the argument names
/// connectors commonly use for a recipient.
fn integration_target(args: &Value) -> Option<String> {
    const TARGET_KEYS: &[&str] = &["to", "recipient_email", "recipient", "user_email", "email"];
    for key in TARGET_KEYS {
        match args.get(key) {
            Some(Value::String(value)) if !value.is_empty() => return Some(value.clone()),
            Some(Value::Array(items)) => {
                let targets: Vec<&str> = items.iter().filter_map(Value::as_str).collect();
                if !targets.is_empty() {
                    return Some(targets.join(", "));
                }
            }
            _ => {}
        }
    }
    None
}

/// "an email", "a draft", "events" — enough grammar for a prompt, no more.
fn with_article(object: &str) -> String {
    let first_word = object.split_whitespace().next().unwrap_or(object);
    if first_word.ends_with('s') {
        return object.to_string();
    }
    let article = match object.chars().next() {
        Some('a') | Some('e') | Some('i') | Some('o') | Some('u') => "an",
        _ => "a",
    };
    format!("{} {}", article, object)
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// The sentence for a batch, built from its riskiest step.
pub fn describe_batch(first_sentence: &str, batch_size: usize) -> String {
    match batch_size {
        0 | 1 => first_sentence.to_string(),
        2 => format!("{}, and one more step", first_sentence),
        n => format!("{}, and {} more steps", first_sentence, n - 1),
    }
}

/// A tool name a person can read. Never `snake_case`, never a bare `bash`.
pub fn friendly_tool_name(tool_name: &str) -> String {
    match tool_name {
        "bash" | "execute_bash" | "run_bash_command" | "shell_execute" | "execute_command"
        | "run_command" => "terminal commands".to_string(),
        "computer" => "your mouse and keyboard".to_string(),
        name if name.starts_with("safari_") || name.starts_with("browser_") => {
            "the browser".to_string()
        }
        // Connected-app tools: "Always allow Gmail actions" when the slug
        // names one app, "connected app actions" when it does not (the
        // multi-execute meta-tool can carry any app, so the label must not
        // promise less than the grant covers).
        name if risk_classifier::is_external_mcp_tool(name)
            || risk_classifier::integration_action_slug(name).is_some() =>
        {
            risk_classifier::integration_action_slug(name)
                .filter(|slug| *slug != risk_classifier::COMPOSIO_MULTI_EXECUTE_TOOL)
                .and_then(|slug| slug.split('_').next())
                .and_then(risk_classifier::toolkit_display_name)
                .map(|app| format!("{} actions", app))
                .unwrap_or_else(|| "connected app actions".to_string())
        }
        name if name.contains("file") => "your files".to_string(),
        name => name.replace('_', " "),
    }
}

/// The tail of a path, so the prompt says `notes.txt` rather than a home
/// directory nobody needs to read.
fn short_path(path: &str) -> Option<String> {
    let trimmed = path.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if name.is_empty() {
        None
    } else {
        Some(clip(name, 60))
    }
}

/// The hostname of a URL, which is the part of a link a person recognises.
fn host_of(url: &str) -> Option<String> {
    let rest = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(url)
        .trim();
    if rest.is_empty() {
        return None;
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    if host.is_empty() {
        None
    } else {
        Some(clip(host, 60))
    }
}

/// Cut to a character budget without splitting a multi-byte character.
fn clip(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let kept: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn critical_always_asks_in_every_mode() {
        // The floor. No mode and no standing grant gets past it.
        for mode in [
            PermissionMode::AskFirst,
            PermissionMode::AskWhenRisky,
            PermissionMode::DontAsk,
        ] {
            assert!(requires_approval(mode, &RiskLevel::Critical, false));
            assert!(requires_approval(mode, &RiskLevel::Critical, true));
        }
    }

    #[test]
    fn ask_first_asks_about_anything_that_changes_the_mac() {
        let mode = PermissionMode::AskFirst;
        assert!(!requires_approval(mode, &RiskLevel::Low, false));
        assert!(requires_approval(mode, &RiskLevel::Medium, false));
        assert!(requires_approval(mode, &RiskLevel::High, false));
    }

    #[test]
    fn ask_when_risky_lets_ordinary_work_through() {
        let mode = PermissionMode::AskWhenRisky;
        assert!(!requires_approval(mode, &RiskLevel::Low, false));
        assert!(!requires_approval(mode, &RiskLevel::Medium, false));
        assert!(requires_approval(mode, &RiskLevel::High, false));
    }

    #[test]
    fn dont_ask_still_keeps_the_floor() {
        let mode = PermissionMode::DontAsk;
        assert!(!requires_approval(mode, &RiskLevel::High, false));
        assert!(requires_approval(mode, &RiskLevel::Critical, false));
    }

    #[test]
    fn a_session_grant_covers_high_but_not_critical() {
        let mode = PermissionMode::AskFirst;
        assert!(!requires_approval(mode, &RiskLevel::High, true));
        assert!(requires_approval(mode, &RiskLevel::Critical, true));
    }

    #[test]
    fn an_unknown_stored_mode_falls_back_to_the_default() {
        assert_eq!(
            PermissionMode::from_setting("full_autonomy"),
            PermissionMode::AskWhenRisky
        );
        assert_eq!(
            PermissionMode::from_setting(""),
            PermissionMode::AskWhenRisky
        );
        assert_eq!(
            PermissionMode::from_setting(MODE_DONT_ASK),
            PermissionMode::DontAsk
        );
    }

    /// Pins the shipped default to the middle mode. The literal in
    /// `constants::settings::defaults::PERMISSION_MODE` is spelled out for the
    /// TypeScript codegen, so nothing but this stops the two drifting.
    #[test]
    fn the_shipped_default_is_ask_when_risky() {
        assert_eq!(
            crate::constants::settings::defaults::PERMISSION_MODE,
            MODE_ASK_WHEN_RISKY
        );
        assert_eq!(PermissionMode::default(), PermissionMode::AskWhenRisky);
    }

    #[test]
    fn modes_round_trip_through_settings() {
        for mode in [
            PermissionMode::AskFirst,
            PermissionMode::AskWhenRisky,
            PermissionMode::DontAsk,
        ] {
            assert_eq!(PermissionMode::from_setting(mode.as_setting()), mode);
        }
    }

    /// The dead-control pin. A timeout denied the tool in the backend and told
    /// the frontend nothing, so the row kept its two buttons and neither did
    /// anything. Every way a wait can end has to settle the row.
    #[test]
    fn every_outcome_settles_the_row() {
        for outcome in ApprovalOutcome::ALL {
            let resolution = outcome.resolution();
            assert!(
                resolution == "approved" || resolution == "denied",
                "{:?} resolved to {:?}, which leaves the row pending",
                outcome,
                resolution
            );
            assert!(!outcome.reason().is_empty());
        }
    }

    #[test]
    fn a_timeout_denies_rather_than_hanging() {
        assert_eq!(ApprovalOutcome::TimedOut.resolution(), "denied");
        assert_eq!(ApprovalOutcome::Allowed.resolution(), "approved");
    }

    #[test]
    fn the_sentence_never_contains_a_raw_tool_name() {
        let cases = [
            ("bash", json!({"command": "npm install left-pad"})),
            ("write_file", json!({"path": "/Users/me/notes.txt"})),
            ("delete_file", json!({"path": "/Users/me/notes.txt"})),
            ("safari_execute_javascript", json!({"javascript": "x"})),
            ("browser_interact", json!({"action": "type"})),
            ("create_scheduled_automation", json!({})),
            ("computer", json!({"action": "key", "text": "cmd+q"})),
        ];
        for (tool, input) in cases {
            let sentence = describe_action(tool, &input);
            assert!(
                !sentence.contains(tool),
                "{:?} leaked its tool name: {:?}",
                tool,
                sentence
            );
            assert!(!sentence.contains('_'), "{:?} reads like code", sentence);
        }
    }

    #[test]
    fn the_sentence_has_no_em_dash() {
        // The old format string joined the tool name and the input with an em
        // dash. That character is banned in this project's writing, and it was
        // on screen.
        let sentence = describe_action("bash", &json!({"command": "sleep 1"}));
        assert!(!sentence.contains('\u{2014}'));
        assert_eq!(sentence, "Run this in the terminal: sleep 1");
    }

    #[test]
    fn descriptions_read_as_sentences() {
        assert_eq!(
            describe_action("write_file", &json!({"path": "/Users/me/docs/notes.txt"})),
            "Write to notes.txt"
        );
        assert_eq!(
            describe_action(
                "browser_navigate",
                &json!({"url": "https://example.com/pay"})
            ),
            "Open example.com"
        );
        assert_eq!(
            describe_action("computer", &json!({"action": "key", "text": "cmd+q"})),
            "Press cmd+q"
        );
    }

    /// The file tools the risk gate started stopping need sentences of their
    /// own. Without these arms the prompt reads "Use text editor str replace",
    /// which is the tool name with the underscores taken out.
    #[test]
    fn the_file_tools_the_gate_now_stops_read_as_sentences() {
        let notes = json!({"path": "/Users/me/docs/notes.txt"});
        let notes_by_file_path = json!({"file_path": "/Users/me/docs/notes.txt"});

        assert_eq!(
            describe_action("text_editor_create", &notes_by_file_path),
            "Write to notes.txt"
        );
        assert_eq!(
            describe_action("text_editor_str_replace", &notes_by_file_path),
            "Change notes.txt"
        );
        assert_eq!(
            describe_action("text_editor_insert", &notes_by_file_path),
            "Change notes.txt"
        );
        assert_eq!(
            describe_action("open_file_and_type", &notes_by_file_path),
            "Change notes.txt"
        );
        assert_eq!(
            describe_action("text_editor_undo_edit", &json!({})),
            "Undo the last file change Juno made"
        );
        assert_eq!(
            describe_action("save_and_close_file", &json!({})),
            "Save the file you have open, and close it"
        );
        assert_eq!(
            describe_action("run_applescript", &json!({"script": "beep"})),
            "Run a script on your Mac"
        );

        // Anthropic's editor says which of the two things it is doing, read
        // from `command` exactly as the risk classifier reads it.
        let mut creating = notes.clone();
        creating["command"] = json!("create");
        assert_eq!(
            describe_action("str_replace_based_edit_tool", &creating),
            "Write to notes.txt"
        );

        let mut replacing = notes.clone();
        replacing["command"] = json!("str_replace");
        assert_eq!(
            describe_action("str_replace_based_edit_tool", &replacing),
            "Change notes.txt"
        );
    }

    #[test]
    fn an_unmapped_tool_still_reads_as_english() {
        let sentence = describe_action("safari_extract_dom", &json!({}));
        assert_eq!(sentence, "Use the browser");
    }

    #[test]
    fn a_batch_counts_its_remaining_steps() {
        assert_eq!(describe_batch("Open example.com", 1), "Open example.com");
        assert_eq!(
            describe_batch("Open example.com", 2),
            "Open example.com, and one more step"
        );
        assert_eq!(
            describe_batch("Open example.com", 4),
            "Open example.com, and 3 more steps"
        );
    }

    /// The acceptance sentence from LAC-4210: the approval prompt for a send
    /// buried in a Composio batch names the action in English, with the
    /// recipient, and never shows a slug.
    #[test]
    fn a_connected_app_send_reads_as_a_sentence_with_the_recipient() {
        let input = json!({
            "tools": [{
                "tool_slug": "GMAIL_SEND_EMAIL",
                "arguments": {"to": "katie@example.com", "subject": "Site is live"}
            }]
        });
        assert_eq!(
            describe_action("mcp_composio_COMPOSIO_MULTI_EXECUTE_TOOL", &input),
            "Send an email to katie@example.com in Gmail"
        );
    }

    /// A batch sentence is led by its riskiest step and counts the rest, so
    /// one prompt covers the whole call without hiding what it covers.
    #[test]
    fn a_batch_of_app_actions_counts_the_rest() {
        let input = json!({
            "tools": [
                {"tool_slug": "GMAIL_FETCH_EMAILS", "arguments": {}},
                {"tool_slug": "GOOGLECALENDAR_EVENTS_LIST", "arguments": {}},
                {"tool_slug": "GMAIL_SEND_EMAIL", "arguments": {"to": "katie@example.com"}}
            ]
        });
        assert_eq!(
            describe_action("mcp_composio_COMPOSIO_MULTI_EXECUTE_TOOL", &input),
            "Send an email to katie@example.com in Gmail, and 2 more app actions"
        );
    }

    /// Slugs with the verb at the end still read verb-first, and no app
    /// sentence ever leaks a slug or an underscore.
    #[test]
    fn app_action_sentences_never_read_as_slugs() {
        let cases = [
            (
                "mcp_composio_GOOGLECALENDAR_EVENTS_LIST",
                json!({}),
                "List events in Google Calendar",
            ),
            (
                "mcp_composio_GMAIL_CREATE_EMAIL_DRAFT",
                json!({}),
                "Create an email draft in Gmail",
            ),
            (
                "mcp_composio_COMPOSIO_SEARCH_TOOLS",
                json!({}),
                "Search tools",
            ),
        ];
        for (tool, input, expected) in cases {
            let sentence = describe_action(tool, &input);
            assert_eq!(sentence, expected);
            assert!(!sentence.contains('_'), "{sentence:?} reads like code");
        }

        // An unreadable batch and a slug-less external tool still get plain
        // words rather than the raw name.
        assert_eq!(
            describe_action("mcp_composio_COMPOSIO_MULTI_EXECUTE_TOOL", &json!({})),
            "Run actions in your connected apps"
        );
        assert_eq!(
            describe_action("mcp_someserver_do_thing", &json!({})),
            "Use a tool from a connected app"
        );
    }

    /// "Don't ask again" on an app action says which app it covers, and the
    /// multi-execute meta-tool, which can carry any app, never claims one.
    #[test]
    fn the_always_allow_label_names_the_app_or_stays_general() {
        assert_eq!(
            friendly_tool_name("mcp_composio_GMAIL_SEND_EMAIL"),
            "Gmail actions"
        );
        assert_eq!(
            friendly_tool_name("mcp_composio_COMPOSIO_MULTI_EXECUTE_TOOL"),
            "connected app actions"
        );
        assert_eq!(
            friendly_tool_name("mcp_someserver_do_thing"),
            "connected app actions"
        );
    }

    #[test]
    fn a_long_command_is_clipped_not_truncated_mid_character() {
        let long = "sleep ".to_string() + &"é".repeat(400);
        let sentence = describe_action("bash", &json!({"command": long}));
        assert!(sentence.chars().count() < 200);
        assert!(sentence.ends_with('…'));
    }
}
