/**
 * What Rust writes into the floating bar's page before any script runs
 * (`startup_windows::boot_script`), so the bar can paint without asking:
 * its config (which look to draw) and the well it was left in.
 *
 * Read only. When the payload is missing (a page reload, a preview frame, the
 * dev server) every caller falls back to the command it always used.
 */

import type { FloatingBarConfig } from "@/types/bar-config";

interface BarBoot {
  bar_config: FloatingBarConfig;
  bar_position: { x: number; y: number } | null;
}

declare global {
  interface Window {
    __JUNO_BAR_BOOT__?: BarBoot | null;
  }
}

function boot(): BarBoot | null {
  if (typeof window === "undefined") return null;
  // Only the bar window itself: a preview frame is not the window.
  if (window.self !== window.top) return null;
  return window.__JUNO_BAR_BOOT__ ?? null;
}

/** The bar's config as it was at launch, or null when Rust wrote none. */
export function bootBarConfig(): FloatingBarConfig | null {
  return boot()?.bar_config ?? null;
}

/**
 * The stored well, once. `undefined` means "not provided, ask"; `null` means
 * "provided: nothing stored yet". Consumed on read, because it is only true at
 * launch: the bar moves, and a later mount must ask for where it is now.
 */
export function takeBootBarPosition(): { x: number; y: number } | null | undefined {
  const payload = boot();
  if (!payload || !("bar_position" in payload)) return undefined;
  const position = payload.bar_position;
  delete (payload as Partial<BarBoot>).bar_position;
  return position ?? null;
}
