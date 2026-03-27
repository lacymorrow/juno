/**
 * Animated components for agent JSX responses.
 *
 * These provide delightful entry animations, micro-interactions,
 * and visual effects that the agent can compose into rich responses.
 * All animations are CSS-based for performance.
 *
 * Registered in availableComponents in jsx-message-renderer.tsx.
 */

import { cn } from "@/lib/utils";
import { useEffect, useMemo, useRef, useState } from "react";

// ============================================================
// AnimatedCard — card with smooth entry animation
// ============================================================

interface AnimatedCardProps {
  children?: React.ReactNode;
  className?: string;
  /** Entry animation variant */
  animation?: "fade-up" | "scale" | "slide-left" | "slide-right" | "none";
  /** Delay before animation starts (ms) */
  delay?: number;
  /** Ambient glow color (tailwind color or hex) */
  glow?: string;
}

export function AnimatedCard({
  children,
  className,
  animation = "fade-up",
  delay = 0,
  glow,
}: AnimatedCardProps) {
  const animClass = {
    "fade-up": "juno-animate-in",
    scale: "juno-animate-scale",
    "slide-left": "juno-animate-slide-left",
    "slide-right": "juno-animate-slide-right",
    none: "",
  }[animation];

  return (
    <div
      className={cn(
        "rounded-xl border bg-card p-4 shadow-sm transition-shadow hover:shadow-md",
        animClass,
        className,
      )}
      style={{
        animationDelay: delay ? `${delay}ms` : undefined,
        ...(glow
          ? {
              boxShadow: `0 0 20px -4px ${glow}`,
              borderColor: glow,
            }
          : {}),
      }}
    >
      {children}
    </div>
  );
}

// ============================================================
// AnimatedNumber — counts up to target value
// ============================================================

interface AnimatedNumberProps {
  /** Target value to count to */
  value: number;
  /** Duration of animation in ms */
  duration?: number;
  /** Prefix (e.g., "$", "~") */
  prefix?: string;
  /** Suffix (e.g., "%", "ms", "°F") */
  suffix?: string;
  /** Number of decimal places */
  decimals?: number;
  /** Additional CSS classes */
  className?: string;
}

export function AnimatedNumber({
  value,
  duration = 1200,
  prefix = "",
  suffix = "",
  decimals = 0,
  className,
}: AnimatedNumberProps) {
  const [displayed, setDisplayed] = useState(0);
  const startRef = useRef<number | null>(null);
  const frameRef = useRef<number>(0);

  useEffect(() => {
    startRef.current = null;

    const animate = (timestamp: number) => {
      if (startRef.current === null) startRef.current = timestamp;
      const elapsed = timestamp - startRef.current;
      const progress = Math.min(elapsed / duration, 1);

      // ease-out cubic
      const eased = 1 - Math.pow(1 - progress, 3);
      setDisplayed(eased * value);

      if (progress < 1) {
        frameRef.current = requestAnimationFrame(animate);
      }
    };

    frameRef.current = requestAnimationFrame(animate);

    return () => cancelAnimationFrame(frameRef.current);
  }, [value, duration]);

  return (
    <span className={cn("tabular-nums juno-animate-in", className)}>
      {prefix}
      {displayed.toFixed(decimals)}
      {suffix}
    </span>
  );
}

// ============================================================
// AnimatedList — stagger-animated list items
// ============================================================

interface AnimatedListProps {
  children?: React.ReactNode;
  className?: string;
  /** Gap between items */
  gap?: number;
}

const GAP_CLASSES: Record<number, string> = {
  0: "space-y-0",
  1: "space-y-1",
  2: "space-y-2",
  3: "space-y-3",
  4: "space-y-4",
};

export function AnimatedList({
  children,
  className,
  gap = 2,
}: AnimatedListProps) {
  const items = Array.isArray(children) ? children : children ? [children] : [];

  return (
    <div className={cn(GAP_CLASSES[gap] ?? "space-y-2", className)}>
      {items.map((child, i) => (
        <div
          key={i}
          className="juno-animate-in"
          style={{ animationDelay: `${i * 60}ms` }}
        >
          {child}
        </div>
      ))}
    </div>
  );
}

// ============================================================
// AnimatedProgress — progress bar with animated fill
// ============================================================

interface AnimatedProgressProps {
  /** Value 0-100 */
  value: number;
  /** Label text */
  label?: string;
  /** Color variant */
  color?: "blue" | "green" | "yellow" | "red" | "purple" | "auto";
  /** Show value label */
  showValue?: boolean;
  /** Additional CSS classes */
  className?: string;
}

export function AnimatedProgress({
  value,
  label,
  color = "auto",
  showValue = true,
  className,
}: AnimatedProgressProps) {
  const clamped = Math.min(100, Math.max(0, value));

  const resolvedColor =
    color === "auto"
      ? clamped > 90
        ? "red"
        : clamped > 70
          ? "yellow"
          : "green"
      : color;

  const colorClasses = {
    blue: "bg-blue-500",
    green: "bg-green-500",
    yellow: "bg-yellow-500",
    red: "bg-red-500",
    purple: "bg-purple-500",
  }[resolvedColor];

  return (
    <div className={cn("space-y-1.5 juno-animate-in", className)}>
      {(label || showValue) && (
        <div className="flex justify-between text-xs">
          {label && <span className="font-medium">{label}</span>}
          {showValue && (
            <span className="text-muted-foreground tabular-nums">
              {clamped}%
            </span>
          )}
        </div>
      )}
      <div className="h-2 bg-muted rounded-full overflow-hidden">
        <div
          className={cn("h-full rounded-full", colorClasses)}
          style={{
            width: `${clamped}%`,
            animation: "juno-fill-width 1s cubic-bezier(0.16, 1, 0.3, 1) both",
          }}
        />
      </div>
    </div>
  );
}

// ============================================================
// GlowBadge — badge with animated glow effect
// ============================================================

interface GlowBadgeProps {
  children?: React.ReactNode;
  className?: string;
  /** Glow color variant */
  color?: "blue" | "green" | "yellow" | "red" | "purple";
}

export function GlowBadge({
  children,
  className,
  color = "blue",
}: GlowBadgeProps) {
  const styles = {
    blue: "bg-blue-500/10 text-blue-500 border-blue-500/30",
    green: "bg-green-500/10 text-green-500 border-green-500/30",
    yellow: "bg-yellow-500/10 text-yellow-500 border-yellow-500/30",
    red: "bg-red-500/10 text-red-500 border-red-500/30",
    purple: "bg-purple-500/10 text-purple-500 border-purple-500/30",
  }[color];

  return (
    <span
      className={cn(
        "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium border",
        "juno-glow juno-animate-scale",
        styles,
        className,
      )}
    >
      {children}
    </span>
  );
}

// ============================================================
// ShimmerText — text with traveling shimmer highlight
// ============================================================

interface ShimmerTextProps {
  children?: React.ReactNode;
  className?: string;
}

export function ShimmerText({ children, className }: ShimmerTextProps) {
  return (
    <span
      className={cn(
        "relative inline-block",
        className,
      )}
    >
      <span className="relative z-10">{children}</span>
      <span
        className="absolute inset-0 juno-shimmer rounded"
        aria-hidden="true"
      />
    </span>
  );
}

// ============================================================
// Confetti — CSS-only celebration burst
// ============================================================

interface ConfettiProps {
  /** Number of confetti pieces */
  count?: number;
  className?: string;
}

const CONFETTI_COLORS = [
  "#FF6B6B",
  "#4ECDC4",
  "#45B7D1",
  "#96CEB4",
  "#FFEAA7",
  "#DDA0DD",
  "#FF9A9E",
  "#A8E6CF",
];

export function Confetti({ count = 12, className }: ConfettiProps) {
  const pieces = useMemo(
    () =>
      Array.from({ length: count }).map((_, i) => ({
        angle: (360 / count) * i,
        distance: 20 + Math.random() * 20,
        size: 3 + Math.random() * 4,
      })),
    [count],
  );

  return (
    <div
      className={cn("relative inline-flex items-center justify-center w-8 h-8", className)}
      aria-hidden="true"
    >
      {pieces.map((piece, i) => (
        <div
          key={i}
          className="absolute rounded-full"
          style={{
            width: `${piece.size}px`,
            height: `${piece.size}px`,
            backgroundColor: CONFETTI_COLORS[i % CONFETTI_COLORS.length],
            animation: `juno-confetti 0.8s cubic-bezier(0.16, 1, 0.3, 1) ${i * 0.03}s both`,
            transform: `rotate(${piece.angle}deg) translateY(-${piece.distance}px)`,
          }}
        />
      ))}
    </div>
  );
}

// ============================================================
// PulseRing — expanding concentric rings
// ============================================================

interface PulseRingProps {
  /** Ring color */
  color?: string;
  /** Size in pixels */
  size?: number;
  className?: string;
}

export function PulseRing({
  color = "rgba(59, 130, 246, 0.4)",
  size = 40,
  className,
}: PulseRingProps) {
  return (
    <div
      className={cn("relative inline-flex items-center justify-center", className)}
      style={{ width: `${size}px`, height: `${size}px` }}
      aria-hidden="true"
    >
      {[0, 0.4, 0.8].map((delay) => (
        <div
          key={delay}
          className="absolute inset-0 rounded-full border-2"
          style={{
            borderColor: color,
            animation: `juno-pulse-ring 2s cubic-bezier(0, 0, 0.2, 1) ${delay}s infinite`,
          }}
        />
      ))}
      <div
        className="w-2 h-2 rounded-full"
        style={{ backgroundColor: color }}
      />
    </div>
  );
}

// ============================================================
// Divider — animated divider line
// ============================================================

interface AnimatedDividerProps {
  className?: string;
  /** Gradient color scheme */
  variant?: "default" | "rainbow" | "blue" | "green";
}

export function AnimatedDivider({
  className,
  variant = "default",
}: AnimatedDividerProps) {
  const gradients = {
    default: "from-transparent via-border to-transparent",
    rainbow:
      "from-red-500/30 via-yellow-500/30 to-blue-500/30",
    blue: "from-transparent via-blue-500/40 to-transparent",
    green: "from-transparent via-green-500/40 to-transparent",
  }[variant];

  return (
    <div
      className={cn(
        "h-px w-full bg-gradient-to-r juno-animate-in",
        gradients,
        className,
      )}
    />
  );
}

// ============================================================
// Stat — large stat display with label
// ============================================================

interface StatProps {
  /** The main value */
  value: string | number;
  /** Label below the value */
  label?: string;
  /** Prefix */
  prefix?: string;
  /** Suffix */
  suffix?: string;
  /** Trend indicator */
  trend?: "up" | "down" | "neutral";
  className?: string;
}

export function Stat({
  value,
  label,
  prefix,
  suffix,
  trend,
  className,
}: StatProps) {
  const trendColors = {
    up: "text-green-500",
    down: "text-red-500",
    neutral: "text-muted-foreground",
  };

  const trendArrows = {
    up: "\u2191",
    down: "\u2193",
    neutral: "\u2192",
  };

  return (
    <div className={cn("text-center juno-animate-in", className)}>
      <div className="text-2xl font-bold tabular-nums">
        {prefix}
        {value}
        {suffix}
        {trend && (
          <span className={cn("text-sm ml-1", trendColors[trend])}>
            {trendArrows[trend]}
          </span>
        )}
      </div>
      {label && (
        <div className="text-xs text-muted-foreground mt-0.5">{label}</div>
      )}
    </div>
  );
}

// ============================================================
// MiniChart — simple bar chart visualization
// ============================================================

interface MiniChartProps {
  /** Array of values (0-100 scale) */
  data?: number[];
  /** Labels for each bar */
  labels?: string[];
  /** Bar color */
  color?: "blue" | "green" | "purple" | "orange";
  /** Height in pixels */
  height?: number;
  className?: string;
}

export function MiniChart({
  data = [],
  labels = [],
  color = "blue",
  height = 60,
  className,
}: MiniChartProps) {
  const maxVal = Math.max(...data, 1);
  const colorClasses = {
    blue: "bg-blue-500",
    green: "bg-green-500",
    purple: "bg-purple-500",
    orange: "bg-orange-500",
  }[color];

  return (
    <div className={cn("juno-animate-in", className)}>
      <div
        className="flex items-end gap-1"
        style={{ height: `${height}px` }}
      >
        {data.map((val, i) => {
          const barHeight = (val / maxVal) * 100;
          return (
            <div key={i} className="flex-1 flex flex-col items-center gap-1">
              <div
                className={cn("w-full rounded-t-sm min-h-[2px]", colorClasses)}
                style={{
                  height: `${barHeight}%`,
                  transformOrigin: "bottom",
                  animation: `juno-grow-up 0.8s cubic-bezier(0.16, 1, 0.3, 1) ${i * 0.08}s both`,
                  opacity: 0.7 + (val / maxVal) * 0.3,
                }}
              />
            </div>
          );
        })}
      </div>
      {labels.length > 0 && (
        <div className="flex gap-1 mt-1">
          {labels.map((label, i) => (
            <div
              key={i}
              className="flex-1 text-center text-[10px] text-muted-foreground truncate"
            >
              {label}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
