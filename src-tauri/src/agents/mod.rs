//! Agent session bookkeeping.
//!
//! This module used to hold a second, parallel agent implementation: an
//! `Orchestrator`, a `SpecializedAgent` trait, an `AgentFactory`, and
//! `SystemAgent` / `BrowserAgent` / `DesktopAgent`, each dispatching tool
//! calls through its own `handle_task`. None of it consulted
//! `risk_classifier` or `permission_policy`, so `SystemAgent` wrote files
//! with nothing asked. It was reachable only through fourteen Tauri commands
//! in `commands/orchestrator.rs` that no caller anywhere invoked, and the
//! orchestrator singleton was still constructed on every launch.
//!
//! It was removed rather than gated, because Juno's real multi-agent path is
//! `crate::agent` (singular) and always was: `anthropic.rs` builds a
//! `DefaultAgentRunner` for the orchestrator and another for each specialist
//! it delegates to, and every tool call in that tree passes through
//! `AgentRunner::check_batch_approval`. Two executors meant one of them was
//! unaudited by construction.
//!
//! What is left is the session registry, which is genuinely shared: the
//! gated runner, the computer-use tools and the session switcher commands all
//! read it. `agents_holds_no_executor` pins the removal, so a second
//! execution path cannot quietly reappear here.

pub mod session;

pub use session::{
    begin_session_run, broadcast_sessions_updated, color_for_slot, AgentSession, AgentSessionId,
    AgentSessionInfo, AgentSessionRegistry, AgentSessionStatus, SessionHandle, SESSION_COLOR_SLOTS,
};

#[cfg(test)]
mod structure_tests {
    /// Juno has exactly one agent execution path, and it is the gated one in
    /// `crate::agent`. This module is session bookkeeping only.
    ///
    /// The removed tree is described in this module's header. The failure it
    /// allowed was silent: a tool dispatcher here never called
    /// `check_batch_approval`, so adding one more agent "just like the
    /// others" added an ungated executor and nothing complained. This test is
    /// the thing that complains.
    ///
    /// If you are adding an agent, add it to `crate::agent` so it runs
    /// through `AgentRunner` and the approval gate. If you are adding real
    /// session bookkeeping, add the file to `ALLOWED` below and say why here.
    #[test]
    fn agents_holds_no_executor() {
        // Every file this module is allowed to contain.
        const ALLOWED: [&str; 2] = ["mod.rs", "session.rs"];

        // Spellings that mean "this file dispatches tool calls".
        const EXECUTOR_MARKERS: [&str; 4] = [
            "SpecializedAgent",
            "fn handle_task",
            "fn execute_tool",
            "commands::text_editor::",
        ];

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("agents");

        let mut unexpected_files = Vec::new();
        let mut executors = Vec::new();

        for entry in walkdir::WalkDir::new(&dir).into_iter().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let name = path
                .strip_prefix(&dir)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if !ALLOWED.contains(&name.as_str()) {
                unexpected_files.push(name.clone());
            }
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            for (i, line) in text.lines().enumerate() {
                let code = line.trim_start();
                // The markers are named in this test and in the module header,
                // so every comment form is skipped: //, /// and //!.
                if code.starts_with("//") {
                    continue;
                }
                for marker in EXECUTOR_MARKERS {
                    let Some(at) = code.find(marker) else {
                        continue;
                    };
                    // The marker spelled inside a string, like the list above.
                    if code[..at].ends_with('"') {
                        continue;
                    }
                    executors.push(format!("{}:{} ({})", name, i + 1, marker));
                }
            }
        }

        assert!(
            unexpected_files.is_empty(),
            "src/agents holds session bookkeeping only; new agents belong in \
             src/agent, behind AgentRunner's approval gate. Unexpected: {unexpected_files:#?}"
        );
        assert!(
            executors.is_empty(),
            "src/agents must not dispatch tool calls: an executor here bypasses \
             risk_classifier and permission_policy, which is exactly the defect \
             removing the old orchestrator fixed. Offenders: {executors:#?}"
        );
    }
}
