#!/usr/bin/env bash
#
# Notarize the release DMG, staple the ticket into it, and replace the
# unstapled copy tauri-action uploaded to the GitHub release.
#
# Tauri notarizes and staples the .app, then wraps it in a .dmg it only
# *signs* (there is no notarize call in the bundler's dmg/mod.rs). Gatekeeper
# judges the file the person double-clicks, so the disk image has to be
# notarized and stapled on its own or it is rejected. See LAC-4196 and
# src-tauri/dmg/README.md.
#
# Lives in a script rather than inline in release-tauri.yml so the verdict and
# re-poll logic can be tested: scripts/__tests__/notarize-dmg.test.sh. That
# moves it out of actionlint's shellcheck reach, so ci.yml shellchecks this
# file explicitly, the same arrangement scripts/release-failure-report.sh has.
#
# Credentials come from the job environment. The signing step in
# release-tauri.yml writes exactly one of the two sets to $GITHUB_ENV along
# with APPLE_NOTARY_MODE saying which:
#
#   api_key   APPLE_API_ISSUER, APPLE_API_KEY, APPLE_API_KEY_PATH
#   password  APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID
#
# Also reads TAG and REPO (for the re-upload) and RUNNER_TEMP (for logs).
# NOTARY_REPOLL_DELAY_BASE exists so the tests do not sleep.

set -euo pipefail

dmg_dir="${1:-target/universal-apple-darwin/release/bundle/dmg}"
tmp="${RUNNER_TEMP:-${TMPDIR:-/tmp}}"
delay_base="${NOTARY_REPOLL_DELAY_BASE:-20}"

# The signing step exports only the secrets that exist, and the workflow step
# is gated on APPLE_SIGNING_IDENTITY alone, so a half-configured set (a revoked
# app-specific password, say) would otherwise surface as "APPLE_PASSWORD:
# unbound variable" from set -u. Name what is missing instead.
if [ "${APPLE_NOTARY_MODE:-password}" = api_key ]; then
  required="APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_PATH"
  creds=(--key "${APPLE_API_KEY_PATH:-}" --key-id "${APPLE_API_KEY:-}" --issuer "${APPLE_API_ISSUER:-}")
else
  required="APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID"
  creds=(--apple-id "${APPLE_ID:-}" --password "${APPLE_PASSWORD:-}" --team-id "${APPLE_TEAM_ID:-}")
fi

missing=""
for var in $required; do
  eval "value=\${$var:-}"
  [ -n "$value" ] || missing="$missing $var"
done
if [ -n "$missing" ]; then
  echo "::error::the build is signed but cannot be notarized; missing secret(s):$missing"
  echo "A signed, unnotarized DMG is rejected by Gatekeeper. Add the secret(s) or unset APPLE_SIGNING_IDENTITY." >&2
  exit 1
fi

shopt -s nullglob
dmgs=("$dmg_dir"/*.dmg)
if [ ${#dmgs[@]} -ne 1 ]; then
  echo "expected exactly one DMG, found ${#dmgs[@]}" >&2
  # The runner's /bin/bash is 3.2, where "${arr[@]}" on an EMPTY array is an
  # unbound variable under set -u — so the line meant to explain the failure
  # would fail instead, taking the diagnostic with it. Same guard as
  # scripts/tauri-build.sh.
  printf '%s\n' ${dmgs[@]+"${dmgs[@]}"} >&2
  exit 1
fi
dmg="${dmgs[0]}"
echo "Notarizing $(basename "$dmg")"

log="$tmp/notary.log"
set +e
xcrun notarytool submit "$dmg" "${creds[@]}" \
  --wait --timeout 30m 2>&1 | tee "$log"
rc=${PIPESTATUS[0]}
set -e

# notarytool has been known to exit 0 on an Invalid verdict, so the status line
# is checked as well as the exit code. Same guard as scripts/tauri-build.sh,
# which has notarized the local build correctly since #560.
status="$(sed -n 's/^ *status: *//p' "$log" | tail -1)"
# Read the submission id from the SUBMIT log and keep it: the re-poll below
# writes its own log, and the id is the only handle on work Apple already has.
id="$(sed -n 's/^ *id: *//p' "$log" | head -1)"

# The upload can succeed while the POLL dies. A runner network blip
# (NSURLErrorDomain -1009, "no network route") during `--wait` takes the step
# down even though Apple has the submission and goes on judging it: the
# v0.8.276 release failed here at 2026-11-11T07:37Z and Apple Accepted that
# very submission anyway, so a 50-minute build and a notarized DMG were thrown
# away by a few seconds of lost DNS.
#
# Re-attach by id rather than resubmitting. `notarytool wait` polls an existing
# submission, so recovery costs seconds, never uploads the DMG a second time,
# and cannot open a duplicate submission. Its output ends in the same
# "  status: <verdict>" line `submit --wait` prints, which is why one parse
# serves both (verified against submission cbf6a189 on 2026-11-11).
#
# Only a MISSING verdict is re-polled. Accepted, Invalid and Rejected are
# terminal, and re-polling a genuine rejection would just delay the error that
# names the unsigned nested binary.
case "${status:-}" in
  Accepted | Invalid | Rejected) ;;
  *)
    if [ -n "$id" ]; then
      waitlog="$tmp/notary-wait.log"
      for attempt in 1 2 3; do
        echo "Submission $id returned no verdict (status: ${status:-none}, exit $rc); re-polling, attempt $attempt of 3."
        sleep $((attempt * delay_base))
        set +e
        xcrun notarytool wait "$id" "${creds[@]}" --timeout 30m 2>&1 | tee "$waitlog"
        rc=${PIPESTATUS[0]}
        set -e
        status="$(sed -n 's/^ *status: *//p' "$waitlog" | tail -1)"
        if [ "$status" = "Accepted" ]; then
          echo "Verdict recovered by re-poll $attempt: Accepted."
          break
        fi
      done
    fi
    ;;
esac

if [ "$rc" -ne 0 ] || [ "$status" != "Accepted" ]; then
  echo "::error::notarization failed (status: ${status:-none}, exit $rc)"
  # "Invalid" alone says nothing; the log names the nested binary that was
  # unsigned or missing the hardened runtime.
  if [ -n "$id" ]; then
    xcrun notarytool log "$id" "${creds[@]}" || true
  fi
  exit 1
fi

xcrun stapler staple "$dmg"
xcrun stapler validate "$dmg"

# Replace the unstapled copy tauri-action uploaded. This touches only the .dmg;
# the updater reads .app.tar.gz + latest.json and is unaffected.
gh release upload "$TAG" "$dmg" --repo "$REPO" --clobber
echo "Stapled and re-uploaded $(basename "$dmg")."
