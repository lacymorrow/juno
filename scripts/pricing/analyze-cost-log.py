#!/usr/bin/env python3
"""Turn a Juno per-turn cost log into the LAC-4124 pricing table.

Consumes the per-turn record defined by LAC-4127:

    timestamp, run_id, turn_index, provider, model_id, input_tokens,
    output_tokens, cache_read_tokens, cache_write_tokens, screenshots,
    duration_ms, outcome, cost_usd

as NDJSON (one object per line) or CSV with that header. Emits the
median/p90 $-per-task table, the trial- and allowance-sizing answers, and a
QA cross-check of the cost the app computed against a price table verified
against live provider docs.

Measurement decisions, stated here because they change the answer:

  * A task is one run_id. Turns inside it are summed.
  * A task succeeded if its final turn's outcome is "success".
  * Allowance math counts EVERY task, succeeded or not. A failed task still
    spends the person's credit, so sizing a trial on successes only would
    oversell it.
  * Cost is recomputed from tokens here, independently of the cost_usd the
    app wrote, and the two are compared. That is the point: it checks the
    app's price table, which no self-reported number can do.
  * Subscription-billed models (the Claude CLI path) have no marginal token
    cost and are reported apart from metered models, never as $0.00 in the
    same column.

Usage:
    analyze-cost-log.py <cost-log.ndjson|.csv> [--prices prices.json]
                        [--tasks-per-month 10,30,60,100,200] [--json]
    analyze-cost-log.py --self-test
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import statistics
import sys
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_PRICES = os.path.join(HERE, "prices.json")

TRIAL_CREDIT_USD = 2.00  # free one-time trial credit (approved structure)
PRO_ALLOWANCE_USD = 6.00  # model cost inside the $12/mo Pro price (~50% margin)

NUMERIC_FIELDS = (
    "turn_index",
    "input_tokens",
    "output_tokens",
    "cache_read_tokens",
    "cache_write_tokens",
    "screenshots",
    "duration_ms",
)


class DataError(Exception):
    """The log or the price table cannot answer the question asked of it."""


# --------------------------------------------------------------------------
# loading


def load_prices(path):
    with open(path) as fh:
        return json.load(fh)["models"]


def load_records(path):
    """Read NDJSON or CSV into a list of normalised dicts."""
    with open(path) as fh:
        text = fh.read()

    rows = []
    stripped = text.lstrip()
    if stripped.startswith("{"):
        for lineno, line in enumerate(text.splitlines(), 1):
            line = line.strip()
            if not line:
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError as exc:
                raise DataError(f"{path}:{lineno}: bad JSON: {exc}") from exc
    else:
        rows = list(csv.DictReader(text.splitlines()))

    if not rows:
        raise DataError(f"{path}: no records")

    for lineno, row in enumerate(rows, 1):
        for field in NUMERIC_FIELDS:
            row[field] = int(float(row.get(field) or 0))
        row["cost_usd"] = float(row.get("cost_usd") or 0.0)
        if not row.get("model_id"):
            raise DataError(f"{path}:{lineno}: record has no model_id")
        if not row.get("run_id"):
            raise DataError(f"{path}:{lineno}: record has no run_id")
    return rows


# --------------------------------------------------------------------------
# cost


def turn_cost(row, prices):
    """Recompute one turn's cost from its tokens. Independent of cost_usd."""
    model = row["model_id"]
    entry = prices.get(model)
    if entry is None:
        raise DataError(
            f"model {model!r} is not in the price table; add it (with a live "
            f"doc source) rather than letting it score as free"
        )
    if entry.get("billing") == "subscription":
        return 0.0

    prompt_tokens = (
        row["input_tokens"] + row["cache_read_tokens"] + row["cache_write_tokens"]
    )
    threshold = entry.get("long_context_threshold")
    prefix = ""
    if threshold is not None and prompt_tokens > threshold:
        if "long_input" not in entry:
            raise DataError(
                f"{model}: a turn used {prompt_tokens} prompt tokens, past its "
                f"{threshold} long-context threshold, but no long_* rates are "
                f"published in the price table. Look them up before trusting "
                f"any number derived from this row."
            )
        prefix = "long_"

    return (
        row["input_tokens"] * entry[prefix + "input"]
        + row["output_tokens"] * entry[prefix + "output"]
        + row["cache_read_tokens"] * entry[prefix + "cache_read"]
        + row["cache_write_tokens"] * entry[prefix + "cache_write"]
    ) / 1_000_000


def percentile(values, pct):
    """Linear-interpolated percentile. Small samples are the caller's problem."""
    if not values:
        return 0.0
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    rank = (len(ordered) - 1) * (pct / 100)
    low = int(rank)
    high = min(low + 1, len(ordered) - 1)
    return ordered[low] + (ordered[high] - ordered[low]) * (rank - low)


# --------------------------------------------------------------------------
# aggregation


def build_tasks(rows, prices):
    """Collapse turns into one record per run_id."""
    grouped = defaultdict(list)
    for row in rows:
        grouped[row["run_id"]].append(row)

    tasks = []
    for run_id, turns in grouped.items():
        turns.sort(key=lambda r: r["turn_index"])
        models = {t["model_id"] for t in turns}
        if len(models) > 1:
            raise DataError(
                f"run {run_id} mixes models {sorted(models)}; a task must run on "
                f"one model for its cost to be attributable"
            )
        tasks.append(
            {
                "run_id": run_id,
                "model_id": turns[0]["model_id"],
                "provider": turns[0].get("provider", ""),
                "turns": len(turns),
                "input_tokens": sum(t["input_tokens"] for t in turns),
                "output_tokens": sum(t["output_tokens"] for t in turns),
                "cache_read_tokens": sum(t["cache_read_tokens"] for t in turns),
                "cache_write_tokens": sum(t["cache_write_tokens"] for t in turns),
                "screenshots": sum(t["screenshots"] for t in turns),
                "duration_ms": sum(t["duration_ms"] for t in turns),
                "succeeded": turns[-1].get("outcome") == "success",
                "cost_usd": sum(turn_cost(t, prices) for t in turns),
                "logged_cost_usd": sum(t["cost_usd"] for t in turns),
            }
        )
    return sorted(tasks, key=lambda t: (t["model_id"], t["run_id"]))


def summarise(tasks, prices):
    by_model = defaultdict(list)
    for task in tasks:
        by_model[task["model_id"]].append(task)

    out = []
    for model, group in sorted(by_model.items()):
        costs = [t["cost_usd"] for t in group]
        entry = prices.get(model, {})
        out.append(
            {
                "model_id": model,
                "billing": entry.get("billing", "metered"),
                "tasks": len(group),
                "success_rate": sum(t["succeeded"] for t in group) / len(group),
                "median_usd": statistics.median(costs),
                "p90_usd": percentile(costs, 90),
                "mean_usd": statistics.fmean(costs),
                "median_turns": statistics.median([t["turns"] for t in group]),
                "median_screenshots": statistics.median(
                    [t["screenshots"] for t in group]
                ),
                "median_duration_s": statistics.median(
                    [t["duration_ms"] / 1000 for t in group]
                ),
            }
        )
    return out


def crosscheck(tasks, tolerance=0.01):
    """Compare our recomputed cost against the cost the app logged."""
    findings = []
    for task in tasks:
        ours, theirs = task["cost_usd"], task["logged_cost_usd"]
        if ours == 0 and theirs == 0:
            continue
        if theirs == 0:
            findings.append((task["run_id"], task["model_id"], ours, theirs,
                             "app logged no cost"))
            continue
        drift = abs(ours - theirs) / max(theirs, 1e-12)
        if drift > tolerance:
            findings.append((task["run_id"], task["model_id"], ours, theirs,
                             f"{drift:.1%} apart"))
    return findings


# --------------------------------------------------------------------------
# report


def render(summaries, tasks, findings, volumes):
    metered = [s for s in summaries if s["billing"] == "metered"]
    sub = [s for s in summaries if s["billing"] == "subscription"]
    lines = []
    w = lines.append

    w("# Juno model cost per task (LAC-4124)")
    w("")
    w(f"{len(tasks)} tasks over {sum(s['tasks'] for s in summaries)} runs.")
    w("")

    thin = [s for s in summaries if s["tasks"] < 5]
    if thin:
        w("> Thin samples, treat the percentiles as directional: "
          + ", ".join(f"{s['model_id']} (n={s['tasks']})" for s in thin))
        w("")

    if metered:
        w("## Metered models")
        w("")
        w("| Model | Tasks | Success | Median $ | p90 $ | Mean $ | Median turns | Median shots | Median s |")
        w("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
        for s in metered:
            w(
                f"| `{s['model_id']}` | {s['tasks']} | {s['success_rate']:.0%} | "
                f"${s['median_usd']:.4f} | ${s['p90_usd']:.4f} | ${s['mean_usd']:.4f} | "
                f"{s['median_turns']:.0f} | {s['median_screenshots']:.0f} | "
                f"{s['median_duration_s']:.0f} |"
            )
        w("")

    if sub:
        w("## Subscription models (no marginal token cost)")
        w("")
        w("| Model | Tasks | Success | Median turns | Median shots | Median s |")
        w("|---|---:|---:|---:|---:|---:|")
        for s in sub:
            w(
                f"| `{s['model_id']}` | {s['tasks']} | {s['success_rate']:.0%} | "
                f"{s['median_turns']:.0f} | {s['median_screenshots']:.0f} | "
                f"{s['median_duration_s']:.0f} |"
            )
        w("")
        w("These run on a Claude Max/Pro seat the person already pays for. "
          "They cost Juno nothing per task, which is what makes the "
          "bring-your-own-plan free tier viable.")
        w("")

    if metered:
        w("## What the money buys")
        w("")
        w(f"A ${TRIAL_CREDIT_USD:.0f} trial credit and a ${PRO_ALLOWANCE_USD:.0f} "
          "monthly allowance, in tasks. Every task counts, including the ones "
          "that fail.")
        w("")
        w("| Model | Trial tasks (median) | Trial tasks (p90) | Pro tasks/mo (median) | Pro tasks/mo (p90) |")
        w("|---|---:|---:|---:|---:|")
        for s in metered:
            def n(budget, cost):
                return f"{int(budget / cost)}" if cost > 0 else "n/a"
            w(
                f"| `{s['model_id']}` | {n(TRIAL_CREDIT_USD, s['median_usd'])} | "
                f"{n(TRIAL_CREDIT_USD, s['p90_usd'])} | "
                f"{n(PRO_ALLOWANCE_USD, s['median_usd'])} | "
                f"{n(PRO_ALLOWANCE_USD, s['p90_usd'])} |"
            )
        w("")
        w("## What a power user costs per month")
        w("")
        w("Task volume is not in this log, so it is a parameter, not a "
          "measurement. Each cell is p90 task cost times that volume; the "
          f"${PRO_ALLOWANCE_USD:.0f} allowance is the break-even line.")
        w("")
        w("| Model | " + " | ".join(f"{v} tasks/mo" for v in volumes) + " |")
        w("|---" * (len(volumes) + 1) + "|")
        for s in metered:
            cells = []
            for v in volumes:
                cost = s["p90_usd"] * v
                over = " **over**" if cost > PRO_ALLOWANCE_USD else ""
                cells.append(f"${cost:.2f}{over}")
            w(f"| `{s['model_id']}` | " + " | ".join(cells) + " |")
        w("")

    w("## Price-table cross-check")
    w("")
    if not findings:
        w("Every task's recomputed cost matches the cost Juno logged, within 1%.")
    else:
        w(f"{len(findings)} task(s) where Juno's own cost disagrees with the "
          "recomputed cost. Juno's price table is wrong, or its usage parsing is.")
        w("")
        w("| Run | Model | Recomputed | Logged | |")
        w("|---|---|---:|---:|---|")
        for run_id, model, ours, theirs, why in findings:
            w(f"| `{run_id}` | `{model}` | ${ours:.4f} | ${theirs:.4f} | {why} |")
    w("")
    return "\n".join(lines)


# --------------------------------------------------------------------------
# self-test


def self_test():
    prices = load_prices(DEFAULT_PRICES)

    # Hand-computed: sonnet-5-5 at $2/$10/$0.10/$2.50 per MTok.
    # 10,000 in + 2,000 out + 50,000 cache read + 5,000 cache write
    #   = 0.02 + 0.02 + 0.005 + 0.0125 = $0.0575
    row = {
        "model_id": "claude-sonnet-5-5",
        "input_tokens": 10_000,
        "output_tokens": 2_000,
        "cache_read_tokens": 50_000,
        "cache_write_tokens": 5_000,
    }
    got = turn_cost(row, prices)
    assert abs(got - 0.0575) < 1e-9, f"sonnet cost {got} != 0.0575"

    # Opus 5.5 at $4/$20/$0.20/$5: 1M in + 1M out = $24 exactly.
    got = turn_cost(
        {
            "model_id": "claude-opus-5-5",
            "input_tokens": 1_000_000,
            "output_tokens": 1_000_000,
            "cache_read_tokens": 0,
            "cache_write_tokens": 0,
        },
        prices,
    )
    assert abs(got - 24.0) < 1e-9, f"opus cost {got} != 24.0"

    # A subscription model is free, not priced.
    assert turn_cost(
        {
            "model_id": "claude-cli:opus",
            "input_tokens": 999_999,
            "output_tokens": 999_999,
            "cache_read_tokens": 0,
            "cache_write_tokens": 0,
        },
        prices,
    ) == 0.0

    # An unknown model must fail loudly, never score as free.
    try:
        turn_cost(
            {
                "model_id": "gpt-6-imaginary",
                "input_tokens": 1,
                "output_tokens": 1,
                "cache_read_tokens": 0,
                "cache_write_tokens": 0,
            },
            prices,
        )
    except DataError:
        pass
    else:
        raise AssertionError("unknown model silently priced at zero")

    # Haiku crosses its long-context threshold and must switch rates.
    short = turn_cost(
        {
            "model_id": "claude-haiku-5-5",
            "input_tokens": 1_000,
            "output_tokens": 0,
            "cache_read_tokens": 0,
            "cache_write_tokens": 0,
        },
        prices,
    )
    long = turn_cost(
        {
            "model_id": "claude-haiku-5-5",
            "input_tokens": 200_000,
            "output_tokens": 0,
            "cache_read_tokens": 0,
            "cache_write_tokens": 0,
        },
        prices,
    )
    assert abs(short - 0.0001) < 1e-12, short
    assert abs(long - 0.1) < 1e-12, long  # 200k * $0.50/MTok, not $0.10

    # gpt-5.5 past 272K has no published long rates: refuse, don't guess.
    try:
        turn_cost(
            {
                "model_id": "gpt-5.5",
                "input_tokens": 300_000,
                "output_tokens": 0,
                "cache_read_tokens": 0,
                "cache_write_tokens": 0,
            },
            prices,
        )
    except DataError:
        pass
    else:
        raise AssertionError("gpt-5.5 long context guessed a rate")

    assert abs(percentile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 90) - 9.1) < 1e-9
    assert percentile([5], 90) == 5

    # End to end over the fixture.
    fixture = os.path.join(HERE, "fixtures", "synthetic-cost-log.ndjson")
    tasks = build_tasks(load_records(fixture), prices)
    assert tasks, "fixture produced no tasks"
    summaries = summarise(tasks, prices)
    assert {s["model_id"] for s in summaries} >= {
        "claude-sonnet-5-5",
        "claude-opus-5-5",
    }
    # The fixture plants one row whose logged cost is deliberately wrong.
    assert crosscheck(tasks), "cross-check missed the planted bad row"
    assert render(summaries, tasks, crosscheck(tasks), [10, 30])

    # A task is one run_id, and a mixed-model run is rejected.
    try:
        build_tasks(
            [
                {"run_id": "r", "turn_index": 0, "model_id": "claude-opus-5-5",
                 "cost_usd": 0, **{f: 0 for f in NUMERIC_FIELDS if f != "turn_index"}},
                {"run_id": "r", "turn_index": 1, "model_id": "claude-sonnet-5-5",
                 "cost_usd": 0, **{f: 0 for f in NUMERIC_FIELDS if f != "turn_index"}},
            ],
            prices,
        )
    except DataError:
        pass
    else:
        raise AssertionError("mixed-model run accepted")

    print("self-test: all assertions passed")
    return 0


# --------------------------------------------------------------------------


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("log", nargs="?", help="cost log (.ndjson or .csv)")
    parser.add_argument("--prices", default=DEFAULT_PRICES)
    parser.add_argument("--tasks-per-month", default="10,30,60,100,200")
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        return self_test()
    if not args.log:
        parser.error("need a cost log, or --self-test")

    try:
        prices = load_prices(args.prices)
        tasks = build_tasks(load_records(args.log), prices)
        summaries = summarise(tasks, prices)
        findings = crosscheck(tasks)
    except DataError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    volumes = [int(v) for v in args.tasks_per_month.split(",") if v.strip()]
    if args.json:
        print(json.dumps({"summaries": summaries, "tasks": tasks,
                          "crosscheck": findings}, indent=2))
    else:
        print(render(summaries, tasks, findings, volumes))
    return 0


if __name__ == "__main__":
    sys.exit(main())
