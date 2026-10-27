# Pricing measurement harness (LAC-4124)

Sizes two numbers: the free trial credit and the Juno Pro monthly allowance.
This directory holds the half of that work that does not need the app to be
instrumented yet — the price table and the analysis. The other half, the
per-turn cost telemetry that produces the input, is LAC-4127.

## Files

| File | What it is |
|---|---|
| `prices.json` | USD per MTok per model, verified against live provider docs 2026-10-16 |
| `analyze-cost-log.py` | Cost log → the $/task table, the sizing answers, and a cross-check |
| `fixtures/` | Invented data for the self-test. Never quote it as a measurement. |

## Running it

```sh
python3 scripts/pricing/analyze-cost-log.py --self-test
python3 scripts/pricing/analyze-cost-log.py ~/Library/Caches/Juno/cost-log.ndjson
```

Takes NDJSON or CSV carrying the LAC-4127 per-turn record:

```
timestamp, run_id, turn_index, provider, model_id, input_tokens, output_tokens,
cache_read_tokens, cache_write_tokens, screenshots, duration_ms, outcome, cost_usd
```

## Producing the log

Recording is off by default. Switch it on for a measurement run in one of
three ways, in order of convenience:

| Where | How |
|---|---|
| In the app | Settings → Advanced → cost logging, or `set_cost_logging_enabled` |
| Headless (`juno-cua`, batch runner, CI) | export `JUNO_COST_LOG=1` |
| A run of its own | also export `JUNO_COST_LOG_PATH=/path/to/batch-7.ndjson` |

`JUNO_COST_LOG_PATH` moves the log off the default
`~/Library/Caches/Juno/cost-log.ndjson`, which is what you want when a batch
should not mix with earlier turns. It is read, never written, by Juno: export
it before launching.

## Is the telemetry still reading the right field?

Every rate here is cross-checked against Juno's own table by a Rust test, and
the arithmetic is covered by the hermetic suite in `src-tauri/src/cost_log.rs`.
Neither of those can tell you that a provider still reports `usage` where Juno
looks for it — a moved field would log every turn at $0.00 and pass all of
them.

That is what `src-tauri/tests/cost_telemetry_live.rs` is for: one real turn per
provider, then the row read back out of the log. It is `#[ignore]`d (it spends
money) and runs by hand, or in CI for Anthropic via the
**Cost telemetry live check** workflow. Run it before any measurement round
you intend to set a price from.

## Decisions baked in

A task is one `run_id`; its turns are summed. A task succeeded if its last
turn's outcome is `success`. **Allowance math counts failed tasks too** — a
failure still spends the person's credit, so sizing on successes alone would
oversell the trial.

Monthly task volume is not in the log, so the power-user table is a
parameterised grid, not a measurement. Pick the column that matches real
usage once there is any.

## The cross-check is the point

The script recomputes every turn's cost from its tokens and compares that
with the `cost_usd` Juno wrote, flagging anything more than 1% apart. A
number the app reports about itself cannot validate the app's own price
table; this can. Treat a finding as a defect in Juno, not in the script,
until the price table is shown correct.

Unknown models and unpublished long-context rates raise rather than
defaulting to zero. A model missing from `prices.json` scoring as free would
be the quietest possible way to undersize a price.

## Two things that move the answer

**Cache reads dominate.** Agentic computer use replays the system prompt and
screenshot history every turn, so cache-read tokens outweigh fresh input by
roughly an order of magnitude. On Claude Sonnet 5.5 a cache read is $0.10/MTok
against $2.00/MTok base input — the cache-read rate, not the headline input
rate, sets the bill. This rate is also where the cached model catalog was
wrong (it said $0.20); `prices.json` follows the live docs.

**The Claude CLI path has no marginal cost.** It bills against a Claude
Max/Pro seat the person already pays for, so it is reported separately rather
than as $0.00 next to metered models. This is the thing that makes the
bring-your-own-plan free tier work, and it is a pricing finding in its own
right.

## Refreshing prices

Re-fetch the two `_sources` URLs in `prices.json` and update `_verified`.
Do not take rates from a model catalog bundled with a tool — that is how a
2x error on the Sonnet cache-read rate got close to shipping (LAC-3106).
