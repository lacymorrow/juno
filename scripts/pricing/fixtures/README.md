# Fixtures

`synthetic-cost-log.ndjson` is **invented data**, written by hand to exercise
`analyze-cost-log.py`. It is not a measurement and must never be quoted as one.

Two tells, both deliberate: every `run_id` says `SYNTHETIC-not-a-measurement`,
and every timestamp is in the year 2000.

The `cost_usd` values are hand-written and do not all agree with the tokens
beside them, so the price-table cross-check has something to catch. Run 005
is the blatant one, at $9.99 against about $0.16 of tokens; two others drift
by under 10%. Three cross-check findings on this fixture is the expected
result, not a bug.

Real measurement data for LAC-4124 lands as a CSV attached to the Paperclip
issue, not in this directory.
