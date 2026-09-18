/**
 * Canvas effects layer for agent-response components.
 *
 * One hook (`useCanvasEffect`) owns the render loop; painters are pure
 * functions of (ctx, frame). The hook handles everything a painter should
 * not have to think about:
 *   - devicePixelRatio scaling (crisp on Retina)
 *   - ResizeObserver-driven sizing
 *   - pausing when the canvas scrolls off-screen or the window is hidden
 *   - prefers-reduced-motion (one static frame, then stop)
 *   - jsdom (no 2D context) — silently no-ops so tests stay green
 *
 * Painters return `false` to end the loop early (one-shot bursts).
 */

import { useEffect, useRef, type RefObject } from "react";

export interface CanvasFrame {
  /** CSS-pixel width (already DPR-corrected via transform) */
  w: number;
  /** CSS-pixel height */
  h: number;
  /** Seconds since the effect started */
  t: number;
  /** Seconds since the previous frame (clamped to avoid tab-switch jumps) */
  dt: number;
  /** True when reduced motion is requested: paint a still, not an animation */
  still: boolean;
  /** True when a `.dark` ancestor is present */
  dark: boolean;
}

export type CanvasPainter = (
  ctx: CanvasRenderingContext2D,
  frame: CanvasFrame,
) => boolean | void;

function reducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function useCanvasEffect(
  canvasRef: RefObject<HTMLCanvasElement | null>,
  painter: CanvasPainter,
): void {
  // Painter identity may change every render; the loop reads the latest.
  const painterRef = useRef(painter);
  painterRef.current = painter;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let w = 0;
    let h = 0;
    let raf = 0;
    let start = 0;
    let last = 0;
    let visible = true;
    let finished = false;
    const still = reducedMotion();
    const dark = canvas.closest(".dark") !== null;

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      w = rect.width;
      h = rect.height;
      canvas.width = Math.max(1, Math.round(w * dpr));
      canvas.height = Math.max(1, Math.round(h * dpr));
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };

    const paint = (now: number) => {
      if (!start) {
        start = now;
        last = now;
      }
      const t = (now - start) / 1000;
      const dt = Math.min((now - last) / 1000, 1 / 20);
      last = now;
      ctx.clearRect(0, 0, w, h);
      const keepGoing = painterRef.current(ctx, { w, h, t, dt, still, dark });
      if (keepGoing === false || still) {
        finished = true;
        return;
      }
      raf = requestAnimationFrame(paint);
    };

    const play = () => {
      if (raf || finished || !visible || document.hidden) return;
      last = 0;
      raf = requestAnimationFrame((now) => {
        // Re-anchor `last` so dt after a pause is ~one frame, not the gap.
        last = now;
        paint(now);
      });
    };
    const pause = () => {
      if (raf) cancelAnimationFrame(raf);
      raf = 0;
    };

    resize();
    play();

    const ro =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver(() => {
            resize();
            if (finished && still) {
              // Repaint the still frame at the new size.
              finished = false;
              start = 0;
              play();
            }
          })
        : null;
    ro?.observe(canvas);

    const io =
      typeof IntersectionObserver !== "undefined"
        ? new IntersectionObserver(([entry]) => {
            visible = entry?.isIntersecting ?? true;
            if (visible) play();
            else pause();
          })
        : null;
    io?.observe(canvas);

    const onVisibility = () => (document.hidden ? pause() : play());
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      pause();
      ro?.disconnect();
      io?.disconnect();
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [canvasRef]);
}

// ============================================================
// Painters
// ============================================================

export interface RGB {
  r: number;
  g: number;
  b: number;
}

const rgba = ({ r, g, b }: RGB, a: number) => `rgba(${r},${g},${b},${a})`;

/** Parse `#rrggbb`, `rgb(...)`, or `rgba(...)`; falls back to blue-500. */
export function parseColor(input: string): RGB {
  const hex = /^#([0-9a-f]{6})$/i.exec(input.trim());
  if (hex) {
    const n = parseInt(hex[1], 16);
    return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
  }
  const fn = /rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/i.exec(input);
  if (fn) return { r: +fn[1], g: +fn[2], b: +fn[3] };
  return { r: 59, g: 130, b: 246 };
}

/**
 * Soft additive glow that breathes. Draws a blurred rounded rect the size of
 * the inset area; the canvas is expected to be padded by `bleed` on all sides.
 */
export function paintGlow(color: RGB, bleed: number, radius: number): CanvasPainter {
  return (ctx, { w, h, t, still }) => {
    const phase = still ? 0.5 : 0.5 + 0.5 * Math.sin(t * Math.PI); // 2s breath
    const blur = bleed * (0.5 + 0.5 * phase);
    const alpha = 0.45 + 0.4 * phase;
    ctx.save();
    ctx.globalCompositeOperation = "lighter";
    ctx.shadowColor = rgba(color, alpha);
    ctx.shadowBlur = blur;
    // Draw the shape far off-canvas and shift its shadow back on: only the
    // shadow lands, so the glow never paints over the badge itself.
    ctx.shadowOffsetX = 10000;
    ctx.fillStyle = "#000";
    ctx.beginPath();
    ctx.roundRect(bleed - 10000, bleed, w - bleed * 2, h - bleed * 2, radius);
    ctx.fill();
    ctx.restore();
  };
}

/** Three expanding rings on staggered phases, plus a solid centre dot. */
export function paintRings(color: RGB, alpha: number): CanvasPainter {
  const phases = [0, 1 / 3, 2 / 3];
  return (ctx, { w, h, t, still }) => {
    const cx = w / 2;
    const cy = h / 2;
    const rMax = Math.min(w, h) / 2 - 1;
    ctx.save();
    ctx.lineWidth = 1.5;
    for (const offset of phases) {
      const p = still ? offset : (t / 2 + offset) % 1;
      const r = rMax * (0.25 + 0.75 * p);
      const a = alpha * (1 - p) * (1 - p);
      ctx.strokeStyle = rgba(color, a);
      ctx.beginPath();
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      ctx.stroke();
    }
    ctx.fillStyle = rgba(color, Math.min(1, alpha * 2));
    ctx.beginPath();
    ctx.arc(cx, cy, rMax * 0.12, 0, Math.PI * 2);
    ctx.fill();
    ctx.restore();
  };
}

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  rot: number;
  vrot: number;
  size: number;
  color: string;
  life: number;
}

/** One-shot confetti burst with gravity, drag, and tumble. Ends when settled. */
export function paintConfetti(colors: string[], count: number): CanvasPainter {
  let particles: Particle[] | null = null;
  const seed = () =>
    Array.from({ length: count }, (_, i) => {
      const angle = -Math.PI / 2 + (Math.random() - 0.5) * Math.PI * 1.4;
      const speed = 120 + Math.random() * 140;
      return {
        x: 0,
        y: 0,
        vx: Math.cos(angle) * speed,
        vy: Math.sin(angle) * speed,
        rot: Math.random() * Math.PI,
        vrot: (Math.random() - 0.5) * 20,
        size: 3 + Math.random() * 4,
        color: colors[i % colors.length],
        life: 1,
      };
    });

  return (ctx, { w, h, dt, still }) => {
    if (still) return false;
    particles ??= seed();
    const cx = w / 2;
    const cy = h * 0.6;
    let alive = 0;
    ctx.save();
    for (const p of particles) {
      if (p.life <= 0) continue;
      p.vy += 420 * dt; // gravity
      p.vx *= 1 - 1.6 * dt; // drag
      p.vy *= 1 - 0.4 * dt;
      p.x += p.vx * dt;
      p.y += p.vy * dt;
      p.rot += p.vrot * dt;
      p.life -= dt / 1.4;
      if (p.life <= 0 || cy + p.y > h + p.size) continue;
      alive++;
      ctx.save();
      ctx.globalAlpha = Math.min(1, p.life * 2);
      ctx.fillStyle = p.color;
      ctx.translate(cx + p.x, cy + p.y);
      ctx.rotate(p.rot);
      // Wide, flat rects read as tumbling paper once they rotate.
      ctx.fillRect(-p.size / 2, -p.size / 4, p.size, p.size / 2);
      ctx.restore();
    }
    ctx.restore();
    return alive > 0;
  };
}

interface Drop {
  x: number;
  y: number;
  len: number;
  speed: number;
  alpha: number;
}

/** Steady rain field with slight wind; optional lightning for storms. */
export function paintRain(storm: boolean): CanvasPainter {
  let drops: Drop[] | null = null;
  let nextFlash = 2 + Math.random() * 4;
  let flash = 0;
  return (ctx, { w, h, t, dt, still, dark }) => {
    const n = Math.max(12, Math.round((w * h) / 2200));
    drops ??= Array.from({ length: n }, () => ({
      x: Math.random() * w,
      y: Math.random() * h,
      len: 6 + Math.random() * 8,
      speed: 260 + Math.random() * 180,
      alpha: 0.25 + Math.random() * 0.35,
    }));
    const wind = 0.18;
    const base: RGB = dark ? { r: 147, g: 197, b: 253 } : { r: 96, g: 165, b: 250 };
    ctx.save();
    ctx.lineCap = "round";
    ctx.lineWidth = 1.2;
    for (const d of drops) {
      if (!still) {
        d.y += d.speed * dt;
        d.x += d.speed * wind * dt;
        if (d.y > h + d.len) {
          d.y = -d.len;
          d.x = Math.random() * w;
        }
        if (d.x > w + 4) d.x -= w + 8;
      }
      ctx.strokeStyle = rgba(base, d.alpha);
      ctx.beginPath();
      ctx.moveTo(d.x, d.y);
      ctx.lineTo(d.x - d.len * wind, d.y - d.len);
      ctx.stroke();
    }
    if (storm && !still) {
      if (t > nextFlash) {
        flash = 1;
        nextFlash = t + 3 + Math.random() * 6;
      }
      if (flash > 0) {
        ctx.fillStyle = `rgba(226,232,240,${flash * 0.35})`;
        ctx.fillRect(0, 0, w, h);
        flash = flash > 0.6 ? flash - 7 * dt : flash - 2.5 * dt;
      }
    }
    ctx.restore();
  };
}

interface Flake {
  x: number;
  y: number;
  r: number;
  speed: number;
  drift: number;
  phase: number;
  alpha: number;
}

/** Drifting snow. Flakes sway on a sine so they never look like a grid. */
export function paintSnow(): CanvasPainter {
  let flakes: Flake[] | null = null;
  return (ctx, { w, h, t, dt, still, dark }) => {
    const n = Math.max(10, Math.round((w * h) / 3000));
    flakes ??= Array.from({ length: n }, () => ({
      x: Math.random() * w,
      y: Math.random() * h,
      r: 1 + Math.random() * 1.8,
      speed: 18 + Math.random() * 22,
      drift: 6 + Math.random() * 10,
      phase: Math.random() * Math.PI * 2,
      alpha: 0.5 + Math.random() * 0.4,
    }));
    const base: RGB = dark ? { r: 255, g: 255, b: 255 } : { r: 165, g: 180, b: 252 };
    ctx.save();
    for (const f of flakes) {
      if (!still) {
        f.y += f.speed * dt;
        if (f.y > h + f.r) {
          f.y = -f.r;
          f.x = Math.random() * w;
        }
      }
      const x = f.x + Math.sin(t * 0.8 + f.phase) * f.drift;
      ctx.fillStyle = rgba(base, f.alpha);
      ctx.beginPath();
      ctx.arc(x, f.y, f.r, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.restore();
  };
}

/** Warm halo with slow-turning rays, anchored to the top-right of the card. */
export function paintSun(): CanvasPainter {
  return (ctx, { w, h, t, still }) => {
    const cx = w - 40;
    const cy = 40;
    const r = Math.min(w, h) * 0.55;
    ctx.save();
    ctx.globalCompositeOperation = "lighter";
    const halo = ctx.createRadialGradient(cx, cy, 0, cx, cy, r);
    halo.addColorStop(0, "rgba(251,191,36,0.28)");
    halo.addColorStop(0.5, "rgba(251,191,36,0.08)");
    halo.addColorStop(1, "rgba(251,191,36,0)");
    ctx.fillStyle = halo;
    ctx.fillRect(0, 0, w, h);
    const rays = 12;
    const spin = still ? 0 : t * 0.08;
    ctx.translate(cx, cy);
    ctx.rotate(spin);
    for (let i = 0; i < rays; i++) {
      ctx.rotate((Math.PI * 2) / rays);
      const g = ctx.createLinearGradient(0, 0, r, 0);
      g.addColorStop(0, "rgba(251,191,36,0.10)");
      g.addColorStop(1, "rgba(251,191,36,0)");
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.moveTo(0, 0);
      ctx.lineTo(r, -r * 0.06);
      ctx.lineTo(r, r * 0.06);
      ctx.closePath();
      ctx.fill();
    }
    ctx.restore();
  };
}
