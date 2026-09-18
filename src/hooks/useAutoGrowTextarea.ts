import { useCallback, useLayoutEffect, useRef, useState } from "react";

/**
 * A textarea that grows with its content, in JavaScript.
 *
 * CSS can do this with `field-sizing: content`, and the codebase asked for it
 * that way, but it is a Chromium feature and this app runs in WKWebView, where
 * it silently does nothing. So the class was present and the behaviour never
 * was: long messages scrolled inside a fixed box instead of opening it up.
 *
 * Lives in one place because all three composers need it, and the floating bar
 * needs the measured height as well: its window is sized to its contents, so a
 * pill that grows without telling the window would just clip.
 */
export function useAutoGrowTextarea({
  value,
  maxHeightPx,
  /** Height of a single line, so a fresh composer starts exactly one line tall. */
  minHeightPx,
}: {
  value: string;
  maxHeightPx: number;
  minHeightPx?: number;
}) {
  const ref = useRef<HTMLTextAreaElement | null>(null);
  // What the caller needs to size a window around: how tall the box ended up.
  const [height, setHeight] = useState(minHeightPx ?? 0);

  const measure = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    // Measure from empty: scrollHeight never shrinks on its own, so without
    // this the box would only ever get taller.
    el.style.height = "auto";
    const natural = el.scrollHeight;
    const next = Math.max(minHeightPx ?? 0, Math.min(natural, maxHeightPx));
    el.style.height = `${next}px`;
    // Scroll only once it has stopped growing, so the caret stays visible.
    el.style.overflowY = natural > maxHeightPx ? "auto" : "hidden";
    setHeight((prev) => (prev === next ? prev : next));
  }, [maxHeightPx, minHeightPx]);

  useLayoutEffect(measure, [measure, value]);

  /** Attach to the textarea; measures as soon as it exists. */
  const attach = useCallback(
    (node: HTMLTextAreaElement | null) => {
      ref.current = node;
      measure();
    },
    [measure]
  );

  return { ref, attach, height, measure };
}

/**
 * Enter sends, Shift+Enter makes a new line.
 *
 * Returns true when the event was a send, so callers can do their own
 * submitting rather than depending on a surrounding form.
 */
export function isSendKey(event: {
  key: string;
  shiftKey: boolean;
  nativeEvent?: { isComposing?: boolean };
}): boolean {
  if (event.key !== "Enter" || event.shiftKey) return false;
  // Mid-composition Enter belongs to the input method, not to us.
  return !event.nativeEvent?.isComposing;
}
