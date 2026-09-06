/**
 * Domain-specific components for agent responses.
 *
 * These are designed to be used by the AI agent in JSX responses
 * for common query types (weather, files, system status, etc.).
 * All components feature delightful entry animations and micro-interactions.
 * All components are registered in availableComponents in jsx-message-renderer.tsx.
 */

import { cn } from "@/lib/utils";
import {
  Cloud,
  Sun,
  CloudRain,
  Snowflake,
  CloudLightning,
  Wind,
  Droplets,
  Thermometer,
  Folder,
  File,
  FileText,
  Image,
  Music,
  Video,
  Archive,
  HardDrive,
  ExternalLink,
  Globe,
  Clock,
  CheckCircle2,
  Circle,
  ListTodo,
  ArrowRight,
  type LucideIcon,
} from "lucide-react";

// ============================================================
// WeatherCard — with animated weather effects
// ============================================================

const WEATHER_ICONS: Record<string, LucideIcon> = {
  sunny: Sun,
  clear: Sun,
  cloudy: Cloud,
  rain: CloudRain,
  rainy: CloudRain,
  snow: Snowflake,
  snowy: Snowflake,
  storm: CloudLightning,
  thunder: CloudLightning,
  windy: Wind,
};

/** Maps condition to a weather animation CSS class */
function getWeatherEffectClass(condition: string): string {
  const c = condition.toLowerCase();
  if (c === "rain" || c === "rainy" || c === "storm" || c === "thunder")
    return "juno-weather-rain";
  if (c === "snow" || c === "snowy") return "juno-weather-snow";
  return "";
}

/** Maps condition to gradient background */
function getWeatherGradient(condition: string): string {
  const c = condition.toLowerCase();
  if (c === "sunny" || c === "clear")
    return "from-amber-50 to-orange-50 dark:from-amber-950/40 dark:to-orange-950/40";
  if (c === "rain" || c === "rainy")
    return "from-slate-100 to-blue-100 dark:from-slate-950 dark:to-blue-950";
  if (c === "storm" || c === "thunder")
    return "from-slate-200 to-purple-100 dark:from-slate-950 dark:to-purple-950/50";
  if (c === "snow" || c === "snowy")
    return "from-blue-50 to-indigo-50 dark:from-blue-950/40 dark:to-indigo-950/40";
  return "from-sky-50 to-blue-50 dark:from-sky-950 dark:to-blue-950";
}

/** Maps condition to icon color */
function getWeatherIconColor(condition: string): string {
  const c = condition.toLowerCase();
  if (c === "sunny" || c === "clear") return "text-amber-500 dark:text-amber-400";
  if (c === "storm" || c === "thunder") return "text-purple-500 dark:text-purple-400";
  if (c === "snow" || c === "snowy") return "text-indigo-400 dark:text-indigo-300";
  return "text-sky-500 dark:text-sky-400";
}

interface WeatherCardProps {
  location?: string;
  temperature?: number;
  unit?: "F" | "C";
  condition?: string;
  high?: number;
  low?: number;
  humidity?: number;
  wind?: string;
  forecast?: Array<{
    day: string;
    high: number;
    low: number;
    condition?: string;
  }>;
}

export function WeatherCard({
  location = "Current Location",
  temperature,
  unit = "F",
  condition = "clear",
  high,
  low,
  humidity,
  wind,
  forecast,
}: WeatherCardProps) {
  const Icon = WEATHER_ICONS[condition.toLowerCase()] || Cloud;
  const weatherEffect = getWeatherEffectClass(condition);
  const gradient = getWeatherGradient(condition);
  const iconColor = getWeatherIconColor(condition);
  const isSunny =
    condition.toLowerCase() === "sunny" || condition.toLowerCase() === "clear";

  return (
    <div
      className={cn(
        "rounded-xl border bg-gradient-to-br p-4 space-y-3 juno-animate-in",
        "shadow-sm hover:shadow-md transition-shadow",
        gradient,
        weatherEffect,
      )}
    >
      {/* Header: location + temp + icon */}
      <div className="flex items-center justify-between">
        <div>
          <div className="text-xs font-medium text-muted-foreground tracking-wide uppercase">
            {location}
          </div>
          {temperature !== undefined && (
            <div className="text-4xl font-bold tracking-tight mt-0.5">
              {temperature}
              <span className="text-lg font-normal text-muted-foreground ml-0.5">
                °{unit}
              </span>
            </div>
          )}
          <div className="text-sm capitalize text-muted-foreground mt-0.5">
            {condition}
          </div>
        </div>
        <div className="relative">
          <Icon
            className={cn(
              "h-12 w-12 transition-transform",
              iconColor,
              isSunny && "juno-float",
            )}
          />
          {/* Sun rays effect */}
          {isSunny && (
            <div
              className="absolute inset-0 rounded-full opacity-20"
              style={{
                background:
                  "radial-gradient(circle, rgba(251,191,36,0.4) 0%, transparent 70%)",
                animation: "juno-sun-rotate 20s linear infinite",
                transform: "scale(1.8)",
              }}
            />
          )}
        </div>
      </div>

      {/* Detail row */}
      {(high !== undefined ||
        low !== undefined ||
        humidity !== undefined ||
        wind) && (
        <div className="flex gap-4 text-xs text-muted-foreground juno-animate-in" style={{ animationDelay: "0.1s" }}>
          {high !== undefined && low !== undefined && (
            <div className="flex items-center gap-1">
              <Thermometer className="h-3 w-3" />
              <span>
                H: {high}° L: {low}°
              </span>
            </div>
          )}
          {humidity !== undefined && (
            <div className="flex items-center gap-1">
              <Droplets className="h-3 w-3" />
              <span>{humidity}%</span>
            </div>
          )}
          {wind && (
            <div className="flex items-center gap-1">
              <Wind className="h-3 w-3" />
              <span>{wind}</span>
            </div>
          )}
        </div>
      )}

      {/* Forecast row */}
      {forecast && forecast.length > 0 && (
        <div className="border-t border-border/50 pt-3 grid grid-cols-3 gap-2 sm:grid-cols-5">
          {forecast.slice(0, 5).map((day, i) => {
            const DayIcon =
              WEATHER_ICONS[(day.condition || "clear").toLowerCase()] || Cloud;
            const dayIconColor = getWeatherIconColor(day.condition || "clear");
            return (
              <div
                key={day.day}
                className="text-center text-xs space-y-1 juno-animate-in"
                style={{ animationDelay: `${0.15 + i * 0.06}s` }}
              >
                <div className="font-medium">{day.day}</div>
                <DayIcon className={cn("h-4 w-4 mx-auto", dayIconColor)} />
                <div className="text-muted-foreground tabular-nums">
                  {day.high}°/{day.low}°
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

// ============================================================
// FileListCard — with staggered file entry animations
// ============================================================

const FILE_ICONS: Record<string, LucideIcon> = {
  folder: Folder,
  document: FileText,
  image: Image,
  audio: Music,
  video: Video,
  archive: Archive,
  file: File,
};

interface FileEntry {
  name: string;
  type?: string;
  size?: string;
  count?: number;
}

interface FileListCardProps {
  title?: string;
  path?: string;
  files?: FileEntry[];
  totalCount?: number;
  totalSize?: string;
}

export function FileListCard({
  title = "Files",
  path,
  files = [],
  totalCount,
  totalSize,
}: FileListCardProps) {
  return (
    <div className="rounded-xl border bg-card p-4 space-y-3 juno-animate-in shadow-sm">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Folder className="h-4 w-4 text-yellow-500" />
          <span className="font-medium text-sm">{title}</span>
        </div>
        {totalCount !== undefined && (
          <span className="text-xs text-muted-foreground tabular-nums">
            {totalCount} items{totalSize ? ` \u00b7 ${totalSize}` : ""}
          </span>
        )}
      </div>

      {path && (
        <div className="text-xs text-muted-foreground font-mono truncate px-1">
          {path}
        </div>
      )}

      {files.length > 0 && (
        <div className="space-y-0.5">
          {files.slice(0, 10).map((file, i) => {
            const Icon =
              FILE_ICONS[(file.type || "file").toLowerCase()] || File;
            return (
              <div
                key={file.name}
                className={cn(
                  "flex items-center justify-between text-sm py-1.5 px-2 rounded-lg",
                  "hover:bg-muted/50 transition-colors juno-animate-in",
                )}
                style={{ animationDelay: `${0.05 + i * 0.04}s` }}
              >
                <div className="flex items-center gap-2 min-w-0">
                  <Icon className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                  <span className="truncate">{file.name}</span>
                  {file.count !== undefined && (
                    <span className="text-xs text-muted-foreground tabular-nums">
                      ({file.count})
                    </span>
                  )}
                </div>
                {file.size && (
                  <span className="text-xs text-muted-foreground shrink-0 ml-2 tabular-nums">
                    {file.size}
                  </span>
                )}
              </div>
            );
          })}
          {files.length > 10 && (
            <div className="text-xs text-muted-foreground text-center pt-1">
              +{files.length - 10} more
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================
// SystemStatusCard — with animated metric bars
// ============================================================

interface SystemMetric {
  label: string;
  value: number; // 0-100
  detail?: string;
}

interface SystemStatusCardProps {
  metrics?: SystemMetric[];
  hostname?: string;
  uptime?: string;
}

function MetricBar({
  label,
  value,
  detail,
  index = 0,
}: SystemMetric & { index?: number }) {
  const clamped = Math.min(100, Math.max(0, value));
  const color =
    clamped > 90
      ? "bg-red-500"
      : clamped > 70
        ? "bg-yellow-500"
        : "bg-green-500";

  return (
    <div
      className="space-y-1 juno-animate-in"
      style={{ animationDelay: `${0.1 + index * 0.08}s` }}
    >
      <div className="flex justify-between text-xs">
        <span className="font-medium">{label}</span>
        <span className="text-muted-foreground tabular-nums">
          {detail || `${clamped}%`}
        </span>
      </div>
      <div className="h-2 bg-muted rounded-full overflow-hidden">
        <div
          className={cn("h-full rounded-full", color)}
          style={{
            width: `${clamped}%`,
            animation: `juno-fill-width 1s cubic-bezier(0.16, 1, 0.3, 1) ${0.2 + index * 0.1}s both`,
          }}
        />
      </div>
    </div>
  );
}

export function SystemStatusCard({
  metrics = [],
  hostname,
  uptime,
}: SystemStatusCardProps) {
  return (
    <div className="rounded-xl border bg-card p-4 space-y-3 juno-animate-in shadow-sm">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <HardDrive className="h-4 w-4 text-blue-500" />
          <span className="font-medium text-sm">
            {hostname || "System Status"}
          </span>
        </div>
        {uptime && (
          <span className="text-xs text-muted-foreground">Up: {uptime}</span>
        )}
      </div>

      <div className="space-y-2.5">
        {metrics.map((metric, i) => (
          <MetricBar key={metric.label} {...metric} index={i} />
        ))}
      </div>
    </div>
  );
}

// ============================================================
// ComparisonCard — with animated option panels
// ============================================================

interface ComparisonOption {
  name: string;
  pros?: string[];
  cons?: string[];
  rating?: number; // 0-5
  recommended?: boolean;
}

interface ComparisonCardProps {
  title?: string;
  options?: ComparisonOption[];
}

export function ComparisonCard({
  title = "Comparison",
  options = [],
}: ComparisonCardProps) {
  return (
    <div className="rounded-xl border bg-card p-4 space-y-3 juno-animate-in shadow-sm">
      <div className="font-medium text-sm">{title}</div>
      <div
        className={cn(
          "grid gap-3",
          options.length === 2
            ? "grid-cols-2"
            : options.length >= 3
              ? "grid-cols-3"
              : "grid-cols-1",
        )}
      >
        {options.map((opt, i) => (
          <div
            key={opt.name}
            className={cn(
              "rounded-lg border p-3 space-y-2 juno-animate-in transition-shadow hover:shadow-sm",
              opt.recommended &&
                "border-green-500 bg-green-50/50 dark:bg-green-950/20 shadow-green-500/10",
            )}
            style={{ animationDelay: `${0.1 + i * 0.1}s` }}
          >
            <div className="flex items-center justify-between">
              <span className="font-medium text-sm">{opt.name}</span>
              {opt.recommended && (
                <span className="text-[10px] font-medium bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300 px-1.5 py-0.5 rounded-full">
                  Recommended
                </span>
              )}
            </div>
            {opt.rating !== undefined && (
              <div className="flex gap-0.5">
                {Array.from({ length: 5 }).map((_, j) => (
                  <div
                    key={j}
                    className={cn(
                      "h-1.5 w-4 rounded-full transition-all",
                      j < opt.rating! ? "bg-yellow-400" : "bg-muted",
                    )}
                    style={{
                      animation:
                        j < opt.rating!
                          ? `juno-scale-in 0.3s cubic-bezier(0.16, 1, 0.3, 1) ${0.3 + j * 0.05}s both`
                          : undefined,
                    }}
                  />
                ))}
              </div>
            )}
            {opt.pros && opt.pros.length > 0 && (
              <div className="space-y-1">
                {opt.pros.map((pro, pi) => (
                  <div
                    key={pro}
                    className="flex items-start gap-1 text-xs text-green-700 dark:text-green-400 juno-animate-in"
                    style={{ animationDelay: `${0.25 + pi * 0.04}s` }}
                  >
                    <CheckCircle2 className="h-3 w-3 mt-0.5 shrink-0" />
                    <span>{pro}</span>
                  </div>
                ))}
              </div>
            )}
            {opt.cons && opt.cons.length > 0 && (
              <div className="space-y-1">
                {opt.cons.map((con, ci) => (
                  <div
                    key={con}
                    className="flex items-start gap-1 text-xs text-red-600 dark:text-red-400 juno-animate-in"
                    style={{
                      animationDelay: `${0.3 + (opt.pros?.length || 0) * 0.04 + ci * 0.04}s`,
                    }}
                  >
                    <Circle className="h-3 w-3 mt-0.5 shrink-0" />
                    <span>{con}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

// ============================================================
// TimerCard — with animated clock
// ============================================================

interface TimerCardProps {
  label?: string;
  duration?: string; // e.g. "5:00", "1:30:00"
  status?: "running" | "paused" | "finished";
}

export function TimerCard({
  label = "Timer",
  duration = "0:00",
  status = "running",
}: TimerCardProps) {
  return (
    <div className="rounded-xl border bg-card p-4 flex items-center gap-4 juno-animate-in shadow-sm">
      <div className="relative">
        <Clock
          className={cn(
            "h-7 w-7 transition-colors",
            status === "running"
              ? "text-blue-500"
              : status === "finished"
                ? "text-green-500"
                : "text-muted-foreground",
          )}
        />
        {status === "running" && (
          <div
            className="absolute inset-0 rounded-full border-2 border-blue-500/40"
            style={{ animation: "juno-pulse-ring 2s ease-out infinite" }}
          />
        )}
      </div>
      <div>
        <div className="text-xs text-muted-foreground">{label}</div>
        <div className="text-2xl font-mono font-bold tabular-nums">
          {duration}
        </div>
      </div>
      <div
        className={cn(
          "text-xs px-2.5 py-1 rounded-full ml-auto font-medium",
          status === "running" &&
            "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300",
          status === "paused" &&
            "bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300",
          status === "finished" &&
            "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300",
        )}
      >
        {status}
      </div>
    </div>
  );
}

// ============================================================
// LinkCard — with hover lift effect
// ============================================================

interface LinkCardProps {
  url?: string;
  title?: string;
  description?: string;
  favicon?: string;
}

export function LinkCard({
  url = "",
  title,
  description,
  favicon,
}: LinkCardProps) {
  const domain = (() => {
    try {
      return new URL(url).hostname;
    } catch {
      return url;
    }
  })();

  return (
    <div
      className={cn(
        "rounded-xl border bg-card p-3 flex items-start gap-3",
        "hover:bg-muted/50 hover:shadow-sm transition-all juno-animate-in shadow-sm",
      )}
    >
      <div className="shrink-0 mt-0.5">
        {favicon ? (
          <img src={favicon} alt="" className="h-5 w-5 rounded" />
        ) : (
          <Globe className="h-5 w-5 text-muted-foreground" />
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="font-medium text-sm truncate">{title || domain}</div>
        {description && (
          <div className="text-xs text-muted-foreground line-clamp-2 mt-0.5">
            {description}
          </div>
        )}
        <div className="flex items-center gap-1 text-xs text-muted-foreground mt-1">
          <ExternalLink className="h-3 w-3" />
          <span className="truncate">{domain}</span>
        </div>
      </div>
    </div>
  );
}

// ============================================================
// TaskSummaryCard — with staggered task animations + progress
// ============================================================

interface TaskItem {
  label: string;
  done?: boolean;
  detail?: string;
}

interface TaskSummaryCardProps {
  title?: string;
  tasks?: TaskItem[];
}

export function TaskSummaryCard({
  title = "Tasks",
  tasks = [],
}: TaskSummaryCardProps) {
  const doneCount = tasks.filter((t) => t.done).length;
  const progress = tasks.length > 0 ? (doneCount / tasks.length) * 100 : 0;

  return (
    <div className="rounded-xl border bg-card p-4 space-y-3 juno-animate-in shadow-sm">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <ListTodo className="h-4 w-4 text-purple-500" />
          <span className="font-medium text-sm">{title}</span>
        </div>
        <span className="text-xs text-muted-foreground tabular-nums">
          {doneCount}/{tasks.length} done
        </span>
      </div>

      {/* Progress bar */}
      {tasks.length > 0 && (
        <div className="h-1.5 bg-muted rounded-full overflow-hidden">
          <div
            className={cn(
              "h-full rounded-full",
              progress === 100 ? "bg-green-500" : "bg-purple-500",
            )}
            style={{
              width: `${progress}%`,
              animation:
                "juno-fill-width 1s cubic-bezier(0.16, 1, 0.3, 1) 0.2s both",
            }}
          />
        </div>
      )}

      {tasks.length > 0 && (
        <div className="space-y-0.5">
          {tasks.map((task, i) => (
            <div
              key={task.label}
              className={cn(
                "flex items-start gap-2 text-sm py-1.5 px-1 rounded-lg juno-animate-in",
                task.done && "text-muted-foreground",
              )}
              style={{ animationDelay: `${0.1 + i * 0.06}s` }}
            >
              {task.done ? (
                <CheckCircle2 className="h-4 w-4 text-green-500 shrink-0 mt-0.5" />
              ) : (
                <ArrowRight className="h-4 w-4 text-purple-500 shrink-0 mt-0.5" />
              )}
              <div>
                <span className={cn(task.done && "line-through")}>
                  {task.label}
                </span>
                {task.detail && (
                  <div className="text-xs text-muted-foreground">
                    {task.detail}
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// Live components (bound to real state) live in their own files.
export { NowPlayingCard, mediaAppLabel } from "./now-playing-card";
export type { NowPlayingCardProps, MediaState, MediaApp } from "./now-playing-card";
