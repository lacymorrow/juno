import { useCallback, useState, type ClipboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";

/**
 * Pictures pasted into a bar's text box, kept as data URLs and sent with the
 * typed text. Shared by the Pill and the Island so both read the clipboard
 * the same way and send the same way.
 */
export interface PastedImages {
  images: string[];
  /** The text box's `onPaste`. Text pastes are left to the browser. */
  onPaste: (event: ClipboardEvent) => void;
  remove: (index: number) => void;
  clear: () => void;
}

export function usePastedImages(): PastedImages {
  const [images, setImages] = useState<string[]>([]);

  /** Read pasted images off the clipboard as data URLs. */
  const onPaste = useCallback((event: ClipboardEvent) => {
    const files = Array.from(event.clipboardData?.items ?? [])
      .filter((item) => item.kind === "file" && item.type.startsWith("image/"))
      .map((item) => item.getAsFile())
      .filter((file): file is File => file !== null);
    if (files.length === 0) return;
    event.preventDefault();
    for (const file of files) {
      const reader = new FileReader();
      reader.onload = () => {
        const url = reader.result;
        if (typeof url === "string") setImages((prev) => [...prev, url]);
      };
      reader.readAsDataURL(file);
    }
  }, []);

  const remove = useCallback((index: number) => {
    setImages((prev) => prev.filter((_, i) => i !== index));
  }, []);
  const clear = useCallback(() => setImages([]), []);

  return { images, onPaste, remove, clear };
}

/**
 * Send text with pictures. The bar-state interaction carries only a string,
 * so an attachment goes straight to the agent rather than being quietly
 * dropped.
 */
export async function dispatchWithImages(query: string, images: string[], who = "Bar"): Promise<void> {
  await invoke(COMMANDS.AGENT_DISPATCH_QUERY, { query, images }).catch((error) =>
    console.error(`${who}: submit failed:`, error),
  );
}
