/**
 * BarFlameBorder — the floating bar's activity indicator: a dialled-down
 * canvas-ui flame that lights up around the pill's border while Juno is doing
 * something, and holds (it does not flash). Colour and intensity carry meaning:
 *
 *   colour     = which kind of activity (see flameForState in FloatingBar)
 *   intensity  = how active (dim = ambient, brighter = engaged)
 *   pulse      = a slow breathe for ongoing work (thinking / processing)
 *
 * Both are ANIMATED toward their targets, never switched, so the border eases
 * between meanings as the mode changes. Heat shimmer (distortion) and edge melt
 * are turned off — this is a clean glowing outline, not fire eating the pill.
 *
 * It sits absolutely behind the pill content at z-index -1 (the pill is
 * `isolate`), licking outward past the opaque face so it reads as a border. It
 * is mounted only while active, so the idle bar costs no GPU. It degrades
 * safely: canvas-ui paints its overlay canvas even where the experimental
 * html-in-canvas API is missing (the macOS webview), and the empty child means
 * there is nothing to melt.
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
const EPS = 0.0015;
const approach = (a: number, b: number) => a + (b - a) * APPROACH;

/** Breathe depth + period for the `pulse` state (ongoing work). */
const BREATHE_AMP = 0.06;
const BREATHE_PERIOD_MS = 1100;

export interface BarFlameBorderProps {
  /** Target flame colour as a hex string; eased toward, not switched. */
  color: string;
  /** Target flame intensity (0..3); eased toward. */
  intensity: number;
  /** Corner radius of the burning outline, in CSS px — match the pill. */
  radius: number;
  /** Slow breathe around the target intensity, for ongoing work. */
  pulse?: boolean;
}

export function BarFlameBorder({ color, intensity, radius, pulse = false }: BarFlameBorderProps) {
  // Live, eased values the flame renders with (in a ref so the loop never
  // stales on a captured value).
  const cur = useRef<{ c: RGB; i: number }>({ c: hexToRgb(color), i: intensity });
  const startedAt = useRef(performance.now());
  const [, rerender] = useReducer((x: number) => x + 1, 0);

  useEffect(() => {
    let running = true;
    let raf = 0;
    const tick = () => {
      if (!running) return;
      const tc = hexToRgb(color);
      const breathe = pulse
        ? Math.sin((performance.now() - startedAt.current) / BREATHE_PERIOD_MS) * BREATHE_AMP
        : 0;
      const targetI = Math.max(0, intensity + breathe);
      const c = cur.current;
      const nc: RGB = [approach(c.c[0], tc[0]), approach(c.c[1], tc[1]), approach(c.c[2], tc[2])];
      const ni = approach(c.i, targetI);
      cur.current = { c: nc, i: ni };
      rerender();
      const colorSettled = nc.every((v, k) => Math.abs(v - tc[k]) < EPS);
      const intensitySettled = Math.abs(ni - targetI) < EPS;
      // Keep animating forever while pulsing; otherwise stop once settled.
      if (colorSettled && intensitySettled && !pulse) return;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => {
      running = false;
      if (raf) cancelAnimationFrame(raf);
    };
  }, [color, intensity, pulse]);

  return (
    <FlameWrap
      color={cur.current.c}
      intensity={cur.current.i}
      radius={radius}
      height={40}
      scale={0.21}
      turbulence={0.14}
      turbulenceReach={8}
      smoke={0.35}
      ember={0.75}
      // No heat shimmer, no edge melt: a clean glowing outline, not fire eating
      // the pill.
      distortion={0}
      melt={0}
      // z-index -1 (pill is isolated) keeps the flame above the pill's dark face
      // but behind its content, so the border licks show and the dot / buttons
      // stay crisp on top.
      style={{ position: "absolute", inset: 0, pointerEvents: "none", zIndex: -1 }}
    >
      {/* Empty phantom box: the flame wraps this; the real pill content sits on
          top in the DOM. Nothing to melt, so the missing html-in-canvas API in
          the macOS webview costs us only the (now-off) content melt. */}
      <div style={{ width: "100%", height: "100%" }} />
    </FlameWrap>
  );
}

export default BarFlameBorder;
