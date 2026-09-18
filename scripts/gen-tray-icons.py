#!/usr/bin/env python3
"""Render the Juno menu bar (tray) icon set.

The tray icon is a five-bar waveform. macOS paints it as a template image, so
every frame is black with an alpha channel and the system picks the tint for
the light or dark menu bar. State lives in the shape, never in color.

The tray-icon crate always displays the image 18 pt tall, so each frame is a
single 36 x 36 px PNG (2x). Geometry is authored in a 22-unit box, the same
space as the option sheet, and scaled to fit.

    python3 scripts/gen-tray-icons.py

Writes src-tauri/icons/tray/waveform/*.png and a contact sheet next to it.
Needs Pillow.
"""

from __future__ import annotations

import math
from pathlib import Path

from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parents[1] / "src-tauri" / "icons" / "tray" / "waveform"
PX = 36  # output size in pixels (18 pt at 2x)
SS = 8  # supersample factor
UNIT = 22.0  # authoring box
S = PX * SS / UNIT  # supersampled pixels per unit
INK = (0, 0, 0)

BAR_X = [2.4 + i * 4.05 for i in range(5)]
BAR_W = 2.3


class Canvas:
    def __init__(self) -> None:
        self.im = Image.new("RGBA", (PX * SS, PX * SS), (0, 0, 0, 0))
        self.d = ImageDraw.Draw(self.im)

    def _fill(self, alpha: float):
        return (*INK, int(round(255 * alpha)))

    def bar(self, x: float, h: float, alpha: float = 1.0) -> None:
        if h <= 0:
            return
        y = 11 - h / 2
        self.d.rounded_rectangle(
            [x * S, y * S, (x + BAR_W) * S, (y + h) * S],
            radius=BAR_W / 2 * S,
            fill=self._fill(alpha),
        )

    def dot(self, cx: float, cy: float, r: float, alpha: float = 1.0) -> None:
        self.d.ellipse(
            [(cx - r) * S, (cy - r) * S, (cx + r) * S, (cy + r) * S],
            fill=self._fill(alpha),
        )

    def stroke(self, pts: list[tuple[float, float]], w: float, alpha: float = 1.0) -> None:
        """Polyline with round caps and round joins."""
        fill = self._fill(alpha)
        scaled = [(x * S, y * S) for x, y in pts]
        self.d.line(scaled, fill=fill, width=int(round(w * S)), joint="curve")
        for x, y in pts:
            self.dot(x, y, w / 2, alpha)

    def bars(self, heights: list[float], alpha: float = 1.0) -> None:
        for x, h in zip(BAR_X, heights):
            self.bar(x, h, alpha)

    def bang(self, cx: float = 11, cy: float = 11, h: float = 9, w: float = 2.3) -> None:
        top = cy - h / 2
        self.stroke([(cx, top), (cx, top + h * 0.58)], w)
        self.dot(cx, cy + h / 2 - 0.2, w * 0.6)

    def render(self) -> Image.Image:
        return self.im.resize((PX, PX), Image.LANCZOS)


IDLE = [3, 5.5, 8, 5.5, 3]
ARMED = [5, 9, 13, 9, 5]
REC_BASE = [8, 14, 17, 12, 7]
REC_PHASE = [0.0, 0.25, 0.5, 0.2, 0.4]


def frame_idle() -> Image.Image:
    c = Canvas()
    c.bars(IDLE)
    return c.render()


def frame_armed() -> Image.Image:
    c = Canvas()
    c.bars(ARMED)
    return c.render()


def frame_recording(i: int, n: int) -> Image.Image:
    t = i / n
    c = Canvas()
    heights = []
    for base, phase in zip(REC_BASE, REC_PHASE):
        s = 0.5 + 0.5 * math.sin(2 * math.pi * (t + phase))
        heights.append(base * (0.45 + 0.55 * s))
    c.bars(heights)
    return c.render()


def frame_transcribing(i: int, n: int) -> Image.Image:
    c = Canvas()
    for k, cx in enumerate((6.0, 11.0, 16.0)):
        phase = (i / n - k / 3) % 1.0
        alpha = 0.3 + 0.7 * (0.5 + 0.5 * math.cos(2 * math.pi * phase))
        c.dot(cx, 11, 1.7, alpha)
    return c.render()


def frame_agent(cursor_on: bool) -> Image.Image:
    c = Canvas()
    c.stroke([(5, 6.5), (10, 11), (5, 15.5)], 2.1)
    if cursor_on:
        c.stroke([(12.5, 15.5), (17.5, 15.5)], 2.1)
    return c.render()


def frame_error() -> Image.Image:
    c = Canvas()
    c.bars([3, 5.5, 0, 5.5, 3])
    c.bang()
    return c.render()


def frame_paused() -> Image.Image:
    c = Canvas()
    c.bars(IDLE, alpha=0.42)
    return c.render()


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    frames: list[tuple[str, Image.Image]] = [
        ("idle", frame_idle()),
        ("armed", frame_armed()),
    ]
    frames += [(f"recording-{i}", frame_recording(i, 8)) for i in range(8)]
    frames += [(f"transcribing-{i}", frame_transcribing(i, 6)) for i in range(6)]
    frames += [("agent-0", frame_agent(True)), ("agent-1", frame_agent(False))]
    frames += [("error", frame_error()), ("paused", frame_paused())]

    for name, im in frames:
        im.save(OUT / f"{name}.png", optimize=True)

    # Contact sheet: every frame on a dark and a light bar, 2x, for review.
    cell, pad = PX + 12, 8
    cols = len(frames)
    sheet = Image.new("RGBA", (cols * cell + pad * 2, 2 * cell + pad * 3), (255, 255, 255, 255))
    for row, (bg, tint) in enumerate((((43, 43, 47), (255, 255, 255)), ((230, 230, 234), (0, 0, 0)))):
        y0 = pad + row * (cell + pad)
        ImageDraw.Draw(sheet).rounded_rectangle(
            [pad, y0, pad + cols * cell, y0 + cell], radius=6, fill=(*bg, 255)
        )
        for col, (_, im) in enumerate(frames):
            tinted = Image.new("RGBA", im.size, (*tint, 0))
            tinted.putalpha(im.getchannel("A"))
            sheet.alpha_composite(tinted, (pad + col * cell + 6, y0 + 6))
    sheet.save(OUT / "contact-sheet.png")
    print(f"wrote {len(frames)} frames to {OUT}")


if __name__ == "__main__":
    main()
