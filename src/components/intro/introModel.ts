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
  /** The bar's drawn footprint, measured by the backend: its centre, size
   *  and corner radius. Only the centre is drawn from. The Pill reports its
   *  hit footprint, which is the shape plus the margin around it, so the
   *  size is no guide to the pill's edge; the centre is the pill's centre. */
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
 * here, press Replay intro in Dev Tools, and look.
 */
export const LOOK = {
  /** How far the cloud reaches from the pill at its largest, as a fraction
   *  of the window's short side. */
  reach: 0.55,
  /** Overall opacity of the smoke, 0..1. */
  density: 0.85,
  /** How fast the smoke churns. 1 is the shipped speed. */
  churn: 1,
  /** The two tones of the cloud, near black and near white. Both are in
   *  every frame, side by side: the light parts read over a dark window and
   *  the dark parts over a light one, whatever the theme, so the smoke never
   *  has to know what it is on. */
  dark: [0.06, 0.06, 0.08] as const,
  light: [0.97, 0.97, 0.98] as const,
  /** How much of the cloud is light rather than dark, 0..1. 0.5 is even. */
  lightness: 0.5,
  /** How strongly each billow is lit from the upper left, 0..1. 0 is flat. */
  relief: 0.6,
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
  uInward: [number, number];
  uDark: [number, number, number];
  uLight: [number, number, number];
  uGain: number;
  uReach: number;
  uChurn: number;
  uLightness: number;
  uRelief: number;
}

/** Does this person prefer reduced motion? Unknown counts as no. */
export const prefersReducedMotion = (): boolean => {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
};

/**
 * Turn the plan into the first frame's uniforms.
 *
 * Two coordinate flips live here and nowhere else: the plan is logical points
 * with y down from the window's top-left, the shader is device pixels with y
 * up from the bottom-left. The inward vector flips with the axis.
 */
export const uniformsFor = (plan: IntroPlan, viewport: Viewport, dpr: number): IntroUniforms => ({
  uRes: [viewport.width * dpr, viewport.height * dpr],
  uTime: 0,
  uT: 0,
  uPill: [plan.pill_x * dpr, (viewport.height - plan.pill_y) * dpr],
  uInward: [plan.inward_x, -plan.inward_y],
  uDark: [LOOK.dark[0], LOOK.dark[1], LOOK.dark[2]],
  uLight: [LOOK.light[0], LOOK.light[1], LOOK.light[2]],
  uGain: LOOK.density,
  uReach: LOOK.reach,
  uChurn: LOOK.churn,
  uLightness: LOOK.lightness,
  uRelief: LOOK.relief,
});

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
 * One pass. Domain-warped fractal noise centred under the bar, densest at
 * the centre with no hole in it, drawn in toward that centre as the sequence
 * advances and dissolved from the outside in, so the last of it goes where
 * the bar is. Kept off the edge-facing side of the window so it never meets
 * an edge. The cloud is two tones, near black and near white together, each
 * billow lit from the upper left. Output is premultiplied.
 */
export const FRAG = /* glsl */ `
  precision highp float;
  uniform vec2 uRes;
  uniform float uTime;
  uniform float uT;
  uniform vec2 uPill;
  uniform vec2 uInward;
  uniform vec3 uDark;
  uniform vec3 uLight;
  uniform float uGain;
  uniform float uReach;
  uniform float uChurn;
  uniform float uLightness;
  uniform float uRelief;

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
  void main() {
    float scale = 1.0 / min(uRes.x, uRes.y);
    // Everything is measured from the centre of the bar, in units of the
    // window's short side.
    vec2 uv = (gl_FragCoord.xy - uPill) * scale;
    float dc = length(uv);

    float bloom  = smoothstep(0.00, 0.32, uT);
    float gather = smoothstep(0.30, 0.68, uT);
    float clear  = smoothstep(0.62, 1.00, uT);

    // The cloud: blooms out from the centre, is drawn back in, breaks up.
    float R = mix(0.06, uReach, bloom) * (1.0 - 0.70 * gather);
    float tm = uTime * 2.2 * uChurn;
    // Drawn in by scaling the field toward the centre, not by shifting it:
    // a shift piles the pattern into bands around the centre.
    vec2 q = uv * (1.0 + gather * 0.9) - uInward * (tm * 0.03);
    float w = fbm(q * 2.0 + tm * 0.12);
    vec2 qw = q * 3.4 + vec2(w * 1.3, -w * 0.9) + vec2(tm * 0.06, tm * 0.10);
    float n = fbm(qw);
    vec2 qh = q * 1.1 + vec2(w * 0.5, 0.0) - tm * 0.04;
    float haze = fbm(qh);
    float fine = fbm(q * 7.0 + vec2(-w * 0.8, w * 0.6) + tm * 0.15);

    // Never far onto the edge-facing side: that is where the screen ends.
    // The cloud still reaches a little past the centre that way, so it sits
    // on the bar and not below it. With no inward vector (mid-screen) this
    // is 1 everywhere and the cloud is round.
    float side = 1.0 - smoothstep(0.04, 0.10, dot(uv, -uInward));
    // And a soft fade short of every window edge, short on the edge-facing
    // side (the bar is close to it) and long elsewhere.
    vec2 e = gl_FragCoord.xy / uRes;
    float mL = mix(0.25, 0.06, max(uInward.x, 0.0));
    float mR = mix(0.25, 0.06, max(-uInward.x, 0.0));
    float mB = mix(0.25, 0.06, max(uInward.y, 0.0));
    float mT = mix(0.25, 0.06, max(-uInward.y, 0.0));
    float edge = smoothstep(0.0, mL, e.x) * smoothstep(0.0, mR, 1.0 - e.x)
               * smoothstep(0.0, mB, e.y) * smoothstep(0.0, mT, 1.0 - e.y) * side;

    // Densest at the centre, with no gap there: the bar is drawn over
    // the smoke, so the smoke runs right under it.
    float env = 1.0 - smoothstep(0.0, R, dc);
    // The threshold rises with distance as it clears, so the outside of the
    // cloud breaks up first and the last wisp goes at the centre.
    float th = mix(0.46, 0.26, bloom) + clear * (0.62 + dc * 2.0);
    float wisps = smoothstep(th, th + 0.34, n);
    float body = smoothstep(th, th + 0.50, haze) * 0.7;
    float detail = smoothstep(th + 0.12, th + 0.40, fine) * 0.25;
    float core = (1.0 - smoothstep(0.0, R * 0.35, dc)) * 0.5 * (1.0 - clear);
    float dens = (wisps + body + detail + core) * env * edge;
    float a = clamp(dens * uGain * (1.0 - clear * 0.6), 0.0, 1.0);

    // Tone. Near-black and near-white smoke in one cloud, patch by patch, so
    // both are in every frame and it reads over any window. Mid-sized
    // blotches decide which is which, and the haze sampled a little toward
    // the upper left gives every billow a lit side.
    float tone = fbm(q * 2.2 + vec2(w * 0.7, -w * 0.4) + tm * 0.05);
    float towardLight = fbm(qh + vec2(-0.12, 0.12));
    float relief = clamp((towardLight - haze) * 6.0, -1.0, 1.0) * uRelief;
    // The split sits where this noise actually centres (it runs low), so
    // 0.5 lightness is an even share of each tone.
    float shade = smoothstep(0.26, 0.48, tone + (uLightness - 0.5) * 0.4) + relief * 0.35;
    vec3 col = mix(uDark, uLight, clamp(shade, 0.0, 1.0));

    gl_FragColor = vec4(col * a, a);
  }
`;
