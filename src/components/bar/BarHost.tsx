import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { UI, EVENTS, TIMEOUTS, COMMANDS } from "@/lib/constants.generated";
import type { FloatingBarConfig } from "@/types/bar-config";

import { FloatingBar } from "@/components/FloatingBar";
import { AppBar } from "@/components/bar/app-bar";
import { IslandBar } from "@/components/bar/island/IslandBar";
import { VoiceAIBar } from "@/components/bar/voice-ai-bar";
// Lazy-load heavy components to avoid pulling Three.js/Rive into shared bundles
const loadOrb = () => import("@/components/bar/elevenlabs-orb-bar");
const loadHalo = () => import("@/components/bar/react-orb-bar");
const loadAvatar = () => import("@/components/bar/persona-bar");
const ElevenLabsOrbBar = lazy(() => loadOrb().then((m) => ({ default: m.ElevenLabsOrbBar })));
const ReactOrbBar = lazy(() => loadHalo().then((m) => ({ default: m.ReactOrbBar })));
const PersonaBar = lazy(() => loadAvatar().then((m) => ({ default: m.PersonaBar })));

/**
 * Fetch and parse the heavy looks once the first bar has painted, so switching
 * to one later lands on warm code instead of a blank window while Three.js or
 * Rive arrive. Idle time first; a timer if the browser offers no idle callback.
 */
function warmHeavyLooks(): () => void {
  const warm = () => {
    void loadOrb();
    void loadHalo();
    void loadAvatar();
  };
  const w = window as Window & {
    requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number;
    cancelIdleCallback?: (id: number) => void;
  };
  if (typeof w.requestIdleCallback === "function") {
    const id = w.requestIdleCallback(warm, { timeout: 3000 });
    return () => w.cancelIdleCallback?.(id);
  }
  const id = window.setTimeout(warm, 1500);
  return () => window.clearTimeout(id);
}

export function BarHost() {
  const [barConfig, setBarConfig] = useState<FloatingBarConfig | null>(null);

  useEffect(() => {
    let mounted = true;
    let unlisten: (() => void) | undefined;

    const load = async () => {
      try {
        const config = await invoke<FloatingBarConfig>(COMMANDS.BAR_UI_GET_BAR_CONFIG);
        if (mounted) setBarConfig(config);
      } catch (error) {
        console.error("Failed to load bar config:", error);
        if (mounted) {
          setBarConfig({
            show_voice_indicator: true,
            enable_animations: true,
            auto_hide: false,
            auto_hide_delay: TIMEOUTS.UI_NOTIFICATION_DISPLAY_MS,
            opacity: 0.95,
            bar_appearance: UI.BAR_APPEARANCES_FLOATING,
            show_glow_border: true,
          });
        }
      }
    };

    const setupListener = async () => {
      try {
        const fn = await listen<FloatingBarConfig>(
          EVENTS.BAR_CONFIG_CHANGED,
          (event) => { if (mounted) setBarConfig(event.payload); }
        );
        if (mounted) unlisten = fn;
        else fn();
      } catch (error) {
        console.error("Failed to setup bar config listener:", error);
      }
    };

    load();
    setupListener();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  const appearance = barConfig?.bar_appearance ?? UI.BAR_APPEARANCES_FLOATING;
  const loaded = barConfig !== null;

  useEffect(() => {
    // Not inside a preview frame: the settings picker keeps three of those
    // mounted, and parsing Three.js and Rive in each would only slow it down.
    if (!loaded || window.self !== window.top) return;
    return warmHeavyLooks();
  }, [loaded]);

  const Component = useMemo(() => {
    switch (appearance) {
      case UI.BAR_APPEARANCES_APP:
        return () => <AppBar />;
      case UI.BAR_APPEARANCES_VOICE_AI:
        return () => <VoiceAIBar barAppearance={appearance} />;
      case UI.BAR_APPEARANCES_DYNAMIC:
        return () => <IslandBar />;
      case UI.BAR_APPEARANCES_ORB:
        return () => (
          <Suspense fallback={null}>
            <ElevenLabsOrbBar barAppearance={appearance} />
          </Suspense>
        );
      case UI.BAR_APPEARANCES_REACT_ORB:
        return () => (
          <Suspense fallback={null}>
            <ReactOrbBar barAppearance={appearance} />
          </Suspense>
        );
      case UI.BAR_APPEARANCES_PERSONA:
        return () => (
          <Suspense fallback={null}>
            <PersonaBar barAppearance={appearance} />
          </Suspense>
        );
      case UI.BAR_APPEARANCES_FLOATING:
      default:
        return () => <FloatingBar barAppearance={appearance} />;
    }
  }, [appearance]);

  // Until the config arrives there is nothing to show. Painting the default
  // look first would flash the wrong bar, and its effects would size the
  // window for a component that is about to be replaced.
  if (!loaded) return null;

  return <Component />;
}
