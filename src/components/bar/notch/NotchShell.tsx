import type { ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { curlMirror, type CurlCorner } from "@/lib/notchWells";
import { SHOULDER, curlUnderClip, type NotchShape } from "./notchModel";

/**
 * The Notch's body. Two forms, one spring.
 *
 * In the notch: a black shape flush with an edge of the display, with the
 * small concave shoulders the hardware notch has where it meets the screen's
 * edge, and round far corners. At the top centre it rests as only the menu bar
 * strip, so on a notched MacBook it is the notch; on the bottom and the sides
 * it is the same tab growing out of that edge.
 *
 * In a corner: the corner of the screen peeled back like a page. The black is
 * what is underneath; the light triangle is the page's underside, folded over.
 * At rest the peel is a small triangle; as the island opens, the peel opens
 * with it into a card whose inner corner stays folded.
 *
 * The island's content layer sits in `shape.content` and crossfades on a
 * posture change exactly as it does in `IslandShell`.
 */

/** Same spring as the Island's shell, so the two looks move alike. */
const SPRING = { type: "spring" as const, stiffness: 380, damping: 34, mass: 1 };
const EASE = { duration: 0.15, ease: "easeOut" as const };

/** The page's underside: light at the folded tip, shaded toward the crease. */
const FLAP_FILL = "linear-gradient(315deg, #fafafa 0%, #e6e6e9 45%, #b8b8be 100%)";

/** The flap: the half of its box below the fold line. */
const FLAP_CLIP = "polygon(100% 0, 100% 100%, 0 100%)";

/** Shadow then hairline, each a triangle `out` points past the flap's fold line. */
const FLAP_EDGES = [
  { out: 2, color: "rgba(0,0,0,0.22)" },
  { out: 1, color: "rgba(0,0,0,0.45)" },
] as const;

interface NotchShellProps {
  shape: NotchShape;
  corner: CurlCorner;
  /** Which content layer is on stage; a change crossfades. */
  layerKey: string;
  reducedMotion: boolean;
  children: ReactNode;
  onClick?: () => void;
}

export function NotchShell({ shape, corner, layerKey, reducedMotion, children, onClick }: NotchShellProps) {
  const transition = reducedMotion ? EASE : SPRING;
  const layerIn = reducedMotion
    ? { opacity: 1, transition: EASE }
    : { opacity: 1, filter: "blur(0px)", transition: { duration: 0.18, delay: 0.06 } };
  const layerStart = reducedMotion ? { opacity: 0 } : { opacity: 0, filter: "blur(6px)" };
  const layerOut = reducedMotion
    ? { opacity: 0, transition: EASE }
    : { opacity: 0, filter: "blur(6px)", transition: { duration: 0.12 } };

  return (
    <motion.div
      data-testid="notch-shell"
      data-form={shape.form}
      data-edge={shape.form === "notch" ? shape.edge : undefined}
      data-corner={shape.form === "curl" ? corner : undefined}
      data-posture={layerKey}
      onClick={onClick}
      initial={false}
      animate={{ width: shape.width, height: shape.height }}
      transition={transition}
      className="relative text-white"
      style={{ willChange: "width, height" }}
    >
      {shape.form === "notch" ? (
        <NotchBody shape={shape} transition={transition} />
      ) : (
        <CurlBody shape={shape} corner={corner} transition={transition} />
      )}
      <motion.div
        className="absolute overflow-hidden"
        initial={false}
        animate={{
          top: shape.content.top,
          left: shape.content.left,
          width: shape.content.width,
          height: shape.content.height,
        }}
        transition={transition}
      >
        <AnimatePresence initial={false}>
          <motion.div
            key={layerKey}
            className="absolute inset-0"
            initial={layerStart}
            animate={layerIn}
            exit={layerOut}
          >
            {children}
          </motion.div>
        </AnimatePresence>
      </motion.div>
    </motion.div>
  );
}

type Transition = typeof SPRING | typeof EASE;

/** Where the shoulders and the body sit for each edge, and which corners are round. */
const EDGE_LAYOUT = {
  top: {
    shoulders: [
      { at: { top: 0, left: 0 }, circle: "0% 100%" },
      { at: { top: 0, right: 0 }, circle: "100% 100%" },
    ],
    body: { top: 0, bottom: 0, left: SHOULDER, right: SHOULDER },
    round: ["borderBottomLeftRadius", "borderBottomRightRadius"],
  },
  bottom: {
    shoulders: [
      { at: { bottom: 0, left: 0 }, circle: "0% 0%" },
      { at: { bottom: 0, right: 0 }, circle: "100% 0%" },
    ],
    body: { top: 0, bottom: 0, left: SHOULDER, right: SHOULDER },
    round: ["borderTopLeftRadius", "borderTopRightRadius"],
  },
  left: {
    shoulders: [
      { at: { top: 0, left: 0 }, circle: "100% 0%" },
      { at: { bottom: 0, left: 0 }, circle: "100% 100%" },
    ],
    body: { top: SHOULDER, bottom: SHOULDER, left: 0, right: 0 },
    round: ["borderTopRightRadius", "borderBottomRightRadius"],
  },
  right: {
    shoulders: [
      { at: { top: 0, right: 0 }, circle: "0% 0%" },
      { at: { bottom: 0, right: 0 }, circle: "0% 100%" },
    ],
    body: { top: SHOULDER, bottom: SHOULDER, left: 0, right: 0 },
    round: ["borderTopLeftRadius", "borderBottomLeftRadius"],
  },
} as const;

function NotchBody({ shape, transition }: { shape: NotchShape; transition: Transition }) {
  // The shoulders: a square of black with a quarter circle cut from the
  // corner away from the screen's edge, so the shape flares into that edge.
  const layout = EDGE_LAYOUT[shape.edge];
  const radii = Object.fromEntries(layout.round.map((corner) => [corner, shape.radius]));
  return (
    <>
      {layout.shoulders.map(({ at, circle }) => (
        <span
          key={circle}
          aria-hidden
          className="absolute block"
          style={{
            ...at,
            width: SHOULDER,
            height: SHOULDER,
            background: `radial-gradient(circle at ${circle}, transparent ${SHOULDER - 0.5}px, #000 ${SHOULDER}px)`,
          }}
        />
      ))}
      <motion.div
        aria-hidden
        className="absolute bg-black"
        style={layout.body}
        initial={false}
        animate={radii}
        transition={transition}
      />
    </>
  );
}

function CurlBody({
  shape,
  corner,
  transition,
}: {
  shape: NotchShape;
  corner: CurlCorner;
  transition: Transition;
}) {
  // Drawn for the top-left corner and mirrored, so every corner peels the
  // same way toward the middle of the screen.
  return (
    <div aria-hidden className="absolute inset-0" style={{ transform: curlMirror(corner) }}>
      <motion.div
        className="absolute inset-0 bg-black"
        initial={false}
        animate={{ clipPath: curlUnderClip(shape.width, shape.height, shape.fold) }}
        transition={transition}
      />
      <motion.div
        className="absolute bottom-0 right-0"
        initial={false}
        animate={{ width: shape.fold, height: shape.fold }}
        transition={transition}
      >
        {/* The edge where the page lifts off the black: two flat triangles
            just outside the flap, a short shadow and a hairline, so the page
            reads on a light wallpaper too. Not a CSS filter: a filter on a
            shape that shrinks leaves its old pixels behind in a transparent
            window, which is the trail a closing corner left. */}
        {FLAP_EDGES.map(({ out, color }) => (
          <div
            key={out}
            aria-hidden
            className="absolute bottom-0 right-0"
            style={{ top: -out, left: -out, background: color, clipPath: FLAP_CLIP }}
          />
        ))}
        <div
          data-testid="notch-curl-flap"
          className="absolute inset-0"
          style={{ background: FLAP_FILL, clipPath: FLAP_CLIP }}
        />
      </motion.div>
    </div>
  );
}
