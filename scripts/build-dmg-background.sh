#!/usr/bin/env bash
#
# Rasterise src-tauri/dmg/background.svg -> src-tauri/dmg/background@2x.png.
#
# Design-time only. CI never runs this: the PNG is committed, because the DMG
# bundler reads the PNG and nothing in the release path should depend on a
# browser being installed.
#
# Why a PNG named "@2x" and not a plain one:
#   Finder loads a disk-image background through AppKit, which honours the
#   "@2x" filename suffix and halves the image's logical size. So a 1320x800
#   file lands as 660x400 POINTS and stays sharp on a Retina display. Drop the
#   suffix and Finder would draw it at 1320x800 points and you would see the
#   top-left quarter. Exodus ships the same trick (image@2x.jpg, 1280x960 in a
#   640pt window), and scripts/verify-dmg.sh asserts the logical size so a
#   rename cannot break it silently.
#
# Why 400 points tall when tauri.conf.json says windowSize.height = 422:
#   create-dmg sets the Finder window's *outer* bounds, which include the 22pt
#   title bar. The background fills the content area underneath it.
#   422 - 22 = 400. verify-dmg.sh checks this against a real built DMG.
set -euo pipefail

cd "$(dirname "$0")/.."

src="src-tauri/dmg/background.svg"
out="src-tauri/dmg/background@2x.png"
width=660
height=400

[ -f "$src" ] || { echo "missing $src" >&2; exit 1; }

chrome="${CHROME:-}"
if [ -z "$chrome" ]; then
  for candidate in \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    "/Applications/Chromium.app/Contents/MacOS/Chromium" \
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"; do
    [ -x "$candidate" ] && { chrome="$candidate"; break; }
  done
fi
if [ -z "$chrome" ]; then
  echo "No Chromium-based browser found to rasterise the SVG." >&2
  echo "Install Google Chrome, or set CHROME=/path/to/browser." >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# --force-device-scale-factor=2 renders a 660x400 viewport at 1320x800 px.
"$chrome" \
  --headless \
  --disable-gpu \
  --hide-scrollbars \
  --user-data-dir="$tmp/profile" \
  --screenshot="$tmp/shot.png" \
  --window-size="${width},${height}" \
  --force-device-scale-factor=2 \
  "file://$PWD/$src" >/dev/null 2>&1

[ -f "$tmp/shot.png" ] || { echo "browser produced no screenshot" >&2; exit 1; }

got="$(sips -g pixelWidth -g pixelHeight "$tmp/shot.png" | awk '/pixel(Width|Height)/ {print $2}' | paste -sd x -)"
want="$((width * 2))x$((height * 2))"
if [ "$got" != "$want" ]; then
  echo "rasterised to $got, expected $want" >&2
  exit 1
fi

mkdir -p "$(dirname "$out")"
cp "$tmp/shot.png" "$out"
echo "wrote $out ($got)"
