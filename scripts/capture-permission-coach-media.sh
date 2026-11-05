#!/bin/bash
# capture-permission-coach-media.sh: the attended capture for the permission
# coaching card (LAC-4195, PR #815).
#
# The card only draws while the permission is OFF, and macOS will not let
# software flip a privacy switch. So this is the one capture that needs a
# person. Their part is three clicks; everything either side of it is in here.
#
#   1. flip Juno's Accessibility switch OFF
#   2. press Grant in Juno                  (the card appears beside Settings)
#   3. flip the switch ON                   (the card says so, then leaves)
#
#   scripts/capture-permission-coach-media.sh [pr-number]
#
# Everything that can fail is checked BEFORE anyone is asked to do anything,
# because a capture that dies after the switch is already off costs the whole
# sitting and leaves Juno without the permission it needs. Screen Recording is
# granted per responsible app, so the terminal running this needs its own
# grant even though the desktop session plainly has one - and a missing grant
# and a locked screen produce the identical `screencapture` error. That is why
# the preflight writes a real frame and measures it instead of trusting an
# exit code.
set -euo pipefail

PR="${1:-815}"
REPO="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
OUT="$REPO/docs/changelog/media/$PR"
PANE="x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
RECORD_SECONDS=22

pause() { printf '\n  >> %s\n     press return when done: ' "$1"; read -r _; }
die() { printf '\n  !! %s\n' "$1" >&2; exit 1; }

# ── preflight: tools ─────────────────────────────────────────────────────────
for tool in ffmpeg screencapture osascript; do
	command -v "$tool" >/dev/null 2>&1 || die "$tool is not installed; cannot capture."
done
[ -x "$REPO/scripts/juno-build.sh" ] || die "scripts/juno-build.sh is missing or not executable."

# ── preflight: can this terminal actually record the screen? ─────────────────
# Take a real frame and measure it. A denied grant and a locked screen both
# fail here, which is the point: either way there is no capture to be had.
# One directory, one cleanup: `mktemp -t X` plus an appended ".png" would leave
# the mktemp'd file behind every run, and screencapture needs the extension.
probe_dir="$(mktemp -d -t juno-screen-probe)"
trap 'rm -rf "$probe_dir"' EXIT
probe="$probe_dir/frame.png"
if ! screencapture -x "$probe" 2>/dev/null || [ ! -s "$probe" ] || [ "$(stat -f%z "$probe")" -lt 10000 ]; then
	die "screencapture produced no usable frame.
     Grant Screen Recording to THIS terminal app (System Settings ->
     Privacy & Security -> Screen Recording), then run this again. If the
     screen is locked, unlock it - the two failures look identical."
fi
echo "== Preflight OK: tools present, screen is recordable =="

mkdir -p "$OUT"

# ── 0. the build has to be the one with the feature in it ────────────────────
echo
echo "== Installing this branch's build (replaces /Applications/Juno.app) =="
echo "   To go back to a release afterwards:  scripts/juno-build.sh <version>"
printf '   install now? [y/N] '
read -r ok
[[ "$ok" == y || "$ok" == Y ]] || die "stopping, nothing changed"
(cd "$REPO" && scripts/juno-build.sh)

# ── 1. put the switch on screen ──────────────────────────────────────────────
open "$PANE"
sleep 2
pause "Flip Juno's Accessibility switch OFF, and leave this pane open"

# ── 2. card up, still photo ──────────────────────────────────────────────────
pause "In Juno, press Grant on Accessibility. The card appears beside Settings"
sleep 1
screencapture -x "$OUT/accessibility-card.png"
[ -s "$OUT/accessibility-card.png" ] || die "the still came out empty; nothing was written."
echo "   still: $OUT/accessibility-card.png"

# ── 3. the moving part: the flip, the receipt, and the card leaving ──────────
echo
echo "== Recording ${RECORD_SECONDS}s. Flip the switch ON a few seconds in, then watch. =="
sleep 1
screencapture -v -V "$RECORD_SECONDS" "$OUT/raw.mov"
[ -s "$OUT/raw.mov" ] || die "the recording came out empty; nothing was written."

# ── 4. encode small, and only then drop the raw ──────────────────────────────
ffmpeg -y -loglevel error -i "$OUT/raw.mov" \
	-vf "scale=iw/2:-2" -r 24 -an -c:v libx264 -crf 30 -pix_fmt yuv420p -movflags +faststart \
	"$OUT/accessibility-grant.mp4"
[ -s "$OUT/accessibility-grant.mp4" ] || die "encode failed; keeping $OUT/raw.mov so the take is not lost."
rm -f "$OUT/raw.mov"

echo
echo "== Done =="
du -h "$OUT"/*
echo
echo "Next: look at them. If they are honest, commit:"
echo "  cd $REPO && git add docs/changelog/media/$PR && git commit -m 'docs(permissions): media for the coaching card (LAC-4195)' && git push"
