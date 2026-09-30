import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { cn } from "@/lib/utils";
import {
  LINGER_MS,
  WAVE_SAMPLES,
  formatCounter,
  type CounterMode,
  type Line,
  type StudioSize,
  type WaveLook,
} from "./studioModel";

/**
 * The studio's body: one light shape whose width, height and corner radius
 * move on a single spring, with the content layer inside crossfading (and
 * blurring a little) whenever the posture changes. Nothing reflows: every
 * layer fills the shell absolutely.
 */

/** A hairline rim, a close shadow and a soft one, so a white surface stands
 *  off a white window and still lifts on a dark one. */
export const STUDIO_SHADOW =
  "0 0 0 0.5px rgba(0,0,0,0.14), 0 1px 2px rgba(0,0,0,0.08), 0 10px 28px rgba(0,0,0,0.16)";

/** One spring for the shape. Settles in about 320ms with no overshoot. */
const SPRING = { type: "spring" as const, stiffness: 380, damping: 34, mass: 1 };
const EASE = { duration: 0.15, ease: "easeOut" as const };

interface StudioShellProps {
  size: StudioSize;
  /** Which content layer is on stage; a change crossfades. */
  layerKey: string;
  reducedMotion: boolean;
  children: ReactNode;
  onClick?: () => void;
  className?: string;
}

export function StudioShell({
  size,
  layerKey,
  reducedMotion,
  children,
  onClick,
  className,
}: StudioShellProps) {
  const layerIn = reducedMotion
    ? { opacity: 1, transition: EASE }
    : { opacity: 1, filter: "blur(0px)", transition: { duration: 0.18, delay: 0.06 } };
  const layerStart = reducedMotion ? { opacity: 0 } : { opacity: 0, filter: "blur(4px)" };
  const layerOut = reducedMotion
    ? { opacity: 0, transition: EASE }
    : { opacity: 0, filter: "blur(4px)", transition: { duration: 0.12 } };

  return (
    <motion.div
      data-testid="studio-shell"
      data-posture={layerKey}
      onClick={onClick}
      initial={false}
      animate={{ width: size.width, height: size.height, borderRadius: size.radius }}
      transition={reducedMotion ? EASE : SPRING}
      className={cn("relative overflow-hidden bg-white text-black", className)}
      style={{ boxShadow: STUDIO_SHADOW, willChange: "width, height, border-radius" }}
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

// === KEYFRAMES ===

const KEYFRAMES = `
@keyframes stu-drift {
  0%, 100% { transform: translateY(0); }
  50%      { transform: translateY(1px); }
}
@keyframes stu-shake {
  0%, 100% { transform: translateX(0); }
  20%  { transform: translateX(-2px); }
  40%  { transform: translateX(2px); }
  60%  { transform: translateX(-1px); }
  80%  { transform: translateX(1px); }
}
/* Reduce Motion is a system setting, not a per-strip choice. */
@media (prefers-reduced-motion: reduce) {
  [style*="stu-"] { animation: none !important; }
}
/* The script scrolls behind a thin, quiet bar; never a grey track. */
.studio-scroll { scrollbar-width: thin; scrollbar-color: rgba(0,0,0,0.18) transparent; }
.studio-scroll::-webkit-scrollbar { width: 6px; }
.studio-scroll::-webkit-scrollbar-track { background: transparent; }
.studio-scroll::-webkit-scrollbar-thumb { background: rgba(0,0,0,0.18); border-radius: 3px; }
`;

function useStudioKeyframes() {
  useEffect(() => {
    const id = "studio-keyframes";
    if (document.getElementById(id)) return;
    const style = document.createElement("style");
    style.id = id;
    style.textContent = KEYFRAMES;
    document.head.appendChild(style);
  }, []);
}

// === THE WAVEFORM STRIP ===

/** Bar width and the gap between bars, in the strip's own units. */
const BAR_W = 2;
const BAR_GAP = 1;
const STRIP_UNITS = WAVE_SAMPLES * (BAR_W + BAR_GAP) - BAR_GAP;

interface WaveStripProps {
  /** The last WAVE_SAMPLES levels, oldest first, each 0..1. */
  samples: number[];
  look: WaveLook;
  width: number;
  height: number;
  className?: string;
}

/**
 * The waveform: the level history, newest at the right, as thin bars mirrored
 * around a centre line. Silence is the hairline, never nothing. State is told
 * by colour (who is speaking) and by whether anything moves.
 */
export function WaveStrip({ samples, look, width, height, className }: WaveStripProps) {
  useStudioKeyframes();
  const animation =
    look.mode === "drift"
      ? "stu-drift 2.4s ease-in-out infinite"
      : look.mode === "shake"
        ? "stu-shake 0.4s ease-out"
        : undefined;
  const mid = height / 2;
  const usable = height - 2;
  const live = samples.some((s) => s > 0.01);
  return (
    <svg
      data-testid="studio-wave"
      data-mode={look.mode}
      data-color={look.color}
      data-live={live ? "true" : "false"}
      width={width}
      height={height}
      viewBox={`0 0 ${STRIP_UNITS} ${height}`}
      preserveAspectRatio="none"
      aria-hidden="true"
      className={cn("block shrink-0", className)}
      style={{ animation, opacity: look.opacity, transition: "opacity 200ms" }}
    >
      <line
        x1={0}
        x2={STRIP_UNITS}
        y1={mid}
        y2={mid}
        stroke={look.color}
        strokeWidth={1}
        opacity={live ? 0.35 : 1}
        vectorEffect="non-scaling-stroke"
        style={{ transition: "stroke 200ms, opacity 200ms" }}
      />
      {live &&
        samples.map((s, i) => {
          const h = s * usable;
          // A sample shorter than the hairline is the hairline; drawing it
          // as a bar turned silence into a dashed line.
          if (h < 1.5) return null;
          return (
            <rect
              key={i}
              x={i * (BAR_W + BAR_GAP)}
              y={mid - h / 2}
              width={BAR_W}
              height={h}
              rx={1}
              fill={look.color}
            />
          );
        })}
    </svg>
  );
}

// === THE TELEPROMPTER ===

interface TeleprompterProps {
  final: string;
  provisional: string;
  className?: string;
}

/**
 * Two lines. The newest words sit at the bottom; older words rise out of the
 * top behind a fade. Final words are solid, provisional words lighter. No
 * horizontal motion, ever.
 */
export function Teleprompter({ final, provisional, className }: TeleprompterProps) {
  const mask = "linear-gradient(to bottom, transparent, black 10px)";
  return (
    <div
      data-testid="studio-prompter"
      className={cn(
        "flex min-w-0 flex-1 flex-col justify-end overflow-hidden text-[13px] leading-[17px] tracking-[-0.01em]",
        className,
      )}
      style={{ height: 34, maskImage: mask, WebkitMaskImage: mask }}
    >
      <p className="m-0 break-words">
        <span className="text-black/85">{final}</span>
        {provisional && (
          <span className="text-black/40" data-testid="studio-provisional">
            {provisional}
          </span>
        )}
      </p>
    </div>
  );
}

// === THE LINE ===

const TONE_CLASS: Record<Line["tone"], string> = {
  live: "text-black/85",
  dim: "text-black/45",
  provisional: "text-black/40",
  error: "text-[#FF453A]",
  plain: "text-black/70",
  speech: "text-[#0A84FF]",
  direction: "italic text-black/50",
};

/** One line beneath the strip. Newest words stay visible: the box clips the
 *  start of a long line and fades its left edge, only once there is more
 *  than fits. */
export function StudioLine({ line, className }: { line: Line; className?: string }) {
  const boxRef = useRef<HTMLDivElement>(null);
  const [overflowing, setOverflowing] = useState(false);
  // Measured again whenever the box changes size: the shell is still
  // springing open when the line first mounts, so a one-time measurement
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
  }, [line.text]);
  const mask = overflowing ? "linear-gradient(to right, transparent, black 16px)" : undefined;
  return (
    <div
      ref={boxRef}
      data-testid="studio-line"
      data-tone={line.tone}
      data-overflowing={overflowing ? "true" : "false"}
      className={cn(
        "min-w-0 flex-1 overflow-hidden whitespace-nowrap text-left text-[13px] leading-none tracking-[-0.01em]",
        TONE_CLASS[line.tone],
        className,
      )}
      style={{ direction: "rtl", maskImage: mask, WebkitMaskImage: mask }}
    >
      <bdi style={{ direction: "ltr", unicodeBidi: "isolate" }}>{line.text}</bdi>
    </div>
  );
}

// === THE TAPE COUNTER ===

/** `mm:ss.f` in tabular figures. It counts while Juno works and holds after. */
export function TapeCounter({ ms, mode, className }: { ms: number; mode: CounterMode; className?: string }) {
  // Nothing to hold when nothing ran: a held 00:00.0 is a number about nothing.
  if (mode === "off" || (mode === "held" && ms === 0)) return null;
  return (
    <span
      data-testid="studio-counter"
      data-mode={mode}
      className={cn(
        "shrink-0 select-none text-[11px] tabular-nums tracking-[0.02em]",
        mode === "running" ? "text-black/55" : "text-black/35",
        className,
      )}
      aria-label={mode === "running" ? `Working for ${formatCounter(ms)}` : `Took ${formatCounter(ms)}`}
    >
      {formatCounter(ms)}
    </span>
  );
}

// === THE TAPE ===

interface TapeProps {
  /** 1 full, 0 empty. The tape is the timer: it runs out right to left. */
  progress: number;
  /** Draw the tape at all. Off while the answer is still arriving, while a
   *  tool waits on Allow, and once the person has engaged with the script. */
  counting: boolean;
  /** The clock is held (pointer over the studio, focus inside it). */
  paused: boolean;
}

/** A hairline under the script header that runs out before the script closes. */
export function Tape({ progress, counting, paused }: TapeProps) {
  const seconds = Math.ceil((progress * LINGER_MS) / 1000);
  return (
    <div
      data-testid="studio-tape"
      data-counting={counting ? "true" : "false"}
      data-paused={paused ? "true" : "false"}
      role="timer"
      aria-label={
        counting ? (paused ? "Closing paused" : `Closes on its own in ${seconds} seconds`) : "Open"
      }
      className="relative h-px w-full bg-black/[0.08]"
    >
      <div
        className="absolute inset-y-0 left-0 bg-black/30"
        style={{
          width: counting ? `${progress * 100}%` : "0%",
          transition: "width 60ms linear",
        }}
      />
    </div>
  );
}

// === THE CLOSE MARK ===

export function CloseMark({ onClose, className }: { onClose: () => void; className?: string }) {
  return (
    <button
      type="button"
      onClick={(event) => {
        event.stopPropagation();
        onClose();
      }}
      aria-label="Close"
      data-testid="studio-close"
      className={cn(
        "flex size-[22px] shrink-0 items-center justify-center rounded-full text-black/40",
        "transition-colors duration-150 hover:bg-black/[0.06] hover:text-black/80",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#0A84FF]/70",
        className,
      )}
    >
      <svg width="8" height="8" viewBox="0 0 8 8" fill="none" aria-hidden="true">
        <path d="M1 1l6 6M7 1L1 7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
    </button>
  );
}
