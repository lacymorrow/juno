import { cn } from "@/lib/utils";
import { LINGER_MS } from "./islandModel";

const SIZE = 22;
const RADIUS = 9;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

interface LingerRingProps {
  /** 1 full, 0 empty. The ring is the timer: it drains clockwise. */
  progress: number;
  /** Draw the ring at all. Off while the answer is still arriving, while a
   *  tool waits on Allow, and once the person has engaged with the card. */
  counting: boolean;
  /** The clock is held (pointer over the island, focus inside it). */
  paused: boolean;
  onClose: () => void;
  className?: string;
}

/**
 * The close control in the card header, with the linger countdown drawn
 * around it. Clicking closes now; leaving it alone closes when the ring runs
 * out. Its label says which, for assistive tech.
 */
export function LingerRing({ progress, counting, paused, onClose, className }: LingerRingProps) {
  const seconds = Math.ceil((progress * LINGER_MS) / 1000);
  const label = counting
    ? paused
      ? "Close (paused)"
      : `Close (closes on its own in ${seconds} seconds)`
    : "Close";
  return (
    <button
      type="button"
      onClick={(event) => {
        event.stopPropagation();
        onClose();
      }}
      aria-label={label}
      data-testid="island-linger"
      data-counting={counting ? "true" : "false"}
      data-paused={paused ? "true" : "false"}
      className={cn(
        "relative flex shrink-0 items-center justify-center rounded-full text-white/40",
        "transition-colors duration-150 hover:bg-white/[0.08] hover:text-white/80",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#0A84FF]/70",
        className,
      )}
      style={{ width: SIZE, height: SIZE }}
    >
      <svg
        width={SIZE}
        height={SIZE}
        viewBox={`0 0 ${SIZE} ${SIZE}`}
        aria-hidden="true"
        className="absolute inset-0"
        style={{ transform: "rotate(-90deg)" }}
      >
        <circle
          cx={SIZE / 2}
          cy={SIZE / 2}
          r={RADIUS}
          fill="none"
          stroke="rgba(255,255,255,0.35)"
          strokeWidth={1.5}
          strokeLinecap="round"
          strokeDasharray={CIRCUMFERENCE}
          strokeDashoffset={counting ? CIRCUMFERENCE * (1 - progress) : CIRCUMFERENCE}
          style={{ transition: "stroke-dashoffset 60ms linear" }}
        />
      </svg>
      <svg width="8" height="8" viewBox="0 0 8 8" fill="none" aria-hidden="true">
        <path d="M1 1l6 6M7 1L1 7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
    </button>
  );
}
