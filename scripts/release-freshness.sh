#!/usr/bin/env bash
# Fails when merges to main are sitting unreleased for too long (LAC-8052).
#
# Probe: the canary feed's latest.json asset timestamp, never a run status.
# A starved macOS job ends `cancelled` with steps=0, which reads as an ordinary
# red CI run, and the release gate then correctly refuses. Every health surface
# stays green while no release ships (twice, ~6 days each: LAC-7193, LAC-6333).
#
# Stale = the oldest non-`release:` commit on main newer than the feed is more
# than MAX_AGE_HOURS old. Env: REPO (default lacymorrow/juno), MAX_AGE_HOURS (6).
set -euo pipefail

REPO="${REPO:-lacymorrow/juno}"
MAX_AGE_HOURS="${MAX_AGE_HOURS:-6}"

feed=$(gh api "repos/$REPO/releases/tags/canary" \
  --jq '[.assets[]|select(.name=="latest.json")][0].updated_at // empty')
if [ -z "$feed" ]; then
  echo "::error title=No canary feed::canary release has no latest.json asset."
  exit 1
fi

# Oldest unreleased merge: commits after the feed, minus the release bot's own.
# per_page=100 and no pagination: past 100 unreleased commits the oldest one is
# not in this page, so the age is understated (it still alarms, just late).
# The `release:` prefix is shipx's version-bump commit; the same match lives in
# .github/workflows/release-every-merge.yml (`release:*)` in the decide step).
# If that commit format changes, update both or every bump reads as unreleased.
oldest=$(gh api "repos/$REPO/commits?sha=main&since=$feed&per_page=100" \
  --jq '[.[]|select(.commit.message|startswith("release:")|not)|.commit.committer.date]|min // empty')

if [ -z "$oldest" ]; then
  echo "Canary feed $feed is current: nothing unreleased on main."
  exit 0
fi

age_h=$(( ( $(date -u +%s) - $(date -u -d "$oldest" +%s 2>/dev/null || date -u -j -f '%Y-%m-%dT%H:%M:%SZ' "$oldest" +%s) ) / 3600 ))
echo "Canary feed: $feed. Oldest unreleased merge on main: $oldest (${age_h}h ago)."

if [ "$age_h" -ge "$MAX_AGE_HOURS" ]; then
  echo "::error title=Releases stalled::main has had unreleased merges for ${age_h}h (limit ${MAX_AGE_HOURS}h); canary latest.json last moved $feed. Read the 'Should this merge' annotations on the newest Release every merge run, and look for cancelled steps=0 macOS jobs in CI."
  exit 1
fi
