/**
 * The lazy-loaded look chunks, in one place so the bar, Settings and
 * onboarding all warm the same modules. Importing a chunk twice is free: the
 * module graph and the HTTP cache (shared with the preview frames) serve it.
 */
export const loadOrb = () => import("@/components/bar/elevenlabs-orb-bar");
export const loadHalo = () => import("@/components/bar/react-orb-bar");
export const loadShaderOrb = () => import("@/components/bar/shader-orb-bar");
export const loadAvatar = () => import("@/components/bar/persona-bar");

/** Start fetching every look now. Failures are silent; the look loads on demand. */
export function preloadLooks(): void {
  for (const load of [loadOrb, loadHalo, loadShaderOrb, loadAvatar]) {
    void load().catch(() => {});
  }
}

/**
 * Fetch and parse the heavy looks once the first paint is done, so a later
 * switch lands on warm code. Idle time first; a timer if there is no idle
 * callback.
 */
export function warmHeavyLooks(): () => void {
  const w = window as Window & {
    requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number;
    cancelIdleCallback?: (id: number) => void;
  };
  if (typeof w.requestIdleCallback === "function") {
    const id = w.requestIdleCallback(preloadLooks, { timeout: 3000 });
    return () => w.cancelIdleCallback?.(id);
  }
  const id = window.setTimeout(preloadLooks, 1500);
  return () => window.clearTimeout(id);
}
