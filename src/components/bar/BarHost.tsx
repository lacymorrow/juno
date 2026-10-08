import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { UI, EVENTS, TIMEOUTS, COMMANDS } from "@/lib/constants.generated";
import type { FloatingBarConfig } from "@/types/bar-config";
import { useEventListener } from "@/hooks/useEventListener";

import { FloatingBar } from "@/components/FloatingBar";
import { bootBarConfig } from "@/lib/barBoot";
import { appearanceEntry } from "./appearanceCatalog";

// The default look (the Pill) loads with the page; the others when chosen.
const AppBar = lazy(() => import("@/components/bar/app-bar").then((m) => ({ default: m.AppBar })));
const IslandBar = lazy(() =>
  import("@/components/bar/island/IslandBar").then((m) => ({ default: m.IslandBar })),
);
const VoiceAIBar = lazy(() =>
  import("@/components/bar/voice-ai-bar").then((m) => ({ default: m.VoiceAIBar })),
);
// Lazy-load heavy components to avoid pulling Three.js/Rive into shared bundles
import { loadOrb, loadHalo, loadShaderOrb, loadAvatar, warmHeavyLooks } from "@/components/bar/lookChunks";

const ElevenLabsOrbBar = lazy(() => loadOrb().then((m) => ({ default: m.ElevenLabsOrbBar })));
const ReactOrbBar = lazy(() => loadHalo().then((m) => ({ default: m.ReactOrbBar })));
const ShaderOrbBar = lazy(() => loadShaderOrb().then((m) => ({ default: m.ShaderOrbBar })));
const PersonaBar = lazy(() => loadAvatar().then((m) => ({ default: m.PersonaBar })));

export function BarHost() {
  // Written into the page by Rust at launch, so the first render already
  // knows which look to draw; the command below still runs and wins.
  const [barConfig, setBarConfig] = useState<FloatingBarConfig | null>(() => bootBarConfig());

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
            bar_appearance: UI.BAR_APPEARANCES_DEFAULT,
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

  // A value no release wrote (or none at all) draws the default look, the same
  // entry the picker shows for it.
  const appearance = appearanceEntry(barConfig?.bar_appearance).value;
  const loaded = barConfig !== null;

  useEffect(() => {
    // Not inside a preview frame: the settings picker keeps three of those
    // mounted, and parsing Three.js, ogl and Rive in each would only slow it
    // down.
    if (!loaded || window.self !== window.top) return;
    return warmHeavyLooks();
  }, [loaded]);

  const Component = useMemo(() => {
    switch (appearance) {
      case UI.BAR_APPEARANCES_APP:
        return () => (
          <Suspense fallback={null}>
            <AppBar />
          </Suspense>
        );
      case UI.BAR_APPEARANCES_VOICE_AI:
        return () => (
          <Suspense fallback={null}>
            <VoiceAIBar barAppearance={appearance} />
          </Suspense>
        );
      case UI.BAR_APPEARANCES_DYNAMIC:
        return () => (
          <Suspense fallback={null}>
            <IslandBar />
          </Suspense>
        );
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
      case UI.BAR_APPEARANCES_SHADER_ORB:
        return () => (
          <Suspense fallback={null}>
            <ShaderOrbBar barAppearance={appearance} />
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

  return (
    <>
      <Component />
      <DebugWindowOutline />
    </>
  );
}

/**
 * In debug mode, a 1px black line around the bar window's own edge, so every
 * native resize and every transition is visible against the desktop. Not in a
 * preview frame: the settings picker's previews are not the window.
 */
function DebugWindowOutline() {
  const [on, setOn] = useState(false);

  const inPreview = window.self !== window.top;

  useEffect(() => {
    if (inPreview) return;
    let mounted = true;
    invoke<boolean>(COMMANDS.CORE_GET_DEBUG_MODE)
      .then((enabled) => { if (mounted) setOn(enabled === true); })
      .catch(() => {});
    return () => { mounted = false; };
  }, [inPreview]);

  useEventListener<boolean>(EVENTS.BAR_DEBUG_MODE_CHANGED, (enabled) => {
    if (!inPreview) setOn(enabled === true);
  });

  if (!on) return null;
  return (
    <div
      data-testid="debug-window-outline"
      aria-hidden
      style={{
        position: "fixed",
        inset: 0,
        border: "1px solid #000",
        pointerEvents: "none",
        zIndex: 2147483647,
      }}
    />
  );
}
