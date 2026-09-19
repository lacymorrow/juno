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
  // The attached element, in state as well as in the ref, so the measurement
  // effect below has something that changes when a textarea appears. See the
  // effect for why a ref alone was not enough.
  const [node, setNode] = useState<HTMLTextAreaElement | null>(null);
  // What the caller needs to size a window around: how tall the box ended up.
  const [height, setHeight] = useState(minHeightPx ?? 0);

  const measure = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    // Measure from empty: scrollHeight never shrinks on its own, so without
    // this the box would only ever get taller.
    el.style.height = "auto";
    // An empty box is one line, and that is known without measuring anything.
    // `scrollHeight` answers with the larger of the content and the box, so a
    // textarea whose height has not settled yet reports its own leftover size
    // rather than the (absent) text, and an empty composer came back several
    // lines tall. There is no text here to be wrong about.
    //
    // Only where a caller has said what one line is. A composer that gave no
    // minimum (the main window's, which takes its resting height from CSS) has
    // nothing to substitute and is measured as it always was.
    const natural =
      el.value === "" && minHeightPx !== undefined ? minHeightPx : el.scrollHeight;
    const next = Math.max(minHeightPx ?? 0, Math.min(natural, maxHeightPx));
    el.style.height = `${next}px`;
    // Scroll only once it has stopped growing, so the caret stays visible.
    el.style.overflowY = natural > maxHeightPx ? "auto" : "hidden";
    setHeight((prev) => (prev === next ? prev : next));
  }, [maxHeightPx, minHeightPx]);

  /**
   * Measure whenever the text changes, and whenever a textarea arrives.
   *
   * `node` is in this list because `value` on its own silently skipped the one
   * measurement that mattered. The bar's composer unmounts whenever the bar
   * leaves its input and mounts again when it comes back, with the same empty
   * value it had when it left, so on the way back in these dependencies were
   * unchanged from the previous render and React did not run this at all. The
   * only measurement left was the one inside `attach`, taken in the commit
   * phase before the pill around it had been laid out at its new size, and
   * nothing corrected it until the first keystroke changed `value`. That is
   * exactly the shape of the bug: a composer that opens several rows tall and
   * collapses to one the moment you type. A new element is a new measurement.
   */
  useLayoutEffect(measure, [measure, value, node]);

  /**
   * Attach to the textarea.
   *
   * Detaching resets the measured height, because a composer that is not on
   * screen has no height and the caller sizes a window around this number.
   * Attaching writes the one-line height straight away, so even a paint that
   * beats the effect above is already the right size rather than whatever
   * `rows` happened to give, and hands the element to that effect to measure
   * properly once the browser has laid the new frame out.
   */
  const attach = useCallback(
    (el: HTMLTextAreaElement | null) => {
      ref.current = el;
      if (!el) {
        setNode(null);
        setHeight(minHeightPx ?? 0);
        return;
      }
      if (minHeightPx !== undefined) el.style.height = `${minHeightPx}px`;
      setNode(el);
    },
    [minHeightPx]
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
