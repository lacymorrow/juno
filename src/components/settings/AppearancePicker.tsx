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

type PreviewStatus = "loading" | "ready" | "failed";

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

  // The current look and its two neighbours stay mounted, the neighbours
  // hidden, so stepping lands on a frame that has already painted (the orb
  // and avatar looks take a moment to load their engines). Each frame reports
  // "ready" once its bar has painted and "failed" if it threw; a current frame
  // silent past the timeout counts as failed too.
  const mounted = useMemo(() => {
    const n = APPEARANCE_CATALOG.length;
    const around = [(index - 1 + n) % n, index, (index + 1) % n];
    return Array.from(new Set(around)).map((i) => APPEARANCE_CATALOG[i]);
  }, [index]) as readonly (typeof APPEARANCE_CATALOG)[number][];
  const [statusByValue, setStatusByValue] = useState<Record<string, PreviewStatus>>({});
  const status: PreviewStatus = statusByValue[entry.value] ?? "loading";
  const timeoutRef = useRef<number | null>(null);
  useEffect(() => {
    if (timeoutRef.current) window.clearTimeout(timeoutRef.current);
    if (status !== "loading") return;
    timeoutRef.current = window.setTimeout(() => {
      setStatusByValue((prev) =>
        (prev[entry.value] ?? "loading") === "loading" ? { ...prev, [entry.value]: "failed" } : prev,
      );
    }, PREVIEW_TIMEOUT_MS);
    return () => {
      if (timeoutRef.current) window.clearTimeout(timeoutRef.current);
    };
  }, [entry.value, status]);

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      if (event.origin !== window.location.origin) return;
      const data = event.data as { type?: string; status?: string; appearance?: string } | null;
      if (!data || data.type !== "juno-bar-preview" || !data.appearance) return;
      const next = data.status === "ready" ? "ready" : data.status === "failed" ? "failed" : null;
      if (!next) return;
      setStatusByValue((prev) => ({ ...prev, [data.appearance as string]: next }));
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);

  // A frame that leaves the neighbourhood unmounts; when it comes back it
  // loads again, so its status starts over too.
  useEffect(() => {
    const keep = new Set<string>(mounted.map((m) => m.value));
    setStatusByValue((prev) => {
      const next: Record<string, PreviewStatus> = {};
      for (const [k, v] of Object.entries(prev)) if (keep.has(k)) next[k] = v;
      return Object.keys(next).length === Object.keys(prev).length ? prev : next;
    });
  }, [mounted]);

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
          {mounted.map((item) => {
            const current = item.value === entry.value;
            const shown = current && status === "ready";
            return (
              <iframe
                key={item.value}
                src={appearancePreviewUrl(item.value, { reducedMotion })}
                title={`Preview of the ${item.name} bar`}
                tabIndex={-1}
                aria-hidden="true"
                data-current={current ? "true" : "false"}
                className={cn(
                  "absolute inset-0 h-full w-full border-0 bg-transparent",
                  "pointer-events-none select-none",
                  shown ? "opacity-100" : "opacity-0",
                  !current && "invisible",
                  !reducedMotion && "transition-opacity duration-150 ease-out",
                )}
              />
            );
          })}
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
