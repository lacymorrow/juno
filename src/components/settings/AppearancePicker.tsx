import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { cn } from "@/lib/utils";
import type { BarAppearance } from "@/components/bar/barAppearance";
import {
  APPEARANCE_CATALOG,
  appearanceEntry,
  appearancePreviewUrl,
} from "@/components/bar/appearanceCatalog";

interface AppearancePickerProps {
  value: string;
  onChange: (value: BarAppearance) => void;
  /** True while the previous choice is still being saved. */
  disabled?: boolean;
}

// If the frame has not painted by then, the preview is not coming; show the
// words instead of a blank stage.
const PREVIEW_TIMEOUT_MS = 4000;

function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(() =>
    typeof window !== "undefined" && typeof window.matchMedia === "function"
      ? window.matchMedia("(prefers-reduced-motion: reduce)").matches
      : false,
  );
  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onChange = () => setReduced(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);
  return reduced;
}

/**
 * Pick how the bar looks by watching it. A stage shows the chosen look going
 * through its moments (resting, listening, dictating, done); arrows and the
 * dots step through the catalog; the choice applies as you browse, so the real
 * bar and the stage change together. There is no confirm and no toast: the bar
 * changing is the feedback.
 */
export function AppearancePicker({ value, onChange, disabled = false }: AppearancePickerProps) {
  const reducedMotion = usePrefersReducedMotion();
  const entry = appearanceEntry(value);
  const index = APPEARANCE_CATALOG.findIndex((e) => e.value === entry.value);

  const step = useCallback(
    (delta: number) => {
      if (disabled) return;
      const next = (index + delta + APPEARANCE_CATALOG.length) % APPEARANCE_CATALOG.length;
      onChange(APPEARANCE_CATALOG[next].value);
    },
    [disabled, index, onChange],
  );

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      step(-1);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      step(1);
    }
  };

  // Loading and failure are per appearance: switching remounts the frame. The
  // frame reports "ready" once the bar has painted and "failed" if it threw;
  // silence past the timeout counts as failure too.
  const [status, setStatus] = useState<"loading" | "ready" | "failed">("loading");
  const timeoutRef = useRef<number | null>(null);
  useEffect(() => {
    setStatus("loading");
    if (timeoutRef.current) window.clearTimeout(timeoutRef.current);
    timeoutRef.current = window.setTimeout(() => {
      setStatus((s) => (s === "loading" ? "failed" : s));
    }, PREVIEW_TIMEOUT_MS);
    return () => {
      if (timeoutRef.current) window.clearTimeout(timeoutRef.current);
    };
  }, [entry.value]);

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      if (event.origin !== window.location.origin) return;
      const data = event.data as { type?: string; status?: string; appearance?: string } | null;
      if (!data || data.type !== "juno-bar-preview" || data.appearance !== entry.value) return;
      if (data.status === "ready") setStatus("ready");
      else if (data.status === "failed") setStatus("failed");
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [entry.value]);

  const src = useMemo(
    () => appearancePreviewUrl(entry.value, { reducedMotion }),
    [entry.value, reducedMotion],
  );

  return (
    <div
      role="group"
      aria-label="Bar appearance"
      aria-roledescription="carousel"
      tabIndex={0}
      onKeyDown={onKeyDown}
      className="space-y-2.5 outline-none focus-visible:ring-2 focus-visible:ring-ring/60 focus-visible:ring-offset-2 focus-visible:ring-offset-card rounded-[10px]"
    >
      <div className="relative">
        <div
          className={cn(
            "relative h-[150px] w-full overflow-hidden rounded-[8px]",
            "bg-[#E9E9EB] dark:bg-[#1C1C1E]",
          )}
        >
          {status !== "ready" && (
            <div
              className="absolute inset-0 flex items-center justify-center text-[12px] text-muted-foreground"
              aria-live="polite"
            >
              {status === "failed" ? "Preview unavailable" : entry.name}
            </div>
          )}
          <iframe
            key={entry.value}
            src={src}
            title={`Preview of the ${entry.name} bar`}
            tabIndex={-1}
            aria-hidden="true"
            className={cn(
              "absolute inset-0 h-full w-full border-0 bg-transparent",
              "pointer-events-none select-none",
              status === "ready" ? "opacity-100" : "opacity-0",
              !reducedMotion && "transition-opacity duration-150 ease-out",
            )}
          />
        </div>

        <button
          type="button"
          aria-label="Previous appearance"
          disabled={disabled}
          onClick={() => step(-1)}
          className={cn(
            "absolute left-2 top-1/2 flex size-7 -translate-y-1/2 items-center justify-center rounded-full",
            "bg-white/80 text-foreground shadow-sm ring-1 ring-black/10 dark:bg-black/60 dark:ring-white/15",
            "hover:bg-white dark:hover:bg-black/80 active:scale-[0.96] disabled:opacity-50",
            !reducedMotion && "transition-[transform,background-color] duration-100 ease-out",
          )}
        >
          <ChevronLeft className="size-4" strokeWidth={2} />
        </button>
        <button
          type="button"
          aria-label="Next appearance"
          disabled={disabled}
          onClick={() => step(1)}
          className={cn(
            "absolute right-2 top-1/2 flex size-7 -translate-y-1/2 items-center justify-center rounded-full",
            "bg-white/80 text-foreground shadow-sm ring-1 ring-black/10 dark:bg-black/60 dark:ring-white/15",
            "hover:bg-white dark:hover:bg-black/80 active:scale-[0.96] disabled:opacity-50",
            !reducedMotion && "transition-[transform,background-color] duration-100 ease-out",
          )}
        >
          <ChevronRight className="size-4" strokeWidth={2} />
        </button>
      </div>

      <div className="flex items-start justify-between gap-4 px-0.5">
        <div className="min-w-0 space-y-0.5">
          <p className="text-[13px] font-medium leading-tight">{entry.name}</p>
          <p className="text-[12px] leading-snug text-muted-foreground">{entry.descriptor}</p>
        </div>
        <div className="flex shrink-0 items-center gap-1.5 pt-1.5" role="tablist" aria-label="Appearances">
          {APPEARANCE_CATALOG.map((item, i) => (
            <button
              key={item.value}
              type="button"
              role="tab"
              aria-selected={i === index}
              aria-label={item.name}
              disabled={disabled}
              onClick={() => i !== index && !disabled && onChange(item.value)}
              className={cn(
                "size-1.5 rounded-full",
                i === index ? "bg-[#007AFF]" : "bg-foreground/25 hover:bg-foreground/45",
                !reducedMotion && "transition-colors duration-150 ease-out",
              )}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
