//! What one turn cost, in tokens and in dollars.
//!
//! Juno has always logged how long a turn took and how many screenshots it
//! needed ([`crate::turn_timing`]), which answers "is it fast enough" but not
//! "what does a task cost to run". Pricing the product needs the second
//! question answered from real traffic, so every model request appends one
//! line here:
//!
//! ```text
//! {"timestamp":"2026-10-16T22:41:08.310Z","run_id":"5f3c…","turn_index":2,
//!  "provider":"anthropic","model_id":"claude-opus-5-5","input_tokens":1204,
//!  "output_tokens":311,"cache_read_tokens":48210,"cache_write_tokens":0,
//!  "screenshots":3,"duration_ms":4120,"outcome":"tool_use","cost_usd":0.0219}
//! ```
//!
//! NDJSON at `~/Library/Caches/Juno/cost-log.ndjson`, one line per model
//! request. **Off unless switched on** (`cost_logging_enabled`, no UI): it is a
//! measurement instrument, not a feature, and it records what the person asked
//! Juno to do in the shape of token counts.
//!
//! # A turn here is one API request, not one spoken turn
//!
//! [`crate::turn_timing`] measures the path to the first sound, so it opens one
//! turn per key release and closes it when audio starts. A single task runs a
//! tool loop of many model requests, and all of them cost money. This module
//! counts every one: a *run* is one task ([`begin_run`]) and `turn_index`
//! counts requests inside it from zero. The two modules deliberately do not
//! share a counter.
//!
//! # Why the app's own cost figure cannot be trusted alone
//!
//! `cost_usd` is computed here from [`Price`] in the model catalog, so it is
//! only as good as that table. `scripts/pricing/analyze-cost-log.py`
//! recomputes every line from its own independently sourced rates and flags
//! disagreement above 1%; that cross-check is the only thing that can catch a
//! wrong rate in Juno, because a number the app reports about itself cannot
//! validate the app's own price table. Emit the field names above verbatim —
//! that script is the consumer.
//!
//! # Three ways a turn can cost nothing, and only one of them is free
//!
//! A `Subscription` model (the Claude CLI and Codex CLI paths) genuinely costs
//! nothing at the margin — the person already pays for the seat, and that is
//! what makes a bring-your-own-plan tier viable. An `Unpublished` model has a
//! price nobody has looked up. A model missing from the catalog has no price
//! at all. All three write `0.0`, so the two that are not free are kept
//! loud: they log a warning here, and they are left out of the analyzer's
//! price table so it raises rather than scoring them as free.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::agent::providers::types::{Price, Provider, Rates};

/// Log target, so these lines can be filtered on their own with `RUST_LOG`.
pub const TARGET: &str = "juno::cost_log";

/// The file, under the OS cache directory. Read by
/// `scripts/pricing/analyze-cost-log.py`.
pub const LOG_FILE_NAME: &str = "cost-log.ndjson";

/// Switches logging on for a process with no settings store behind it —
/// `juno-cua`, the batch runner, CI. The measurement has to be runnable
/// outside the UI, and those paths have no `AppHandle` to read a setting with.
pub const ENV_FLAG: &str = "JUNO_COST_LOG";

/// Sends the log somewhere other than the cache directory. Exported by the
/// caller before Juno starts — a measurement run that wants one file per
/// batch, or the live test, which must not append to the person's own log.
/// Read, never set: `std::env::set_var` is unsafe in a multithreaded process.
pub const ENV_LOG_PATH: &str = "JUNO_COST_LOG_PATH";

/// The column order of the CSV export, and the field order of each NDJSON
/// record. Fixed: it is a consumed contract, not a display choice.
pub const COLUMNS: [&str; 13] = [
    "timestamp",
    "run_id",
    "turn_index",
    "provider",
    "model_id",
    "input_tokens",
    "output_tokens",
    "cache_read_tokens",
    "cache_write_tokens",
    "screenshots",
    "duration_ms",
    "outcome",
    "cost_usd",
];

/// Tokens billed for one model request, by category.
///
/// Anthropic reports `input_tokens` **excluding** cache reads and cache
/// writes, so the four add up to the prompt without double counting. The other
/// providers report fewer categories; the ones they do not bill for stay zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl TokenUsage {
    /// Take the larger of each field.
    ///
    /// A streamed response reports usage twice: `message_start` carries the
    /// prompt counts with `output_tokens` still near zero, and `message_delta`
    /// carries the final output count — on some paths without repeating the
    /// prompt counts. Summing would double the prompt; replacing wholesale
    /// would drop it. Each category only ever grows within one request, so the
    /// maximum is the whole of it.
    pub fn merge_max(&mut self, other: TokenUsage) {
        self.input = self.input.max(other.input);
        self.output = self.output.max(other.output);
        self.cache_read = self.cache_read.max(other.cache_read);
        self.cache_write = self.cache_write.max(other.cache_write);
    }

    /// Everything that went *into* the model. Decides which long-context tier
    /// applies, and is the figure cache reads dominate on a computer-use turn.
    pub fn prompt_tokens(&self) -> u64 {
        self.input
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }

    /// Whether the provider reported anything at all. A request that reported
    /// no usage is not a free request, so it is never written as one.
    pub fn is_empty(&self) -> bool {
        *self == TokenUsage::default()
    }

    /// Read the four fields out of an Anthropic-shaped `usage` object. Every
    /// field is optional: `message_delta.usage` often carries only
    /// `output_tokens`.
    pub fn from_anthropic_json(usage: &serde_json::Value) -> TokenUsage {
        let field = |key: &str| {
            usage
                .get(key)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
        };
        TokenUsage {
            input: field("input_tokens"),
            output: field("output_tokens"),
            cache_read: field("cache_read_input_tokens"),
            cache_write: field("cache_creation_input_tokens"),
        }
    }
}

/// One model request's cost. Serialized as one NDJSON line.
///
/// `Deserialize` as well as `Serialize`: the CSV export reads the log back,
/// so the record is its own parser and the two can never disagree about a
/// field name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CostRecord {
    /// RFC 3339, UTC, milliseconds.
    pub timestamp: String,
    /// The task these turns belong to. The analyzer sums a run into one task.
    pub run_id: String,
    /// This request's position in the run, from zero.
    pub turn_index: u64,
    pub provider: String,
    /// The **billing** key, not always the wire model name — see
    /// [`billing_model_id`].
    pub model_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    /// Screenshots captured during this turn. The other half of what a
    /// computer-use turn costs: they are why cache reads grow.
    pub screenshots: u64,
    pub duration_ms: u64,
    /// The provider's stop reason (`end_turn`, `tool_use`, `refusal`, …), or
    /// `error`. The analyzer reads the **last** turn of a run to decide whether
    /// the task succeeded.
    pub outcome: String,
    pub cost_usd: f64,
}

/// The key this model is priced under.
///
/// For a metered API this is the model ID as sent. For a subscription path it
/// is prefixed with the provider (`claude-cli:opus`), because the alias alone
/// says nothing about how it is billed and would otherwise be averaged in with
/// metered traffic at $0.00 — making the fleet look free. The prefixed form is
/// also what `scripts/pricing/prices.json` keys those entries by.
pub fn billing_model_id(provider: &Provider, model: &str) -> String {
    match provider {
        Provider::ClaudeCli => format!("claude-cli:{model}"),
        Provider::CodexCli => format!("codex-cli:{model}"),
        Provider::Copilot => format!("copilot:{model}"),
        _ => model.to_string(),
    }
}

/// The rates to apply to a prompt of `prompt_tokens`, picking the
/// long-context tier when the request is past its threshold.
fn tier_for(rates: &Rates, prompt_tokens: u64) -> (f64, f64, f64, f64) {
    match &rates.long_context {
        Some(long) if prompt_tokens > long.threshold_tokens => {
            (long.input, long.output, long.cache_read, long.cache_write)
        }
        _ => (
            rates.input,
            rates.output,
            rates.cache_read,
            rates.cache_write,
        ),
    }
}

/// What `usage` costs at `price`, in USD.
///
/// Returns `None` when the model has no published per-token rate — a
/// subscription seat or an unpriced model. The caller records `0.0` and says
/// why; see the module docs.
pub fn cost_usd(price: &Price, usage: &TokenUsage) -> Option<f64> {
    let rates = match price {
        Price::Metered(rates) => rates,
        Price::Subscription | Price::Unpublished => return None,
    };
    let (input, output, cache_read, cache_write) = tier_for(rates, usage.prompt_tokens());
    Some(
        (usage.input as f64 * input
            + usage.output as f64 * output
            + usage.cache_read as f64 * cache_read
            + usage.cache_write as f64 * cache_write)
            / 1_000_000.0,
    )
}

/// The catalog price for `model` under `provider`, if the catalog has it.
fn catalog_price(provider: &Provider, model: &str) -> Option<&'static Price> {
    provider
        .model_definitions()
        .iter()
        .find(|def| def.id == model)
        .map(|def| &def.price)
}

// ---------------------------------------------------------------------------
// Process state
//
// One open run, a turn counter and a screenshot counter, in the same shape as
// `turn_timing`: a passive global that costs one uncontended lock when nothing
// is being measured.

#[derive(Debug, Default)]
struct Run {
    id: String,
    next_turn: u64,
    /// Screenshots since the last turn was recorded, not since the run began.
    screenshots_this_turn: u64,
}

static RUN: Mutex<Option<Run>> = Mutex::new(None);
static ENABLED: AtomicBool = AtomicBool::new(false);
static ENV_FORCED: OnceLock<bool> = OnceLock::new();
/// Models already warned about, so one unpriced model does not fill the log.
static WARNED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn with_run<T>(f: impl FnOnce(&mut Option<Run>) -> T) -> T {
    // A panic elsewhere while holding this lock leaves plain data behind.
    // Cost telemetry is not worth refusing to run over.
    let mut guard = RUN.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

/// Whether turns are being recorded.
pub fn is_enabled() -> bool {
    *ENV_FORCED.get_or_init(|| {
        std::env::var(ENV_FLAG)
            .map(|v| matches!(v.trim(), "1" | "true" | "yes"))
            .unwrap_or(false)
    }) || ENABLED.load(Ordering::Relaxed)
}

/// Switch recording on or off. Called from the settings store at startup and
/// whenever settings change. The env flag wins while it is set.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// Open a run — one task, however many model requests it takes. Subsequent
/// turns are attributed to `run_id` until the next call.
pub fn begin_run(run_id: impl Into<String>) {
    let id = run_id.into();
    with_run(|run| {
        *run = Some(Run {
            id,
            next_turn: 0,
            screenshots_this_turn: 0,
        });
    });
}

/// Close the open run. Turns recorded after this get their own one-off run id
/// rather than being folded into a task that has finished.
pub fn end_run() {
    with_run(|run| *run = None);
}

/// Count a screenshot against the turn being built. Reset when that turn is
/// recorded, so each turn reports its own.
pub fn note_screenshot() {
    with_run(|run| {
        if let Some(run) = run.as_mut() {
            run.screenshots_this_turn = run.screenshots_this_turn.saturating_add(1);
        }
    });
}

/// Whether this request earns a row.
///
/// Pure, so the two reasons to drop one are stated in one place: logging is
/// off, or the provider reported nothing. A request with no token counts is
/// not a free request, so it is never written as one.
fn should_record(enabled: bool, usage: &TokenUsage) -> bool {
    enabled && !usage.is_empty()
}

/// Build the record for a finished model request and write it.
pub fn record_turn(
    provider: &Provider,
    model: &str,
    usage: TokenUsage,
    duration_ms: u64,
    outcome: &str,
) {
    if !should_record(is_enabled(), &usage) {
        return;
    }
    let record = with_run(|run| build_record_in(run, provider, model, usage, duration_ms, outcome));
    append(&record);
}

/// [`record_turn`]'s record-building half, against a caller-supplied run.
///
/// Taking the run as a parameter rather than reaching for the global is what
/// lets the tests number turns and count screenshots without sharing state
/// with every other test in the binary.
fn build_record_in(
    run: &mut Option<Run>,
    provider: &Provider,
    model: &str,
    usage: TokenUsage,
    duration_ms: u64,
    outcome: &str,
) -> CostRecord {
    let (run_id, turn_index, screenshots) = match run.as_mut() {
        Some(run) => {
            let index = run.next_turn;
            run.next_turn += 1;
            let shots = std::mem::take(&mut run.screenshots_this_turn);
            (run.id.clone(), index, shots)
        }
        // A turn outside any run still costs money. Give it a run of its own
        // rather than dropping it or attributing it to the last task.
        None => (format!("unscoped-{}", uuid::Uuid::new_v4()), 0, 0),
    };

    let price = catalog_price(provider, model);
    let cost = price.and_then(|price| cost_usd(price, &usage));
    if cost.is_none() {
        warn_unpriced(provider, model, price);
    }

    CostRecord {
        timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        run_id,
        turn_index,
        provider: provider.as_str().to_string(),
        model_id: billing_model_id(provider, model),
        input_tokens: usage.input,
        output_tokens: usage.output,
        cache_read_tokens: usage.cache_read,
        cache_write_tokens: usage.cache_write,
        screenshots,
        duration_ms,
        outcome: outcome.to_string(),
        cost_usd: cost.unwrap_or(0.0),
    }
}

/// Say once per model why a turn was priced at zero — unless it is a
/// subscription seat, where zero is the correct and expected answer.
fn warn_unpriced(provider: &Provider, model: &str, price: Option<&Price>) {
    if matches!(price, Some(Price::Subscription)) {
        return;
    }
    let key = format!("{}/{}", provider.as_str(), model);
    let mut warned = WARNED.lock().unwrap_or_else(|e| e.into_inner());
    if warned.iter().any(|seen| seen == &key) {
        return;
    }
    warned.push(key);
    let reason = match price {
        Some(_) => "the provider publishes no rate for it",
        None => "it is not in the model catalog",
    };
    tracing::warn!(
        target: TARGET,
        "[CostLog] {} on {} is logged at $0.00 because {}. That is a missing \
         number, not a free turn — do not size a price on these rows.",
        model,
        provider.as_str(),
        reason
    );
}

/// Where the log lives. `dirs::cache_dir` rather than Tauri's `app_cache_dir`
/// so the headless paths (`juno-cua`, the batch runner) write to the same file
/// without an `AppHandle`. [`ENV_LOG_PATH`] overrides it.
pub fn log_path() -> Option<PathBuf> {
    resolve_log_path(
        std::env::var(ENV_LOG_PATH).ok().as_deref(),
        dirs::cache_dir(),
    )
}

/// The path decision on its own, so it is testable without touching the
/// process environment. A blank override is treated as unset rather than as a
/// request to write to the empty path.
fn resolve_log_path(override_path: Option<&str>, cache_dir: Option<PathBuf>) -> Option<PathBuf> {
    match override_path.map(str::trim) {
        Some(p) if !p.is_empty() => Some(PathBuf::from(p)),
        _ => Some(cache_dir?.join("Juno").join(LOG_FILE_NAME)),
    }
}

fn append(record: &CostRecord) {
    let Some(path) = log_path() else {
        tracing::warn!(target: TARGET, "[CostLog] no cache directory; dropping record");
        return;
    };
    if let Err(e) = append_to(&path, record) {
        // Telemetry must never take a turn down with it.
        tracing::warn!(target: TARGET, "[CostLog] could not write {}: {}", path.display(), e);
    }
}

fn append_to(path: &Path, record: &CostRecord) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(record)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// CSV export

/// One CSV field, quoted only when it has to be.
fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Render records as CSV, in [`COLUMNS`] order, header first.
pub fn to_csv(records: &[CostRecord]) -> String {
    let mut out = COLUMNS.join(",");
    out.push('\n');
    for r in records {
        let row = [
            r.timestamp.clone(),
            r.run_id.clone(),
            r.turn_index.to_string(),
            r.provider.clone(),
            r.model_id.clone(),
            r.input_tokens.to_string(),
            r.output_tokens.to_string(),
            r.cache_read_tokens.to_string(),
            r.cache_write_tokens.to_string(),
            r.screenshots.to_string(),
            r.duration_ms.to_string(),
            r.outcome.clone(),
            // Six decimals: a cheap turn is fractions of a cent, and rounding
            // to four would score the Haiku rows as free.
            format!("{:.6}", r.cost_usd),
        ];
        out.push_str(
            &row.iter()
                .map(|f| csv_field(f))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

/// Read the NDJSON log back. A malformed line is skipped with a warning rather
/// than failing the whole export: a half-written last line must not cost you
/// the measurement.
pub fn read_log() -> Result<Vec<CostRecord>, String> {
    let path = log_path().ok_or_else(|| "No cache directory on this system".to_string())?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    Ok(parse_log(&text))
}

/// Parse NDJSON into records, skipping blank and malformed lines.
pub fn parse_log(text: &str) -> Vec<CostRecord> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .filter_map(|(n, line)| match serde_json::from_str::<CostRecord>(line) {
            Ok(record) => Some(record),
            Err(e) => {
                tracing::warn!(target: TARGET, "[CostLog] skipping line {}: {}", n + 1, e);
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(input: u64, output: u64, cache_read: u64, cache_write: u64) -> TokenUsage {
        TokenUsage {
            input,
            output,
            cache_read,
            cache_write,
        }
    }

    /// Opus 5.5 is Juno's default, so this is the rate that sets the price.
    /// Hand-computed from the live table (2026-10-16): $4 input, $20 output,
    /// $0.20 cache read, $5 cache write per MTok.
    #[test]
    fn opus_5_5_costs_what_the_published_rates_say() {
        let price = catalog_price(
            &Provider::Anthropic,
            crate::agent::providers::types::model_ids::CLAUDE_OPUS_5_5,
        )
        .expect("Opus 5.5 is in the catalog");
        // 10k fresh input, 2k output, 100k cache reads, 5k cache writes.
        //   0.010 * 4.00 = 0.040
        //   0.002 * 20.00 = 0.040
        //   0.100 * 0.20 = 0.020
        //   0.005 * 5.00 = 0.025
        //                = 0.125
        let cost =
            cost_usd(price, &usage(10_000, 2_000, 100_000, 5_000)).expect("Opus 5.5 is metered");
        assert!(
            (cost - 0.125).abs() < 1e-9,
            "expected $0.125, got ${cost:.9}"
        );
    }

    /// The rate a cached model catalog got wrong by 2x, and the one that
    /// dominates an agentic computer-use turn (LAC-4127).
    #[test]
    fn sonnet_5_5_cache_reads_are_ten_cents_per_mtok() {
        let price = catalog_price(
            &Provider::Anthropic,
            crate::agent::providers::types::model_ids::CLAUDE_SONNET_5_5,
        )
        .expect("Sonnet 5.5 is in the catalog");
        // A million cache reads and nothing else: the rate, isolated.
        let cost = cost_usd(price, &usage(0, 0, 1_000_000, 0)).expect("metered");
        assert!(
            (cost - 0.10).abs() < 1e-9,
            "cache reads must be $0.10/MTok, not the $0.20 a cached catalog \
             reported; got ${cost:.9}"
        );
    }

    #[test]
    fn a_subscription_seat_has_no_marginal_cost_and_an_unpriced_model_has_no_cost_at_all() {
        // Both return None, and the caller writes 0.0 — but only one of them
        // is actually free, which is why they are separate variants.
        assert_eq!(cost_usd(&Price::Subscription, &usage(9, 9, 9, 9)), None);
        assert_eq!(cost_usd(&Price::Unpublished, &usage(9, 9, 9, 9)), None);
    }

    #[test]
    fn the_long_context_tier_takes_over_above_its_threshold() {
        let price = Price::per_mtok_tiered(
            1.0, 10.0, 0.1, 1.25,    // short context
            100_000, // threshold
            5.0, 50.0, 0.5, 6.25, // long context
        );
        // 100k prompt tokens exactly: still short context.
        let short = cost_usd(&price, &usage(100_000, 0, 0, 0)).expect("metered");
        assert!((short - 0.1).abs() < 1e-9, "got ${short:.9}");
        // One token more: every rate switches, including output.
        let long = cost_usd(&price, &usage(100_001, 1_000_000, 0, 0)).expect("metered");
        let expected = 100_001.0 * 5.0 / 1_000_000.0 + 50.0;
        assert!((long - expected).abs() < 1e-9, "got ${long:.9}");
    }

    /// LAC-8411: `gpt-5.6-sol` is the OpenAI default and bills the WHOLE request
    /// at the long tier above 272K prompt tokens. Each assert guards one field,
    /// so a mutant that drops the threshold, swaps input and output, or nudges
    /// one rate turns exactly one line red.
    #[test]
    fn gpt_5_6_sol_bills_a_long_prompt_at_the_long_tier() {
        use crate::agent::providers::types::model_ids::OPENAI_SOL_5_6;
        let price =
            catalog_price(&Provider::OpenAI, OPENAI_SOL_5_6).expect("Sol is in the catalog");
        let Price::Metered(rates) = price else {
            panic!("Sol is metered");
        };
        let long = rates
            .long_context
            .as_ref()
            .expect("Sol must declare its long-context tier");
        assert_eq!(long.threshold_tokens, 272_000, "threshold");
        assert!((long.input - 8.0).abs() < 1e-9, "long input");
        assert!((long.output - 30.0).abs() < 1e-9, "long output");
        assert!((long.cache_read - 0.8).abs() < 1e-9, "long cache read");
        assert!((long.cache_write - 10.0).abs() < 1e-9, "long cache write");
        assert!((rates.cache_write - 5.0).abs() < 1e-9, "short cache write");

        // Behaviour, not just declaration: 1M output tokens isolates the output
        // rate on each side of the line.
        let short = cost_usd(price, &usage(272_000, 1_000_000, 0, 0)).expect("metered");
        assert!(
            (short - (272_000.0 * 4.0 / 1e6 + 20.0)).abs() < 1e-9,
            "short {short}"
        );
        let over = cost_usd(price, &usage(272_001, 1_000_000, 0, 0)).expect("metered");
        assert!(
            (over - (272_001.0 * 8.0 / 1e6 + 30.0)).abs() < 1e-9,
            "long {over}"
        );
    }

    /// `gpt-5.3-codex` was checked in the same pass: OpenAI publishes no
    /// long-context tier for it, so declaring one would overbill.
    #[test]
    fn gpt_5_3_codex_has_no_long_context_tier() {
        use crate::agent::providers::types::model_ids::OPENAI_CODEX_5_3;
        let Some(Price::Metered(rates)) = catalog_price(&Provider::OpenAI, OPENAI_CODEX_5_3) else {
            panic!("Codex 5.3 is metered");
        };
        assert!(rates.long_context.is_none());
    }

    /// Both tables must carry the same tier, not just the same short rates.
    #[test]
    fn the_analyzer_table_carries_sols_long_tier_too() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/pricing/prices.json");
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read prices.json"))
                .expect("prices.json is JSON");
        let e = &json["models"]["gpt-5.6-sol"];
        assert_eq!(
            e["long_context_threshold"].as_u64(),
            Some(272_000),
            "threshold"
        );
        for (k, v) in [
            ("long_input", 8.0),
            ("long_output", 30.0),
            ("long_cache_read", 0.8),
            ("long_cache_write", 10.0),
            ("cache_write", 5.0),
        ] {
            assert_eq!(e[k].as_f64(), Some(v), "{k}");
        }
    }

    #[test]
    fn prompt_tokens_decide_the_tier_including_cached_ones() {
        // Cache reads are most of an agentic prompt. A tier chosen on fresh
        // input alone would stay on the cheap rates forever.
        assert_eq!(usage(10, 999, 50, 40).prompt_tokens(), 100);
    }

    #[test]
    fn streamed_usage_arrives_twice_and_must_not_double_the_prompt() {
        // message_start: the prompt, output not yet produced.
        let mut acc = TokenUsage::from_anthropic_json(&serde_json::json!({
            "input_tokens": 1200,
            "output_tokens": 1,
            "cache_read_input_tokens": 48000,
            "cache_creation_input_tokens": 2000
        }));
        // message_delta: the final output count, prompt fields absent.
        acc.merge_max(TokenUsage::from_anthropic_json(&serde_json::json!({
            "output_tokens": 311
        })));
        assert_eq!(acc, usage(1200, 311, 48000, 2000));
    }

    #[test]
    fn a_usage_block_missing_every_field_reads_as_zero_not_an_error() {
        let parsed = TokenUsage::from_anthropic_json(&serde_json::json!({}));
        assert!(parsed.is_empty());
    }

    #[test]
    fn subscription_paths_are_keyed_apart_from_metered_ones() {
        // `opus` alone would be averaged in with metered Anthropic traffic at
        // $0.00 and make the whole fleet look free.
        assert_eq!(
            billing_model_id(&Provider::ClaudeCli, "opus"),
            "claude-cli:opus"
        );
        assert_eq!(
            billing_model_id(&Provider::CodexCli, "gpt-6-luna"),
            "codex-cli:gpt-6-luna"
        );
        assert_eq!(
            billing_model_id(&Provider::Copilot, "claude-opus-5.5"),
            "copilot:claude-opus-5.5"
        );
        assert_eq!(
            billing_model_id(&Provider::Anthropic, "claude-opus-5-5"),
            "claude-opus-5-5"
        );
    }

    /// A record built against a caller-owned run, so tests never share the
    /// process-wide one with each other.
    fn record_in(run: &mut Option<Run>, duration_ms: u64, outcome: &str) -> CostRecord {
        build_record_in(
            run,
            &Provider::Anthropic,
            "claude-opus-5-5",
            usage(1, 1, 0, 0),
            duration_ms,
            outcome,
        )
    }

    fn open_run(id: &str) -> Option<Run> {
        Some(Run {
            id: id.to_string(),
            next_turn: 0,
            screenshots_this_turn: 0,
        })
    }

    #[test]
    fn a_run_numbers_its_turns_and_each_turn_reports_its_own_screenshots() {
        let mut run = open_run("run-a");
        if let Some(open) = run.as_mut() {
            open.screenshots_this_turn = 2;
        }
        let first = record_in(&mut run, 10, "tool_use");
        if let Some(open) = run.as_mut() {
            open.screenshots_this_turn = 1;
        }
        let second = record_in(&mut run, 20, "end_turn");

        assert_eq!((first.turn_index, first.screenshots), (0, 2));
        // Not cumulative: the second turn took one screenshot, not three.
        assert_eq!((second.turn_index, second.screenshots), (1, 1));
        assert_eq!(first.run_id, "run-a");
        assert_eq!(second.run_id, "run-a");
    }

    #[test]
    fn the_global_run_counts_screenshots_against_the_turn_being_built() {
        // The wiring the providers actually call, exercised once. It shares
        // process state, so it asserts only what it set itself.
        begin_run("global-run");
        note_screenshot();
        note_screenshot();
        let counted = with_run(|run| {
            run.as_mut()
                .map(|r| std::mem::take(&mut r.screenshots_this_turn))
        });
        end_run();
        assert_eq!(counted, Some(2));
        // Closed: a later screenshot has nowhere to land, and does not panic.
        note_screenshot();
        assert!(with_run(|run| run.is_none()));
    }

    #[test]
    fn a_turn_outside_any_run_still_gets_a_run_of_its_own() {
        let mut none = None;
        let record = record_in(&mut none, 5, "end_turn");
        assert!(record.run_id.starts_with("unscoped-"), "{}", record.run_id);
        assert_eq!(record.turn_index, 0);
    }

    #[test]
    fn nothing_is_recorded_while_logging_is_off_or_usage_is_empty() {
        let real = usage(1, 1, 0, 0);
        // Off by default: a measurement instrument that records what the
        // person asked Juno to do does not run unasked.
        assert!(!should_record(false, &real));
        // On, but the provider reported nothing: not a free turn, no row.
        assert!(!should_record(true, &TokenUsage::default()));
        assert!(should_record(true, &real));
    }

    #[test]
    fn csv_carries_the_agreed_columns_in_the_agreed_order() {
        let record = build_record_in(
            &mut open_run("csv"),
            &Provider::Anthropic,
            "claude-opus-5-5",
            usage(100, 200, 300, 400),
            1234,
            "end_turn",
        );
        let csv = to_csv(&[record]);
        let mut lines = csv.lines();
        assert_eq!(lines.next().unwrap_or_default(), COLUMNS.join(","));
        let row: Vec<&str> = lines.next().unwrap_or_default().split(',').collect();
        assert_eq!(row.len(), COLUMNS.len());
        assert_eq!(row[3], "anthropic");
        assert_eq!(row[4], "claude-opus-5-5");
        assert_eq!(row[5..9], ["100", "200", "300", "400"]);
        assert_eq!(row[10], "1234");
        assert_eq!(row[11], "end_turn");
    }

    #[test]
    fn a_comma_in_a_field_does_not_shift_every_column_after_it() {
        let mut record = build_record_in(
            &mut open_run("csv-quoting"),
            &Provider::Anthropic,
            "claude-opus-5-5",
            usage(1, 1, 0, 0),
            1,
            "weird,outcome",
        );
        record.run_id = "a\"quoted\" id".to_string();
        let csv = to_csv(&[record]);
        assert!(csv.contains("\"weird,outcome\""), "{csv}");
        assert!(csv.contains("\"a\"\"quoted\"\" id\""), "{csv}");
    }

    #[test]
    fn a_half_written_last_line_does_not_cost_you_the_rest_of_the_log() {
        let good = r#"{"timestamp":"2026-10-16T00:00:00.000Z","run_id":"r","turn_index":0,"provider":"anthropic","model_id":"claude-opus-5-5","input_tokens":1,"output_tokens":2,"cache_read_tokens":3,"cache_write_tokens":4,"screenshots":5,"duration_ms":6,"outcome":"end_turn","cost_usd":0.5}"#;
        let text = format!("{good}\n\n{{\"timestamp\":\"trunc");
        let records = parse_log(&text);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].output_tokens, 2);
    }

    /// The record round-trips through the exact field names
    /// `scripts/pricing/analyze-cost-log.py` reads. Renaming one silently
    /// breaks the measurement, so the names are asserted, not assumed.
    #[test]
    fn the_json_field_names_are_the_ones_the_analyzer_reads() {
        let record = build_record_in(
            &mut open_run("schema"),
            &Provider::ClaudeCli,
            "opus",
            usage(1, 2, 3, 4),
            7,
            "end_turn",
        );
        let value = serde_json::to_value(&record).expect("serializes");
        let object = value.as_object().expect("an object");
        for column in COLUMNS {
            assert!(object.contains_key(column), "missing field {column}");
        }
        assert_eq!(object.len(), COLUMNS.len(), "unexpected extra fields");
        // A subscription seat prices at zero, and says so under its own key.
        assert_eq!(
            object.get("model_id").and_then(|v| v.as_str()),
            Some("claude-cli:opus")
        );
        assert_eq!(object.get("cost_usd").and_then(|v| v.as_f64()), Some(0.0));
    }

    fn all_providers() -> [Provider; 7] {
        [
            Provider::Anthropic,
            Provider::OpenAI,
            Provider::Rig,
            Provider::Gemini,
            Provider::ClaudeCli,
            Provider::CodexCli,
            Provider::Copilot,
        ]
    }

    /// Juno's price table and the analyzer's must agree, or the cross-check
    /// that is supposed to catch a wrong rate just reports a difference
    /// between two tables nobody has reconciled.
    ///
    /// This is the regression test for the whole ticket: the analyzer prices
    /// every turn independently and flags >1% disagreement, which only means
    /// something if both sides are keyed the same way and sourced from the
    /// same live docs.
    ///
    /// Long-context pricing has exactly two acceptable shapes, and both are
    /// asserted. Juno carries a tier, in which case the threshold and all
    /// four rates must match; or Juno carries none, in which case prices.json
    /// must document none either — no rates and **no threshold**. A model
    /// with no tier in Juno is billed at the short rate however long the
    /// prompt gets, so either half of a tier present only in prices.json is a
    /// silent underprice on Juno's side, and one the cross-check could never
    /// report, because the two tables would still agree with each other
    /// (LAC-8405).
    #[test]
    fn the_rust_price_table_agrees_with_the_analyzer_price_table() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/pricing/prices.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let json: serde_json::Value = serde_json::from_str(&text).expect("prices.json is JSON");
        let models = json
            .get("models")
            .and_then(|m| m.as_object())
            .expect("prices.json has a models object");

        let rate = |entry: &serde_json::Value, key: &str| -> f64 {
            entry.get(key).and_then(|v| v.as_f64()).unwrap_or(f64::NAN)
        };

        for provider in all_providers() {
            for def in provider.model_definitions() {
                let key = billing_model_id(&provider, def.id);
                match &def.price {
                    Price::Metered(rates) => {
                        let entry = models.get(&key).unwrap_or_else(|| {
                            panic!(
                                "{key} is priced in Juno but missing from prices.json; the \
                                 analyzer will raise on every row for it"
                            )
                        });
                        assert_eq!(
                            entry.get("billing").and_then(|v| v.as_str()),
                            Some("metered"),
                            "{key} is metered in Juno but not in prices.json"
                        );
                        for (name, ours) in [
                            ("input", rates.input),
                            ("output", rates.output),
                            ("cache_read", rates.cache_read),
                            ("cache_write", rates.cache_write),
                        ] {
                            let theirs = rate(entry, name);
                            assert!(
                                (ours - theirs).abs() < 1e-9,
                                "{key} {name}: Juno says {ours}, prices.json says {theirs}. \
                                 Re-read the LIVE pricing page and fix whichever is wrong."
                            );
                        }
                        // The long-context tier is a second, independent set of
                        // rates, and comparing only the four above is how a
                        // swapped `long_input`/`long_output` pair passes
                        // (LAC-8403). Every turn under the threshold prices
                        // identically on both sides, so the tables agree right
                        // up to the first long prompt in production.
                        match &rates.long_context {
                            Some(long) => {
                                let threshold = long.threshold_tokens;
                                assert_eq!(
                                    entry.get("long_context_threshold").and_then(|v| v.as_u64()),
                                    Some(threshold),
                                    "{key}: Juno switches to long-context rates above \
                                     {threshold} prompt tokens and prices.json does not \
                                     agree, so the same turn is priced from two different \
                                     tiers."
                                );
                                for (name, ours) in [
                                    ("long_input", long.input),
                                    ("long_output", long.output),
                                    ("long_cache_read", long.cache_read),
                                    ("long_cache_write", long.cache_write),
                                ] {
                                    let theirs = rate(entry, name);
                                    assert!(
                                        (ours - theirs).abs() < 1e-9,
                                        "{key} {name}: Juno says {ours}, prices.json says \
                                         {theirs}. Re-read the LIVE pricing page and fix \
                                         whichever is wrong."
                                    );
                                }
                            }
                            // Juno has no tier here, so every prompt for
                            // this model is billed at the short rate whatever
                            // its length. That is only safe while the provider
                            // documents no tier either, so any trace of one in
                            // prices.json is refused — the bare threshold
                            // included.
                            //
                            // A bare threshold used to be allowed, on the
                            // reading that the analyzer's own `DataError`
                            // covered the case. It does not: it covers the
                            // analyzer, while Juno prices the same turn at the
                            // short rate and the two tables never disagree
                            // (LAC-8405). This arm only ever sees models Juno
                            // can select, because the loop is over Juno's
                            // catalog, so a hit is always a live underprice.
                            None => {
                                assert!(
                                    entry.get("long_context_threshold").is_none(),
                                    "{key}: prices.json documents a long-context threshold \
                                     and Juno declares no tier, so Juno bills every prompt \
                                     past it at the short rate. Read the live pricing page \
                                     and declare the tier with `Price::per_mtok_tiered`. A \
                                     provider that publishes the threshold but not the rates \
                                     leaves neither table able to price a long prompt, which \
                                     is a decision to take, not a default to accept."
                                );
                                for name in [
                                    "long_input",
                                    "long_output",
                                    "long_cache_read",
                                    "long_cache_write",
                                ] {
                                    assert!(
                                        entry.get(name).is_none(),
                                        "{key}: prices.json publishes {name} but Juno has no \
                                         long-context tier for it, so Juno underprices every \
                                         prompt past the threshold. Declare the tier with \
                                         `Price::per_mtok_tiered`."
                                    );
                                }
                            }
                        }
                    }
                    Price::Subscription => {
                        let entry = models
                            .get(&key)
                            .unwrap_or_else(|| panic!("{key} is missing from prices.json"));
                        assert_eq!(
                            entry.get("billing").and_then(|v| v.as_str()),
                            Some("subscription"),
                            "{key} is a seat in Juno but not in prices.json"
                        );
                    }
                    // Deliberately absent, so the analyzer raises instead of
                    // quietly pricing an unknown model at zero.
                    Price::Unpublished => assert!(
                        !models.contains_key(&key),
                        "{key} has no published rate in Juno but prices.json has one; \
                         if a rate was published, price it on both sides"
                    ),
                }
            }
        }
    }

    /// The other end of the arm above. `prices.json` carries four OpenAI keys
    /// Juno has never had in its catalog (`gpt-5`, `gpt-5.1`, `gpt-5.2`,
    /// `gpt-5.5`), left over from reading OpenAI's pricing page whole when
    /// cost telemetry shipped — Juno's OpenAI rows were already
    /// `gpt-5.6-sol` and `gpt-5.3-codex` by then (LAC-4127, #826). Nothing
    /// reaches them: the parity test loops over Juno's catalog, and the
    /// analyzer only looks a key up when a cost-log row names it, which Juno
    /// has never been able to write.
    ///
    /// `gpt-5.5` is the one that matters, as the only key with a
    /// `long_context_threshold` and no rates. That shape was read once as a
    /// live model Juno underprices above 272K prompt tokens (LAC-8405), when
    /// in fact Juno cannot select it and never prices a turn for it. The same
    /// shape on a model Juno *can* select is a real underprice; this test
    /// fails the day one of these keys becomes a catalog model without a
    /// tier, which is the only way the misreading could come true.
    #[test]
    fn a_rateless_long_context_threshold_sits_on_a_model_juno_cannot_select() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/pricing/prices.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let json: serde_json::Value = serde_json::from_str(&text).expect("prices.json is JSON");
        let models = json
            .get("models")
            .and_then(|m| m.as_object())
            .expect("prices.json has a models object");

        let mut catalog = std::collections::HashSet::new();
        for provider in all_providers() {
            for def in provider.model_definitions() {
                catalog.insert(billing_model_id(&provider, def.id));
            }
        }

        // Positive control. Without it this test passes on an empty table, and
        // dropping a reference key is then silent rather than deliberate.
        assert!(
            models.contains_key("gpt-5.5"),
            "gpt-5.5 is the entry this test exists for; if it was removed on \
             purpose, retire this test with it"
        );

        for (key, entry) in models {
            let rateless_tier =
                entry.get("long_context_threshold").is_some() && entry.get("long_input").is_none();
            if rateless_tier {
                assert!(
                    !catalog.contains(key),
                    "{key} is in Juno's catalog and prices.json gives it a long-context \
                     threshold with no rates, so Juno bills every prompt past it at the \
                     short rate and the analyzer raises on the same row. Look the rates \
                     up and declare them on both sides."
                );
            }
        }
    }

    /// Every model Juno can select must be priced, or the first measurement
    /// run silently scores it as free.
    #[test]
    fn every_catalog_model_declares_a_price() {
        for provider in all_providers() {
            for def in provider.model_definitions() {
                // `Unpublished` is a declaration too: it says someone looked
                // and the provider publishes nothing.
                if let Price::Metered(rates) = &def.price {
                    assert!(
                        rates.input > 0.0 && rates.output > 0.0,
                        "{} has a metered price with a zero rate",
                        def.id
                    );
                }
            }
        }
    }

    // --- Where the log is written ---

    /// The measurement harness points this at one file per batch, and the
    /// live wire test points it at a scratch file so it never appends to the
    /// person's own log.
    #[test]
    fn an_override_path_wins_over_the_cache_directory() {
        let resolved = resolve_log_path(
            Some("/tmp/batch-7.ndjson"),
            Some(PathBuf::from("/Users/someone/Library/Caches")),
        );
        assert_eq!(resolved, Some(PathBuf::from("/tmp/batch-7.ndjson")));
    }

    /// An exported-but-empty variable is how a shell passes "unset", and
    /// writing to the empty path would lose the whole measurement.
    #[test]
    fn a_blank_override_falls_back_rather_than_writing_to_nowhere() {
        for blank in ["", "   "] {
            let resolved =
                resolve_log_path(Some(blank), Some(PathBuf::from("/Users/someone/Caches")));
            assert_eq!(
                resolved,
                Some(PathBuf::from("/Users/someone/Caches/Juno/cost-log.ndjson")),
                "{blank:?} should read as unset"
            );
        }
    }

    #[test]
    fn with_no_override_the_log_sits_under_the_cache_directory() {
        let resolved = resolve_log_path(None, Some(PathBuf::from("/Users/someone/Caches")));
        assert_eq!(
            resolved,
            Some(PathBuf::from("/Users/someone/Caches/Juno/cost-log.ndjson"))
        );
    }

    /// No cache directory and no override means there is nowhere to write;
    /// `append` warns and drops the record rather than failing the turn.
    #[test]
    fn no_cache_directory_and_no_override_resolves_to_nothing() {
        assert_eq!(resolve_log_path(None, None), None);
    }
}
