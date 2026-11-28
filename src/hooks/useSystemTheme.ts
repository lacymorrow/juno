import { useEffect, useLayoutEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type SystemTheme = "light" | "dark";

/**
 * The OS appearance, kept in sync with macOS light/dark like the native
 * System Settings window. Reads the window's theme from Tauri and follows
 * `onThemeChanged`; falls back to the CSS media query when Tauri isn't
 * present (e.g. the Vite dev server in a plain browser).
 */
export function useSystemTheme(): SystemTheme {
  const [theme, setTheme] = useState<SystemTheme>(() => {
    if (typeof window !== "undefined" && window.matchMedia) {
      return window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    }
    return "light";
  });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;

    (async () => {
      try {
        const w = getCurrentWindow();
        const current = await w.theme();
        if (active && current) setTheme(current === "dark" ? "dark" : "light");
        unlisten = await w.onThemeChanged(({ payload }) => {
          setTheme(payload === "dark" ? "dark" : "light");
        });
      } catch {
        // Not running under Tauri — fall back to the media query when present.
        if (typeof window === "undefined" || !window.matchMedia) return;
        const mq = window.matchMedia("(prefers-color-scheme: dark)");
        const onChange = (e: MediaQueryListEvent) =>
          setTheme(e.matches ? "dark" : "light");
        mq.addEventListener("change", onChange);
        unlisten = () => mq.removeEventListener("change", onChange);
      }
    })();

    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  return theme;
}

/**
 * Puts the OS appearance on this window's `<html>` as the `dark` class, which
 * is what the theme tokens and every `dark:` utility key off. It goes on the
 * document rather than a wrapper because Radix portals menus, dialogs and
 * popovers to `document.body`. Each Tauri window owns its own document.
 */
export function useDocumentTheme(): SystemTheme {
  const theme = useSystemTheme();

  // Before paint, so a dark window never draws a light frame first.
  useLayoutEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", theme === "dark");
    return () => root.classList.remove("dark");
  }, [theme]);

  return theme;
}
