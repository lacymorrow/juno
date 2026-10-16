/**
 * Animated components for agent JSX responses.
 *
 * Built on the shadcn parts (`ui/card`, `ui/progress`) and drawn with the Mac
 * tokens from `styles/agent-content.css`: system blue, label and fill scale,
 * no purple. Motion is short and explains the data; the effects (glow, shimmer,
 * pulse ring, confetti) stay available and stay quiet. `prefers-reduced-motion`
 * collapses every animation here to its end state (see `globals.css`).
 *
 * Registered in availableComponents in jsx-message-renderer.tsx.
 */

import { Card, CardContent } from "@/components/ui/card";
import { ChartContainer, type ChartConfig } from "@/components/ui/chart";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import { useEffect, useMemo, useRef, useState } from "react";
import { Bar, BarChart, Cell, XAxis, YAxis } from "recharts";

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

// ============================================================
// AnimatedCard: a shadcn Card with its content padded
// ============================================================

interface AnimatedCardProps {
  children?: React.ReactNode;
  className?: string;
  /** Kept for older answers; every card enters the same way now. */
  animation?: "fade-up" | "scale" | "slide-left" | "slide-right" | "none";
  /** Delay before animation starts (ms) */
  delay?: number;
  /** Optional soft edge tint (any CSS color). Leave it off unless it means something. */
  glow?: string;
}

export function AnimatedCard({ children, className, delay = 0, glow }: AnimatedCardProps) {
  return (
    <Card
      className={className}
      style={{
        animationDelay: delay ? `${delay}ms` : undefined,
        ...(glow ? { boxShadow: `0 0 12px -4px ${glow}`, borderColor: glow } : {}),
      }}
    >
      <CardContent className="py-3">{children}</CardContent>
    </Card>
  );
}

// ============================================================
// AnimatedNumber: counts up to target value
// ============================================================

interface AnimatedNumberProps {
  /** Target value to count to */
  value: number;
  /** Duration of the count in ms. Values under 20 are read as seconds. */
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
  duration = 600,
  prefix = "",
  suffix = "",
  decimals = 0,
  className,
}: AnimatedNumberProps) {
  const [displayed, setDisplayed] = useState(() => (prefersReducedMotion() ? value : 0));
  const startRef = useRef<number | null>(null);
  const frameRef = useRef<number>(0);
  // `duration={2}` means two seconds, not two milliseconds.
  const ms = duration < 20 ? duration * 1000 : duration;

  useEffect(() => {
    if (prefersReducedMotion()) {
      setDisplayed(value);
      return;
    }
    startRef.current = null;

    const animate = (timestamp: number) => {
      if (startRef.current === null) startRef.current = timestamp;
      const progress = Math.min((timestamp - startRef.current) / ms, 1);
      // ease-out cubic
      setDisplayed((1 - Math.pow(1 - progress, 3)) * value);
      if (progress < 1) {
        frameRef.current = requestAnimationFrame(animate);
      }
    };

    frameRef.current = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(frameRef.current);
  }, [value, ms]);

  return (
    <span className={cn("tabular-nums", className)}>
      {prefix}
      {displayed.toFixed(decimals)}
      {suffix}
    </span>
  );
}

// ============================================================
// AnimatedList: rows enter one after another (first three only)
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

export function AnimatedList({ children, className, gap = 2 }: AnimatedListProps) {
  const items = Array.isArray(children) ? children : children ? [children] : [];

  return (
    <div className={cn(GAP_CLASSES[gap] ?? "space-y-2", className)}>
      {items.map((child, i) => (
        <div
          key={i}
          className="juno-animate-in"
          style={{ animationDelay: `${Math.min(i, 3) * 40}ms` }}
        >
          {child}
        </div>
      ))}
    </div>
  );
}

// ============================================================
// Meter: a shadcn Progress with a label row (AnimatedProgress is the old name)
// ============================================================

interface MeterProps {
  /** Value 0-100 */
  value: number;
  /** Label text */
  label?: string;
  /** `warning` is the one other fill, for a number that is a problem for this person. */
  tone?: "warning";
  /** Show the value on the right */
  showValue?: boolean;
  /** Replaces the percentage on the right: "13.1 of 16 GB". */
  detail?: string;
  /** Additional CSS classes */
  className?: string;
}

export function Meter({ value, label, tone, showValue = true, detail, className }: MeterProps) {
  const clamped = Math.round(Math.min(100, Math.max(0, Number(value) || 0)));

  return (
    <div className={cn("space-y-1.5", className)}>
      {(label || showValue) && (
        <div className="flex items-baseline justify-between gap-3">
          {label && <span className="min-w-0 truncate text-[13px] text-[var(--label,inherit)]">{label}</span>}
          {showValue && (
            <span
              className={cn(
                "ml-auto text-[11px] tabular-nums text-muted-foreground",
                tone === "warning" && "text-[var(--system-orange,#ff9500)]",
              )}
            >
              {detail || `${clamped}%`}
            </span>
          )}
        </div>
      )}
      <Progress value={clamped} tone={tone} aria-label={label} />
    </div>
  );
}

interface AnimatedProgressProps extends MeterProps {
  /**
   * Older answers pass a colour. System blue is the default; yellow, orange
   * and red read as `warning`, everything else (including "auto") as blue.
   */
  color?: string;
}

/** The earlier tag for Meter. Same bar; the `color` prop folds into `tone`. */
export function AnimatedProgress({ color, tone, ...rest }: AnimatedProgressProps) {
  const resolved = tone ?? (color && ["yellow", "orange", "red"].includes(color) ? "warning" : undefined);
  return <Meter {...rest} tone={resolved} />;
}

// ============================================================
// GlowBadge: badge with a soft glow
// ============================================================

interface GlowBadgeProps {
  children?: React.ReactNode;
  className?: string;
  /** Tint. System blue by default; there is no purple. */
  color?: "blue" | "green" | "yellow" | "red" | "purple" | string;
}

export function GlowBadge({ children, className, color = "blue" }: GlowBadgeProps) {
  const styles =
    {
      green: "bg-[#34c759]/10 text-[#248a3d] border-[#34c759]/30 dark:text-[#30d158]",
      yellow: "bg-[#ff9500]/10 text-[#c93400] border-[#ff9500]/30 dark:text-[#ff9f0a]",
      red: "bg-[#ff3b30]/10 text-[#d70015] border-[#ff3b30]/30 dark:text-[#ff453a]",
    }[color] ?? "bg-primary/10 text-primary border-primary/30";

  return (
    <span
      className={cn(
        "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium border",
        "juno-glow",
        styles,
        className,
      )}
    >
      {children}
    </span>
  );
}

// ============================================================
// ShimmerText: text with a traveling highlight
// ============================================================

interface ShimmerTextProps {
  children?: React.ReactNode;
  className?: string;
}

export function ShimmerText({ children, className }: ShimmerTextProps) {
  return (
    <span className={cn("relative inline-block", className)}>
      <span className="relative z-10">{children}</span>
      <span className="absolute inset-0 juno-shimmer rounded" aria-hidden="true" />
    </span>
  );
}

// ============================================================
// Confetti: CSS-only celebration burst
// ============================================================

interface ConfettiProps {
  /** Number of confetti pieces */
  count?: number;
  className?: string;
}

const CONFETTI_COLORS = [
  "#FF3B30",
  "#FF9500",
  "#FFCC00",
  "#34C759",
  "#5AC8FA",
  "#007AFF",
  "#FF2D55",
  "#30B0C7",
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
            animation: `juno-confetti 0.8s ease-out ${i * 0.03}s both`,
            transform: `rotate(${piece.angle}deg) translateY(-${piece.distance}px)`,
          }}
        />
      ))}
    </div>
  );
}

// ============================================================
// PulseRing: expanding concentric rings
// ============================================================

interface PulseRingProps {
  /** Ring color */
  color?: string;
  /** Size in pixels */
  size?: number;
  className?: string;
}

export function PulseRing({
  color = "color-mix(in srgb, var(--primary) 45%, transparent)",
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
      <div className="w-2 h-2 rounded-full" style={{ backgroundColor: color }} />
    </div>
  );
}

// ============================================================
// AnimatedDivider: a hairline that fades in
// ============================================================

interface AnimatedDividerProps {
  className?: string;
  /** Older answers pass "rainbow" or "green"; every variant is a quiet line now. */
  variant?: "default" | "rainbow" | "blue" | "green" | string;
}

export function AnimatedDivider({ className, variant = "default" }: AnimatedDividerProps) {
  const gradient =
    variant === "default"
      ? "from-transparent via-border to-transparent"
      : "from-transparent via-primary/40 to-transparent";

  return (
    <div
      className={cn("h-px w-full bg-gradient-to-r juno-animate-in", gradient, className)}
    />
  );
}

// ============================================================
// Stat: a number and its label; Stats lays a row of them out
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
  /** Trend indicator, in the label colour (no red or green) */
  trend?: "up" | "down" | "neutral";
  /** One big number per card (temperature) */
  hero?: boolean;
  className?: string;
}

const TREND_ARROWS = { up: "↑", down: "↓", neutral: "→" };

export function Stat({ value, label, prefix, suffix, trend, hero, className }: StatProps) {
  return (
    <div className={cn("min-w-0", className)}>
      <div
        className={cn(
          "tabular-nums font-semibold",
          hero ? "text-[28px] leading-none tracking-[-0.02em]" : "text-[17px] leading-tight tracking-[-0.01em]",
        )}
      >
        {prefix}
        {value}
        {suffix}
        {trend && <span className="ml-1 text-[11px] font-normal text-muted-foreground">{TREND_ARROWS[trend]}</span>}
      </div>
      {label && <div className="text-[11px] text-muted-foreground">{label}</div>}
    </div>
  );
}

export function Stats({ children, className }: { children?: React.ReactNode; className?: string }) {
  return <div className={cn("flex flex-wrap gap-x-6 gap-y-2", className)}>{children}</div>;
}

// ============================================================
// MiniChart: a shadcn chart (recharts) with one bar series
// ============================================================

interface MiniChartProps {
  /** Array of values */
  data?: number[];
  /** Labels for each bar */
  labels?: string[];
  /** Index of the bar the answer is about; the others go quiet. */
  highlight?: number;
  /** Older answers pass a colour; bars are system blue, `orange` is the warning tone. */
  color?: string;
  /** Bar area height in pixels */
  height?: number;
  className?: string;
}

const AXIS_HEIGHT = 16;

export function MiniChart({ data, labels, highlight, color = "blue", height = 56, className }: MiniChartProps) {
  const values = (Array.isArray(data) ? data : []).map((v) => (Number.isFinite(Number(v)) ? Number(v) : 0));
  const names = Array.isArray(labels) ? labels : [];

  if (values.length === 0) {
    return <div className={cn("text-[11px] text-muted-foreground", className)}>No data.</div>;
  }

  const rows = values.map((value, i) => ({ label: String(names[i] ?? ""), value }));
  const hasLabels = names.length > 0;
  const config = {
    value: { label: "Value", color: color === "orange" ? "var(--system-orange, #ff9500)" : "var(--chart-1)" },
  } satisfies ChartConfig;
  const summary = rows.map((r) => (r.label ? `${r.label} ${r.value}` : String(r.value))).join(", ");

  return (
    <ChartContainer
      config={config}
      role="img"
      aria-label={`Bar chart: ${summary}`}
      className={cn("aspect-auto w-full justify-stretch", className)}
      style={{ height: height + (hasLabels ? AXIS_HEIGHT : 0) }}
    >
      <BarChart data={rows} margin={{ top: 0, right: 0, bottom: 0, left: 0 }} barCategoryGap={3}>
        <XAxis
          dataKey="label"
          hide={!hasLabels}
          tickLine={false}
          axisLine={false}
          interval={rows.length > 8 ? 1 : 0}
          height={AXIS_HEIGHT}
          tick={{ fontSize: 10 }}
        />
        <YAxis hide domain={[0, "dataMax"]} />
        <Bar
          dataKey="value"
          fill="var(--color-value)"
          radius={[2, 2, 0, 0]}
          isAnimationActive={!prefersReducedMotion()}
          animationDuration={300}
          animationEasing="ease-out"
        >
          {rows.map((_, i) => (
            <Cell
              key={i}
              fill={highlight === undefined || i === highlight ? "var(--color-value)" : "var(--fill-2, var(--muted))"}
            />
          ))}
        </Bar>
      </BarChart>
    </ChartContainer>
  );
}
