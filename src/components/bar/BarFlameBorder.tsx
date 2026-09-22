/**
 * BarFlameBorder — the floating bar's activity indicator: a dialled-down
 * canvas-ui flame that licks around the pill's border. A quick flash when
 * dictation / transcription starts, a steady low ember while the agent works.
 *
 * It is mounted ONLY while active (a flash or a working turn), never on the idle
 * bar, so the WebGL loop costs nothing at rest. It sits absolutely behind the
 * pill's content and licks OUTWARD past the opaque pill, so it reads as a
 * glowing border rather than tinting the pill's face.
 *
 * The colour and intensity are animated, not switched: the border can mean
 * different things at different moments (a session's own colour while it works,
 * a bright flash at capture), and it eases between them instead of jumping.
 *
 * Tuning lives here and in the caller: the fixed flame shape is the caller's
 * chosen preset (low intensity, short reach); `color` and `intensity` are driven
 * by bar state. The component degrades safely: canvas-ui paints the flame on its
 * overlay canvas even where the experimental html-in-canvas API is missing (as
 * in the macOS webview), and the empty child means there is no content to melt.
 */

import { useEffect, useRef, useReducer } from "react";
import { FlameWrap } from "@/components/canvasui/FlameWrap";

type RGB = [number, number, number];

/** #RRGGBB / #RGB -> [r,g,b] in 0..1. Falls back to system blue on a bad value. */
function hexToRgb(hex: string): RGB {
  const h = (hex || "").replace("#", "");
  const full = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  const n = Number.parseInt(full, 16);
  if (full.length !== 6 || Number.isNaN(n)) return [10 / 255, 132 / 255, 255 / 255];
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

/** Per-frame ease toward the target (~350ms to settle at 60fps). */
const APPROACH = 0.16;
const EPS = 0.002;
const approach = (a: number, b: number) => a + (b - a) * APPROACH;

export interface BarFlameBorderProps {
  /** Target flame colour as a hex string; eased toward, not switched. */
  color: string;
  /** Target flame intensity (0..3); eased toward. Flash is high, steady is low. */
  intensity: number;
  /** Corner radius of the burning outline, in CSS px — match the pill. */
  radius: number;
}

export function BarFlameBorder({ color, intensity, radius }: BarFlameBorderProps) {
  // Live, eased values the flame actually renders with (kept in a ref so the
  // animation loop never stales on a captured value).
  const cur = useRef<{ c: RGB; i: number }>({ c: hexToRgb(color), i: intensity });
  const [, rerender] = useReducer((x: number) => x + 1, 0);

  useEffect(() => {
    let running = true;
    let raf = 0;
    const tick = () => {
      if (!running) return;
      const tc = hexToRgb(color);
      const c = cur.current;
      const nc: RGB = [approach(c.c[0], tc[0]), approach(c.c[1], tc[1]), approach(c.c[2], tc[2])];
      const ni = approach(c.i, intensity);
      cur.current = { c: nc, i: ni };
      rerender();
      const settled =
        Math.abs(ni - intensity) < EPS &&
        nc.every((v, k) => Math.abs(v - tc[k]) < EPS);
      if (settled) {
        cur.current = { c: tc, i: intensity }; // snap to exact target
        rerender();
        return;
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => {
      running = false;
      if (raf) cancelAnimationFrame(raf);
    };
  }, [color, intensity]);

  return (
    <FlameWrap
      color={cur.current.c}
      intensity={cur.current.i}
      radius={radius}
      // Caller's dialled-down preset: a short, gentle ember, not a bonfire.
      height={40}
      scale={0.21}
      turbulence={0.14}
      turbulenceReach={8}
      distortion={1.5}
      smoke={0.35}
      ember={0.75}
      // z-index -1 (with the pill isolated) keeps the flame above the pill's
      // dark face but behind its content, so the border licks show and the dot
      // / buttons stay crisp on top.
      style={{ position: "absolute", inset: 0, pointerEvents: "none", zIndex: -1 }}
    >
      {/* Empty phantom box: the flame wraps this, the real pill content sits on
          top in the DOM. Nothing to melt, so the missing html-in-canvas API in
          the macOS webview costs us only the content distortion, not the flame. */}
      <div style={{ width: "100%", height: "100%" }} />
    </FlameWrap>
  );
}

export default BarFlameBorder;
