import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import {
  SYSTEM_BLUE,
  SYSTEM_GREEN,
  SYSTEM_RED,
  inkFor,
  type DotLook,
  type Rail,
  type Segment,
  type Theme,
} from "./narratorModel";

/**
 * The strip's parts: the dot, a bead, the words of a segment, and the rail.
 * Flat, in the system's light or dark, with system blue as the one accent.
 * State is told by colour and motion; there are no icons.
 */

// === THEME ===

export interface Palette {
  /** The strip's fill. */
  surface: string;
  /** The sheet's fill, a step off the strip so the two read as layers. */
  sheet: string;
  /** Text and the resting dot. */
  ink: string;
  /** The hairline around the strip and the sheet. */
  edge: string;
  /** The empty rail. */
  rail: string;
  shadow: string;
}

export function paletteFor(theme: Theme): Palette {
  if (theme === "dark") {
    return {
      surface: "#1C1C1E",
      sheet: "#232326",
      ink: inkFor("dark"),
      edge: "rgba(255,255,255,0.14)",
      rail: "rgba(255,255,255,0.12)",
      shadow: "0 1px 2px rgba(0,0,0,0.45), 0 10px 28px rgba(0,0,0,0.4)",
    };
  }
  return {
    surface: "#FFFFFF",
    sheet: "#F7F7F9",
    ink: inkFor("light"),
    edge: "rgba(0,0,0,0.12)",
    rail: "rgba(0,0,0,0.1)",
    shadow: "0 1px 2px rgba(0,0,0,0.12), 0 10px 28px rgba(0,0,0,0.16)",
  };
}

// === KEYFRAMES ===

const KEYFRAMES = `
@keyframes nar-slow {
  0%, 100% { opacity: 0.4; transform: scale(1); }
  50%      { opacity: 0.6; transform: scale(1.08); }
}
@keyframes nar-breathe {
  0%, 100% { opacity: 0.6; transform: scale(1); }
  50%      { opacity: 1; transform: scale(1.25); }
}
@keyframes nar-orbit {
  0%   { transform: translateX(0) scale(1); }
  25%  { transform: translateX(3px) scale(0.92); }
  50%  { transform: translateX(0) scale(1); }
  75%  { transform: translateX(-3px) scale(0.92); }
  100% { transform: translateX(0) scale(1); }
}
@keyframes nar-ripple {
  0%   { box-shadow: 0 0 0 0 rgba(10,132,255,0.35); }
  100% { box-shadow: 0 0 0 8px rgba(10,132,255,0); }
}
@keyframes nar-shake {
  0%, 100% { transform: translateX(0); }
  20%  { transform: translateX(-2px); }
  40%  { transform: translateX(2px); }
  60%  { transform: translateX(-1px); }
  80%  { transform: translateX(1px); }
}
@keyframes nar-flash {
  0%   { opacity: 1; transform: scale(1.5); }
  100% { opacity: 0.6; transform: scale(1); }
}
@keyframes nar-bead {
  0%, 100% { opacity: 1; transform: scale(1); }
  50%      { opacity: 0.45; transform: scale(0.8); }
}
/* Reduce Motion is a system setting, not a per-dot choice. */
@media (prefers-reduced-motion: reduce) {
  [style*="nar-"] { animation: none !important; }
}
/* The sheet body scrolls behind a thin, quiet bar; never a white track. */
.narrator-scroll { scrollbar-width: thin; scrollbar-color: rgba(127,127,127,0.35) transparent; }
.narrator-scroll::-webkit-scrollbar { width: 6px; }
.narrator-scroll::-webkit-scrollbar-track { background: transparent; }
.narrator-scroll::-webkit-scrollbar-thumb { background: rgba(127,127,127,0.35); border-radius: 3px; }
`;

function useKeyframes() {
  useEffect(() => {
    const id = "narrator-keyframes";
    if (document.getElementById(id)) return;
    const style = document.createElement("style");
    style.id = id;
    style.textContent = KEYFRAMES;
    document.head.appendChild(style);
  }, []);
}

// === THE DOT ===

const DOT_ANIMATION: Record<DotLook["motion"], string | undefined> = {
  slow: "nar-slow 4s ease-in-out infinite",
  breathe: "nar-breathe 1.4s ease-in-out infinite",
  still: undefined,
  orbit: "nar-orbit 1.1s ease-in-out infinite",
  ripple: "nar-ripple 1.4s ease-out infinite",
  shake: "nar-shake 0.4s ease-out",
  flash: "nar-flash 0.6s ease-out forwards",
};

/** One dot, 8px, at the strip's left end. Its colour and motion are the state. */
export function NarratorDot({ look }: { look: DotLook }) {
  useKeyframes();
  const size = 8;
  return (
    <span
      className="relative flex shrink-0 items-center justify-center"
      style={{ width: size, height: size }}
      data-testid="bar-dot"
      data-motion={look.motion}
      aria-hidden="true"
    >
      <span
        className="block"
        style={{
          width: size,
          height: size,
          borderRadius: size,
          backgroundColor: look.color,
          opacity: look.opacity,
          animation: DOT_ANIMATION[look.motion],
          transition: "background-color 200ms ease, opacity 200ms ease",
        }}
      />
    </span>
  );
}

// === A BEAD ===

/** A step on the timeline: 6px, ink when done, blue and pulsing while live,
 *  red when it failed. Collapsed steps are the bead alone, with a tooltip. */
export function Bead({
  segment,
  theme,
  title,
}: {
  segment: Segment;
  theme: Theme;
  title?: string;
}) {
  const color = segment.failed ? SYSTEM_RED : segment.live ? SYSTEM_BLUE : inkFor(theme);
  const opacity = segment.failed || segment.live ? 1 : segment.tone === "dim" ? 0.3 : 0.5;
  return (
    <span
      className="relative flex shrink-0 items-center justify-center"
      style={{ width: 6, height: 6 }}
      data-testid="bar-bead"
      data-live={segment.live ? "true" : "false"}
      data-failed={segment.failed ? "true" : "false"}
      title={title}
      aria-hidden={title ? undefined : "true"}
      aria-label={title}
    >
      <span
        className="block rounded-full"
        style={{
          width: 6,
          height: 6,
          backgroundColor: color,
          opacity,
          animation: segment.live ? "nar-bead 1s ease-in-out infinite" : undefined,
          transition: "background-color 200ms ease, opacity 200ms ease",
        }}
      />
    </span>
  );
}

// === WORDS ===

const TONE_STYLE: Record<Segment["tone"], { opacity: number; color?: string; italic?: boolean }> = {
  live: { opacity: 0.92 },
  plain: { opacity: 0.78 },
  dim: { opacity: 0.42 },
  provisional: { opacity: 0.6, color: SYSTEM_GREEN, italic: true },
  green: { opacity: 1, color: SYSTEM_GREEN },
  error: { opacity: 1, color: SYSTEM_RED },
};

/**
 * The words of one segment, on one line. Read from the start with an ellipsis
 * (a question, a step, the answer), or, with `tail`, keeping the newest words
 * visible: the box is right-to-left so overflow clips the start, with a fade
 * on the left edge once there is more than fits. No motion either way, so a
 * long dictation reads like a ticker.
 */
export function Words({
  segment,
  theme,
  tail = false,
  className,
  onClick,
  ariaLabel,
  ariaExpanded,
}: {
  segment: Segment;
  theme: Theme;
  tail?: boolean;
  className?: string;
  onClick?: () => void;
  ariaLabel?: string;
  ariaExpanded?: boolean;
}) {
  const boxRef = useRef<HTMLElement>(null);
  const [overflowing, setOverflowing] = useState(false);
  useLayoutEffect(() => {
    const el = boxRef.current;
    if (!el || !tail) return;
    const measure = () => setOverflowing(el.scrollWidth > el.clientWidth + 1);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [segment.text, tail]);
  const style = TONE_STYLE[segment.tone];
  const mask = tail && overflowing ? "linear-gradient(to right, transparent, black 20px)" : undefined;
  const shared = {
    "data-testid": `bar-${segment.kind}`,
    "data-tone": segment.tone,
    className: cn(
      "min-w-0 overflow-hidden whitespace-nowrap text-left text-[13px] leading-none tracking-[-0.01em]",
      !tail && "text-ellipsis",
      onClick && "cursor-pointer rounded-[4px] outline-none focus-visible:ring-2 focus-visible:ring-[#0A84FF]/70",
      style.italic && "italic",
      className,
    ),
    style: {
      color: style.color ?? inkFor(theme),
      opacity: style.opacity,
      direction: tail ? ("rtl" as const) : ("ltr" as const),
      maskImage: mask,
      WebkitMaskImage: mask,
      transition: "opacity 200ms ease, color 200ms ease",
    },
  };
  const text = tail ? <bdi style={{ direction: "ltr", unicodeBidi: "isolate" }}>{segment.text}</bdi> : segment.text;
  if (onClick) {
    return (
      <button
        ref={boxRef as React.RefObject<HTMLButtonElement>}
        type="button"
        onClick={(event) => {
          event.stopPropagation();
          onClick();
        }}
        aria-label={ariaLabel}
        aria-expanded={ariaExpanded}
        title={segment.text}
        {...shared}
      >
        {text}
      </button>
    );
  }
  return (
    <span ref={boxRef as React.RefObject<HTMLSpanElement>} title={tail ? undefined : segment.text} {...shared}>
      {text}
    </span>
  );
}

// === THE RAIL ===

interface RailBarProps {
  rail: Rail;
  /** Left edge, in px from the rail's start, of the bead the rail reaches
   *  (measured on screen by the strip); null when there is none. */
  beadX: number | null;
  /** The rail's full length, in px. */
  length: number;
  palette: Palette;
  reducedMotion: boolean;
}

/**
 * The 2px rail along the strip's bottom edge. Blue while Juno works, green
 * while you speak, red where it failed. It fills to the live bead, past it
 * while the answer streams, and drains from the right once the turn is done.
 */
export function RailBar({ rail, beadX, length, palette, reducedMotion }: RailBarProps) {
  let width = 0;
  let color = SYSTEM_BLUE;
  let ease = reducedMotion ? "none" : "width 260ms cubic-bezier(0.2, 0.7, 0.2, 1)";
  const room = beadX !== null ? length - beadX : length;
  switch (rail.kind) {
    case "meter":
      width = Math.round(length * 0.35 * rail.level);
      color = SYSTEM_GREEN;
      ease = reducedMotion ? "none" : "width 80ms linear";
      break;
    case "toBead":
      width = beadX ?? Math.round(length * 0.15);
      break;
    case "streaming":
      width = (beadX ?? 0) + Math.round(room * Math.max(rail.fraction, 0.04));
      break;
    case "full":
      width = length;
      break;
    case "drain":
      width = Math.round(length * rail.progress);
      ease = reducedMotion ? "none" : "width 60ms linear";
      break;
    case "failed":
      width = beadX !== null ? beadX + 6 : Math.round(length * rail.fraction);
      color = SYSTEM_RED;
      break;
    case "empty":
    default:
      width = 0;
  }
  return (
    <div
      className="absolute left-0 right-0 overflow-hidden"
      style={{ bottom: 0, height: 2, backgroundColor: palette.rail }}
      data-testid="bar-rail"
      data-rail={rail.kind}
      aria-hidden="true"
    >
      <div
        className="h-full"
        style={{
          width: Math.max(0, Math.min(length, width)),
          backgroundColor: color,
          transition: `${ease}, background-color 200ms ease`,
        }}
      />
    </div>
  );
}
