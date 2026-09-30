import { useEffect } from "react";
import { animate, motion, useMotionValue } from "motion/react";
import {
  CIRCUMFERENCE,
  GAP_SHARE,
  NARROW_WINDOW,
  RING_BOX,
  RING_RADIUS,
  TRAVEL_ARC,
  type RingLook,
} from "./haloModel";

/**
 * The ring itself: one SVG, one circle for the track and one for the gauge.
 * Fill is a stroke offset, travel is a rotating dash, the ticks are short
 * radial lines, the gap is a dash pattern with a red piece in the hole, and
 * the speaking pulses are copies of the track scaling out. Nothing here
 * decides what to draw; the model does.
 */

/** The SVG is the narrow window: the ring centred, room for pulses around it. */
export const RING_SVG = NARROW_WINDOW.width;
const C = RING_SVG / 2;
/** Where the disc edge sits. */
const DISC_R = RING_BOX / 2;
/** Degrees the travelling arc spans; its leading edge is this far past its start. */
const TRAVEL_DEG = TRAVEL_ARC * 360;
const TRAVEL_MS = 1400;

const KEYFRAMES = `
@keyframes halo-breathe {
  0%, 100% { opacity: 0.35; transform: scale(1); }
  50%      { opacity: 0.6; transform: scale(1.03); }
}
@keyframes halo-pulse {
  0%   { transform: scale(1); opacity: 0.35; }
  100% { transform: scale(1.42); opacity: 0; }
}
@keyframes halo-tick {
  0%   { transform: scaleY(0); opacity: 0; }
  100% { transform: scaleY(1); opacity: 1; }
}
@keyframes halo-gap {
  0%, 100% { opacity: 1; }
  50%      { opacity: 0.55; }
}
@media (prefers-reduced-motion: reduce) {
  [style*="halo-"] { animation: none !important; }
}
`;

function useRingKeyframes() {
  useEffect(() => {
    const id = "halo-ring-keyframes";
    if (document.getElementById(id)) return;
    const style = document.createElement("style");
    style.id = id;
    style.textContent = KEYFRAMES;
    document.head.appendChild(style);
  }, []);
}

/** Rotate about the ring's centre, whatever the element's own box is. */
const ABOUT_CENTRE = { transformBox: "fill-box", transformOrigin: "center" } as const;

interface HaloRingProps {
  look: RingLook;
  /** Degrees from 12 o'clock of every tick this turn has earned. */
  ticks: number[];
  /** Where the error gap sits, degrees from 12 o'clock. */
  gapAt: number;
  reducedMotion: boolean;
}

/**
 * The travelling arc's rotation. Runs forever while Juno works; when a tool
 * holds it, it glides forward to the tick instead of snapping there.
 */
function useTravel(look: RingLook, reducedMotion: boolean) {
  const rotate = useMotionValue(0);
  const travelling = look.verb === "travel";
  const { holdAt, slow } = look;
  useEffect(() => {
    if (!travelling) {
      rotate.set(0);
      return;
    }
    if (holdAt !== null || reducedMotion) {
      // Leading edge on the tick: the arc starts TRAVEL_DEG before it. Always
      // move forward so a hold never runs the arc backwards.
      const target = (holdAt ?? TRAVEL_DEG) - TRAVEL_DEG;
      const now = rotate.get();
      const forward = ((((target - now) % 360) + 360) % 360);
      const controls = animate(rotate, now + forward, { duration: 0.4, ease: "easeOut" });
      return () => controls.stop();
    }
    const from = rotate.get();
    const controls = animate(rotate, from + 360, {
      duration: (TRAVEL_MS / 1000) * (slow ? 2 : 1),
      ease: "linear",
      repeat: Infinity,
    });
    return () => controls.stop();
  }, [travelling, holdAt, slow, reducedMotion, rotate]);
  return rotate;
}

export function HaloRing({ look, ticks, gapAt, reducedMotion }: HaloRingProps) {
  useRingKeyframes();
  const rotate = useTravel(look, reducedMotion);
  const { verb } = look;

  // The track is the quiet ring. It carries the look's colour whenever the
  // gauge is not drawn on top of it.
  const gaugeOn = verb === "fill" || verb === "drain" || verb === "close";
  const trackColor = gaugeOn || verb === "travel" ? "#FFFFFF" : look.color;
  const trackOpacity = gaugeOn || verb === "travel" ? 0.16 : verb === "gap" ? 0 : look.opacity;
  const breathe = verb === "rest" && look.breathe && !reducedMotion;

  const gaugeOffset = CIRCUMFERENCE * (1 - look.level);
  const gaugeTransition =
    verb === "close" ? "stroke-dashoffset 400ms ease-out, stroke 200ms" : "stroke-dashoffset 120ms linear, stroke 200ms";

  const gapLen = CIRCUMFERENCE * GAP_SHARE;
  const gapStart = gapAt - GAP_SHARE * 180;

  return (
    <svg
      width={RING_SVG}
      height={RING_SVG}
      viewBox={`0 0 ${RING_SVG} ${RING_SVG}`}
      className="block"
      data-testid="halo-ring"
      data-verb={verb}
      data-hold={look.holdAt === null ? undefined : String(look.holdAt)}
      data-level={look.level.toFixed(2)}
      aria-hidden="true"
    >
      {/* The disc: a dark backing so the ring and the words inside read on any wallpaper. */}
      <circle cx={C} cy={C} r={DISC_R} fill="#000000" />
      <circle cx={C} cy={C} r={DISC_R} fill="none" stroke="rgba(255,255,255,0.10)" strokeWidth={0.5} />

      {/* Speaking: the ring pulses outward in soft concentric rings. */}
      {verb === "pulse" && !reducedMotion &&
        [0, 1, 2].map((i) => (
          <circle
            key={i}
            cx={C}
            cy={C}
            r={RING_RADIUS}
            fill="none"
            stroke="#FFFFFF"
            strokeWidth={1.5}
            style={{
              ...ABOUT_CENTRE,
              animation: `halo-pulse 1.8s ease-out ${i * 0.6}s infinite`,
              opacity: 0,
            }}
          />
        ))}

      {/* The track: the thin, quiet ring. */}
      <circle
        cx={C}
        cy={C}
        r={RING_RADIUS}
        fill="none"
        stroke={trackColor}
        strokeWidth={2}
        style={{
          ...ABOUT_CENTRE,
          opacity: trackOpacity,
          transition: "opacity 200ms, stroke 200ms",
          animation: breathe ? "halo-breathe 6s ease-in-out infinite" : undefined,
        }}
      />

      {/* Fill, drain, close: a stroke offset, clockwise from 12 o'clock. */}
      {gaugeOn && (
        <circle
          cx={C}
          cy={C}
          r={RING_RADIUS}
          fill="none"
          stroke={look.color}
          strokeWidth={3}
          strokeLinecap="round"
          strokeDasharray={CIRCUMFERENCE}
          strokeDashoffset={gaugeOffset}
          transform={`rotate(-90 ${C} ${C})`}
          style={{ opacity: look.opacity, transition: gaugeTransition }}
          data-testid="halo-gauge"
        />
      )}

      {/* Travel: an indeterminate arc, or the same arc paused at a tick. */}
      {verb === "travel" && (
        <motion.g style={{ rotate, ...ABOUT_CENTRE }} data-testid="halo-travel">
          <circle
            cx={C}
            cy={C}
            r={RING_RADIUS}
            fill="none"
            stroke={look.color}
            strokeWidth={3}
            strokeLinecap="round"
            strokeDasharray={`${CIRCUMFERENCE * TRAVEL_ARC} ${CIRCUMFERENCE}`}
            transform={`rotate(-90 ${C} ${C})`}
            style={{ opacity: look.opacity }}
          />
        </motion.g>
      )}

      {/* Error: the ring with a gap where it failed, the gap painted red. */}
      {verb === "gap" && (
        <>
          <circle
            cx={C}
            cy={C}
            r={RING_RADIUS}
            fill="none"
            stroke="#FFFFFF"
            strokeWidth={2}
            strokeLinecap="round"
            strokeDasharray={`${CIRCUMFERENCE - gapLen} ${gapLen}`}
            transform={`rotate(${gapStart + GAP_SHARE * 360 - 90} ${C} ${C})`}
            style={{ opacity: look.opacity }}
          />
          <circle
            cx={C}
            cy={C}
            r={RING_RADIUS}
            fill="none"
            stroke="#FF453A"
            strokeWidth={3}
            strokeLinecap="round"
            strokeDasharray={`${gapLen * 0.7} ${CIRCUMFERENCE}`}
            transform={`rotate(${gapAt - GAP_SHARE * 0.7 * 180 - 90} ${C} ${C})`}
            /* No transform-box here: it would move the attribute rotation's
               centre off the ring. The animation only touches opacity. */
            style={{ animation: reducedMotion ? undefined : "halo-gap 1.6s ease-in-out infinite" }}
            data-testid="halo-gap"
          />
        </>
      )}

      {/* Ticks: one fixed mark per tool call that finished. */}
      {/* The rotation lives on the group so the mark's own scale-in cannot override it. */}
      {ticks.map((angle, i) => (
        <g key={i} transform={`rotate(${angle} ${C} ${C})`} data-testid="halo-tick" data-angle={angle}>
          <line
            x1={C}
            y1={C - RING_RADIUS - 4}
            x2={C}
            y2={C - RING_RADIUS + 4}
            stroke="#FFFFFF"
            strokeWidth={2}
            strokeLinecap="round"
            style={{
              ...ABOUT_CENTRE,
              opacity: 0.9,
              animation: reducedMotion ? undefined : "halo-tick 200ms ease-out",
            }}
          />
        </g>
      ))}
    </svg>
  );
}
