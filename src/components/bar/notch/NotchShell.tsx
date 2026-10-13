import type { ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { curlMirror, type CurlCorner } from "@/lib/notchWells";
import { SHOULDER, curlUnderClip, type NotchShape } from "./notchModel";

/**
 * The Notch's body. Two forms, one spring.
 *
 * In the notch: a black shape flush with the top of the display, with the
 * small concave shoulders the hardware notch has where it meets the screen's
 * edge, and round bottom corners. At rest it is only the menu bar strip, so on
 * a notched MacBook it is the notch.
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

function NotchBody({ shape, transition }: { shape: NotchShape; transition: Transition }) {
  // The shoulders: a square of black with a quarter circle cut from its lower
  // outer corner, so the shape flares into the screen's top edge.
  const shoulder = (side: "left" | "right") => ({
    [side]: 0,
    width: SHOULDER,
    height: SHOULDER,
    background: `radial-gradient(circle at ${side === "left" ? "0%" : "100%"} 100%, transparent ${SHOULDER - 0.5}px, #000 ${SHOULDER}px)`,
  });
  return (
    <>
      <span aria-hidden className="absolute top-0 block" style={shoulder("left")} />
      <span aria-hidden className="absolute top-0 block" style={shoulder("right")} />
      <motion.div
        aria-hidden
        className="absolute top-0 bg-black"
        style={{ left: SHOULDER, right: SHOULDER }}
        initial={false}
        animate={{
          height: shape.height,
          borderBottomLeftRadius: shape.radius,
          borderBottomRightRadius: shape.radius,
        }}
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
      {/* The shadow sits on a wrapper: a clip-path would cut its own. */}
      <motion.div
        className="absolute bottom-0 right-0"
        initial={false}
        animate={{ width: shape.fold, height: shape.fold }}
        transition={transition}
        // A hairline so the page reads on a light wallpaper too, and a
        // short shadow where it lifts off the black.
        style={{
          filter: "drop-shadow(0 0 0.5px rgba(0,0,0,0.45)) drop-shadow(-1px -1px 2px rgba(0,0,0,0.4))",
        }}
      >
        <div
          data-testid="notch-curl-flap"
          className="h-full w-full"
          style={{ background: FLAP_FILL, clipPath: "polygon(100% 0, 100% 100%, 0 100%)" }}
        />
      </motion.div>
    </div>
  );
}
