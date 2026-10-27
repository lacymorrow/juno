//! # Live cost-telemetry wire tests (LAC-4127)
//!
//! One real turn per provider, then the assertion that the acceptance criterion
//! actually asks for: **the CSV export has a row for it**, with the tokens the
//! provider reported and a dollar figure that matches the price table.
//!
//! The hermetic tests in `cost_log` prove the arithmetic and the record shape
//! against recorded frames. They cannot prove that the `usage` block is still
//! where Juno looks for it — that is a contract with four live APIs, and the
//! only thing that checks it is a real request. A provider moving `usage` one
//! level deeper would pass every unit test in the repo and silently log every
//! turn at $0.00.
//!
//! ## Running them
//!
//! `#[ignore]`d so `cargo test` stays hermetic and free. Each test needs its
//! own credential and **all** of them need a scratch log path, because they
//! assert on the contents of the log and must never append to the log of the
//! person running them:
//!
//! ```bash
//! export JUNO_COST_LOG_PATH="$(mktemp -t juno-cost)".ndjson
//!
//! # Anthropic, OpenAI and Gemini all run in CI too, from their repo secrets:
//! # .github/workflows/cost-telemetry-live.yml, "all" or one provider.
//! ANTHROPIC_API_KEY=sk-... cargo test --manifest-path src-tauri/Cargo.toml \
//!     --test cost_telemetry_live anthropic -- --ignored --nocapture --test-threads=1
//!
//! # OpenAI / Gemini
//! OPENAI_API_KEY=sk-...  cargo test ... --test cost_telemetry_live openai -- --ignored --nocapture --test-threads=1
//! GEMINI_API_KEY=...     cargo test ... --test cost_telemetry_live gemini -- --ignored --nocapture --test-threads=1
//!
//! # Claude CLI: no key, needs `claude` installed and signed in
//! cargo test ... --test cost_telemetry_live claude_cli -- --ignored --nocapture --test-threads=1
//! ```
//!
//! `--test-threads=1` is **required** when running more than one of them. The
//! open run is process-global (one app, one task at a time), so two tests in
//! flight would attribute each other's turns and both would see the wrong
//! number of rows. Each test filters the log by its own run id, which catches
//! that as a failure rather than hiding it — but a failure is not the point.
//!
//! ## What they cost
//!
//! The cheapest catalogued model per provider, a one-line prompt and a 32-token
//! ceiling: fractions of a cent per provider, per run. They are not on the PR
//! path for that reason all the same — a test that spends money on every push
//! is a test people turn off.
//!
//! ## A missing credential fails, it does not skip
//!
//! A silent skip is how a gate reports success for work it never did. Each test
//! panics with the variable it wanted, and the CI job decides which tests to
//! run from the secrets it actually has.

use juno_lib::agent::core::{AgentAction, Message, Role};
use juno_lib::agent::providers::anthropic::AnthropicBrain;
use juno_lib::agent::providers::claude_cli::ClaudeCliBrain;
use juno_lib::agent::providers::gemini::GeminiBrain;
use juno_lib::agent::providers::openai::OpenAIBrain;
use juno_lib::agent::providers::types::{model_ids, Price, Provider};
use juno_lib::agent::traits::AgentBrain;
use juno_lib::cost_log::{self, CostRecord};

/// Short, closed, and cheap to answer. A question with a long answer spends
/// output tokens to tell us nothing extra about the `usage` block.
const PROMPT: &str = "Reply with the single word: OK";

/// Enough for the word, the punctuation and a stop.
const MAX_TOKENS: u32 = 32;

/// The scratch log every test asserts against.
///
/// Required rather than defaulted: the default is the real log under the cache
/// directory, and a test that appends to the person's own measurement data is
/// worse than a test that refuses to run.
fn scratch_log() -> std::path::PathBuf {
    let raw = std::env::var(cost_log::ENV_LOG_PATH).unwrap_or_default();
    assert!(
        !raw.trim().is_empty(),
        "set {} to a scratch file first — these tests read the log back and must \
         not append to your real one. See the module docs.",
        cost_log::ENV_LOG_PATH
    );
    let path = cost_log::log_path().expect("an override always resolves");
    assert_eq!(
        path,
        std::path::PathBuf::from(raw.trim()),
        "cost_log::log_path() ignored {}",
        cost_log::ENV_LOG_PATH
    );
    path
}

fn credential(var: &str) -> String {
    let value = std::env::var(var).unwrap_or_default();
    assert!(
        !value.trim().is_empty(),
        "{var} is not set, so this provider cannot be checked. This test fails \
         rather than skipping: a skipped live check that reports success is how \
         an unverified acceptance criterion gets signed off."
    );
    value.trim().to_string()
}

/// Opens a run of its own so the assertions can find this turn's rows and
/// nothing else, however many tests share the file.
fn begin(run_id: &str) {
    cost_log::set_enabled(true);
    cost_log::begin_run(run_id.to_string());
}

/// Every row this run wrote, read back through the same reader the CSV export
/// command uses.
fn rows_for(run_id: &str) -> Vec<CostRecord> {
    cost_log::end_run();
    let all = cost_log::read_log().expect("the log is readable");
    all.into_iter().filter(|r| r.run_id == run_id).collect()
}

/// The assertions that are the same for every provider.
///
/// Returns the row, so a provider-specific test can add its own.
fn assert_one_priced_row(run_id: &str, provider: &Provider, model: &str) -> CostRecord {
    let rows = rows_for(run_id);
    assert_eq!(
        rows.len(),
        1,
        "one request should write exactly one row, got {rows:#?}"
    );
    let row = rows.into_iter().next().expect("length checked");

    assert_eq!(row.provider, provider.as_str());
    assert_eq!(
        row.model_id,
        cost_log::billing_model_id(provider, model),
        "the row must be keyed the way scripts/pricing/prices.json keys it"
    );

    // The point of the whole exercise: the provider still reports a prompt
    // size, and Juno still finds it. Zero here means the `usage` block moved.
    assert!(
        row.input_tokens + row.cache_read_tokens + row.cache_write_tokens > 0,
        "no prompt tokens recorded — the provider's usage block has moved: {row:#?}"
    );
    assert!(row.output_tokens > 0, "no output tokens recorded: {row:#?}");
    assert!(row.duration_ms > 0, "a real request takes measurable time");
    assert!(
        !row.outcome.is_empty() && row.outcome != "incomplete",
        "the turn should have reported a stop reason, got {:?}",
        row.outcome
    );

    // And the dollars agree with the catalog, recomputed here from the row's
    // own token counts — the same independent-recompute check the analyzer
    // does, so a wrong rate cannot pass through unnoticed on either side.
    let usage = cost_log::TokenUsage {
        input: row.input_tokens,
        output: row.output_tokens,
        cache_read: row.cache_read_tokens,
        cache_write: row.cache_write_tokens,
    };
    let def = provider
        .model_definitions()
        .iter()
        .find(|d| d.id == model)
        .expect("the model under test is in the catalog");
    match (&def.price, cost_log::cost_usd(&def.price, &usage)) {
        (Price::Metered(_), Some(expected)) => {
            assert!(
                (row.cost_usd - expected).abs() < 1e-9,
                "recorded ${} but the catalog rates give ${expected}",
                row.cost_usd
            );
            assert!(expected > 0.0, "a metered turn costs something");
        }
        (Price::Subscription, _) => {
            assert_eq!(row.cost_usd, 0.0, "a seat has no marginal per-token cost")
        }
        (price, computed) => panic!("{model} is priced {price:?}, computed {computed:?}"),
    }

    // Finally the criterion as written: the export has a row for this turn.
    let csv = cost_log::to_csv(std::slice::from_ref(&row));
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 2, "header plus one row, got {csv}");
    assert_eq!(lines[0], cost_log::COLUMNS.join(","));
    eprintln!("\n{} verified:\n{csv}", provider.as_str());

    row
}

fn one_user_turn() -> Vec<Message> {
    vec![Message::new(Role::User, PROMPT)]
}

/// A model that answers at all. Any of these means the round-trip worked; an
/// `Error` action means it did not.
fn assert_answered(action: AgentAction) {
    match action {
        AgentAction::Finish(text) | AgentAction::RespondToUser(text) => {
            assert!(!text.trim().is_empty(), "the model answered with nothing")
        }
        AgentAction::ExecuteTool(_) | AgentAction::Think => {
            panic!("no tools were offered, so neither of these is a valid answer")
        }
        AgentAction::Error(e) => panic!("the provider refused the turn: {e}"),
    }
}

#[tokio::test]
#[ignore = "spends money: needs ANTHROPIC_API_KEY and JUNO_COST_LOG_PATH"]
async fn anthropic_turn_writes_a_priced_row() {
    scratch_log();
    let key = credential("ANTHROPIC_API_KEY");
    let model = model_ids::CLAUDE_HAIKU_4_5;

    let brain = AnthropicBrain::new(key, Some(model.to_string()), Some(MAX_TOKENS), None)
        .expect("brain construction only builds an HTTP client");

    let run_id = "live-anthropic";
    begin(run_id);
    assert_answered(
        brain
            .decide_next_action(&one_user_turn(), &[])
            .await
            .expect("the request reached the API"),
    );

    let row = assert_one_priced_row(run_id, &Provider::Anthropic, model);
    // Anthropic is the one provider that reports `input_tokens` exclusive of
    // the cache categories, so a prompt this small must land in `input`.
    assert!(
        row.input_tokens > 0,
        "fresh input tokens expected: {row:#?}"
    );
}

#[tokio::test]
#[ignore = "spends money: needs OPENAI_API_KEY and JUNO_COST_LOG_PATH"]
async fn openai_turn_writes_a_priced_row() {
    scratch_log();
    let key = credential("OPENAI_API_KEY");
    let model = model_ids::OPENAI_CODEX_5_3;

    let brain = OpenAIBrain::new(key, Some(model.to_string()), Some(MAX_TOKENS), None, None)
        .expect("brain construction only builds an HTTP client");

    let run_id = "live-openai";
    begin(run_id);
    assert_answered(
        brain
            .decide_next_action(&one_user_turn(), &[])
            .await
            .expect("the request reached the API"),
    );

    assert_one_priced_row(run_id, &Provider::OpenAI, model);
}

#[tokio::test]
#[ignore = "spends money: needs GEMINI_API_KEY and JUNO_COST_LOG_PATH"]
async fn gemini_turn_writes_a_priced_row() {
    scratch_log();
    let key = credential("GEMINI_API_KEY");
    let model = model_ids::GEMINI_3_5_FLASH_LITE;

    let brain = GeminiBrain::new(
        key,
        Some(model.to_string()),
        Some(MAX_TOKENS as i32),
        None,
        None,
    )
    .expect("brain construction only builds an HTTP client");

    let run_id = "live-gemini";
    begin(run_id);
    assert_answered(
        brain
            .decide_next_action(&one_user_turn(), &[])
            .await
            .expect("the request reached the API"),
    );

    assert_one_priced_row(run_id, &Provider::Gemini, model);
}

/// The one provider with no key and no marginal cost: a Claude Max seat.
///
/// It still has to write a row, keyed apart from metered traffic, or the
/// measurement averages a free turn in with paid ones and reports the fleet as
/// cheaper than it is. No GitHub runner can run this — a runner has no
/// signed-in subscription — so it is local-only by nature.
#[tokio::test]
#[ignore = "needs `claude` installed and signed in, plus JUNO_COST_LOG_PATH"]
async fn claude_cli_turn_writes_a_zero_cost_row() {
    scratch_log();
    let model = "haiku";

    let config = juno_lib::settings::ProviderConfig {
        id: "claude-cli".to_string(),
        api_key: None,
        model: Some(model.to_string()),
        max_tokens: None,
        temperature: None,
        system_prompt: None,
        effort: Some("low".to_string()),
        // Juno's own tool server only: the person's claude.ai connectors have
        // nothing to do with what a turn costs, and loading them makes the
        // prompt bigger and the test slower.
        load_account_mcp: false,
    };
    let brain = ClaudeCliBrain::from_config(&config)
        .expect("the claude binary is installed — see the module docs");

    let run_id = "live-claude-cli";
    begin(run_id);
    assert_answered(
        brain
            .decide_next_action(&one_user_turn(), &[])
            .await
            .expect("the CLI answered"),
    );

    let row = assert_one_priced_row(run_id, &Provider::ClaudeCli, model);
    assert_eq!(
        row.model_id, "claude-cli:haiku",
        "a seat's rows must be keyed apart from metered Anthropic traffic"
    );
}
