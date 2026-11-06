#!/usr/bin/env bash
#
# Render scripts/release-failure-report.sh and assert what it says.
#
#   scripts/__tests__/release-failure-report.test.sh
#
# Runs in CI (.github/workflows/ci.yml, "Release failure report") and takes
# about a second. Needs bash and jq. Nothing else, and no network.
#
# WHY THIS EXISTS (LAC-7209)
#
# The release-failure report was edited three times in two days and every one
# of those edits was verified by hand-extracting the step to /tmp, stubbing
# `gh` and `date` around it and reading the rendered body. Three throwaway
# harnesses, none committed, each rebuilt from scratch. Every bug they caught
# was the same shape - a number that was wrong but looked right (a streak of
# 13 reported as 1, then as 13 for a streak of 12, then a floor printed as a
# total) - and that is exactly the class a render test catches and a reader
# does not.
#
# HOW IT STUBS
#
# `gh` is replaced by a script on PATH. Every READ it answers applies the real
# `--jq` expression the script passed to a fixture file, so the jq is under
# test rather than stubbed over - that is where all three bugs above lived.
# Every WRITE (`issue create`, `issue comment`, `issue close`) is recorded and
# the `--body` captured, which is the seam the rendered Markdown comes out of.
#
# `date` is replaced too, but only for "now": a call carrying "-d" is handed
# to the real date, so the ISO-8601 parse stays real. That parse is GNU-only,
# which is where this script runs, so the one case that asserts an elapsed
# time SKIPS on BSD date (macOS) and runs for real in CI. Everything else is
# portable.
set -euo pipefail

here=$(cd -- "$(dirname -- "$0")" && pwd)
SCRIPT="$here/../release-failure-report.sh"
# Captured before PATH is shadowed, so the date stub can delegate a real parse
# to it. Shadowing PATH is the whole mechanism, so "command date" is not an
# escape hatch here - it finds the stub.
REAL_DATE=$(command -v date)

# Fixed clock. NOW is 1793491200 + 3d4h, so the one elapsed-time case below
# has an exact expected string instead of a tolerance.
WAS_AT="2026-11-01T00:00:00Z"   # = epoch 1793491200
NOW=1793764800                  # = WAS_AT + 3 days 4 hours

# Every case the harness is supposed to run. Asserted at the end, because a
# case that silently stops running is a gate that silently stops gating: the
# failure mode of a grep-based check is a pass, not a red. Bump it when you
# add a case; a mismatch is a failure even when every case passed.
EXPECTED_CASES=18

cases=0
failures=0
skipped=0
name=''
stub=''

ok()   { printf '  ok   %s\n' "$1"; }
bad()  { printf '  FAIL %s\n       %s\n' "$1" "$2"; failures=$((failures + 1)); }

begin() {
  name="$1"
  cases=$((cases + 1))
  printf '\n%s\n' "$name"
  stub=$(mktemp -d "${TMPDIR:-/tmp}/relreport.XXXXXX")
  mkdir -p "$stub/bin"
  write_stubs
  # Defaults every case starts from: no open issue, no failed steps found, no
  # releases, no prior runs. A case overwrites only what it is about.
  printf '[]'                     > "$stub/open.json"
  printf '{"jobs":[]}'            > "$stub/jobs.json"
  printf '[]'                     > "$stub/releases.json"
  printf '{"workflow_runs":[]}'   > "$stub/runs.json"
  : > "$stub/calls"
  : > "$stub/body"
}

write_stubs() {
  cat > "$stub/bin/gh" <<'GH'
#!/usr/bin/env bash
# Stubbed gh. Reads answer the caller's own --jq against a fixture; writes are
# recorded. $STUB names the fixture directory.
set -u
printf '%s\n' "$*" >> "$STUB/calls"

# The expression the caller passed. "gh api --jq" takes exactly one.
jqexpr=''
prev=''
for a in "$@"; do
  [ "$prev" = "--jq" ] && jqexpr="$a"
  prev="$a"
done

apply() { # $1 = fixture basename
  # An error body on STDOUT with a non-zero exit, which is what the real gh
  # does and the reason the caller validates what it gets back.
  if [ -f "$STUB/$1.err" ]; then cat "$STUB/$1.err"; exit 1; fi
  if [ ! -f "$STUB/$1.json" ]; then echo "gh stub: no fixture $1.json" >&2; exit 9; fi
  # Honour per_page. A stub that ignores it cannot see a page-size bug, and
  # the caller's floor marker is decided entirely by whether the page came
  # back full - so a request that asks for fewer runs than the marker tests
  # against would silently never mark, and this harness would stay green.
  # (That mutant survived until the truncation went in.)
  per=$(printf '%s' "${2:-}" | sed -n 's/.*[?&]per_page=\([0-9][0-9]*\).*/\1/p')
  if [ -n "$per" ]; then
    PER="$per" jq 'if type == "object" and has("workflow_runs")
                   then .workflow_runs |= .[0:(env.PER | tonumber)]
                   elif type == "array" then .[0:(env.PER | tonumber)]
                   else . end' "$STUB/$1.json" | jq -r "$jqexpr"
  else
    jq -r "$jqexpr" < "$STUB/$1.json"
  fi
}

capture_body() {
  prev=''
  for a in "$@"; do
    [ "$prev" = "--body" ] && printf '%s' "$a" > "$STUB/body"
    prev="$a"
  done
}

case "${1:-}" in
  issue)
    case "${2:-}" in
      list)             apply open ;;
      comment|create)   capture_body "$@" ;;
      close)            : ;;
      *) echo "gh stub: unexpected issue subcommand: $*" >&2; exit 9 ;;
    esac
    ;;
  api)
    case "${2:-}" in
      */actions/runs/*/jobs)   apply jobs "$2" ;;
      */actions/workflows/*)   apply runs "$2" ;;
      */releases*)             apply releases "$2" ;;
      *) echo "gh stub: unexpected api path: ${2:-}" >&2; exit 9 ;;
    esac
    ;;
  *) echo "gh stub: unexpected invocation: $*" >&2; exit 9 ;;
esac
GH

  cat > "$stub/bin/date" <<'DATE'
#!/usr/bin/env bash
# Stubbed date. A "-d" call is a parse and goes to the real date, so the
# ISO-8601 handling under test is real; anything else is the "now" read and
# comes back fixed, so an elapsed time is deterministic.
set -u
for a in "$@"; do
  if [ "$a" = "-d" ]; then exec "$REAL_DATE" "$@"; fi
done
printf '%s\n' "$STUB_NOW"
DATE

  chmod +x "$stub/bin/gh" "$stub/bin/date"
}

# Run the script under the stubs. $1 overrides RESULT; the rest are extra
# VAR=value prefixes for the one case that removes a variable.
render() {
  local result="${1:-failure}"
  set +e
  env PATH="$stub/bin:$PATH" \
      STUB="$stub" STUB_NOW="$NOW" REAL_DATE="$REAL_DATE" \
      GH_TOKEN=stub-token \
      REPO=lacymorrow/juno \
      RESULT="$result" \
      TAG=v0.8.263 \
      RUN_URL=https://github.com/lacymorrow/juno/actions/runs/7 \
      GITHUB_RUN_ID=999 \
      GITHUB_WORKFLOW_REF=lacymorrow/juno/.github/workflows/release-tauri.yml@refs/heads/main \
      bash "$SCRIPT" > "$stub/stdout" 2> "$stub/stderr"
  rc=$?
  set -e
}

# A window of $1 completed runs, all with conclusion $2, newest first. The
# GitHub API returns at most per_page items, so a list of exactly 40 is how
# "there may be more" reaches the script.
runs_all() {
  jq -n --argjson n "$1" --arg c "$2" \
    '{workflow_runs: [range($n) | {id: (. + 1), status: "completed", conclusion: $c}]}' \
    > "$stub/runs.json"
}

# A window from an explicit newest-first list of conclusions. "cancelled" and
# "in_progress" are meaningful here: the first is dropped, the second is what
# this run itself looks like while its own report renders.
runs_list() {
  printf '%s\n' "$@" | jq -R . | jq -s \
    '{workflow_runs: [to_entries[] | {id: (.key + 1), status: "completed", conclusion: .value}]}' \
    > "$stub/runs.json"
}

body_has()    { if grep -qF -- "$2" "$stub/body"; then ok "$1"; else bad "$1" "missing from body: $2"; fi; }
body_lacks()  { if grep -qF -- "$2" "$stub/body"; then bad "$1" "present in body but should not be: $2"; else ok "$1"; fi; }
calls_have()  { if grep -qF -- "$2" "$stub/calls"; then ok "$1"; else bad "$1" "no such gh call: $2"; fi; }
calls_lack()  { if grep -qF -- "$2" "$stub/calls"; then bad "$1" "gh was called and should not have been: $2"; else ok "$1"; fi; }
exited()      { if [ "$rc" -eq "$2" ]; then ok "$1"; else bad "$1" "exit $rc, expected $2"; fi; }

# --- the success path -------------------------------------------------------

begin "success with an open issue: comments the fix and closes it"
printf '[{"number":926}]' > "$stub/open.json"
render success
exited "exits 0" 0
calls_have "comments on the open issue" "issue comment 926"
calls_have "says what fixed it" "Fixed: v0.8.263 built and published"
calls_have "closes it" "issue close 926"

begin "success with no open issue: writes nothing"
render success
exited "exits 0" 0
calls_lack "opens no issue" "issue create"
calls_lack "comments nowhere" "issue comment"

# --- the streak count ------------------------------------------------------
#
# The number in the body is prior + this run. So a window whose first success
# sits at index 2 is a streak of 3.

begin "streak counts to the first success and ignores a cancelled run"
runs_list failure cancelled failure success
render
exited "exits 0" 0
body_has "2 prior failures plus this one" "Consecutive failed release dispatches: 3"
body_lacks "claims no floor: the window holds a success" "dispatches: 3+"
body_has "escalates past the first failure" "re-dispatching is no longer a reasonable first move"

begin "an empty window is a first failure"
render
exited "exits 0" 0
body_has "prints 1" "Consecutive failed release dispatches: 1"
body_lacks "no floor marker" "dispatches: 1+"
body_has "gives first-failure advice" "does not on its own mean APPLE_PASSWORD is wrong"

begin "a window whose newest run succeeded is a first failure"
runs_list success
render
exited "exits 0" 0
body_has "prints 1" "Consecutive failed release dispatches: 1"
body_has "gives first-failure advice" "Otherwise re-dispatch the tag once"

# LAC-7209 item 2. This is the case the marker used to get wrong: three
# dispatches ever, all failed, printed "4+" when 3 was the entire history and
# the count was exact.
begin "a short all-failed history is exact, not a floor (LAC-7209)"
runs_all 2 failure
render
exited "exits 0" 0
body_has "prints the exact streak" "Consecutive failed release dispatches: 3"
body_lacks "no floor marker: the page came back short" "dispatches: 3+"

begin "a full page marks a floor (LAC-7199)"
runs_all 40 failure
render
exited "exits 0" 0
body_has "40 visible plus this one, at least" "Consecutive failed release dispatches: 41+"
body_has "escalates" "re-dispatching is no longer a reasonable first move"

# The boundary, from the other side. 39 is a page that came back short, so the
# count is the whole history and takes no marker.
begin "one short of a full page takes no marker"
runs_all 39 failure
render
exited "exits 0" 0
body_has "prints the exact streak" "Consecutive failed release dispatches: 40"
body_lacks "no floor marker" "dispatches: 40+"

# This run is excluded by id, not by status. It is COMPLETED here on purpose:
# while its own report renders it is normally still in_progress, so the status
# filter appears to exclude it and an in_progress fixture would pass with the
# id filter deleted. A re-run is completed, and then only the id saves it.
begin "this run is excluded from its own streak, even once completed"
jq -n '{workflow_runs: [{id: 999, status: "completed", conclusion: "failure"},
                        {id: 1, status: "completed", conclusion: "failure"}]}' > "$stub/runs.json"
render
exited "exits 0" 0
body_has "counts the one prior failure, not itself" "Consecutive failed release dispatches: 2"
body_lacks "does not count itself twice" "dispatches: 3"

# Nothing countable in a full page: the page is full, so a floor marker would
# normally be right, but a streak of "at least 1" is not a measurement of
# anything. The count degrades to a plain first failure.
begin "a full page with nothing countable in it claims no floor"
jq -n '{workflow_runs: [range(40) | {id: (. + 1), status: "completed", conclusion: "cancelled"}]}' \
  > "$stub/runs.json"
render
exited "exits 0" 0
body_has "prints 1" "Consecutive failed release dispatches: 1"
body_lacks "no floor marker on a zero count" "dispatches: 1+"

begin "a runs-API error body on stdout degrades to a first failure"
printf '{"message":"Not Found","documentation_url":"https://docs.github.com/rest"}' > "$stub/runs.err"
render
exited "still reports: the count failing must not cost the alarm" 0
body_has "prints 1" "Consecutive failed release dispatches: 1"
body_lacks "an unparsable count claims no floor" "dispatches: 1+"

# --- the last published release --------------------------------------------

begin "an unparsable publish date says so instead of measuring"
jq -n '[{tag_name: "v0.8.263", published_at: "whenever"}]' > "$stub/releases.json"
render
exited "exits 0" 0
body_has "names the tag" "Last published release: v0.8.263"
body_has "refuses to invent an interval" "at an unknown time"
body_lacks "no 0d 0h" "0d 0h ago"

begin "a parsable publish date reports how long the app has been unshippable"
if "$REAL_DATE" -u -d "$WAS_AT" +%s >/dev/null 2>&1; then
  jq -n --arg at "$WAS_AT" '[{tag_name: "v0.8.263", published_at: $at}]' > "$stub/releases.json"
  render
  exited "exits 0" 0
  body_has "reports the elapsed time" "Last published release: v0.8.263, 3d 4h ago"
else
  printf '  SKIP this needs GNU date ("date -u -d"); it runs in CI on ubuntu\n'
  skipped=$((skipped + 1))
fi

begin "cua-v tags are a different product and do not count as a release"
jq -n '[{tag_name: "cua-v1.2.3", published_at: "2026-11-01T00:00:00Z"},
        {tag_name: "v0.8.263",   published_at: "whenever"}]' > "$stub/releases.json"
render
body_has "skips past the cua tag" "Last published release: v0.8.263"

# --- where the report lands -------------------------------------------------

begin "a failure with no open issue opens one"
render
calls_have "creates the issue" "issue create"
calls_have "with the title the next run searches for" "Release builds are failing"
calls_lack "comments nowhere" "issue comment"

begin "a failure with an open issue comments on it instead"
printf '[{"number":926}]' > "$stub/open.json"
render
calls_have "comments on it" "issue comment 926"
calls_lack "opens no second issue" "issue create"

begin "the failed step is named"
jq -n '{jobs: [{conclusion: "failure",
                steps: [{conclusion: "success", name: "Configure Apple code signing"},
                        {conclusion: "failure", name: "Check the notary service"}]}]}' > "$stub/jobs.json"
render
body_has "names the step that actually failed" "Failed step: Check the notary service"
body_lacks "not the step that passed" "Failed step: Configure Apple code signing"

# --- the environment contract -----------------------------------------------

begin "a missing input is named rather than aborting as an unbound variable"
set +e
env PATH="$stub/bin:$PATH" STUB="$stub" STUB_NOW="$NOW" REAL_DATE="$REAL_DATE" \
    GH_TOKEN=stub-token RESULT=failure TAG=v0.8.263 RUN_URL=https://example.invalid/7 \
    GITHUB_RUN_ID=999 \
    bash "$SCRIPT" > "$stub/stdout" 2> "$stub/stderr"
rc=$?
set -e
exited "exits 2" 2
if grep -qF 'REPO is unset' "$stub/stderr"; then ok "names the variable"; else bad "names the variable" "stderr was: $(cat "$stub/stderr")"; fi
calls_lack "writes nothing" "issue"

# --- verdict ----------------------------------------------------------------

printf '\n%s\n' "-----"
total=$((cases))
printf 'cases: %s  failed assertions: %s  skipped cases: %s\n' "$total" "$failures" "$skipped"

# The count tripwire. A harness that quietly stops running half its cases
# still exits 0, so the number of cases is asserted as well as their results.
if [ "$total" -ne "$EXPECTED_CASES" ]; then
  printf 'FAIL ran %s cases, expected %s. Update EXPECTED_CASES when you add one.\n' "$total" "$EXPECTED_CASES"
  exit 1
fi

if [ "$failures" -ne 0 ]; then
  printf 'FAIL %s assertion(s)\n' "$failures"
  exit 1
fi

printf 'ok, %s cases\n' "$total"
