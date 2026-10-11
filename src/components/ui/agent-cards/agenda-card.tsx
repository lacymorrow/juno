/**
 * AgendaCard: events and reminders, one shape.
 *
 * Each row is a title, when it happens (already worded by the backend, such as
 * "Tomorrow 9:00 AM"), where, and whether it is done. The Mac app tools hand the
 * finished tag back and the model passes it through, the way `<NowPlayingCard>`
 * is emitted. The card holds no state and reads nothing: it draws what it is
 * given.
 *
 * Two quiet states live here so no other component has to know about them:
 * nothing to show (one line, the backend's own words), and access turned off
 * (one sentence and one button that opens the right row of System Settings).
 *
 * Flat on purpose: system blue is the only accent, no gradients, no glow.
 */

import { invoke } from "@tauri-apps/api/core";
import { Check, MapPin } from "lucide-react";
import { useCallback, useState } from "react";
import { COMMANDS } from "@/lib/constants.generated";
import { cn } from "@/lib/utils";

export interface AgendaCardItem {
  title: string;
  /** Worded by the backend: "Today 3:00 PM", "Friday", "Oct 20, all day". */
  when?: string;
  where?: string;
  done?: boolean;
  /** Events get a bar, reminders get a circle. */
  kind?: "event" | "reminder" | string;
}

export interface AgendaCardProps {
  items?: AgendaCardItem[];
  /** What to say when there are no items. */
  empty?: string;
  /** Set when access is off. Names the app: "Reminders". */
  needsAccess?: string;
  /** Where the one switch lives. Only System Settings links are followed. */
  settingsUrl?: string;
  className?: string;
}

const SETTINGS_PREFIX = "x-apple.systempreferences:";

/** System blue, the one accent. */
const BLUE = "text-[#0a84ff]";

function Marker({ item }: { item: AgendaCardItem }) {
  if (item.kind === "event") {
    return (
      <span
        aria-hidden="true"
        className={cn("mt-0.5 h-4 w-[3px] shrink-0 rounded-full", item.done ? "bg-muted-foreground/40" : "bg-[#0a84ff]")}
      />
    );
  }
  return (
    <span
      aria-hidden="true"
      className={cn(
        "mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border",
        item.done ? "border-[#0a84ff] bg-[#0a84ff] text-white" : "border-muted-foreground/50",
      )}
    >
      {item.done ? <Check className="h-3 w-3" strokeWidth={3} /> : null}
    </span>
  );
}

export function AgendaCard({
  items,
  empty = "Nothing to show.",
  needsAccess,
  settingsUrl,
  className,
}: AgendaCardProps) {
  const [opening, setOpening] = useState(false);
  const rows = Array.isArray(items) ? items : [];

  const openSettings = useCallback(async () => {
    if (!settingsUrl || !settingsUrl.startsWith(SETTINGS_PREFIX)) return;
    setOpening(true);
    try {
      await invoke(COMMANDS.DESKTOP_OPEN_URL, { url: settingsUrl });
    } catch (err) {
      console.error("AgendaCard: could not open System Settings:", err);
    } finally {
      setOpening(false);
    }
  }, [settingsUrl]);

  if (needsAccess) {
    const canOpen = !!settingsUrl && settingsUrl.startsWith(SETTINGS_PREFIX);
    return (
      <div
        className={cn("flex items-center justify-between gap-3 rounded-xl border bg-card p-4", className)}
        data-testid="agenda-card"
        data-state="needs-access"
      >
        <p className="text-sm">Juno needs your OK to use {needsAccess}.</p>
        {canOpen ? (
          <button
            type="button"
            onClick={() => void openSettings()}
            disabled={opening}
            className={cn(
              "h-7 shrink-0 rounded-md bg-[#0a84ff] px-3 text-xs font-medium text-white",
              "hover:bg-[#0a84ff]/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              "disabled:opacity-50",
            )}
          >
            Turn on {needsAccess}
          </button>
        ) : null}
      </div>
    );
  }

  if (rows.length === 0) {
    return (
      <div
        className={cn("rounded-xl border bg-card p-4 text-sm text-muted-foreground", className)}
        data-testid="agenda-card"
        data-state="empty"
      >
        {empty}
      </div>
    );
  }

  return (
    <ul
      className={cn("divide-y rounded-xl border bg-card", className)}
      data-testid="agenda-card"
      data-state="items"
    >
      {rows.map((item, index) => (
        <li key={`${item.title}-${index}`} className="flex items-start gap-3 px-4 py-3">
          <Marker item={item} />
          <div className="min-w-0 flex-1">
            <div className={cn("truncate text-sm font-medium", item.done && "text-muted-foreground line-through")}>
              {item.title}
            </div>
            {item.when || item.where ? (
              <div className="mt-0.5 flex flex-wrap items-center gap-x-3 text-xs text-muted-foreground">
                {item.when ? <span className={cn("tabular-nums", !item.done && BLUE)}>{item.when}</span> : null}
                {item.where ? (
                  <span className="inline-flex min-w-0 items-center gap-1">
                    <MapPin className="h-3 w-3 shrink-0" aria-hidden="true" />
                    <span className="truncate">{item.where}</span>
                  </span>
                ) : null}
              </div>
            ) : null}
          </div>
        </li>
      ))}
    </ul>
  );
}
