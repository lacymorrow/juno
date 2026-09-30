import { useEffect, useMemo, useRef, type RefObject } from "react";
import { useTexture } from "@react-three/drei";
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import * as THREE from "three";
// Vendored so the orb renders offline.
import perlinNoiseUrl from "@/assets/perlin-noise.png";
import { STAGE, loopMaySleep, settled, type OrbLook, type OrbTargets } from "./orbModel";

/**
 * The orb itself: one circle with a shader (the ElevenLabs orb's lobes and
 * noisy rings, kept), driven every frame from a ref the bar writes into. The
 * canvas is sized once and never resized; the orb scales its own mesh. The
 * loop runs while anything is moving and sleeps (`frameloop="demand"`) once
 * the orb is at rest and has settled, so a resting orb costs nothing.
 *
 * What state drives, per frame: the two ramp colours, the mesh scale, the
 * ring swell (uInputVolume), the flow turbulence (uOutputVolume), the flow's
 * rotation speed (uAnimation), the opacity, and one-shot impulses (a flinch
 * for an error, a bloom for done).
 */

export interface OrbDrive {
  look: OrbLook;
  targets: OrbTargets;
  /** Increment to play the look's one-shot (flinch or bloom) again. */
  impulse: number;
}

export type Frameloop = "always" | "demand";

interface OrbCanvasProps {
  drive: RefObject<OrbDrive>;
  frameloop: Frameloop;
  /** The orb has come to rest and the loop may sleep. */
  onSettled: () => void;
  seed?: number;
  /** Dark lobes on a black disc (true) or coloured lobes on white (false). */
  inverted?: boolean;
}

export function OrbCanvas({ drive, frameloop, onSettled, seed = 42, inverted = false }: OrbCanvasProps) {
  return (
    <div style={{ width: STAGE, height: STAGE, pointerEvents: "none" }} data-testid="orb-canvas">
      <Canvas
        frameloop={frameloop}
        dpr={[1, 2]}
        resize={{ debounce: 0 }}
        gl={{ alpha: true, antialias: true, premultipliedAlpha: true }}
      >
        <Scene drive={drive} seed={seed} inverted={inverted} onSettled={onSettled} />
      </Canvas>
    </div>
  );
}

/** How long a one-shot lasts. */
const IMPULSE_MS = 480;
/** Pulse depth while Juno works: a 3% swell and back. */
const PULSE_DEPTH = 0.03;
/** Easing rates per frame at 60fps. */
const EASE_COLOR = 0.08;
const EASE_SCALE = 0.16;
const EASE_LEVEL = 0.25;
const EASE_OPACITY = 0.1;

function Scene({
  drive,
  seed,
  inverted,
  onSettled,
}: {
  drive: RefObject<OrbDrive>;
  seed: number;
  inverted: boolean;
  onSettled: () => void;
}) {
  const { gl } = useThree();
  const meshRef = useRef<THREE.Mesh<THREE.CircleGeometry, THREE.ShaderMaterial>>(null);
  const perlinNoiseTexture = useTexture(perlinNoiseUrl);
  const random = useMemo(() => splitmix32(seed), [seed]);
  const offsets = useMemo(
    () => new Float32Array(Array.from({ length: 7 }, () => random() * Math.PI * 2)),
    [random],
  );

  // Eased values. They live outside React so a state change never re-renders
  // the canvas; the loop reads the drive ref and walks toward it.
  const target1 = useRef(new THREE.Color("#6E6E73"));
  const target2 = useRef(new THREE.Color("#AEAEB2"));
  const current = useRef({ scale: 0.5, swell: 0, turbulence: 0.12, opacity: 0 });
  const pulseClock = useRef(0);
  const impulseSeen = useRef(0);
  const impulseStart = useRef<number | null>(null);
  const impulseKind = useRef<"flinch" | "bloom" | null>(null);
  const asleep = useRef(false);
  const onSettledRef = useRef(onSettled);
  onSettledRef.current = onSettled;

  useFrame((_, delta) => {
    const mesh = meshRef.current;
    const d = drive.current;
    if (!mesh || !d) return;
    const u = mesh.material.uniforms;
    const { look, targets } = d;
    const dt = Math.min(delta, 0.1);
    const step = dt * 60;
    const cur = current.current;

    // Colours.
    target1.current.set(look.tint[0]);
    target2.current.set(look.tint[1]);
    u.uColor1.value.lerp(target1.current, EASE_COLOR * step);
    u.uColor2.value.lerp(target2.current, EASE_COLOR * step);

    // Size, swell, turbulence, opacity.
    cur.scale += (targets.scale - cur.scale) * Math.min(1, EASE_SCALE * step);
    cur.swell += (targets.swell - cur.swell) * Math.min(1, EASE_LEVEL * step);
    cur.turbulence += (targets.turbulence - cur.turbulence) * Math.min(1, EASE_LEVEL * step);
    cur.opacity += (look.opacity - cur.opacity) * Math.min(1, EASE_OPACITY * step);

    // The working pulse: a short swell whose period is the progress.
    let pulse = 1;
    if (look.pulsePeriod > 0) {
      pulseClock.current += dt / look.pulsePeriod;
      pulse = 1 + PULSE_DEPTH * (0.5 - 0.5 * Math.cos(pulseClock.current * 2 * Math.PI));
    } else {
      pulseClock.current = 0;
    }

    // One-shots. A flinch recoils; a bloom swells. Either plays once per impulse.
    if (d.impulse !== impulseSeen.current) {
      impulseSeen.current = d.impulse;
      if (look.motion === "flinch" || look.motion === "bloom") {
        impulseStart.current = performance.now();
        impulseKind.current = look.motion;
      }
    }
    let shot = 1;
    if (impulseStart.current !== null) {
      const x = (performance.now() - impulseStart.current) / IMPULSE_MS;
      if (x >= 1) {
        impulseStart.current = null;
      } else {
        const arc = Math.sin(Math.PI * x);
        shot = impulseKind.current === "flinch" ? 1 - 0.12 * arc : 1 + 0.16 * arc;
      }
    }

    u.uTime.value += dt * 0.5;
    u.uAnimation.value += dt * look.spin;
    u.uInputVolume.value = cur.swell;
    u.uOutputVolume.value = cur.turbulence;
    u.uOpacity.value = cur.opacity;
    mesh.scale.setScalar(cur.scale * pulse * shot);

    // Sleep once nothing is moving and the look allows it.
    const colorsSettled =
      colorClose(u.uColor1.value, target1.current) && colorClose(u.uColor2.value, target2.current);
    const still =
      colorsSettled &&
      impulseStart.current === null &&
      look.pulsePeriod === 0 &&
      look.spin === 0 &&
      settled({ ...cur }, { ...targets, opacity: look.opacity });
    if (still && loopMaySleep(look)) {
      // Land exactly on the targets so the last frame is the resting frame.
      cur.scale = targets.scale;
      cur.swell = targets.swell;
      cur.turbulence = targets.turbulence;
      cur.opacity = look.opacity;
      u.uColor1.value.copy(target1.current);
      u.uColor2.value.copy(target2.current);
      mesh.scale.setScalar(cur.scale);
      if (!asleep.current) {
        asleep.current = true;
        onSettledRef.current();
      }
    } else {
      asleep.current = false;
    }
  });

  useEffect(() => {
    const canvas = gl.domElement;
    const onContextLost = (event: Event) => {
      event.preventDefault();
      setTimeout(() => gl.forceContextRestore(), 1);
    };
    canvas.addEventListener("webglcontextlost", onContextLost, false);
    return () => canvas.removeEventListener("webglcontextlost", onContextLost, false);
  }, [gl]);

  const uniforms = useMemo(() => {
    perlinNoiseTexture.wrapS = THREE.RepeatWrapping;
    perlinNoiseTexture.wrapT = THREE.RepeatWrapping;
    return {
      uColor1: new THREE.Uniform(new THREE.Color("#6E6E73")),
      uColor2: new THREE.Uniform(new THREE.Color("#AEAEB2")),
      uOffsets: { value: offsets },
      uPerlinTexture: new THREE.Uniform(perlinNoiseTexture),
      uTime: new THREE.Uniform(0),
      uAnimation: new THREE.Uniform(0.1),
      uInverted: new THREE.Uniform(inverted ? 1 : 0),
      uInputVolume: new THREE.Uniform(0),
      uOutputVolume: new THREE.Uniform(0.12),
      uOpacity: new THREE.Uniform(0),
    };
  }, [perlinNoiseTexture, offsets, inverted]);

  return (
    <mesh ref={meshRef} scale={0.5}>
      <circleGeometry args={[3.5, 64]} />
      <shaderMaterial
        uniforms={uniforms}
        fragmentShader={fragmentShader}
        vertexShader={vertexShader}
        transparent
      />
    </mesh>
  );
}

function colorClose(a: THREE.Color, b: THREE.Color): boolean {
  return Math.abs(a.r - b.r) + Math.abs(a.g - b.g) + Math.abs(a.b - b.b) < 0.004;
}

function splitmix32(a: number) {
  return function () {
    a |= 0;
    a = (a + 0x9e3779b9) | 0;
    let t = a ^ (a >>> 16);
    t = Math.imul(t, 0x21f0aaad);
    t = t ^ (t >>> 15);
    t = Math.imul(t, 0x735a2d97);
    return ((t = t ^ (t >>> 15)) >>> 0) / 4294967296;
  };
}

const vertexShader = /* glsl */ `
varying vec2 vUv;

void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const fragmentShader = /* glsl */ `
uniform float uTime;
uniform float uAnimation;
uniform float uInverted;
uniform float uOffsets[7];
uniform vec3 uColor1;
uniform vec3 uColor2;
uniform float uInputVolume;
uniform float uOutputVolume;
uniform float uOpacity;
uniform sampler2D uPerlinTexture;
varying vec2 vUv;

const float PI = 3.14159265358979323846;

// One soft-edged oval in polar space, with its gradient.
bool drawOval(vec2 polarUv, vec2 polarCenter, float a, float b, bool reverseGradient, float softness, out vec4 color) {
    vec2 p = polarUv - polarCenter;
    float oval = (p.x * p.x) / (a * a) + (p.y * p.y) / (b * b);
    float edge = smoothstep(1.0, 1.0 - softness, oval);
    if (edge > 0.0) {
        float gradient = reverseGradient ? (1.0 - (p.x / a + 1.0) / 2.0) : ((p.x / a + 1.0) / 2.0);
        gradient = mix(0.5, gradient, 0.1);
        color = vec4(vec3(gradient), 0.85 * edge);
        return true;
    }
    return false;
}

// Greyscale to a four-stop ramp: black, colour 1, colour 2, white.
vec3 colorRamp(float grayscale, vec3 color1, vec3 color2, vec3 color3, vec3 color4) {
    if (grayscale < 0.33) {
        return mix(color1, color2, grayscale * 3.0);
    } else if (grayscale < 0.66) {
        return mix(color2, color3, (grayscale - 0.33) * 3.0);
    } else {
        return mix(color3, color4, (grayscale - 0.66) * 3.0);
    }
}

vec2 hash2(vec2 p) {
    return fract(sin(vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)))) * 43758.5453);
}

float noise2D(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    float n = mix(
        mix(dot(hash2(i + vec2(0.0, 0.0)), f - vec2(0.0, 0.0)),
            dot(hash2(i + vec2(1.0, 0.0)), f - vec2(1.0, 0.0)), u.x),
        mix(dot(hash2(i + vec2(0.0, 1.0)), f - vec2(0.0, 1.0)),
            dot(hash2(i + vec2(1.0, 1.0)), f - vec2(1.0, 1.0)), u.x),
        u.y
    );
    return 0.5 + 0.5 * n;
}

float sharpRing(vec3 decomposed, float time) {
    float noise = mix(
        noise2D(vec2(decomposed.x, time) * 5.0),
        noise2D(vec2(decomposed.y, time) * 5.0),
        decomposed.z
    );
    noise = (noise - 0.5) * 2.5;
    return 1.0 + noise * 0.3 * 1.5;
}

float smoothRing(vec3 decomposed, float time) {
    float noise = mix(
        noise2D(vec2(decomposed.x, time) * 6.0),
        noise2D(vec2(decomposed.y, time) * 6.0),
        decomposed.z
    );
    noise = (noise - 0.5) * 5.0;
    return 0.9 + noise * 0.2;
}

float flow(vec3 decomposed, float time) {
    return mix(
        texture(uPerlinTexture, vec2(time, decomposed.x / 2.0)).r,
        texture(uPerlinTexture, vec2(time, decomposed.y / 2.0)).r,
        decomposed.z
    );
}

void main() {
    vec2 uv = vUv * 2.0 - 1.0;
    float radius = length(uv);
    float theta = atan(uv.y, uv.x);
    if (theta < 0.0) theta += 2.0 * PI;

    // Angle decomposed so noise samples without a seam.
    vec3 decomposed = vec3(
        theta / (2.0 * PI),
        mod(theta / (2.0 * PI) + 0.5, 1.0) + 1.0,
        abs(theta / PI - 1.0)
    );

    // Flow distortion: turbulence widens it, the animation clock turns it.
    float noise = flow(decomposed, radius * 0.03 - uAnimation * 0.2) - 0.5;
    theta += noise * mix(0.08, 0.25, uOutputVolume);

    vec4 color = vec4(1.0, 1.0, 1.0, 1.0);

    float originalCenters[7] = float[7](0.0, 0.5 * PI, 1.0 * PI, 1.5 * PI, 2.0 * PI, 2.5 * PI, 3.0 * PI);
    float centers[7];
    for (int i = 0; i < 7; i++) {
        centers[i] = originalCenters[i] + 0.5 * sin(uTime / 20.0 + uOffsets[i]);
    }

    float a, b;
    vec4 ovalColor;
    for (int i = 0; i < 7; i++) {
        float n = texture(uPerlinTexture, vec2(mod(centers[i] + uTime * 0.05, 1.0), 0.5)).r;
        a = 0.5 + n * 0.3;
        // The swell pulls the lobes in so the rings read as the voice.
        b = n * mix(3.5, 2.5, uInputVolume);
        bool reverseGradient = (i % 2 == 1);
        float distTheta = min(
            abs(theta - centers[i]),
            min(abs(theta + 2.0 * PI - centers[i]), abs(theta - 2.0 * PI - centers[i]))
        );
        if (drawOval(vec2(distTheta, radius), vec2(0.0, 0.0), a, b, reverseGradient, 0.9, ovalColor)) {
            // Lobes fade toward the rim so the edge reads as light, not as
            // the end of a slice.
            float toRim = smoothstep(0.45, 1.0, radius);
            float lobe = ovalColor.a * mix(1.0, 0.3, toRim);
            color.rgb = mix(color.rgb, ovalColor.rgb, lobe);
            color.a = max(color.a, lobe);
        }
    }
    // The centre is one soft wash, never the point where seven slices meet.
    color.rgb = mix(color.rgb, vec3(0.52), smoothstep(0.42, 0.0, radius) * 0.9);

    // Two noisy rings that rise with the swell.
    float ringRadius1 = sharpRing(decomposed, uTime * 0.1);
    float ringRadius2 = smoothRing(decomposed, uTime * 0.1);
    float inputRadius1 = radius + uInputVolume * 0.2;
    float inputRadius2 = radius + uInputVolume * 0.15;
    float opacity1 = mix(0.2, 0.6, uInputVolume);
    float opacity2 = mix(0.15, 0.45, uInputVolume);
    float ringAlpha1 = (inputRadius2 >= ringRadius1) ? opacity1 : 0.0;
    float ringAlpha2 = smoothstep(ringRadius2 - 0.05, ringRadius2 + 0.05, inputRadius1) * opacity2;
    float totalRingAlpha = max(ringAlpha1, ringAlpha2);
    color.rgb = 1.0 - (1.0 - color.rgb) * (1.0 - vec3(1.0) * totalRingAlpha);

    // Ramp: black, colour 1, colour 2, white. Inverted, the disc is dark and
    // the lobes carry the colour.
    float luminance = mix(color.r, 1.0 - color.r, uInverted);
    color.rgb = colorRamp(luminance, vec3(0.0), uColor1, uColor2, vec3(1.0));
    color.a *= uOpacity;
    gl_FragColor = color;
}
`;
