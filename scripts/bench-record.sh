#!/bin/bash
# bench-record.sh: record one bar appearance from the preview route as an MP4.
#
# The settings picker and the docs use still frames; a release post wants
# motion. This records `/__bar-preview` (the real bar components on the fake
# Tauri layer, no backend, no real window) through agent-browser and converts
# the WebM to an H.264 MP4 that X, Slack and QuickTime all play.
#
#   scripts/bench-record.sh <appearance> [mode] [seconds] [out.mp4]
#
#   appearance  floating | app | voice_ai | dynamic | orb | react_orb | persona
#   mode        loop (default: the picker's resting/listening/dictating/done loop)
#               card (one full turn: question, answer with a component, ring)
#               spoken (one spoken turn: your words, a tool step, the answer said sentence by sentence)
#               state=<bar state> (hold one frame, for a still in motion)
#               demo=<name> (any demo the preview route knows)
#   seconds     how long to record (default 10)
#   out.mp4     default docs/frontend/screenshots/<appearance>-<mode>.mp4
#
# Needs `bun run dev` on :1420, agent-browser and ffmpeg. A PNG poster of the
# last second lands next to the MP4 so a PR can show a frame without playing it.
# BENCH_BG sets the page colour (URL-encoded; default the picker's stage grey).
# BENCH_QUERY appends extra preview params (e.g. "&pin=frame"); BENCH_VIEWPORT
# sets the stage size as "<w> <h>" (default 480 400).
set -euo pipefail

APPEARANCE="${1:-dynamic}"
MODE="${2:-loop}"
SECONDS_TO_RECORD="${3:-10}"
OUT="${4:-}"

BASE="${BENCH_URL:-http://localhost:1420}"
case "$MODE" in
  loop) QUERY="";;
  card) QUERY="&demo=card";;
  spoken) QUERY="&demo=spoken";;
  state=*) QUERY="&state=${MODE#state=}";;
  demo=*) QUERY="&demo=${MODE#demo=}";;
  *) echo "bench-record: unknown mode '$MODE' (loop | card | spoken | state=<bar state> | demo=<name>)" >&2; exit 2;;
esac
URL="$BASE/__bar-preview?appearance=$APPEARANCE$QUERY${BENCH_QUERY:-}"

if [ -z "$OUT" ]; then
  SAFE_MODE="${MODE//=/-}"
  OUT="docs/frontend/screenshots/$APPEARANCE-$SAFE_MODE.mp4"
fi
OUT="$(cd "$(dirname "$OUT")" && pwd)/$(basename "$OUT")"
WEBM="${OUT%.mp4}.webm"
POSTER="${OUT%.mp4}.png"

command -v agent-browser >/dev/null || { echo "bench-record: agent-browser not found" >&2; exit 2; }
command -v ffmpeg >/dev/null || { echo "bench-record: ffmpeg not found" >&2; exit 2; }
curl -sf "$BASE/" >/dev/null || { echo "bench-record: nothing answers at $BASE (run: bun run dev)" >&2; exit 2; }

# Open first and wait until the bar has painted, then start the recording and
# only then start the script: the clip begins on the first beat, not on the
# blank page a cold dev server serves for a second or two.
# shellcheck disable=SC2086
agent-browser set viewport ${BENCH_VIEWPORT:-480 400} >/dev/null
agent-browser open "$URL&start=manual&bg=${BENCH_BG:-%23E9E9EB}" >/dev/null
agent-browser wait '[data-preview-ready="true"]' >/dev/null
agent-browser wait 800 >/dev/null
agent-browser record start "$WEBM" >/dev/null
agent-browser eval "window.__junoBenchStart()" >/dev/null
agent-browser wait "$((SECONDS_TO_RECORD * 1000))" >/dev/null
agent-browser record stop >/dev/null

# H.264, yuv420p and even dimensions: what every player and X's uploader want.
ffmpeg -y -v error -i "$WEBM" \
  -vf "scale=trunc(iw/2)*2:trunc(ih/2)*2,format=yuv420p" \
  -c:v libx264 -preset slow -crf 20 -movflags +faststart -an "$OUT"
ffmpeg -y -v error -sseof -1 -i "$OUT" -frames:v 1 "$POSTER"
rm -f "$WEBM"

echo "$OUT"
echo "$POSTER"
