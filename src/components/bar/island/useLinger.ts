import { useEffect, useRef, useState } from "react";

interface UseLingerOptions {
  /** Count down at all. Off: the clock is reset and idle. */
  running: boolean;
  /** Hold the clock where it is (pointer over the island, focus inside it). */
  paused: boolean;
  durationMs: number;
  /** Any change starts the clock over: new content, a fresh start after
   *  engagement. */
  resetKey: string;
  onExpire: () => void;
}

/**
 * The linger clock behind the ring. Wall-clock based (Date.now) so a paused
 * clock resumes from where it stopped, and so tests can drive it with fake
 * timers.
 */
export function useLinger({ running, paused, durationMs, resetKey, onExpire }: UseLingerOptions) {
  const [remaining, setRemaining] = useState(durationMs);
  const remainingRef = useRef(durationMs);
  const onExpireRef = useRef(onExpire);
  onExpireRef.current = onExpire;

  // A new key, or a clock that is switched off, starts from the top.
  useEffect(() => {
    remainingRef.current = durationMs;
    setRemaining(durationMs);
  }, [resetKey, durationMs, running]);

  useEffect(() => {
    if (!running || paused) return;
    const startedAt = Date.now();
    const from = remainingRef.current;
    const id = window.setInterval(() => {
      const left = from - (Date.now() - startedAt);
      if (left <= 0) {
        window.clearInterval(id);
        remainingRef.current = 0;
        setRemaining(0);
        onExpireRef.current();
        return;
      }
      remainingRef.current = left;
      setRemaining(left);
    }, 50);
    return () => window.clearInterval(id);
  }, [running, paused, resetKey, durationMs]);

  return {
    /** Milliseconds left. */
    remaining,
    /** 1 at the start, 0 when it runs out. */
    progress: durationMs > 0 ? Math.max(0, Math.min(1, remaining / durationMs)) : 0,
  };
}
