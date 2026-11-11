#!/usr/bin/env bash
#
# Open, update or close the "Release builds are failing" tracking issue.
#
#   scripts/release-failure-report.sh
#
# Called by the `report` job in .github/workflows/release-tauri.yml, which is
# the only caller. Everything it needs arrives in the environment:
#
#   GH_TOKEN               a token with issues:write, actions:read, contents:read
#   REPO                   owner/name
#   RESULT                 the build-tauri job's result ("success" or not)
#   TAG                    the tag that was being released
#   RUN_URL               link back to this run
#   GITHUB_RUN_ID          this run, excluded from its own streak count
#   GITHUB_WORKFLOW_REF    optional; names the workflow file to count runs of
#
# A failed release used to be silent. From v0.8.245 to v0.8.249 every build
# died at notarization (an Apple app-specific password had been revoked) and
# nothing said so: merges kept landing, versions kept being tagged, and no
# build reached anyone for a day. A failure now opens one issue, which GitHub
# notifies the owner of; later failures comment on it, and the next release
# that builds closes it.
#
# WHY THIS IS A FILE AND NOT A `run:` BLOCK (LAC-7209)
#
# It used to be ~180 lines inline in release-tauri.yml. It was edited three
# times in two days (LAC-7193, LAC-7196, LAC-7199) and every one of those
# edits was verified by hand-extracting the block to /tmp, stubbing `gh` and
# `date` around it and rendering the body. Three times, three throwaway
# harnesses, none of them committed. Every bug those edits fixed was a
# wrong-number-that-looks-right, which is the class a rendering test catches
# and a reader does not.
#
# So the harness is committed too: scripts/__tests__/release-failure-report.test.sh
# runs this script against stubbed `gh` and real `jq`, and CI runs it on every
# PR. Change anything here and run it.
#
# One thing the move cost: actionlint runs shellcheck over a workflow's `run:`
# blocks, and it cannot see a separate file. The harness runs shellcheck on
# this script itself to put that back.
set -euo pipefail

# This script is the whole of a step that exists to raise an alarm, so a
# missing input must say which one rather than abort as "unbound variable" 180
# lines later. Not GH_TOKEN: `gh` reports that one better than this can.
for required in REPO RESULT TAG RUN_URL GITHUB_RUN_ID; do
  if [ -z "${!required:-}" ]; then
    echo "release-failure-report.sh: \$$required is unset or empty" >&2
    exit 2
  fi
done

# How far back the streak is counted. Used twice - as the page size asked for
# and as the test for whether that page came back full - so it is named once.
# Exported because jq reads it as env.WINDOW; see the streak query below for
# why that is not a "--arg".
WINDOW=40
export WINDOW

title="Release builds are failing"
open=$(gh issue list --repo "$REPO" --state open --search "\"$title\" in:title" --json number --jq '.[0].number // empty')
if [ "$RESULT" = "success" ]; then
  if [ -n "$open" ]; then
    gh issue comment "$open" --repo "$REPO" --body "Fixed: $TAG built and published. $RUN_URL"
    gh issue close "$open" --repo "$REPO"
  fi
  exit 0
fi
# Name the step that actually failed instead of guessing at a
# cause. The jobs API answers immediately, while run logs are
# usually still locked this soon after the failure, so the logs are
# deliberately not read here.
failed=$(gh api "repos/$REPO/actions/runs/$GITHUB_RUN_ID/jobs" \
  --jq '.jobs[] | select(.conclusion == "failure") | .steps[] | select(.conclusion == "failure") | .name' \
  2>/dev/null | head -5 | paste -sd '; ' - || true)

# How many times has this already been reported, and how long has the
# app been unshippable? Both numbers were computable right here and
# neither was in the body, so every comment on the tracking issue read
# as if it were the first one. #926 collected 13 of them over six days
# while the advice below stayed frozen at the advice for a FIRST
# failure: twelve consecutive reds in, it was still saying "re-dispatch
# the tag first" and "replace APPLE_PASSWORD only when red on two runs
# in a row", a condition met on day one. A reader who followed it
# burned a macOS runner to re-learn the same 401 (LAC-7193).
#
# Every query below is best-effort. This runs only when the release is
# already broken, so failing to COUNT the breakage must never also cost
# the report: hence the "|| true" and the ${x:-} fallbacks, which
# degrade to the first-failure wording.
#
# The workflow file name comes from the run rather than a literal, so
# renaming that file cannot silently pin the streak at 1 - a wrong
# number that looks right is the bug being fixed here.
wf=${GITHUB_WORKFLOW_REF:-}
wf=${wf%%@*}
wf=${wf##*/}
# This run is excluded by id rather than by status. It is normally
# still in_progress while its own report job runs, so a status filter
# appears to exclude it too - but only by luck, and a re-run or a
# timing change would then double-count it. Excluding the id is true
# either way. Verified by rendering this body against a completed run
# (LAC-7193): the status filter alone reported 13 for a streak of 12.
# The run id is read with jq's env.*, not --arg: "gh api --jq" takes a
# single expression string and silently treated a "--arg" after it as
# the expression, which collapsed the count to 1 (same run, LAC-7193).
#
# Cancelled runs are dropped rather than counted as failures. The
# report job's own "if" refuses to report a cancelled build, so
# counting one as a failed dispatch would put a number in the body for
# a run that never got a body of its own. Dropping it also means a
# cancellation between two real failures neither breaks the streak nor
# pads it.
#
# THE "+" MARKS A FLOOR, AND ONLY A FLOOR (LAC-7199, corrected LAC-7209).
# With no success in the window there is nothing to index against, so the
# count is the number of failures VISIBLE, which is a floor whenever the
# window is smaller than the history. "40" where the truth is "40 or more" is
# the same class of wrong-number-that-looks-right as the two bugs above.
#
# But a short history is not a truncated one. Asking for 40 and getting 3 back
# means 3 is every dispatch this workflow has ever had, and the count is then
# exact - a workflow with three dispatches ever, all failed, printed "4+" and
# the "+" was claiming a floor against a number that was already the total.
# The marker is therefore tied to the page coming back FULL, not to the
# absence of a success.
#
# `total_count` was the other candidate and is deliberately not used: when it
# is absent or null, "null > 40" is false in jq, so the marker would silently
# drop and the body would claim an exactness it had not measured. That is this
# same bug inverted. `.workflow_runs | length` cannot be absent.
#
# No jq $-variables: inside a single-quoted expression a jq "$x" is
# indistinguishable from an unexpanded shell one, so shellcheck reports SC2016
# on it (SC2016 is also why the two-stage object pipeline below carries values
# forward as fields instead). Note also that a comment whose first word is the
# linter's own name is read as a directive rather than prose: that is SC1072,
# and it is an error, not an info.
prior=$(gh api "repos/$REPO/actions/workflows/${wf:-release-tauri.yml}/runs?event=workflow_dispatch&per_page=$WINDOW" \
  --jq '{page: (.workflow_runs | length),
         seen: [.workflow_runs[]
                | select((.id | tostring) != env.GITHUB_RUN_ID)
                | select(.status == "completed")
                | select(.conclusion != "cancelled")
                | .conclusion]}
        | {page, n: (.seen | length), i: (.seen | index("success"))}
        | if .i != null then "\(.i)"
          elif .page >= (env.WINDOW | tonumber) then "\(.n)+"
          else "\(.n)"
          end' \
  2>/dev/null || true)
# "gh api" prints its error body on STDOUT, so 2>/dev/null does not
# keep a 404 out of $prior, and feeding that JSON to $(( )) aborted
# the whole step - which would have silenced the only alarm this
# exists to raise. Caught by running this script against a renamed
# workflow (LAC-7193). Anything that is not a plain integer is 0, and
# such a value also loses its "+", so it cannot claim a floor it never
# measured. A floor of 0 loses it too: a window with no countable
# dispatch in it is not a streak that overflowed the window.
atleast=''
case "${prior:-}" in
  *+) atleast='+'; prior=${prior%+} ;;
esac
case "${prior:-}" in
  ''|*[!0-9]*) prior=0; atleast='' ;;
esac
if [ "$prior" -eq 0 ]; then
  atleast=''
fi
streak=$(( prior + 1 ))
# What the body prints. $streak stays numeric so the comparison that
# picks the advice below still works.
streak_text="$streak$atleast"

# Prereleases included on purpose: `prerelease` is the default update
# channel, so a prerelease IS a shipped release to every install.
# cua-v* tags are a different product and would mask the outage.
last=$(gh api "repos/$REPO/releases?per_page=60" \
  --jq 'map(select(.tag_name | startswith("v"))) | .[0] | "\(.tag_name) \(.published_at)"' \
  2>/dev/null || true)
# Same hazard as $prior above: an error body would otherwise be
# reported as the last published release. Require "v<tag> <stamp>".
case "${last:-}" in
  v*' '*) ;;
  *) last='' ;;
esac
last_tag=${last%% *}
last_at=${last#* }
# No "|| echo $now" fallback here. That printed "0d 0h ago" on a
# date it could not parse, which reads as a freshly published release
# and would have hidden exactly the outage this is meant to surface.
# An unparsed date must degrade to saying so (LAC-7193).
# $stale carries its own tail, "ago" included, because the unparsed
# case must not take one: "an unknown time ago" reads as a measured
# interval rather than a missing measurement (LAC-7199).
#
# "date -u -d" is GNU: this runs on ubuntu-latest. On BSD date it fails, which
# lands in the branch below and degrades to the unparsed wording rather than
# lying - so the harness skips its one elapsed-time case on macOS and asserts
# it in CI.
stale="at an unknown time"
if [ -n "${last_at:-}" ] && [ "$last_at" != "${last:-}" ]; then
  was=$(date -u -d "$last_at" +%s 2>/dev/null || true)
  if [ -n "$was" ]; then
    now=$(date -u +%s)
    stale="$(( (now - was) / 86400 ))d $(( ((now - was) % 86400) / 3600 ))h ago"
  fi
fi

# The paragraph below used to read "Notarization failures (HTTP 401)
# mean the APPLE_PASSWORD app-specific password needs replacing". It
# asserted a cause this cannot observe, and it was wrong: that
# sentence opened #926, a password was rotated on the strength of it,
# and the next build 401'd on the new one (LAC-6331). State the
# evidence and let whoever reads the log draw the conclusion.
#
# A heredoc, not a quoted string: this runs only when the release is
# already broken, which is the worst possible moment to find an
# escaping bug, so the text carries no shell metacharacters. The bodies
# start at column 0 even inside the `if` below, because `<<EOF` without the
# dash keeps leading whitespace and ten spaces of it is a Markdown code
# block. (Inline in the workflow this was invisible: YAML strips a block
# scalar's common indentation before the shell ever sees it.)
head=$(cat <<EOF
The release build for $TAG failed, so nothing was published and nobody gets this version.

Consecutive failed release dispatches: $streak_text
Last published release: ${last_tag:-unknown}, $stale

Run: $RUN_URL
Failed step: ${failed:-unknown, see the run}

Read that step's log before changing anything. On a notarization failure also read the "Configure Apple code signing" step, which PASSES and is therefore invisible to "gh run view --log-failed" - its warning lines are where a malformed credential gets named, and four runs in a row were misdiagnosed because nobody looked there (LAC-6333).
EOF
)

# The advice depends on the streak, because the right first move and
# the right twelfth move are opposites. Re-dispatching is correct once
# (a 401 really can be reachability) and wrong every time after.
if [ "$streak" -le 1 ]; then
  advice=$(cat <<EOF
A notarization "HTTP status code: 401" does not on its own mean APPLE_PASSWORD is wrong. The endpoint behind it, notary/v2/asp, IS the app-specific-password token exchange, so a runner that cannot reach Apple gets a 401 rather than a network error, and the same unchanged credential has succeeded within hours of a 401.

But if the signing step warned that APPLE_PASSWORD does not match the app-specific-password shape, the 401 is already explained and re-dispatching will not clear it. Replace the secret.

Otherwise re-dispatch the tag once:

    gh workflow run release-tauri.yml --ref main -f tag=$TAG -f prerelease=true

If that is also red, stop re-dispatching: this comment will say so, and name the credential to change instead.
EOF
)
else
  advice=$(cat <<EOF
This is failure $streak_text in a row, so re-dispatching is no longer a reasonable first move. Do not dispatch this tag again expecting a different result. Change a credential or the code first. Those dispatches may have carried different tags and failed for different reasons, so read the earlier comments here before settling on one cause.

On a 401 from "Check the notary service", read APPLE_PASSWORD_SHAPE in the "Configure Apple code signing" log. "bad" means the stored value is not an app-specific password - those are always exactly 19 characters, xxxx-xxxx-xxxx-xxxx - and nothing but replacing it will clear this.

The durable fix is the App Store Connect API key path (APPLE_API_ISSUER, APPLE_API_KEY, APPLE_API_KEY_P8_B64): it signs a JWT on the runner and never calls notary/v2/asp at all. Those three are read from the \`release\` environment, so set them there, not at repo scope.
EOF
)
fi

body="$head

$advice"
if [ -n "$open" ]; then
  gh issue comment "$open" --repo "$REPO" --body "$body"
else
  gh issue create --repo "$REPO" --title "$title" --body "$body"
fi
