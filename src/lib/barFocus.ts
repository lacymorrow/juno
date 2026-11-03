import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";

/**
 * Give the bar the keyboard because the person is about to type in it (a type
 * button, a click in its input). Rust does the activating: on Windows the bar
 * is non-activating at rest and the window's own `setFocus` would press a
 * synthetic Alt that eats the next key; elsewhere it is that same `setFocus`.
 */
export function focusBarForTyping(): void {
  invoke(COMMANDS.BAR_FOCUS_BAR_FOR_TYPING).catch((error) =>
    console.debug("Bar: focus for typing failed:", error),
  );
}
