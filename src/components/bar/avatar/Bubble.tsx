import { forwardRef, useEffect, type CSSProperties, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { BUBBLE_DEPTH, BUBBLE_MAX_WIDTH, SYSTEM_BLUE, SYSTEM_GREEN, SYSTEM_RED } from "./avatarModel";

/**
 * A comic bubble. One dark shape for both speakers; the tail says who is
 * talking. Juno's tail points at the head (up when bubbles hang below it,
 * down when they rise above). Yours points right, off the panel, to you.
 * A thought has no tail: two small circles lead to the head instead.
 */

export type BubbleTail = "head" | "you" | "thought" | "none";

export type BubbleEdge = "plain" | "listen" | "dictation" | "error";

const EDGE: Record<BubbleEdge, string> = {
  plain: "rgba(255,255,255,0.18)",
  listen: SYSTEM_BLUE,
  dictation: SYSTEM_GREEN,
  error: SYSTEM_RED,
};

const FILL = "#1C1C1E";

const STYLES = `
.av-bubble { transform-origin: var(--av-origin, 50% 0%); }
.av-bubble[data-shown="false"] { opacity: 0; transform: scale(0.96) translateY(var(--av-enter, -4px)); }
.av-bubble[data-shown="true"]  { opacity: 1; transform: none; transition: opacity 180ms ease-out, transform 220ms cubic-bezier(.2,.8,.2,1); }
.av-bubble-scroll { scrollbar-width: thin; scrollbar-color: rgba(255,255,255,0.22) transparent; }
.av-bubble-scroll::-webkit-scrollbar { width: 6px; }
.av-bubble-scroll::-webkit-scrollbar-track { background: transparent; }
.av-bubble-scroll::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.22); border-radius: 3px; }
@keyframes av-think {
  0%, 100% { transform: translateY(0); opacity: 0.45; }
  50%      { transform: translateY(-3px); opacity: 1; }
}
@media (prefers-reduced-motion: reduce) {
  .av-bubble { transition: none !important; }
  .av-think-dot { animation: none !important; }
}
`;

function useBubbleStyles() {
  useEffect(() => {
    const id = "avatar-bubble-styles";
    if (document.getElementById(id)) return;
    const style = document.createElement("style");
    style.id = id;
    style.textContent = STYLES;
    document.head.appendChild(style);
  }, []);
}

interface BubbleProps {
  tail: BubbleTail;
  /** Bubbles rise above the head, so Juno's tail points down. */
  facingUp: boolean;
  edge?: BubbleEdge;
  /** The window has grown enough to hold this bubble: fade and scale it in.
   *  Before that it is laid out (so it can be measured) but invisible. */
  shown: boolean;
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  role?: string;
  "aria-label"?: string;
  /** Test and state hooks (`data-testid`, `data-tone`) pass straight through. */
  [data: `data-${string}`]: string | undefined;
}

/** The little triangle. Drawn in the bubble's fill with the same hairline. */
function Tail({ direction, edge }: { direction: "up" | "down" | "right"; edge: BubbleEdge }) {
  const stroke = EDGE[edge];
  const strokeWidth = edge === "plain" ? 0.5 : 1;
  if (direction === "right") {
    return (
      <svg
        className="pointer-events-none absolute"
        style={{ right: -7, top: "50%", marginTop: -6 }}
        width="8"
        height="12"
        viewBox="0 0 8 12"
        aria-hidden="true"
      >
        <path d="M0 1 L7.5 6 L0 11" fill={FILL} stroke={stroke} strokeWidth={strokeWidth} strokeLinejoin="round" />
        <path d="M0 1.5 L0 10.5" stroke={FILL} strokeWidth="2" />
      </svg>
    );
  }
  const up = direction === "up";
  return (
    <svg
      className="pointer-events-none absolute"
      style={{ left: "50%", marginLeft: -7, [up ? "top" : "bottom"]: -7 }}
      width="14"
      height="8"
      viewBox="0 0 14 8"
      aria-hidden="true"
    >
      <path
        d={up ? "M1 8 L5 0.5 L13 8" : "M1 0 L5 7.5 L13 0"}
        fill={FILL}
        stroke={stroke}
        strokeWidth={strokeWidth}
        strokeLinejoin="round"
      />
      <path d={up ? "M1.5 8 L12.5 8" : "M1.5 0 L12.5 0"} stroke={FILL} strokeWidth="2" />
    </svg>
  );
}

/** Two circles between the head and a thought. */
function ThoughtDots({ facingUp }: { facingUp: boolean }) {
  const side = facingUp ? "bottom" : "top";
  return (
    <span
      className="pointer-events-none absolute"
      style={{ left: "50%", marginLeft: -6, [side]: -14, width: 12, height: 14 }}
      aria-hidden="true"
    >
      <span
        className="absolute rounded-full"
        style={{ width: 6, height: 6, left: 3, [side]: 6, background: FILL, boxShadow: `0 0 0 0.5px ${EDGE.plain}` }}
      />
      <span
        className="absolute rounded-full"
        style={{ width: 4, height: 4, left: 0, [side]: 0, background: FILL, boxShadow: `0 0 0 0.5px ${EDGE.plain}` }}
      />
    </span>
  );
}

export const Bubble = forwardRef<HTMLDivElement, BubbleProps>(function Bubble(
  { tail, facingUp, edge = "plain", shown, children, className, style, role, ...rest },
  ref,
) {
  useBubbleStyles();
  const juno = tail === "head" || tail === "thought";
  const originY = juno ? (facingUp ? "100%" : "0%") : "50%";
  const originX = juno ? "50%" : "100%";
  return (
    <div
      {...rest}
      ref={ref}
      role={role}
      data-shown={shown ? "true" : "false"}
      data-tail={tail}
      data-edge={edge}
      className={cn(
        "av-bubble relative box-border text-[13px] leading-[1.45] tracking-[-0.01em] text-white/90",
        tail === "thought" ? "rounded-[22px]" : "rounded-[16px]",
        className,
      )}
      style={{
        maxWidth: BUBBLE_MAX_WIDTH,
        width: "fit-content",
        transition: "max-width 220ms cubic-bezier(.2,.8,.2,1)",
        background: FILL,
        boxShadow: `0 0 0 ${edge === "plain" ? 0.5 : 1}px ${EDGE[edge]}, ${BUBBLE_DEPTH}`,
        ["--av-origin" as string]: `${originX} ${originY}`,
        ["--av-enter" as string]: juno ? (facingUp ? "4px" : "-4px") : "0px",
        ...style,
      }}
    >
      {tail === "head" && <Tail direction={facingUp ? "down" : "up"} edge={edge} />}
      {tail === "you" && <Tail direction="right" edge={edge} />}
      {tail === "thought" && <ThoughtDots facingUp={facingUp} />}
      {children}
    </div>
  );
});

/** The moving mark inside a thought: three dots that rise in turn. */
export function ThinkingMark() {
  return (
    <span className="inline-flex items-center gap-[3px]" aria-hidden="true" data-testid="avatar-thinking-mark">
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="av-think-dot block size-[5px] rounded-full bg-white"
          style={{ animation: `av-think 1.1s ease-in-out ${i * 0.16}s infinite` }}
        />
      ))}
    </span>
  );
}
