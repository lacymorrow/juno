import { useCallback, useRef, useState } from "react";
import { EVENTS } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";

/**
 * How loud Juno's voice is right now, as Rust measures it.
 *
 * Rust opens a session when sound actually starts coming out (not when the
 * text is sent to the engine), streams a smoothed 0..1 level about 60 times a
 * second, and ends the session when the player exits or Escape kills it. This
 * hook only mirrors that; it never guesses. Any look can use it.
 *
 * Subscribe in the smallest component that moves with the voice (the Avatar's
 * face, not the whole bar), since it re-renders at the stream's rate.
 */
export interface SpeechLevel {
  /** Sound is coming out of Juno now. */
  speaking: boolean;
  /** 0 (silent) to 1 (loudest part of this utterance). */
  level: number;
}

interface LevelPayload {
  session?: number;
  level?: number;
}

interface SessionPayload {
  session?: number;
}

const SILENT: SpeechLevel = { speaking: false, level: 0 };

export function useSpeechLevel(): SpeechLevel {
  const [state, setState] = useState<SpeechLevel>(SILENT);
  const sessionRef = useRef<number | null>(null);

  const onStarted = useCallback((payload: SessionPayload) => {
    sessionRef.current = payload?.session ?? null;
    setState({ speaking: true, level: 0 });
  }, []);

  const onLevel = useCallback((payload: LevelPayload) => {
    const raw = typeof payload?.level === "number" && Number.isFinite(payload.level) ? payload.level : 0;
    const level = Math.min(1, Math.max(0, raw));
    if (payload?.session !== undefined) sessionRef.current = payload.session;
    setState((prev) => (prev.speaking && prev.level === level ? prev : { speaking: true, level }));
  }, []);

  const onEnded = useCallback((payload: SessionPayload) => {
    // An older utterance ending must not close the mouth on a newer one.
    const ended = payload?.session;
    if (ended !== undefined && sessionRef.current !== null && ended !== sessionRef.current) return;
    sessionRef.current = null;
    setState(SILENT);
  }, []);

  useEventListener<SessionPayload>(EVENTS.TTS_SPEECH_STARTED, onStarted);
  useEventListener<LevelPayload>(EVENTS.TTS_SPEECH_LEVEL, onLevel);
  useEventListener<SessionPayload>(EVENTS.TTS_SPEECH_ENDED, onEnded);

  return state;
}
