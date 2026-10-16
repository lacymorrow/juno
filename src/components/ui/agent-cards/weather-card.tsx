/**
 * WeatherCard: current conditions and an optional five-day row.
 *
 * A shadcn Card in the Mac tokens: location in the header, one hero number,
 * the condition as a word, a single caption line for the rest, and five fixed
 * forecast columns at any width. No gradient, no loop, no tinted glyph. Only
 * the parts the agent gave are drawn; nothing is invented, so a card with no
 * temperature shows the location and the condition and nothing else.
 */

import {
  Cloud,
  CloudFog,
  CloudLightning,
  CloudRain,
  CloudSun,
  Snowflake,
  Sun,
  Wind,
  type LucideIcon,
} from "lucide-react";
import { Stat } from "@/components/ui/animated-components";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

const WEATHER_ICONS: Record<string, LucideIcon> = {
  sunny: Sun,
  clear: Sun,
  cloudy: Cloud,
  overcast: Cloud,
  "partly cloudy": CloudSun,
  "partly-cloudy": CloudSun,
  fog: CloudFog,
  foggy: CloudFog,
  rain: CloudRain,
  rainy: CloudRain,
  snow: Snowflake,
  snowy: Snowflake,
  storm: CloudLightning,
  thunder: CloudLightning,
  windy: Wind,
};

function iconFor(condition?: string): LucideIcon {
  return WEATHER_ICONS[(condition ?? "").toLowerCase().trim()] ?? Cloud;
}

function sentence(text: string): string {
  return text ? text.charAt(0).toUpperCase() + text.slice(1) : text;
}

interface ForecastDay {
  day: string;
  high: number;
  low: number;
  condition?: string;
}

export interface WeatherCardProps {
  location?: string;
  temperature?: number;
  unit?: "F" | "C";
  condition?: string;
  high?: number;
  low?: number;
  humidity?: number;
  wind?: string;
  /** When the reading was taken, as the backend wants it said: "3:41 PM". */
  updated?: string;
  forecast?: ForecastDay[];
  className?: string;
}

const isNum = (n: unknown): n is number => typeof n === "number" && Number.isFinite(n);

export function WeatherCard({
  location = "Current location",
  temperature,
  unit = "F",
  condition,
  high,
  low,
  humidity,
  wind,
  updated,
  forecast,
  className,
}: WeatherCardProps) {
  const Icon = iconFor(condition);
  const hasTemp = isNum(temperature);
  const days = (Array.isArray(forecast) ? forecast : [])
    .filter((d) => d && typeof d.day === "string" && isNum(d.high) && isNum(d.low))
    .slice(0, 5);

  const facts = [
    isNum(high) || isNum(low)
      ? [isNum(high) ? `H ${high}°` : null, isNum(low) ? `L ${low}°` : null].filter(Boolean).join("  ")
      : null,
    isNum(humidity) ? `Humidity ${humidity}%` : null,
    wind ? `Wind ${wind}` : null,
  ].filter(Boolean) as string[];

  const empty = !hasTemp && !condition;

  return (
    <Card className={className} data-testid="weather-card" data-state={empty ? "empty" : "ready"}>
      <CardHeader>
        <CardTitle className="min-w-0 truncate">{location}</CardTitle>
        {updated && <CardAction className="text-[11px] text-muted-foreground">Updated {updated}</CardAction>}
      </CardHeader>
      <CardContent>
        {empty ? (
          <div className="py-1 text-[13px] text-muted-foreground">No weather to show.</div>
        ) : (
          <div className="flex items-center justify-between gap-3">
            <div className="flex min-w-0 items-baseline gap-2.5">
              {hasTemp && (
                <span role="img" aria-label={`${temperature} degrees ${unit === "C" ? "Celsius" : "Fahrenheit"}`}>
                  <Stat hero value={temperature} suffix={"°"} />
                </span>
              )}
              {condition && <span className="truncate text-[13px] text-muted-foreground">{sentence(condition)}</span>}
            </div>
            {condition && <Icon aria-hidden="true" className="size-7 shrink-0 text-muted-foreground" strokeWidth={1.5} />}
          </div>
        )}

        {facts.length > 0 && (
          <div className="mt-1.5 flex flex-wrap gap-x-2 text-[11px] tabular-nums text-muted-foreground">
            {facts.map((fact, i) => (
              <span key={fact}>
                {i > 0 && <span aria-hidden="true">{"· "}</span>}
                {fact}
              </span>
            ))}
          </div>
        )}

        {days.length > 0 && (
          <div className="mt-3 grid grid-cols-5 gap-2 border-t border-[var(--sep,var(--border))] pt-2.5 text-center">
            {days.map((day, i) => {
              const DayIcon = iconFor(day.condition);
              return (
                <div key={`${day.day}-${i}`} className="min-w-0 space-y-1">
                  <div className="truncate text-[11px] text-muted-foreground">{day.day}</div>
                  <DayIcon aria-hidden="true" className="mx-auto size-4 text-muted-foreground" strokeWidth={1.5} />
                  <div className="text-[11px] font-medium tabular-nums">
                    {day.high} / {day.low}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
