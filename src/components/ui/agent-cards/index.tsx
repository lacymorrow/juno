/**
 * Domain-specific components for agent responses.
 *
 * These are designed to be used by the AI agent in JSX responses
 * for common query types (weather, files, system status, etc.).
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
// WeatherCard
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

interface WeatherCardProps {
  location?: string;
  temperature?: number;
  unit?: "F" | "C";
  condition?: string;
  high?: number;
  low?: number;
  humidity?: number;
  wind?: string;
  forecast?: Array<{ day: string; high: number; low: number; condition?: string }>;
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

  return (
    <div className="rounded-lg border bg-gradient-to-br from-sky-50 to-blue-50 dark:from-sky-950 dark:to-blue-950 p-4 space-y-3">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-sm font-medium text-muted-foreground">{location}</div>
          {temperature !== undefined && (
            <div className="text-3xl font-bold">
              {temperature}°{unit}
            </div>
          )}
          <div className="text-sm capitalize text-muted-foreground">{condition}</div>
        </div>
        <Icon className="h-10 w-10 text-sky-500 dark:text-sky-400" />
      </div>

      {(high !== undefined || low !== undefined || humidity !== undefined || wind) && (
        <div className="flex gap-4 text-xs text-muted-foreground">
          {high !== undefined && low !== undefined && (
            <div className="flex items-center gap-1">
              <Thermometer className="h-3 w-3" />
              <span>H: {high}° L: {low}°</span>
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

      {forecast && forecast.length > 0 && (
        <div className="border-t pt-2 grid grid-cols-3 gap-2">
          {forecast.slice(0, 5).map((day) => {
            const DayIcon = WEATHER_ICONS[(day.condition || "clear").toLowerCase()] || Cloud;
            return (
              <div key={day.day} className="text-center text-xs space-y-1">
                <div className="font-medium">{day.day}</div>
                <DayIcon className="h-4 w-4 mx-auto text-sky-500" />
                <div className="text-muted-foreground">
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
// FileListCard
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
    <div className="rounded-lg border bg-card p-4 space-y-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Folder className="h-4 w-4 text-yellow-500" />
          <span className="font-medium text-sm">{title}</span>
        </div>
        {totalCount !== undefined && (
          <span className="text-xs text-muted-foreground">
            {totalCount} items{totalSize ? ` · ${totalSize}` : ""}
          </span>
        )}
      </div>

      {path && (
        <div className="text-xs text-muted-foreground font-mono truncate">{path}</div>
      )}

      {files.length > 0 && (
        <div className="space-y-1">
          {files.slice(0, 10).map((file) => {
            const Icon = FILE_ICONS[(file.type || "file").toLowerCase()] || File;
            return (
              <div
                key={file.name}
                className="flex items-center justify-between text-sm py-1 px-2 rounded hover:bg-muted/50"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <Icon className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                  <span className="truncate">{file.name}</span>
                  {file.count !== undefined && (
                    <span className="text-xs text-muted-foreground">({file.count})</span>
                  )}
                </div>
                {file.size && (
                  <span className="text-xs text-muted-foreground shrink-0 ml-2">{file.size}</span>
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
// SystemStatusCard
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

function MetricBar({ label, value, detail }: SystemMetric) {
  const color =
    value > 90
      ? "bg-red-500"
      : value > 70
        ? "bg-yellow-500"
        : "bg-green-500";

  return (
    <div className="space-y-1">
      <div className="flex justify-between text-xs">
        <span className="font-medium">{label}</span>
        <span className="text-muted-foreground">{detail || `${value}%`}</span>
      </div>
      <div className="h-2 bg-muted rounded-full overflow-hidden">
        <div
          className={cn("h-full rounded-full transition-all", color)}
          style={{ width: `${Math.min(100, Math.max(0, value))}%` }}
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
    <div className="rounded-lg border bg-card p-4 space-y-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <HardDrive className="h-4 w-4 text-blue-500" />
          <span className="font-medium text-sm">{hostname || "System Status"}</span>
        </div>
        {uptime && (
          <span className="text-xs text-muted-foreground">Up: {uptime}</span>
        )}
      </div>

      <div className="space-y-2">
        {metrics.map((metric) => (
          <MetricBar key={metric.label} {...metric} />
        ))}
      </div>
    </div>
  );
}

// ============================================================
// ComparisonCard
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
    <div className="rounded-lg border bg-card p-4 space-y-3">
      <div className="font-medium text-sm">{title}</div>
      <div className={cn(
        "grid gap-3",
        options.length === 2 ? "grid-cols-2" : options.length >= 3 ? "grid-cols-3" : "grid-cols-1"
      )}>
        {options.map((opt) => (
          <div
            key={opt.name}
            className={cn(
              "rounded-md border p-3 space-y-2",
              opt.recommended && "border-green-500 bg-green-50/50 dark:bg-green-950/20",
            )}
          >
            <div className="flex items-center justify-between">
              <span className="font-medium text-sm">{opt.name}</span>
              {opt.recommended && (
                <span className="text-[10px] font-medium bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300 px-1.5 py-0.5 rounded">
                  Recommended
                </span>
              )}
            </div>
            {opt.rating !== undefined && (
              <div className="flex gap-0.5">
                {Array.from({ length: 5 }).map((_, i) => (
                  <div
                    key={i}
                    className={cn(
                      "h-1.5 w-4 rounded-full",
                      i < opt.rating! ? "bg-yellow-400" : "bg-muted",
                    )}
                  />
                ))}
              </div>
            )}
            {opt.pros && opt.pros.length > 0 && (
              <div className="space-y-1">
                {opt.pros.map((pro) => (
                  <div key={pro} className="flex items-start gap-1 text-xs text-green-700 dark:text-green-400">
                    <CheckCircle2 className="h-3 w-3 mt-0.5 shrink-0" />
                    <span>{pro}</span>
                  </div>
                ))}
              </div>
            )}
            {opt.cons && opt.cons.length > 0 && (
              <div className="space-y-1">
                {opt.cons.map((con) => (
                  <div key={con} className="flex items-start gap-1 text-xs text-red-600 dark:text-red-400">
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
// TimerCard
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
    <div className="rounded-lg border bg-card p-4 flex items-center gap-4">
      <Clock
        className={cn(
          "h-6 w-6",
          status === "running"
            ? "text-blue-500 animate-pulse"
            : status === "finished"
              ? "text-green-500"
              : "text-muted-foreground",
        )}
      />
      <div>
        <div className="text-xs text-muted-foreground">{label}</div>
        <div className="text-2xl font-mono font-bold">{duration}</div>
      </div>
      <div
        className={cn(
          "text-xs px-2 py-0.5 rounded-full ml-auto",
          status === "running" && "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300",
          status === "paused" && "bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300",
          status === "finished" && "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300",
        )}
      >
        {status}
      </div>
    </div>
  );
}

// ============================================================
// LinkCard
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
    <div className="rounded-lg border bg-card p-3 flex items-start gap-3 hover:bg-muted/50 transition-colors">
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
          <div className="text-xs text-muted-foreground line-clamp-2">{description}</div>
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
// TaskSummaryCard
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

  return (
    <div className="rounded-lg border bg-card p-4 space-y-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <ListTodo className="h-4 w-4 text-purple-500" />
          <span className="font-medium text-sm">{title}</span>
        </div>
        <span className="text-xs text-muted-foreground">
          {doneCount}/{tasks.length} done
        </span>
      </div>

      {tasks.length > 0 && (
        <div className="space-y-1">
          {tasks.map((task) => (
            <div
              key={task.label}
              className={cn(
                "flex items-start gap-2 text-sm py-1",
                task.done && "text-muted-foreground",
              )}
            >
              {task.done ? (
                <CheckCircle2 className="h-4 w-4 text-green-500 shrink-0 mt-0.5" />
              ) : (
                <ArrowRight className="h-4 w-4 text-blue-500 shrink-0 mt-0.5" />
              )}
              <div>
                <span className={cn(task.done && "line-through")}>{task.label}</span>
                {task.detail && (
                  <div className="text-xs text-muted-foreground">{task.detail}</div>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
