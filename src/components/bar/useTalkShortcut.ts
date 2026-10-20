import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useEventListener } from "@/hooks/useEventListener";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { shortcutCaps } from "@/components/settings/KeyCaps";
import type { TriggerHints } from "@/components/onboarding/Onboarding";

/**
 * A trigger hint as tooltip detail: the gesture and the key caps, "Hold ⌃⌥".
 * Null when nothing keyboard-shaped is bound.
 */
export function hintDetail(hint: { shortcut: string; gesture: string } | null): string | null {
  if (!hint || !hint.shortcut.trim()) return null;
  const caps = shortcutCaps(hint.shortcut);
  if (caps.length === 0) return null;
  return `${hint.gesture} ${caps.map((cap) => cap.glyph).join("")}`;
}

/**
 * The talk shortcut, named in the Talk control's tooltip by the Pill and the
 * Island. Read from the live trigger registry, never written here, so the
 * tooltip cannot teach a dead key.
 */
export function useTalkShortcut(): string | null {
  const [triggerHints, setTriggerHints] = useState<TriggerHints | null>(null);
  const readTriggerHints = useCallback(() => {
    invoke<TriggerHints>(COMMANDS.TRIGGERS_GET_TRIGGER_HINTS)
      .then((hints) => setTriggerHints(hints ?? null))
      .catch((error) => console.debug("bar: no trigger hints:", error));
  }, []);
  useEffect(() => {
    readTriggerHints();
  }, [readTriggerHints]);
  useEventListener(EVENTS.TRIGGERS_CHANGED, readTriggerHints);
  return hintDetail(triggerHints?.agent ?? null);
}
