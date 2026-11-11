#!/usr/bin/env bash
#
# Behaviour tests for scripts/notarize-dmg.sh.
#
# `xcrun` and `gh` are stubbed on PATH, so nothing here talks to Apple or to
# GitHub. The stub's submit output is copied from the real v0.8.276 release log
# (run 42974458280), including the shape that caused the bug: the upload
# succeeds, the id is printed, and then the POLL dies with no status line.
#
# The point of the suite is the verdict/re-poll state machine. A DMG is
# notarized once per release and a lost verdict costs a 50-minute build, so the
# branches have to be exercised somewhere other than production.

set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="$here/../notarize-dmg.sh"

pass=0
fail=0

# --- stub harness ---------------------------------------------------------

setup() {
  work="$(mktemp -d)"
  state="$work/state"
  bin="$work/bin"
  mkdir -p "$state" "$bin" "$work/dmg" "$work/tmp"
  : >"$state/calls"

  cat >"$bin/xcrun" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "xcrun $*" >>"$STATE/calls"
tool="$1"; shift
case "$tool" in
  notarytool)
    sub="$1"; shift
    case "$sub" in
      submit)
        case "${STUB_SUBMIT:-accepted}" in
          accepted)
            echo "Conducting pre-submission checks and initiating connection to the Apple notary service..."
            echo "Submission ID received"
            echo "  id: cbf6a189-cab9-42f4-9d9d-9062793c2449"
            echo "Successfully uploaded file"
            echo "Waiting for processing to complete."
            echo "Processing complete"
            echo "  id: cbf6a189-cab9-42f4-9d9d-9062793c2449"
            echo "  status: Accepted"
            exit 0 ;;
          invalid)
            echo "Submission ID received"
            echo "  id: cbf6a189-cab9-42f4-9d9d-9062793c2449"
            echo "Processing complete"
            echo "  status: Invalid"
            # notarytool has been seen to exit 0 on an Invalid verdict.
            exit 0 ;;
          rejected)
            echo "  id: cbf6a189-cab9-42f4-9d9d-9062793c2449"
            echo "  status: Rejected"
            exit 0 ;;
          pollfail)
            # The v0.8.276 shape: uploaded fine, then the poll lost the network.
            echo "Submission ID received"
            echo "  id: cbf6a189-cab9-42f4-9d9d-9062793c2449"
            echo "Successfully uploaded file"
            echo "  id: cbf6a189-cab9-42f4-9d9d-9062793c2449"
            echo "Waiting for processing to complete. Wait timeout is set to 1800.0 second(s)."
            echo 'Current status: In Progress....Error: HTTPError(statusCode: nil, error: Error Domain=NSURLErrorDomain Code=-1009 "The Internet connection appears to be offline.")'
            exit 1 ;;
          pollfail_noid)
            # Died before Apple ever answered, so there is nothing to re-poll.
            echo 'Error: HTTPError(statusCode: nil, error: NSURLErrorDomain Code=-1009)'
            exit 1 ;;
        esac ;;
      wait)
        n=$(( $(cat "$STATE/waits" 2>/dev/null || echo 0) + 1 ))
        echo "$n" >"$STATE/waits"
        # STUB_WAIT_OK_ON=<n> accepts on the nth re-poll and fails before it;
        # otherwise STUB_WAIT decides every call. Kept as nested ifs because
        # `a || b && c` groups as `(a || b) && c` in bash, which silently
        # inverted this the first time it was written.
        case "${STUB_WAIT:-accepted}" in
          invalid)
            echo "  id: cbf6a189-cab9-42f4-9d9d-9062793c2449"
            echo "  status: Invalid"
            exit 0 ;;
          accepted_badrc)
            # Apple says Accepted but notarytool still exits non-zero.
            echo "  id: cbf6a189-cab9-42f4-9d9d-9062793c2449"
            echo "  status: Accepted"
            exit 1 ;;
        esac
        accept=no
        if [ -n "${STUB_WAIT_OK_ON:-}" ]; then
          [ "$n" -ge "$STUB_WAIT_OK_ON" ] && accept=yes
        elif [ "${STUB_WAIT:-accepted}" = accepted ]; then
          accept=yes
        fi
        if [ "$accept" = yes ]; then
          echo "Waiting for processing to complete."
          echo "Processing complete"
          echo "  id: cbf6a189-cab9-42f4-9d9d-9062793c2449"
          echo "  status: Accepted"
          exit 0
        fi
        echo 'Error: HTTPError(statusCode: nil, error: NSURLErrorDomain Code=-1009)'
        exit 1 ;;
      log)
        echo '{"issues": null}'
        exit 0 ;;
    esac ;;
  stapler)
    echo "stapler $1 ok"
    exit 0 ;;
esac
exit 0
STUB

  cat >"$bin/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "gh $*" >>"$STATE/calls"
exit 0
STUB

  chmod +x "$bin/xcrun" "$bin/gh"
}

teardown() { rm -rf "$work"; }

# run <expected-exit> -- runs the script with the stubs on PATH
run() {
  STATE="$state" PATH="$bin:$PATH" RUNNER_TEMP="$work/tmp" \
    TAG=v0.9.9 REPO=lacymorrow/juno NOTARY_REPOLL_DELAY_BASE=0 \
    bash "$script" "$work/dmg" >"$work/out" 2>&1
  echo $?
}

ok()   { pass=$((pass + 1)); printf '  ok   %s\n' "$1"; }
bad()  { fail=$((fail + 1)); printf '  FAIL %s\n' "$1"; sed 's/^/       /' "$work/out" | tail -12; }

check() { # check <label> <condition-result 0/1>
  if [ "$2" -eq 0 ]; then ok "$1"; else bad "$1"; fi
}

# `--` is mandatory: every interesting pattern here starts with "--", which
# grep otherwise reads as its own option and exits 2. Absence is then checked
# by exit code rather than by negation, so a broken grep fails the assertion
# instead of passing it vacuously.
called() { grep -qF -- "$1" "$state/calls"; }
not_called() {
  local rc
  grep -qF -- "$1" "$state/calls"
  rc=$?
  case $rc in
    0) return 1 ;;
    1) return 0 ;;
    *) printf '       grep failed (rc=%s) checking absence of: %s\n' "$rc" "$1" >&2
       return 1 ;;
  esac
}
waits()      { cat "$state/waits" 2>/dev/null || echo 0; }

# A helper rather than an inline test so the assertion chain below ends in a
# command: `$?` after a chain ending in `[ ... ]` refers to the condition, and
# SC2319 rightly flags that. (A comment starting with the linter's own name is
# read as a directive, which is why this sentence is phrased around it.)
submitted_once() { [ "$(grep -cF -- 'notarytool submit' "$state/calls")" = 1 ]; }

api_key_env() {
  export APPLE_NOTARY_MODE=api_key
  export APPLE_API_ISSUER=11111111-2222-3333-4444-555555555555
  export APPLE_API_KEY=CTPB4GJ59H
  export APPLE_API_KEY_PATH="$work/AuthKey.p8"
  : >"$APPLE_API_KEY_PATH"
  unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID 2>/dev/null || true
}

password_env() {
  export APPLE_NOTARY_MODE=password
  export APPLE_ID=someone@example.com
  export APPLE_PASSWORD=abcd-efgh-ijkl-mnop
  export APPLE_TEAM_ID=TEAMID1234
  unset APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_PATH 2>/dev/null || true
}

one_dmg() { : >"$work/dmg/Juno_0.9.9_universal.dmg"; }

# --- cases ----------------------------------------------------------------

echo "notarize-dmg.sh"

# 1. Accepted on the first submit: no re-poll, stapled, re-uploaded.
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=accepted run)
[ "$rc" = 0 ] && called "stapler staple" && called "stapler validate" \
  && called "gh release upload v0.9.9" && [ "$(waits)" = 0 ]
check "accepted on submit: staples, re-uploads, never re-polls" $?
teardown

# 2. The API-key credential flags are the ones passed.
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=accepted run)
[ "$rc" = 0 ] && called "--key $work/AuthKey.p8" && called "--key-id CTPB4GJ59H" \
  && called "--issuer 11111111-2222-3333-4444-555555555555" && not_called "--apple-id"
check "api_key mode passes --key/--key-id/--issuer and no --apple-id" $?
teardown

# 3. Password mode still passes the old flags.
setup; password_env; one_dmg
rc=$(STUB_SUBMIT=accepted run)
[ "$rc" = 0 ] && called "--apple-id someone@example.com" && called "--team-id TEAMID1234" \
  && not_called "--issuer"
check "password mode passes --apple-id/--password/--team-id" $?
teardown

# 4. THE REGRESSION: upload succeeded, poll died, verdict recovered by re-poll.
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=pollfail STUB_WAIT=accepted run)
[ "$rc" = 0 ] && [ "$(waits)" = 1 ] && called "notarytool wait cbf6a189-cab9-42f4-9d9d-9062793c2449" \
  && called "stapler staple" && called "gh release upload v0.9.9"
check "lost poll + Accepted on re-poll: recovers, staples, uploads once" $?
teardown

# 5. A lost poll never resubmits the DMG.
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=pollfail STUB_WAIT=accepted run)
[ "$rc" = 0 ] && submitted_once
check "lost poll: submits exactly once (no duplicate submission)" $?
teardown

# 6. Re-poll keeps trying up to three times.
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=pollfail STUB_WAIT=fail STUB_WAIT_OK_ON=3 run)
[ "$rc" = 0 ] && [ "$(waits)" = 3 ] && called "stapler staple"
check "lost poll: recovers on the third re-poll" $?
teardown

# 7. Three failed re-polls fail the step, and nothing is stapled or uploaded.
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=pollfail STUB_WAIT=fail run)
[ "$rc" = 1 ] && [ "$(waits)" = 3 ] && not_called "stapler staple" \
  && not_called "gh release upload" && grep -q '::error::notarization failed' "$work/out"
check "lost poll, all re-polls fail: exits 1, no staple, no upload" $?
teardown

# 8. Invalid is terminal: fail fast, fetch the log, do not re-poll.
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=invalid run)
[ "$rc" = 1 ] && [ "$(waits)" = 0 ] && called "notarytool log cbf6a189-cab9-42f4-9d9d-9062793c2449" \
  && not_called "stapler staple" && grep -q 'status: Invalid' "$work/out"
check "Invalid verdict: fails fast with the notary log, no re-poll" $?
teardown

# 9. Rejected is terminal too.
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=rejected run)
[ "$rc" = 1 ] && [ "$(waits)" = 0 ] && not_called "stapler staple"
check "Rejected verdict: fails fast, no re-poll" $?
teardown

# 10. No submission id means there is nothing to re-poll.
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=pollfail_noid run)
[ "$rc" = 1 ] && [ "$(waits)" = 0 ] && not_called "stapler staple"
check "submit died before an id: no re-poll, exits 1" $?
teardown

# 11. A half-configured API-key set is named, and Apple is never contacted.
setup; api_key_env; one_dmg; unset APPLE_API_ISSUER
rc=$(STUB_SUBMIT=accepted run)
[ "$rc" = 1 ] && grep -q 'missing secret(s): APPLE_API_ISSUER' "$work/out" \
  && not_called "notarytool submit"
check "half-configured api_key set: names the missing secret, no submit" $?
teardown

# 12. Zero DMGs is an error, not a silent success.
setup; api_key_env
rc=$(STUB_SUBMIT=accepted run)
[ "$rc" = 1 ] && grep -q 'expected exactly one DMG, found 0' "$work/out" && not_called "notarytool submit"
check "zero DMGs: exits 1 with the count (bash 3.2 empty-array guard holds)" $?
teardown

# 13. Two DMGs is ambiguous and must not be guessed at.
setup; api_key_env; one_dmg; : >"$work/dmg/Juno_0.9.9_aarch64.dmg"
rc=$(STUB_SUBMIT=accepted run)
[ "$rc" = 1 ] && grep -q 'expected exactly one DMG, found 2' "$work/out" && not_called "notarytool submit"
check "two DMGs: exits 1 rather than picking one" $?
teardown

# 14. A terminal verdict ON A RE-POLL stops the loop (PR #975 review, 1).
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=pollfail STUB_WAIT=invalid run)
[ "$rc" = 1 ] && [ "$(waits)" = 1 ] && not_called "stapler staple" \
  && called "notarytool log cbf6a189-cab9-42f4-9d9d-9062793c2449" \
  && grep -q 'terminal verdict Invalid' "$work/out"
check "re-poll returns Invalid: stops after one, does not burn the backoff" $?
teardown

# 15. The re-poll and the log fetch carry the credentials, not just the submit.
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=pollfail STUB_WAIT=accepted run)
[ "$rc" = 0 ] && grep -qE -- 'notarytool wait \S+ --key .* --key-id CTPB4GJ59H --issuer ' "$state/calls"
check "re-poll passes the credentials (dropping them would not pass unnoticed)" $?
teardown

# 16. Accepted with a non-zero exit stays a failure, and does not announce
#     recovery first (PR #975 review, 3).
setup; api_key_env; one_dmg
rc=$(STUB_SUBMIT=pollfail STUB_WAIT=accepted_badrc run)
[ "$rc" = 1 ] && [ "$(waits)" = 1 ] && not_called "stapler staple" \
  && not_called "gh release upload" && ! grep -q 'Verdict recovered' "$work/out"
check "Accepted but exit non-zero: fails, with no recovery claim" $?
teardown

printf '\n%s passed, %s failed\n' "$pass" "$fail"
[ "$fail" -eq 0 ]
