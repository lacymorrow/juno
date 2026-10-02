import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { cn } from "@/lib/utils";
import type { DotLook, IslandSize, Words } from "./islandModel";
import { VOICE_TRANSITION, voiceScale } from "../voiceLevel";

/**
 * The island's body: one black shape whose width, height and corner radius
 * move on a single spring, with the content layer inside crossfading (and
 * blurring, the way iOS does it) whenever the posture changes. Nothing
 * reflows: every layer fills the shell absolutely.
 */

/**
 * Hairline plus depth so the black shape reads on a black wallpaper too. The
 * deepest layer must end inside SHADOW_PAD or the window edge slices it into
 * a hard line; islandShadow.test.ts holds it there.
 */
export const ISLAND_GLOW =
  "0 0 0 0.5px rgba(255,255,255,0.22), 0 1px 2px rgba(0,0,0,0.5), 0 4px 12px rgba(0,0,0,0.35)";

/** One spring for the shape. Settles in about 320ms with no corner overshoot. */
const SPRING = { type: "spring" as const, stiffness: 380, damping: 34, mass: 1 };
const EASE = { duration: 0.15, ease: "easeOut" as const };

interface IslandShellProps {
  size: IslandSize;
  /** Which content layer is on stage; a change crossfades. */
  layerKey: string;
  reducedMotion: boolean;
  children: ReactNode;
  onClick?: () => void;
  className?: string;
}

export function IslandShell({
  size,
  layerKey,
  reducedMotion,
  children,
  onClick,
  className,
}: IslandShellProps) {
  const layerIn = reducedMotion
    ? { opacity: 1, transition: EASE }
    : { opacity: 1, filter: "blur(0px)", scale: 1, transition: { duration: 0.18, delay: 0.06 } };
  const layerStart = reducedMotion ? { opacity: 0 } : { opacity: 0, filter: "blur(6px)", scale: 0.98 };
  const layerOut = reducedMotion
    ? { opacity: 0, transition: EASE }
    : { opacity: 0, filter: "blur(6px)", transition: { duration: 0.12 } };

  return (
    <motion.div
      data-testid="island-shell"
      data-posture={layerKey}
      onClick={onClick}
      initial={false}
      animate={{ width: size.width, height: size.height, borderRadius: size.radius }}
      transition={reducedMotion ? EASE : SPRING}
      className={cn("relative overflow-hidden bg-black text-white", className)}
      style={{ boxShadow: ISLAND_GLOW, willChange: "width, height, border-radius" }}
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
  );
}

// === THE DOT ===

const DOT_KEYFRAMES = `
@keyframes isl-slow {
  0%, 100% { opacity: 0.4; transform: scale(1); }
  50%      { opacity: 0.6; transform: scale(1.08); }
}
@keyframes isl-breathe {
  0%, 100% { opacity: 0.6; transform: scale(1); }
  50%      { opacity: 1; transform: scale(1.25); }
}
@keyframes isl-ring {
  0%   { transform: scale(1); opacity: 0.3; }
  100% { transform: scale(3.5); opacity: 0; }
}
@keyframes isl-orbit {
  0%   { transform: translateX(0) scale(1); }
  25%  { transform: translateX(4px) scale(0.92); }
  50%  { transform: translateX(0) scale(1); }
  75%  { transform: translateX(-4px) scale(0.92); }
  100% { transform: translateX(0) scale(1); }
}
@keyframes isl-ripple {
  0%   { box-shadow: 0 0 0 0 rgba(255,255,255,0.14); }
  100% { box-shadow: 0 0 0 8px rgba(255,255,255,0); }
}
@keyframes isl-shake {
  0%, 100% { transform: translateX(0); }
  20%  { transform: translateX(-2px); }
  40%  { transform: translateX(2px); }
  60%  { transform: translateX(-1px); }
  80%  { transform: translateX(1px); }
}
@keyframes isl-flash {
  0%   { opacity: 1; transform: scale(1.5); }
  100% { opacity: 0.5; transform: scale(1); }
}
/* Reduce Motion is a system setting, not a per-dot choice. */
@media (prefers-reduced-motion: reduce) {
  [style*="isl-"] { animation: none !important; }
}
/* The card body scrolls behind a thin, quiet bar; never a white track. */
.island-scroll { scrollbar-width: thin; scrollbar-color: rgba(255,255,255,0.22) transparent; }
.island-scroll::-webkit-scrollbar { width: 6px; }
.island-scroll::-webkit-scrollbar-track { background: transparent; }
.island-scroll::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.22); border-radius: 3px; }
`;

function useDotKeyframes() {
  useEffect(() => {
    const id = "island-dot-keyframes";
    if (document.getElementById(id)) return;
    const style = document.createElement("style");
    style.id = id;
    style.textContent = DOT_KEYFRAMES;
    document.head.appendChild(style);
  }, []);
}

const DOT_ANIMATION: Record<DotLook["motion"], string | undefined> = {
  slow: "isl-slow 4s ease-in-out infinite",
  breathe: "isl-breathe 1.4s ease-in-out infinite",
  still: undefined,
  orbit: "isl-orbit 1.1s ease-in-out infinite",
  ripple: "isl-ripple 1.4s ease-out infinite",
  shake: "isl-shake 0.4s ease-out",
  flash: "isl-flash 0.6s ease-out forwards",
};

/** One dot, 7px. State is told by its colour and motion, never an icon. */
export function IslandDot({
  look,
  size = 7,
  level = 0,
}: {
  look: DotLook;
  size?: number;
  /** The mic level while the island listens (0..1). The dot swells with it;
   *  0 leaves the dot at its own size and motion. */
  level?: number;
}) {
  useDotKeyframes();
  const swell = voiceScale(level);
  const base = {
    width: size,
    height: size,
    borderRadius: size,
    backgroundColor: look.color,
    opacity: look.opacity,
  };
  const animation = DOT_ANIMATION[look.motion];
  return (
    <span
      className="relative flex shrink-0 items-center justify-center"
      style={{ width: size, height: size, transform: `scale(${swell})`, transition: VOICE_TRANSITION }}
      data-testid="island-dot"
      data-motion={look.motion}
      data-swell={swell.toFixed(2)}
      aria-hidden="true"
    >
      <span className="relative z-10 block" style={{ ...base, animation }} />
      {look.motion === "breathe" && (
        <span
          className="absolute block"
          style={{ ...base, opacity: 0.25, animation: "isl-ring 2s ease-out infinite" }}
        />
      )}
    </span>
  );
}

// === THE WORDS ===

const TONE_CLASS: Record<Words["tone"], string> = {
  live: "text-white/90",
  dim: "text-white/45",
  provisional: "text-white/55 italic",
  error: "text-[#FF6961]",
  plain: "text-white/75",
};

/**
 * The one line inside the ear and status postures. Newest words stay visible:
 * the box is right-to-left so overflow clips the start of the line, and a
 * fade appears on the left edge only once there is more than fits. No motion,
 * so a long dictation reads like a ticker without scrolling.
 */
export function IslandWords({ words }: { words: Words }) {
  const boxRef = useRef<HTMLDivElement>(null);
  const [overflowing, setOverflowing] = useState(false);
  // Measured again whenever the box changes size: the shell is still
  // springing open when the words first mount, so a one-time measurement
  // would call every line overflowing.
  useLayoutEffect(() => {
    const el = boxRef.current;
    if (!el) return;
    const measure = () => setOverflowing(el.scrollWidth > el.clientWidth + 1);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [words.text]);
  const mask = overflowing ? "linear-gradient(to right, transparent, black 20px)" : undefined;
  return (
    <div
      ref={boxRef}
      data-testid="island-words"
      data-overflowing={overflowing ? "true" : "false"}
      className={cn(
        "min-w-0 flex-1 overflow-hidden whitespace-nowrap text-left text-[13px] leading-none tracking-[-0.01em]",
        TONE_CLASS[words.tone],
      )}
      style={{ direction: "rtl", maskImage: mask, WebkitMaskImage: mask }}
    >
      <bdi style={{ direction: "ltr", unicodeBidi: "isolate" }}>{words.text}</bdi>
    </div>
  );
}
