#!/usr/bin/env bash
#
# Verify a built Juno .dmg is the thing a stranger can actually open.
#
#   scripts/verify-dmg.sh path/to/Juno_x.y.z_universal.dmg
#
# Every expectation is read from src-tauri/tauri.conf.json, so this script and
# the bundle config cannot drift apart.
#
# It exists because both of the defects this guards against are INVISIBLE to
# anyone building locally:
#
#   1. Gatekeeper. Tauri notarizes and staples the .app, then wraps it in a
#      .dmg it only signs (crates/tauri-bundler/.../dmg/mod.rs calls
#      sign::sign and never notarizes). A signed-but-unnotarized disk image is
#      REJECTED by Gatekeeper, so the download is blocked before the notarised
#      app inside is ever reachable. Releases v0.8.x up to and including
#      v0.8.201 shipped that way.
#
#   2. The window layout. tauri-bundler passes --skip-jenkins to bundle_dmg.sh
#      whenever CI=true unless TAURI_BUNDLER_DMG_IGNORE_CI=true is set, and
#      --skip-jenkins skips the AppleScript that applies the background and the
#      icon positions. The DMG still builds. It just silently comes out stock.
#
# Neither reproduces on a developer's Mac. Both are one assertion away.
set -euo pipefail

skip_gatekeeper=0
dmg=""
while [ $# -gt 0 ]; do
  case "$1" in
    # For a PR check, which builds unsigned: assert the window layout only.
    # Signing and notarization are asserted on the release build, where the
    # Apple credentials actually exist.
    --skip-gatekeeper) skip_gatekeeper=1 ;;
    -*) echo "unknown option: $1" >&2; exit 2 ;;
    *) dmg="$1" ;;
  esac
  shift
done

if [ -z "$dmg" ] || [ ! -f "$dmg" ]; then
  echo "usage: $0 [--skip-gatekeeper] <path-to-dmg>" >&2
  exit 2
fi
dmg="$(cd "$(dirname "$dmg")" && pwd)/$(basename "$dmg")"

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
conf="$repo_root/src-tauri/tauri.conf.json"

# Finder's title bar on a disk-image window with the toolbar hidden. Measured
# on macOS 26 against a real built DMG: outer 660x432 -> content 660x400.
# verified below, so a macOS change surfaces here rather than as a gap along
# the bottom edge of the window.
TITLE_BAR_PT=32

fail=0
note() { printf '  %s\n' "$*"; }
ok()   { printf '\033[32m  ok\033[0m   %s\n' "$*"; }
bad()  { printf '\033[31m  FAIL\033[0m %s\n' "$*"; fail=1; }

# ---------------------------------------------------------------- expectations
# Assigned first and eval'd second on purpose: `eval "$(cmd)"` throws away
# cmd's exit status, so a config without a dmg block would eval nothing and
# then die on "EXP_BG: unbound variable" — naming neither the file nor the
# cause.
expectations="$(python3 - "$conf" <<'PY'
import json, sys, shlex
d = json.load(open(sys.argv[1]))
try:
    dmg = d["bundle"]["macOS"]["dmg"]
except KeyError:
    sys.exit("tauri.conf.json has no bundle.macOS.dmg block: "
             "there is no configured layout to verify")
out = {
    "EXP_BG": dmg["background"],
    "EXP_W": dmg["windowSize"]["width"],
    "EXP_H": dmg["windowSize"]["height"],
    "EXP_APP_X": dmg["appPosition"]["x"],
    "EXP_APP_Y": dmg["appPosition"]["y"],
    "EXP_FLD_X": dmg["applicationFolderPosition"]["x"],
    "EXP_FLD_Y": dmg["applicationFolderPosition"]["y"],
    "EXP_NAME": d["productName"],
}
for k, v in out.items():
    print(f"{k}={shlex.quote(str(v))}")
PY
)" || exit 1
eval "$expectations"
bg_name="$(basename "$EXP_BG")"

echo "Verifying $(basename "$dmg")"
echo "  against $EXP_W x $EXP_H window, icons at ($EXP_APP_X,$EXP_APP_Y) and ($EXP_FLD_X,$EXP_FLD_Y)"
echo

# ------------------------------------------------------------------ gatekeeper
if [ "$skip_gatekeeper" -eq 1 ]; then
  echo "Gatekeeper: skipped (--skip-gatekeeper); layout only"
  echo
else
echo "Gatekeeper (what a downloaded copy is judged on)"
if codesign -dv "$dmg" >/dev/null 2>&1; then
  auth="$(codesign -dv --verbose=2 "$dmg" 2>&1 | sed -n 's/^Authority=//p' | head -1)"
  ok "disk image is signed: $auth"
else
  bad "disk image is not signed"
fi

if xcrun stapler validate "$dmg" >/dev/null 2>&1; then
  ok "notarization ticket is stapled to the disk image"
else
  bad "no stapled notarization ticket on the disk image"
  note "Tauri does not notarize DMGs. release-tauri.yml must do it:"
  note "  xcrun notarytool submit --wait && xcrun stapler staple"
fi

# This is the exact assessment Gatekeeper performs when the file carries a
# quarantine flag, which is what a browser download has.
if assess="$(spctl -a -t open --context context:primary-signature "$dmg" 2>&1)"; then
  ok "spctl accepts the disk image"
else
  bad "spctl rejects the disk image: $(echo "$assess" | sed -n '2p')"
  note "a stranger sees \"Apple cannot check it for malicious software\""
fi
fi
echo

# ----------------------------------------------------------------------- mount
mnt="$(mktemp -d)/mnt"
mkdir -p "$mnt"
cleanup() { hdiutil detach "$mnt" >/dev/null 2>&1 || true; }
trap cleanup EXIT
hdiutil attach "$dmg" -readonly -nobrowse -mountpoint "$mnt" >/dev/null

echo "Contents"
[ -L "$mnt/Applications" ] && ok "Applications symlink present" || bad "no /Applications symlink to drag onto"
[ -d "$mnt/$EXP_NAME.app" ] && ok "$EXP_NAME.app present" || bad "no $EXP_NAME.app"

if [ -f "$mnt/.background/$bg_name" ]; then
  ok "background embedded as .background/$bg_name"
else
  bad "no .background/$bg_name — the window is stock"
  note "set TAURI_BUNDLER_DMG_IGNORE_CI=true in the release workflow"
fi
[ -f "$mnt/.DS_Store" ] && ok ".DS_Store written (Finder layout saved)" || bad "no .DS_Store — no layout was applied"
echo

# ----------------------------------------- background resolution and point size
if [ -f "$mnt/.background/$bg_name" ]; then
  echo "Background"
  logical="$(osascript -l JavaScript -e "
    ObjC.import('AppKit');
    var i = \$.NSImage.alloc.initWithContentsOfFile('$mnt/.background/$bg_name');
    i.isNil() ? 'nil' : i.size.width + 'x' + i.size.height;
  " 2>/dev/null)"
  pixels="$(sips -g pixelWidth -g pixelHeight "$mnt/.background/$bg_name" 2>/dev/null \
            | awk '/pixel(Width|Height)/ {print $2}' | paste -sd x -)"
  want="${EXP_W}x$((EXP_H - TITLE_BAR_PT))"
  if [ "$logical" = "$want" ]; then
    ok "logical size ${logical}pt fills the content area (pixels ${pixels})"
  else
    bad "logical size ${logical}pt, expected ${want}pt (pixels ${pixels})"
    note "an '@2x' suffix is what halves a 2x file's logical size; do not rename it"
  fi
  echo
fi

# ------------------------------------------------- icon positions from .DS_Store
echo "Icon layout (.DS_Store)"
if [ ! -f "$mnt/.DS_Store" ]; then
  bad "cannot read icon positions: no .DS_Store (see above)"
else
python3 - "$mnt/.DS_Store" "$EXP_NAME.app" "$EXP_APP_X" "$EXP_APP_Y" "$EXP_FLD_X" "$EXP_FLD_Y" <<'PY' || fail=1
import struct, sys

path, app_name, ax, ay, fx, fy = sys.argv[1], sys.argv[2], *map(int, sys.argv[3:7])
blob = open(path, 'rb').read()

# .DS_Store records are: name length (4, BE) + name (UTF-16BE) + 4-byte
# structure id + 4-byte type + payload. Rather than walk the Buddy allocator's
# B-tree, find each 'Iloc' structure id and read the filename backwards from
# it; an Iloc payload is a 16-byte blob whose first two BE longs are x and y.
#
# A linear scan is safe here only because this file is one create-dmg just
# wrote for a handful of items — a freshly built image has four Iloc records
# and no stale ones in the allocator's free space. It is not a general
# .DS_Store parser, so rather than trust that assumption silently, a duplicate
# or unreadable record is reported as a failure instead of being resolved by
# guessing which copy is current.
found = {}
seen = {}
pos = 0
while True:
    pos = blob.find(b'Iloc', pos)
    if pos < 0:
        break
    start = pos + 4
    if blob[start:start + 4] != b'blob':
        pos += 4
        continue
    (length,) = struct.unpack('>I', blob[start + 4:start + 8])
    if length >= 8:
        x, y = struct.unpack('>II', blob[start + 8:start + 16])
        # Recover the name: try every plausible UTF-16BE length ending at pos.
        for n in range(1, 65):
            name_end = pos
            name_start = name_end - 2 * n
            if name_start - 4 < 0:
                continue
            if struct.unpack('>I', blob[name_start - 4:name_start])[0] != n:
                continue
            try:
                name = blob[name_start:name_end].decode('utf-16-be')
            except UnicodeDecodeError:
                break
            seen[name] = seen.get(name, 0) + 1
            found[name] = (x, y)
            break
    pos += 4

rc = 0
for name, want in ((app_name, (ax, ay)), ('Applications', (fx, fy))):
    if seen.get(name, 0) > 1:
        # Two records for one item means the scan is reading stale allocator
        # space, and picking either one would be a guess. Say so rather than
        # pass or fail the release on a coin toss.
        print(f'\033[31m  FAIL\033[0m {seen[name]} icon positions recorded for '
              f'{name!r}; cannot tell which is current')
        rc = 1
        continue
    got = found.get(name)
    if got is None:
        print(f'\033[31m  FAIL\033[0m no icon position recorded for {name!r}')
        rc = 1
    elif got != want:
        print(f'\033[31m  FAIL\033[0m {name} at {got[0]},{got[1]} — expected {want[0]},{want[1]}')
        rc = 1
    else:
        print(f'\033[32m  ok\033[0m   {name} at {got[0]},{got[1]}')
sys.exit(rc)
PY
fi

echo
if [ "$fail" -ne 0 ]; then
  echo "FAILED — this disk image is not ready to put in front of a stranger."
  exit 1
fi
echo "All checks passed."
