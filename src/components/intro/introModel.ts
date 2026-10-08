/**
 * The intro reveal: what the smoke looks like, and the arithmetic that turns
 * the backend's plan into shader uniforms.
 *
 * The backend (`src-tauri/src/intro.rs`) owns the clock and the geometry: it
 * decides when the bar shows, how long the sequence runs, where the pill is
 * inside the intro window, and which way the open screen lies. This file
 * only draws. Everything a person can see is a number in `LOOK`; everything
 * else is plumbing.
 *
 * Spec: docs/plans/intro-reveal.md.
 */

/** What the backend hands the intro window. Logical points, origin at the
 *  window's top-left, y down. */
export interface IntroPlan {
  duration_ms: number;
  bar_at_ms: number;
  /** The pill's centre, size and corner radius: whatever shape the bar is
   *  actually drawing, measured by the backend. */
  pill_x: number;
  pill_y: number;
  pill_w: number;
  pill_h: number;
  pill_radius: number;
  /** Unit vector from the pill toward the open screen, or zero mid-screen. */
  inward_x: number;
  inward_y: number;
}

/**
 * The tweakables. Each one is something a person can see; change a number
 * here, replay the intro from Dev Tools, and look.
 */
export const LOOK = {
  /** How far the cloud reaches from the pill at its largest, as a fraction
   *  of the window's short side. */
  reach: 0.55,
  /** Overall opacity of the smoke, 0..1. */
  density: 0.65,
  /** How fast the smoke churns. 1 is the shipped speed. */
  churn: 1,
  /** The lit rim around the pill as it condenses, 0..1. 0 switches it off. */
  rim: 1,
  /** Smoke over a dark desktop: near white. */
  colorOnDark: [0.96, 0.96, 0.98] as const,
  /** Smoke over a light desktop: charcoal. */
  colorOnLight: [0.22, 0.23, 0.26] as const,
} as const;

/** The window the intro draws into, logical points. */
export interface Viewport {
  width: number;
  height: number;
}

/** Every uniform the shader reads, in the shader's own units (device px,
 *  y up). `uT` and `uTime` start at 0 and advance each frame. */
export interface IntroUniforms {
  uRes: [number, number];
  uTime: number;
  uT: number;
  uPill: [number, number];
  uHalf: [number, number];
  uRadius: number;
  uInward: [number, number];
  uColor: [number, number, number];
  uGain: number;
  uReach: number;
  uChurn: number;
  uRim: number;
  /** 1 draws smoke, 0 draws only the rim (Reduce Motion). */
  uSmoke: number;
}

/** Does this person prefer reduced motion? Unknown counts as no. */
export const prefersReducedMotion = (): boolean => {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
};

/** Is the desktop dark? Unknown counts as dark, which is where the pill is
 *  hardest to see and the smoke matters most. */
export const prefersDark = (): boolean => {
  try {
    return window.matchMedia("(prefers-color-scheme: dark)").matches;
  } catch {
    return true;
  }
};

/**
 * Turn the plan into the first frame's uniforms.
 *
 * Two coordinate flips live here and nowhere else: the plan is logical points
 * with y down from the window's top-left, the shader is device pixels with y
 * up from the bottom-left. The inward vector flips with the axis.
 */
export const uniformsFor = (
  plan: IntroPlan,
  viewport: Viewport,
  dpr: number,
  dark: boolean,
  reducedMotion: boolean,
): IntroUniforms => {
  const color = dark ? LOOK.colorOnDark : LOOK.colorOnLight;
  return {
    uRes: [viewport.width * dpr, viewport.height * dpr],
    uTime: 0,
    uT: 0,
    uPill: [plan.pill_x * dpr, (viewport.height - plan.pill_y) * dpr],
    uHalf: [(plan.pill_w / 2) * dpr, (plan.pill_h / 2) * dpr],
    uRadius: plan.pill_radius * dpr,
    uInward: [plan.inward_x, -plan.inward_y],
    uColor: [color[0], color[1], color[2]],
    uGain: LOOK.density,
    uReach: LOOK.reach,
    uChurn: LOOK.churn,
    uRim: LOOK.rim,
    uSmoke: reducedMotion ? 0 : 1,
  };
};

/** Where in the sequence we are, 0..1, for a frame at `now`. */
export const progressAt = (now: number, start: number, durationMs: number): number => {
  if (durationMs <= 0) return 1;
  return Math.min(1, Math.max(0, (now - start) / durationMs));
};

export const VERT = /* glsl */ `
  attribute vec2 position;
  void main() {
    gl_Position = vec4(position, 0.0, 1.0);
  }
`;

/**
 * One pass. Domain-warped fractal noise, pulled in toward the pill as the
 * sequence advances, dissolved by threshold at the end, and kept off the
 * edge-facing side of the window so it never meets an edge. A lit rim traces
 * the pill as it condenses. Output is premultiplied.
 */
export const FRAG = /* glsl */ `
  precision highp float;
  uniform vec2 uRes;
  uniform float uTime;
  uniform float uT;
  uniform vec2 uPill;
  uniform vec2 uHalf;
  uniform float uRadius;
  uniform vec2 uInward;
  uniform vec3 uColor;
  uniform float uGain;
  uniform float uReach;
  uniform float uChurn;
  uniform float uRim;
  uniform float uSmoke;

  float hash(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
  }
  float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(
      mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
      mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x),
      u.y
    );
  }
  const mat2 ROT = mat2(0.80, 0.60, -0.60, 0.80);
  float fbm(vec2 p) {
    float v = 0.0;
    float a = 0.5;
    for (int i = 0; i < 5; i++) {
      v += a * noise(p);
      p = ROT * p * 2.02 + 7.3;
      a *= 0.5;
    }
    return v;
  }
  float sdRound(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
  }

  void main() {
    float scale = 1.0 / min(uRes.x, uRes.y);
    vec2 p = gl_FragCoord.xy - uPill;
    float dpx = sdRound(p, uHalf, uRadius);
    float d = dpx * scale;
    vec2 uv = p * scale;

    float bloom  = smoothstep(0.00, 0.32, uT);
    float gather = smoothstep(0.30, 0.68, uT);
    float clear  = smoothstep(0.62, 1.00, uT);

    // The rim: the pill's edge lit as it condenses, settling as the smoke goes.
    float rim = smoothstep(0.50, 0.72, uT) * (1.0 - smoothstep(0.80, 1.0, uT)) * uRim;
    float ring = (1.0 - smoothstep(0.0, 1.5, abs(dpx))) * rim;
    float halo = exp(-max(dpx, 0.0) / (uHalf.y * 1.3)) * rim * 0.35;
    float rimA = clamp(ring + halo, 0.0, 1.0);

    // The cloud: blooms out, is drawn in, breaks up.
    float R = mix(0.08, uReach, bloom) * (1.0 - 0.70 * gather);
    vec2 dir = p / max(length(p), 1.0);
    float tm = uTime * 2.2 * uChurn;
    vec2 q = uv - dir * (gather * 0.42) - uInward * (tm * 0.03);
    float w = fbm(q * 2.0 + tm * 0.12);
    float n = fbm(q * 3.4 + vec2(w * 1.3, -w * 0.9) + vec2(tm * 0.06, tm * 0.10));
    float haze = fbm(q * 1.1 + vec2(w * 0.5, 0.0) - tm * 0.04);
    float fine = fbm(q * 7.0 + vec2(-w * 0.8, w * 0.6) + tm * 0.15);

    // Never on the edge-facing side: that is where the screen ends. With no
    // inward vector (mid-screen) this is 1 everywhere and the cloud is round.
    float side = 1.0 - smoothstep(0.0, uHalf.y * 1.6, dot(p, -uInward));
    // And a soft fade inside every window edge, short on the edge-facing
    // side (the pill is close to it) and long elsewhere.
    vec2 e = gl_FragCoord.xy / uRes;
    float mL = mix(0.25, 0.06, max(uInward.x, 0.0));
    float mR = mix(0.25, 0.06, max(-uInward.x, 0.0));
    float mB = mix(0.25, 0.06, max(uInward.y, 0.0));
    float mT = mix(0.25, 0.06, max(-uInward.y, 0.0));
    float edge = smoothstep(0.0, mL, e.x) * smoothstep(0.0, mR, 1.0 - e.x)
               * smoothstep(0.0, mB, e.y) * smoothstep(0.0, mT, 1.0 - e.y) * side;

    float env = 1.0 - smoothstep(R * 0.10, R, max(d, 0.0));
    float th = mix(0.46, 0.26, bloom) + clear * 0.62;
    float wisps = smoothstep(th, th + 0.34, n);
    float body = smoothstep(th, th + 0.50, haze) * 0.6;
    float detail = smoothstep(th + 0.12, th + 0.40, fine) * 0.25;
    float dens = (wisps + body + detail) * env * edge;
    // The pill pushes the smoke out as it forms.
    float inside = 1.0 - smoothstep(-0.004, 0.0, d);
    dens *= 1.0 - inside * gather;
    float lit = 1.0 + 0.2 * gather * (1.0 - smoothstep(0.0, 0.12, max(d, 0.0)));
    float a = clamp(dens * uGain * (1.0 - clear * 0.6), 0.0, 1.0) * uSmoke;

    // Rim over smoke, premultiplied.
    vec3 col = uColor * lit * a * (1.0 - rimA) + vec3(rimA);
    gl_FragColor = vec4(col, a * (1.0 - rimA) + rimA);
  }
`;
