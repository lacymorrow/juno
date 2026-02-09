import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { UI, EVENTS, TIMEOUTS } from "@/lib/constants.generated";
import type { FloatingBarConfig } from "@/types/bar-config";

import { FloatingBar } from "@/components/FloatingBar";
import { AppBar } from "@/components/bar/app-bar";
import { DynamicBar } from "@/components/bar/dynamic-bar";
import { VoiceAIBar } from "@/components/bar/voice-ai-bar";
import { ElevenLabsBar } from "@/components/bar/elevenlabs-bar";

export function BarHost() {
  const [barConfig, setBarConfig] = useState<FloatingBarConfig | null>(null);

  useEffect(() => {
    let mounted = true;
    let unlisten: (() => void) | undefined;

    const load = async () => {
      try {
        const config = await invoke<FloatingBarConfig>("ui_get_bar_config");
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

  const Component = useMemo(() => {
    switch (appearance) {
      case UI.BAR_APPEARANCES_APP:
        return () => <AppBar />;
      case UI.BAR_APPEARANCES_VOICE_AI:
        return () => <VoiceAIBar barAppearance={appearance} />;
      case UI.BAR_APPEARANCES_DYNAMIC:
        return () => <DynamicBar barAppearance={appearance} />;
      case "elevenlabs":
        return () => <ElevenLabsBar barAppearance={appearance} />;
      case UI.BAR_APPEARANCES_FLOATING:
      default:
        return () => <FloatingBar barAppearance={appearance} />;
    }
  }, [appearance]);

  return <Component />;
}


