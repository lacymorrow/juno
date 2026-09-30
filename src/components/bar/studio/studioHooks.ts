import { useEffect, useRef, useState } from "react";
import {
  WAVE_SAMPLES,
  WAVE_SAMPLE_MS,
  smoothLevel,
  speechEnvelope,
  type CounterMode,
  type WaveMode,
} from "./studioModel";

/** A flat strip: every sample is silence. */
const SILENCE: number[] = Array.from({ length: WAVE_SAMPLES }, () => 0);

interface WaveSamplesOptions {
  mode: WaveMode;
  /** `audioLevel` from the bar state, 0..1. */
  level: number;
  /** The sentence Juno is speaking, for the synthesised envelope. */
  spokenText: string;
  reducedMotion: boolean;
}

/**
 * The waveform's ring buffer. Every WAVE_SAMPLE_MS it pushes one smoothed
 * level: the mic level while you speak, the speech envelope while Juno
 * speaks, silence otherwise. The clock stops once the strip has settled
 * flat, so an idle deck costs nothing. Reduce Motion gets a static row of
 * the current level instead of a scrolling history.
 */
export function useWaveSamples({ mode, level, spokenText, reducedMotion }: WaveSamplesOptions): number[] {
  const [samples, setSamples] = useState<number[]>(SILENCE);
  const levelRef = useRef(level);
  levelRef.current = level;
  const smoothedRef = useRef(0);
  const speechStartRef = useRef(0);
  useEffect(() => {
    if (mode === "speech") speechStartRef.current = Date.now();
  }, [mode, spokenText]);

  const sounding = mode === "live" || mode === "speech";
  const settled = !sounding && smoothedRef.current < 0.01 && samples === SILENCE;

  useEffect(() => {
    if (settled) return;
    let last = Date.now();
    const tick = () => {
      const now = Date.now();
      const dt = now - last;
      last = now;
      const target =
        mode === "live"
          ? levelRef.current
          : mode === "speech"
            ? speechEnvelope(spokenText, now - speechStartRef.current)
            : 0;
      const next = smoothLevel(smoothedRef.current, target, dt);
      smoothedRef.current = next;
      if (!sounding && next < 0.01) {
        smoothedRef.current = 0;
        setSamples(SILENCE);
        window.clearInterval(id);
        return;
      }
      setSamples((prev) => {
        if (reducedMotion) return Array.from({ length: WAVE_SAMPLES }, () => next);
        const out = prev.length === WAVE_SAMPLES ? prev.slice(1) : SILENCE.slice(1);
        out.push(next);
        return out;
      });
    };
    const id = window.setInterval(tick, WAVE_SAMPLE_MS);
    return () => window.clearInterval(id);
  }, [mode, spokenText, sounding, settled, reducedMotion]);

  return samples;
}

/**
 * The tape counter's clock. Starts from 00:00.0 when the mode turns to
 * running, keeps its number while held, and forgets it when off.
 */
export function useTapeCounter(mode: CounterMode): number {
  const [ms, setMs] = useState(0);
  const startedAtRef = useRef<number | null>(null);
  const heldRef = useRef(0);
  useEffect(() => {
    if (mode === "off") {
      startedAtRef.current = null;
      heldRef.current = 0;
      setMs(0);
      return;
    }
    if (mode === "held") {
      startedAtRef.current = null;
      setMs(heldRef.current);
      return;
    }
    const startedAt = Date.now() - heldRef.current;
    startedAtRef.current = startedAt;
    const id = window.setInterval(() => {
      const elapsed = Date.now() - startedAt;
      heldRef.current = elapsed;
      setMs(elapsed);
    }, 100);
    return () => window.clearInterval(id);
  }, [mode]);
  return ms;
}
