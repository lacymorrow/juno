#!/usr/bin/env python3
"""Crop a full-screen capture down to one Finder window.

    BOUNDS="l, t, r, b" DESKTOP_W="1440" crop-window.py <screen.png> <out.png>

BOUNDS is what `Finder -> get bounds of window 1` returns, in points.
DESKTOP_W is the desktop's width in points. The capture is in pixels, so the
ratio between the two is the display's backing scale (2 on a Retina Mac, 1 on
a CI runner).

Used by .github/workflows/dmg-preview.yml. Kept out of the workflow because an
embedded heredoc is harder to read and impossible to run by hand.
"""
import os
import sys

from PIL import Image

PAD = 12  # a little desktop around the window, so the edges are visible


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    src, dst = sys.argv[1], sys.argv[2]

    raw = os.environ.get("BOUNDS", "").strip()
    desktop_w = os.environ.get("DESKTOP_W", "").strip()
    if not raw or not desktop_w:
        print("no BOUNDS/DESKTOP_W; keeping the full-screen capture", file=sys.stderr)
        return 0

    try:
        left, top, right, bottom = (int(v.strip()) for v in raw.split(","))
        scale = Image.open(src).width / int(desktop_w)
    except (ValueError, ZeroDivisionError) as exc:
        print(f"cannot parse geometry ({exc}); keeping the full capture", file=sys.stderr)
        return 0

    im = Image.open(src)
    box = (
        max(0, int((left - PAD) * scale)),
        max(0, int((top - PAD) * scale)),
        min(im.width, int((right + PAD) * scale)),
        min(im.height, int((bottom + PAD) * scale)),
    )
    if box[2] <= box[0] or box[3] <= box[1]:
        print(f"degenerate crop {box}; keeping the full capture", file=sys.stderr)
        return 0

    im.crop(box).save(dst)
    print(f"cropped {box} at scale {scale:g} -> {dst}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
