/**
 * Live preview of one bar appearance (route `/__bar-preview`).
 *
 * The settings window frames this route to show a person what each look does
 * before they pick it. It runs the REAL bar components through `BarHost` on the
 * same fake Tauri layer the state harness uses, so nothing here can move,
 * resize or focus a real window: every call the bar makes lands in the shim.
 *
 * A short script walks the bar through the moments that matter (resting,
 * listening, dictating, done) by emitting the same `bar-state-update` events
 * Rust would, then loops. With `?motion=reduced` it holds a single frame.
 *
 * Query: `appearance=<bar_appearance value>`, `motion=reduced` (optional).
 */

import { Component, useEffect, useLayoutEffect, useMemo, useState, type ReactNode } from "react";
import { emit } from "@tauri-apps/api/event";
import { EVENTS, UI } from "@/lib/constants.generated";
import { BarHost } from "@/components/bar/BarHost";
import { appearanceEntry } from "@/components/bar/appearanceCatalog";
import { harness, installBarHarnessTauri, setPreviewAppearance } from "./barHarnessTauri";

// The sentence the preview "dictates". Short enough to fit every look, long
// enough to show words arriving.
const DEMO_SENTENCE = "Send the draft to Maya and ask for Friday.";

interface Beat {
  state: string;
  /** How long this beat holds, in ms. */
  hold: number;
}

// One loop of the preview. Durations are tuned so the whole cycle reads in
// about eight seconds, which is roughly how long a person lingers on a picker.
const SCRIPT: Beat[] = [
  { state: UI.BAR_STATES_DEFAULT, hold: 1400 },
  { state: UI.BAR_STATES_LISTENING, hold: 1600 },
  { state: UI.BAR_STATES_DICTATING, hold: 2600 },
  { state: UI.BAR_STATES_TRANSCRIBING, hold: 700 },
  { state: UI.BAR_STATES_SUCCESS, hold: 1100 },
];

interface Frame {
  barState: string;
  audioLevel: number;
  transcriptionText: string;
}

function emitFrame({ barState, audioLevel, transcriptionText }: Frame): void {
  void emit(EVENTS.BAR_STATE_UPDATE, {
    barState,
    inputValue: "",
    lastSubmittedValue: "",
    currentError: null,
    transcriptionText,
    spokenText: "",
    voiceMode: UI.VOICE_MODES_IDLE,
    audioLevel,
    isAgentWorking: false,
    isDictationMode: barState === UI.BAR_STATES_DICTATING,
    isAlwaysListening: false,
    agentState: null,
  });
}

/** What the preview tells the settings window that frames it. */
export type PreviewMessage = { type: "juno-bar-preview"; status: "ready" | "failed"; appearance: string };

function post(status: PreviewMessage["status"], appearance: string): void {
  if (window.parent === window) return;
  const message: PreviewMessage = { type: "juno-bar-preview", status, appearance };
  window.parent.postMessage(message, window.location.origin);
}

/**
 * A bar that throws must not leave a silent blank stage. The boundary reports
 * the failure to the settings window so it can say so, and logs the cause.
 */
class PreviewBoundary extends Component<
  { appearance: string; children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch(error: unknown) {
    console.error(`[bar-preview] ${this.props.appearance} failed to render:`, error);
    post("failed", this.props.appearance);
  }
  render() {
    return this.state.failed ? null : this.props.children;
  }
}

function readQuery(): { appearance: string; reducedMotion: boolean } {
  const params = new URLSearchParams(window.location.search);
  const appearance = appearanceEntry(params.get("appearance")).value;
  const reducedMotion =
    params.get("motion") === "reduced" ||
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  return { appearance, reducedMotion };
}

function useHarnessWindow() {
  const [snap, setSnap] = useState(() => {
    const { frame, driven } = harness.snapshot();
    return { frame, driven };
  });
  useEffect(
    () =>
      harness.subscribe(() => {
        const { frame, driven } = harness.snapshot();
        setSnap({ frame, driven });
      }),
    [],
  );
  return snap;
}

/**
 * Natural size of the mounted bar, for looks that never size a window. Takes
 * a callback ref: the wrapper mounts after the shim is ready, so a plain ref
 * would still be empty when the observer effect first ran.
 */
function useNaturalSize() {
  const [el, setEl] = useState<HTMLDivElement | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    if (!el || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setSize({ width, height });
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [el]);
  return { ref: setEl, size };
}

export default function AppearancePreview() {
  const { appearance, reducedMotion } = useMemo(readQuery, []);

  // The shim must exist before any bar effect calls `listen` or `invoke`, and
  // the host must read the requested appearance on its first config fetch.
  const [ready, setReady] = useState(false);
  useLayoutEffect(() => {
    setPreviewAppearance(appearance);
    installBarHarnessTauri();
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    setReady(true);
  }, [appearance]);

  // Drive the script. Listening breathes the audio level; dictating reveals
  // the sentence a word at a time. Everything else is a single emit.
  useEffect(() => {
    if (!ready) return;

    if (reducedMotion) {
      emitFrame({
        barState: UI.BAR_STATES_DICTATING,
        audioLevel: 0.5,
        transcriptionText: DEMO_SENTENCE,
      });
      return;
    }

    let cancelled = false;
    const timers: number[] = [];
    const later = (fn: () => void, ms: number) => {
      timers.push(window.setTimeout(fn, ms));
    };

    const playBeat = (index: number) => {
      if (cancelled) return;
      const beat = SCRIPT[index % SCRIPT.length];
      const next = () => playBeat(index + 1);

      if (beat.state === UI.BAR_STATES_LISTENING) {
        // A gentle breath, 10 Hz, so meters and orbs have something to follow.
        const start = performance.now();
        const tick = () => {
          if (cancelled) return;
          const t = (performance.now() - start) / 1000;
          const level = 0.35 + 0.3 * Math.sin(t * 5) + 0.1 * Math.sin(t * 13);
          emitFrame({ barState: beat.state, audioLevel: level, transcriptionText: "" });
          if (performance.now() - start < beat.hold) later(tick, 100);
          else next();
        };
        tick();
        return;
      }

      if (beat.state === UI.BAR_STATES_DICTATING) {
        const words = DEMO_SENTENCE.split(" ");
        const step = beat.hold / (words.length + 2);
        words.forEach((_, i) => {
          later(() => {
            emitFrame({
              barState: beat.state,
              audioLevel: 0.5,
              transcriptionText: words.slice(0, i + 1).join(" "),
            });
          }, step * i);
        });
        later(next, beat.hold);
        return;
      }

      emitFrame({
        barState: beat.state,
        audioLevel: 0,
        transcriptionText: beat.state === UI.BAR_STATES_TRANSCRIBING ? DEMO_SENTENCE : "",
      });
      later(next, beat.hold);
    };

    playBeat(0);
    return () => {
      cancelled = true;
      timers.forEach((id) => window.clearTimeout(id));
    };
  }, [ready, reducedMotion]);

  const { frame, driven } = useHarnessWindow();
  const { ref: naturalRef, size: natural } = useNaturalSize();

  // Tell the framing window once the bar has painted at a real size.
  const painted = driven ? frame.width > 0 : natural.width > 0;
  useEffect(() => {
    if (ready && painted) post("ready", appearance);
  }, [ready, painted, appearance]);

  // A bar that sizes its own window (the pill, the island, the studio) gets a
  // window of exactly that size, like the harness gives it. A bar that never
  // does (the app bar, the orbs, the avatar) is laid out at its natural size.
  // Either way the result is centred and shrunk only if it would not fit, so
  // every look is shown at the size a person would actually see it.
  const [viewport, setViewport] = useState({ w: window.innerWidth, h: window.innerHeight });
  useEffect(() => {
    const onResize = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  const box = driven ? { width: frame.width, height: frame.height } : natural;
  const scale =
    box.width > 0 && box.height > 0
      ? Math.min(1, viewport.w / box.width, viewport.h / box.height)
      : 1;
  const left = (viewport.w - box.width * scale) / 2;
  const top = (viewport.h - box.height * scale) / 2;

  if (!ready) return null;

  return (
    <div
      className="relative h-screen w-screen overflow-hidden"
      data-appearance={appearance}
      data-preview-ready="true"
      aria-hidden="true"
    >
      <PreviewBoundary appearance={appearance}>
        {driven ? (
          <>
            <style>{`.jbp-window > div{width:100%!important;height:100%!important;min-width:0!important;min-height:0!important;}`}</style>
            <div
              className="jbp-window absolute origin-top-left"
              style={{ left, top, width: frame.width, height: frame.height, transform: `scale(${scale})` }}
            >
              <BarHost />
            </div>
          </>
        ) : (
          <div
            ref={naturalRef}
            className="absolute origin-top-left inline-block"
            style={{ left, top, transform: `scale(${scale})` }}
          >
            <BarHost />
          </div>
        )}
      </PreviewBoundary>
    </div>
  );
}
