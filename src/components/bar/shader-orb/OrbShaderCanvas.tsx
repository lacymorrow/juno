import { useEffect, useRef, type RefObject } from "react";
import { Renderer, Program, Mesh, Triangle } from "ogl";
import type { OrbLook, OrbTargets } from "./shaderOrbModel";
import { REST_DRIFT_MS, hueDelta, loopMaySleep, settled } from "./shaderOrbModel";

/**
 * The Orb's canvas: the React Bits orb (reactbits.dev), a self-contained ogl
 * WebGL shader, driven by state instead of by the mouse.
 *
 * The shader is the original's: the same simplex-noise rim, the same two
 * swept lobes over a deep core, the same light orbiting inside it. What
 * changed is where it gets its numbers. The original read `hue` and
 * `hoverIntensity` as React props and rebuilt its WebGL context whenever
 * either changed, which with an audio-reactive intensity meant tearing the
 * context down several times a second. Now the context is built once and the
 * bar writes into a ref that this reads every frame, so nothing here
 * re-renders and nothing is rebuilt.
 *
 * Eight uniforms, each of them something a person can see:
 *
 * | uniform    | what it is |
 * |---|---|
 * | `uTime`    | the inside's own clock, advanced at the look's flow rate |
 * | `uRot`     | rigid rotation of the whole sphere |
 * | `uHue`     | degrees of rotation applied to all three base colours |
 * | `uSat`     | chroma: 0 grey, 1 the full base colours |
 * | `uMono`    | collapses the three colours onto one red object (error) |
 * | `uScale`   | how much of the canvas the sphere fills |
 * | `uRipple`  | surface disturbance: your voice, Juno's speech |
 * | `uBright`  | how present the sphere is |
 *
 * Because `uTime` is advanced here rather than taken from the frame clock,
 * `flow: 0` freezes the inside rather than slowing it, which is what lets an
 * approval stop the sphere dead.
 *
 * Spec and the full state table: docs/plans/orb-appearance.md.
 */

/** What the bar writes every render; this reads it every frame. */
export interface OrbDrive {
  look: OrbLook;
  targets: OrbTargets;
  /** Bumped once per entry into a one-shot motion (recoil, bloom). */
  impulse: number;
}

export type Frameloop = "always" | "demand";

/** How fast the eased values chase their targets, per frame at 60fps. */
const EASE = {
  hue: 0.12,
  sat: 0.1,
  mono: 0.14,
  bright: 0.1,
  scale: 0.16,
  ripple: 0.22,
  flow: 0.08,
  spin: 0.08,
} as const;

/** A one-shot motion lasts this long and moves the sphere by this much. */
const IMPULSE_MS = 560;
const RECOIL_DEPTH = -0.12;
const BLOOM_DEPTH = 0.14;
/** The working pulse: how far the sphere swells and shrinks. */
const PULSE_DEPTH = 0.03;

const vert = /* glsl */ `
  precision highp float;
  attribute vec2 position;
  attribute vec2 uv;
  varying vec2 vUv;
  void main() {
    vUv = uv;
    gl_Position = vec4(position, 0.0, 1.0);
  }
`;

const frag = /* glsl */ `
  precision highp float;

  uniform float uTime;
  uniform vec3 iResolution;
  uniform float uHue;
  uniform float uSat;
  uniform float uMono;
  uniform float uScale;
  uniform float uRipple;
  uniform float uBright;
  uniform float uRot;
  varying vec2 vUv;

  const vec3 LUMA = vec3(0.299, 0.587, 0.114);
  const vec3 MONO_RGB = vec3(1.0, 0.27, 0.23);

  vec3 rgb2yiq(vec3 c) {
    float y = dot(c, LUMA);
    float i = dot(c, vec3(0.596, -0.274, -0.322));
    float q = dot(c, vec3(0.211, -0.523, 0.312));
    return vec3(y, i, q);
  }

  vec3 yiq2rgb(vec3 c) {
    float r = c.x + 0.956 * c.y + 0.621 * c.z;
    float g = c.x - 0.272 * c.y - 0.647 * c.z;
    float b = c.x - 1.106 * c.y + 1.703 * c.z;
    return vec3(r, g, b);
  }

  vec3 adjustHue(vec3 color, float hueDeg) {
    float hueRad = hueDeg * 3.14159265 / 180.0;
    vec3 yiq = rgb2yiq(color);
    float cosA = cos(hueRad);
    float sinA = sin(hueRad);
    float i = yiq.y * cosA - yiq.z * sinA;
    float q = yiq.y * sinA + yiq.z * cosA;
    yiq.y = i;
    yiq.z = q;
    return yiq2rgb(yiq);
  }

  /** Hue, then chroma, then the one collapse to red. */
  vec3 stateColor(vec3 base) {
    vec3 c = adjustHue(base, uHue);
    c = mix(vec3(dot(c, LUMA)), c, uSat);
    c = mix(c, dot(c, LUMA) * MONO_RGB * 1.5, uMono);
    return c;
  }

  vec3 hash33(vec3 p3) {
    p3 = fract(p3 * vec3(0.1031, 0.11369, 0.13787));
    p3 += dot(p3, p3.yxz + 19.19);
    return -1.0 + 2.0 * fract(vec3(
      p3.x + p3.y,
      p3.x + p3.z,
      p3.y + p3.z
    ) * p3.zyx);
  }

  float snoise3(vec3 p) {
    const float K1 = 0.333333333;
    const float K2 = 0.166666667;
    vec3 i = floor(p + (p.x + p.y + p.z) * K1);
    vec3 d0 = p - (i - (i.x + i.y + i.z) * K2);
    vec3 e = step(vec3(0.0), d0 - d0.yzx);
    vec3 i1 = e * (1.0 - e.zxy);
    vec3 i2 = 1.0 - e.zxy * (1.0 - e);
    vec3 d1 = d0 - (i1 - K2);
    vec3 d2 = d0 - (i2 - K1);
    vec3 d3 = d0 - 0.5;
    vec4 h = max(0.6 - vec4(
      dot(d0, d0),
      dot(d1, d1),
      dot(d2, d2),
      dot(d3, d3)
    ), 0.0);
    vec4 n = h * h * h * h * vec4(
      dot(d0, hash33(i)),
      dot(d1, hash33(i + i1)),
      dot(d2, hash33(i + i2)),
      dot(d3, hash33(i + 1.0))
    );
    return dot(vec4(31.316), n);
  }

  vec4 extractAlpha(vec3 colorIn) {
    float a = max(max(colorIn.r, colorIn.g), colorIn.b);
    return vec4(colorIn.rgb / (a + 1e-5), a);
  }

  const vec3 baseColor1 = vec3(0.611765, 0.262745, 0.996078);
  const vec3 baseColor2 = vec3(0.298039, 0.760784, 0.913725);
  const vec3 baseColor3 = vec3(0.062745, 0.078431, 0.600000);
  const float innerRadius = 0.6;
  const float noiseScale = 0.65;

  float light1(float intensity, float attenuation, float dist) {
    return intensity / (1.0 + dist * attenuation);
  }

  float light2(float intensity, float attenuation, float dist) {
    return intensity / (1.0 + dist * dist * attenuation);
  }

  vec4 draw(vec2 uv) {
    vec3 color1 = stateColor(baseColor1);
    vec3 color2 = stateColor(baseColor2);
    vec3 color3 = stateColor(baseColor3);

    float ang = atan(uv.y, uv.x);
    float len = length(uv);
    float invLen = len > 0.0 ? 1.0 / len : 0.0;

    float n0 = snoise3(vec3(uv * noiseScale, uTime * 0.5)) * 0.5 + 0.5;
    float r0 = mix(mix(innerRadius, 1.0, 0.4), mix(innerRadius, 1.0, 0.6), n0);
    float d0 = distance(uv, (r0 * invLen) * uv);
    float v0 = light1(1.0, 10.0, d0);
    v0 *= smoothstep(r0 * 1.05, r0, len);
    float cl = cos(ang + uTime * 2.0) * 0.5 + 0.5;

    float a = uTime * -1.0;
    vec2 pos = vec2(cos(a), sin(a)) * r0;
    float d = distance(uv, pos);
    float v1 = light2(1.5, 5.0, d);
    v1 *= light1(1.0, 50.0, d0);

    float v2 = smoothstep(1.0, mix(innerRadius, 1.0, n0 * 0.5), len);
    float v3 = smoothstep(innerRadius, mix(innerRadius, 1.0, 0.5), len);

    vec3 col = mix(color1, color2, cl);
    col = mix(color3, col, v0);
    col = (col + v1) * v2 * v3;
    col = clamp(col, 0.0, 1.0);

    return extractAlpha(col);
  }

  vec4 mainImage(vec2 fragCoord) {
    vec2 center = iResolution.xy * 0.5;
    float size = min(iResolution.x, iResolution.y);
    vec2 uv = (fragCoord - center) / size * 2.0;

    // The sphere scales itself inside a canvas that is sized once.
    uv /= max(uScale, 0.05);

    float angle = uRot;
    float s = sin(angle);
    float c = cos(angle);
    uv = vec2(c * uv.x - s * uv.y, s * uv.x + c * uv.y);

    // The original's hover warp, now the voice: the same expression, read
    // from state rather than from the pointer.
    uv.x += uRipple * 0.1 * sin(uv.y * 10.0 + uTime);
    uv.y += uRipple * 0.1 * sin(uv.x * 10.0 + uTime);

    return draw(uv);
  }

  void main() {
    vec2 fragCoord = vUv * iResolution.xy;
    vec4 col = mainImage(fragCoord);
    gl_FragColor = vec4(col.rgb * col.a, col.a) * uBright;
  }
`;

interface OrbShaderCanvasProps {
  /** The bar writes the look and its targets here; this reads them per frame. */
  drive: RefObject<OrbDrive>;
  /** The bar's record of whether the loop is awake. Setting it back to
   *  "always" is what wakes a sleeping loop; the loop itself decides when to
   *  sleep and says so through `onSettled`. */
  frameloop: Frameloop;
  /** Called once the loop has gone to sleep, so the bar can record it. */
  onSettled: () => void;
  /** The canvas is square and sized once. */
  size: number;
}

export function OrbShaderCanvas({ drive, frameloop, onSettled, size }: OrbShaderCanvasProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  // The loop reads these through refs so the WebGL context is built once and
  // never torn down for a prop change.
  const settledRef = useRef(onSettled);
  settledRef.current = onSettled;
  // Set by the loop so a "demand" to "always" flip can restart it in place.
  const wakeRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    // A WebGL context is not guaranteed: software rendering may be off, the
    // GPU may be blocklisted, or the page may have exhausted the context
    // limit. The bar must never go down with it, so failing here leaves the
    // host empty and the rest of the appearance keeps working.
    let renderer: Renderer;
    try {
      renderer = new Renderer({ alpha: true, premultipliedAlpha: false });
    } catch {
      return;
    }
    const gl = renderer.gl;
    if (!gl) return;
    gl.clearColor(0, 0, 0, 0);
    host.appendChild(gl.canvas);

    const geometry = new Triangle(gl);
    const start = drive.current?.targets;
    const program = new Program(gl, {
      vertex: vert,
      fragment: frag,
      uniforms: {
        uTime: { value: 0 },
        uRot: { value: 0 },
        iResolution: { value: [0, 0, 1] },
        uHue: { value: start?.hue ?? 0 },
        uSat: { value: start?.sat ?? 0.35 },
        uMono: { value: 0 },
        uScale: { value: start?.scale ?? 0.52 },
        uRipple: { value: 0 },
        uBright: { value: start?.bright ?? 0.55 },
      },
    });
    const mesh = new Mesh(gl, { geometry, program });

    const resize = () => {
      const dpr = Math.min(2, Math.max(1, window.devicePixelRatio || 1));
      renderer.setSize(size * dpr, size * dpr);
      gl.canvas.style.width = `${size}px`;
      gl.canvas.style.height = `${size}px`;
      program.uniforms.iResolution.value = [gl.canvas.width, gl.canvas.height, 1];
    };
    window.addEventListener("resize", resize);
    resize();

    // Everything the loop carries between frames.
    const now: OrbTargets = { ...(start ?? { hue: 0, sat: 0.35, mono: 0, bright: 0.55, scale: 0.52, ripple: 0, flow: 0.08, spin: 0 }) };
    let flowTime = 0;
    let rot = 0;
    let pulseTime = 0;
    let lastFrame = 0;
    let lastImpulse = drive.current?.impulse ?? 0;
    let impulseAt = -1;
    let impulseDepth = 0;
    let steadySince = -1;
    let rafId = 0;
    let asleep = false;

    const draw = (t: number) => {
      const dt = lastFrame ? Math.min(0.1, (t - lastFrame) * 0.001) : 0.016;
      lastFrame = t;
      const d = drive.current;
      const look = d?.look;
      const target = d?.targets;
      if (!look || !target) {
        rafId = requestAnimationFrame(draw);
        return;
      }

      // 60fps-relative easing, so a slow frame does not ease less.
      const step = (rate: number) => Math.min(1, rate * dt * 60);
      now.hue += hueDelta(now.hue, target.hue) * step(EASE.hue);
      now.sat += (target.sat - now.sat) * step(EASE.sat);
      now.mono += (target.mono - now.mono) * step(EASE.mono);
      now.bright += (target.bright - now.bright) * step(EASE.bright);
      now.scale += (target.scale - now.scale) * step(EASE.scale);
      now.ripple += (target.ripple - now.ripple) * step(EASE.ripple);
      now.flow += (target.flow - now.flow) * step(EASE.flow);
      now.spin += (target.spin - now.spin) * step(EASE.spin);

      // The inside has its own clock, so flow 0 freezes it rather than
      // slowing it: that is how an approval stops the sphere dead.
      flowTime += dt * now.flow;
      rot += dt * now.spin;

      const impulse = d.impulse;
      if (impulse !== lastImpulse) {
        lastImpulse = impulse;
        impulseAt = t;
        impulseDepth = look.motion === "recoil" ? RECOIL_DEPTH : BLOOM_DEPTH;
      }
      let offset = 0;
      if (impulseAt >= 0) {
        const p = (t - impulseAt) / IMPULSE_MS;
        if (p >= 1) {
          impulseAt = -1;
        } else {
          // Out hard, back soft: one gesture, not a wobble.
          offset = impulseDepth * Math.sin(Math.PI * p) * (1 - p * 0.5);
        }
      }
      if (look.pulsePeriod > 0) {
        pulseTime += dt;
        offset += PULSE_DEPTH * Math.sin((2 * Math.PI * pulseTime) / look.pulsePeriod);
      } else {
        pulseTime = 0;
      }

      const u = program.uniforms;
      u.uTime.value = flowTime;
      u.uRot.value = rot;
      u.uHue.value = now.hue;
      u.uSat.value = now.sat;
      u.uMono.value = now.mono;
      u.uScale.value = Math.max(0.05, now.scale + offset);
      u.uRipple.value = now.ripple;
      u.uBright.value = now.bright;
      renderer.render({ scene: mesh });

      // Sleep only once the look allows it, the easing has finished, and the
      // inside has had time to drift to a stop. A cut to a still frame would
      // read as a stall; coming to rest reads as rest.
      // Deliberately not gated on the `frameloop` prop: the bar only sets it
      // to "demand" in response to `onSettled`, so reading it here would mean
      // the loop could never reach the state that stops it.
      const quiet =
        loopMaySleep(look) &&
        impulseAt < 0 &&
        look.pulsePeriod === 0 &&
        settled(now, target);
      if (quiet) {
        if (steadySince < 0) steadySince = t;
        if (t - steadySince >= REST_DRIFT_MS) {
          asleep = true;
          settledRef.current();
          return;
        }
      } else {
        steadySince = -1;
      }
      rafId = requestAnimationFrame(draw);
    };

    rafId = requestAnimationFrame(draw);

    // Woken by the bar: the loop restarts where it stopped.
    const wake = () => {
      if (!asleep) return;
      asleep = false;
      steadySince = -1;
      lastFrame = 0;
      rafId = requestAnimationFrame(draw);
    };
    wakeRef.current = wake;

    return () => {
      wakeRef.current = null;
      cancelAnimationFrame(rafId);
      window.removeEventListener("resize", resize);
      if (gl.canvas.parentNode === host) host.removeChild(gl.canvas);
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    };
    // The drive and the callbacks are refs; the canvas is built once per size.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [size]);

  useEffect(() => {
    if (frameloop === "always") wakeRef.current?.();
  }, [frameloop]);

  return (
    <div
      ref={hostRef}
      data-testid="orb-shader"
      className="pointer-events-none"
      style={{ width: size, height: size }}
    />
  );
}
