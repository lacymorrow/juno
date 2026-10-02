import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { COMMANDS, UI } from "@/lib/constants.generated";
import type { FloatingBarConfig } from "@/types/bar-config";
import type { BarAppearance } from "@/components/bar/barAppearance";

/**
 * The saved bar appearance and the one way to change it. Settings and
 * onboarding both read and write through here, so a choice made in either
 * place goes through the same Rust command and the real bar switches at once.
 * Starts on the default look until the saved one arrives.
 */
export function useBarAppearance() {
  const [value, setValue] = useState<string>(UI.BAR_APPEARANCES_DEFAULT);
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const choseRef = useRef(false);

  useEffect(() => {
    let alive = true;
    invoke<{ bar_appearance?: string }>(COMMANDS.BAR_UI_GET_BAR_CONFIG)
      .then((config) => {
        if (alive && !choseRef.current && config?.bar_appearance) setValue(config.bar_appearance);
      })
      .catch((error) => console.error("Failed to load bar appearance:", error));
    return () => {
      alive = false;
    };
  }, []);

  // No success toast: the bar itself changes on screen, and the picker's stage
  // shows the new look. A toast would be a second, later, weaker signal.
  const change = useCallback(
    async (next: BarAppearance) => {
      if (savingRef.current) return;
      savingRef.current = true;
      choseRef.current = true;
      setSaving(true);
      const previous = value;
      setValue(next);
      try {
        const current = await invoke<FloatingBarConfig>(COMMANDS.BAR_UI_GET_BAR_CONFIG);
        await invoke(COMMANDS.BAR_UI_SET_BAR_CONFIG, {
          config: { ...current, bar_appearance: next },
        });
      } catch (error) {
        setValue(previous);
        console.error("Failed to update bar appearance:", error);
        toast.error("Failed to update bar appearance", { description: error as string });
      } finally {
        savingRef.current = false;
        setSaving(false);
      }
    },
    [value],
  );

  return { value, saving, change };
}
