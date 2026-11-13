#!/bin/bash
# usage: capture-state.sh <app-binary> <signedin|signedout|notinstalled> <outdir>
# Launches a real Juno build under a throwaway HOME, captures its window only
# (screencapture -l, so no desktop content), and fails if the capture is blank.
set -euo pipefail
BIN=$1; STATE=$2; OUT=$3
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"
H=$(mktemp -d "${TMPDIR:-/tmp}/juno-smoke-home.XXXXXX")
D="$H/Library/Application Support/com.juno.desktop"
mkdir -p "$H/.local/bin" "$H/.codex" "$D"
if [ "$STATE" != notinstalled ]; then
  printf '#!/bin/sh\nexit 0\n' > "$H/.local/bin/codex"; chmod +x "$H/.local/bin/codex"
fi
[ "$STATE" = signedin ] && echo '{"auth_mode":"chatgpt"}' > "$H/.codex/auth.json"
echo '{"last_phase":"api-key"}' > "$D/onboarding_analytics.json"

(cd "$H" && HOME="$H" PATH=/usr/bin:/bin nohup "$BIN" >"$OUT/run-$STATE.log" 2>&1 & echo $! > "$H/pid")
PID=$(cat "$H/pid")
trap 'kill "$PID" 2>/dev/null || true; rm -rf "$H"' EXIT

# Poll for the window instead of sleeping a fixed time.
WIN=""
for _ in $(seq 1 40); do
  kill -0 "$PID" 2>/dev/null || { echo "FAIL: app exited early (see run-$STATE.log)"; exit 1; }
  WIN=$(swift "$HERE/find-window.swift" "$PID" 2>/dev/null || true)
  [ -n "$WIN" ] && break
  sleep 1
done
[ -n "$WIN" ] || { echo "FAIL: no window found for pid $PID"; exit 1; }
sleep 3  # let the first paint settle
PNG="$OUT/$STATE.png"
screencapture -x -o -l "$WIN" "$PNG" || { echo "FAIL: screencapture exited non-zero"; exit 1; }
echo "state=$STATE pid=$PID win=$WIN"
swift "$HERE/check-png.swift" "$PNG"
