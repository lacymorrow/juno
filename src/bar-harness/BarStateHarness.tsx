/**
 * Bar state harness (route `/__bar-harness`, unlinked).
 *
 * A diagnostic bench for the floating bar's graphical behaviour: it renders the
 * REAL `FloatingBar` (and, through it, the real chat pane, roster strip and
 * snap-well math) with no Rust backend, and gives a developer hand controls to
 * drive every state, resize the simulated window container, and drop the bar
 * into any gravity well. It exists to reproduce resize, snap and self-resize
 * bugs by hand.
 *
 * `barHarnessTauri` is imported FIRST, on purpose: its module side-effect
 * installs the fake Tauri internals before the bar module below is evaluated,
 * so the bar's listeners and geometry reads have somewhere to land.
 */

import { harness } from "./barHarnessTauri";

import { useCallback, useEffect, useMemo, useState, useSyncExternalStore, type ReactNode } from "react";
import { emit } from "@tauri-apps/api/event";

import { FloatingBar } from "@/components/FloatingBar";
import { EVENTS, UI } from "@/lib/constants.generated";
import { computeWells, type MonitorRect, type Well } from "@/lib/snapWells";

// === STATE CATALOG ===
// Every value the backend UIManager can put the bar in, grouped the way the bar
// itself groups them (idle / input / voice / working / terminal).

interface StatePreset {
  value: string;
  label: string;
}

const STATE_GROUPS: { title: string; states: StatePreset[] }[] = [
  {
    title: "Idle",
    states: [
      { value: UI.BAR_STATES_DEFAULT, label: "default (compact pill)" },
      { value: UI.BAR_STATES_DICTATION_READY, label: "dictation ready" },
      { value: UI.BAR_STATES_SHRINKING, label: "shrinking" },
    ],
  },
  {
    title: "Input",
    states: [
      { value: UI.BAR_STATES_INPUT, label: "input (composer open)" },
      { value: UI.BAR_STATES_EXPANDING, label: "expanding" },
    ],
  },
  {
    title: "Voice",
    states: [
      { value: UI.BAR_STATES_LISTENING, label: "listening" },
      { value: UI.BAR_STATES_ALWAYS_LISTENING, label: "always listening" },
      { value: UI.BAR_STATES_DICTATING, label: "dictating" },
      { value: UI.BAR_STATES_TRANSCRIBING, label: "transcribing" },
    ],
  },
  {
    title: "Working",
    states: [
      { value: UI.BAR_STATES_SUBMITTING, label: "submitting" },
      { value: UI.BAR_STATES_LOADING, label: "loading" },
      { value: UI.BAR_STATES_AGENT_RESPONDING, label: "agent responding" },
      { value: UI.BAR_STATES_FINISHING, label: "finishing" },
      { value: UI.BAR_STATES_STOPPING, label: "stopping" },
    ],
  },
  {
    title: "Terminal",
    states: [
      { value: UI.BAR_STATES_SUCCESS, label: "success" },
      { value: UI.BAR_STATES_ERROR, label: "error" },
      { value: UI.BAR_STATES_SPEAKING, label: "speaking" },
    ],
  },
];

/** Human name for a well's slot fractions, for the placement buttons. */
function slotName(fx: number, fy: number): string {
  const col = fx === 0 ? "left" : fx === 1 ? "right" : "center";
  const row = fy === 0 ? "top" : fy === 1 ? "bottom" : "middle";
  if (row === "middle" && col === "center") return "center";
  return `${row} ${col}`;
}

// System blue is the only accent, per the Juno design rules; the panel chrome
// stays flat and neutral so it reads like a macOS utility window.
const ACCENT = "#0A84FF";

function useHarness() {
  return useSyncExternalStore(harness.subscribe, harness.snapshot, harness.snapshot);
}

// === SMALL PANEL PRIMITIVES ===

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="border-b border-black/10 px-4 py-3">
      <h2 className="mb-2 text-[11px] font-semibold uppercase tracking-wide text-black/45">
        {title}
      </h2>
      {children}
    </section>
  );
}

function Btn({
  active,
  onClick,
  children,
}: {
  active?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-md border px-2.5 py-1 text-[12px] transition-colors"
      style={
        active
          ? { background: ACCENT, borderColor: ACCENT, color: "#fff" }
          : { background: "#fff", borderColor: "rgba(0,0,0,0.14)", color: "#1d1d1f" }
      }
    >
      {children}
    </button>
  );
}

function NumberField({
  label,
  value,
  onChange,
  step = 1,
}: {
  label: string;
  value: number;
  onChange: (n: number) => void;
  step?: number;
}) {
  return (
    <label className="flex items-center justify-between gap-2 text-[12px] text-black/70">
      <span>{label}</span>
      <input
        type="number"
        value={Math.round(value)}
        step={step}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-20 rounded-md border border-black/15 bg-white px-2 py-1 text-right text-[12px] tabular-nums outline-none focus:border-[#0A84FF]"
      />
    </label>
  );
}

// === THE HARNESS ===

export default function BarStateHarness() {
  const { monitor, frame, requested, freezeAutoResize } = useHarness();
  const scale = monitor.scaleFactor;

  // Backend-state fields the developer can shape. They are re-emitted as one
  // `bar-state-update` whenever any of them changes, exactly as Rust would.
  const [barState, setBarState] = useState<string>(UI.BAR_STATES_DEFAULT);
  const [audioLevel, setAudioLevel] = useState(0.6);
  const [errorText, setErrorText] = useState("Something went wrong");
  const [agentLabel, setAgentLabel] = useState("thinking");
  const [transcriptText, setTranscriptText] = useState("");
  const [spokenText, setSpokenText] = useState("Here is what I found.");

  // Independent signals the bar listens for on their own events.
  const [hovered, setHovered] = useState(false);
  const [driving, setDriving] = useState(false);
  // The app the "using the mouse in X" label names while Juno drives.
  const drivingApp = "Safari";
  const [wake, setWake] = useState<"none" | "paused" | "armed">("none");
  const [rosterCount, setRosterCount] = useState(0);

  // Simulated screen zoom, so a 1440-wide desktop fits the panel.
  const [zoom, setZoom] = useState(0.5);

  const emitState = useCallback(() => {
    const working =
      barState === UI.BAR_STATES_SUBMITTING ||
      barState === UI.BAR_STATES_LOADING ||
      barState === UI.BAR_STATES_AGENT_RESPONDING ||
      barState === UI.BAR_STATES_FINISHING ||
      barState === UI.BAR_STATES_STOPPING;
    void emit(EVENTS.BAR_STATE_UPDATE, {
      barState,
      inputValue: "",
      lastSubmittedValue: "",
      currentError: barState === UI.BAR_STATES_ERROR ? errorText : null,
      transcriptionText: transcriptText,
      spokenText,
      voiceMode: UI.VOICE_MODES_IDLE,
      audioLevel,
      isAgentWorking: working,
      isDictationMode: barState === UI.BAR_STATES_DICTATING,
      isAlwaysListening: barState === UI.BAR_STATES_ALWAYS_LISTENING,
      agentState:
        barState === UI.BAR_STATES_LOADING || barState === UI.BAR_STATES_AGENT_RESPONDING
          ? agentLabel
          : null,
    });
  }, [barState, audioLevel, errorText, agentLabel, transcriptText, spokenText]);

  // Re-assert the full bar state on every field change.
  useEffect(() => emitState(), [emitState]);

  // Hover crosses the native tracking-area events; parking the cursor outside
  // the frame on leave lets the bar's leave-verify believe it.
  useEffect(() => {
    if (hovered) {
      harness.setCursor(frame.x + frame.width / 2, frame.y + frame.height / 2);
      void emit(EVENTS.SYSTEM_MOUSE_ENTERED_WINDOW, null);
    } else {
      harness.setCursor(-1000, -1000);
      void emit(EVENTS.SYSTEM_MOUSE_LEFT_WINDOW, null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hovered]);

  useEffect(() => {
    void emit(EVENTS.INPUT_CONTROL_STATE, driving ? { active: true, target_app: drivingApp } : { active: false });
  }, [driving, drivingApp]);

  useEffect(() => {
    // `phrases` non-empty means a wake phrase is configured; `listening` is
    // whether the engine is actually running (armed) or paused.
    void emit(EVENTS.VOICE_TRIGGER_LISTENING, {
      listening: wake === "armed",
      phrases: wake === "none" ? [] : ["hey juno"],
    });
  }, [wake]);

  useEffect(() => {
    const sessions = Array.from({ length: rosterCount }, (_, i) => ({
      id: `harness-${i}`,
      agent_name: `agent-${i + 1}`,
      color_slot: i % 8,
      display_color: ["#0A84FF", "#30D158", "#FF9F0A", "#FF375F", "#BF5AF2", "#64D2FF", "#FFD60A", "#5E5CE6"][i % 8],
      status: "running" as const,
      current_action: "working",
      started_at_ms: Date.now(),
      last_activity_ms: Date.now(),
      focused: i === 0,
    }));
    void emit(EVENTS.AGENT_SESSIONS_UPDATED, sessions);
  }, [rosterCount]);

  // Wells for the current window footprint, in physical px (the bar's units).
  const wells = useMemo<Well[]>(() => {
    const rect: MonitorRect = {
      position: { x: 0, y: 0 },
      size: { width: monitor.width, height: monitor.height },
      scaleFactor: scale,
    };
    return computeWells([rect], {
      windowWidth: frame.width / scale,
      windowHeight: frame.height / scale,
      includeCenter: true,
    });
  }, [monitor.width, monitor.height, scale, frame.width, frame.height]);

  const placeAtWell = useCallback((w: Well) => {
    harness.setFrame({ x: w.x, y: w.y });
  }, []);

  // Logical geometry for the on-screen render.
  const screenW = monitor.width / scale;
  const screenH = monitor.height / scale;
  const winLeft = frame.x / scale;
  const winTop = frame.y / scale;
  const winW = frame.width / scale;
  const winH = frame.height / scale;

  return (
    <div
      className="flex h-screen w-screen overflow-hidden bg-[#f5f5f7] text-[#1d1d1f]"
      style={{ fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui, sans-serif" }}
    >
      {/* h-screen/w-screen on the real bar root would fill the whole viewport;
          scope it to fill the simulated window container instead. Nothing about
          the bar component changes, only how its root sizes inside the bench. */}
      <style>{`.jbh-window > div{width:100%!important;height:100%!important;min-width:0!important;min-height:0!important;}`}</style>

      {/* ---- Controls ---- */}
      <aside className="flex h-full w-[320px] shrink-0 flex-col overflow-y-auto border-r border-black/10 bg-white">
        <header className="border-b border-black/10 px-4 py-3">
          <h1 className="text-[13px] font-semibold">Floating bar harness</h1>
          <p className="mt-0.5 text-[11px] text-black/45">
            Real components, no backend. Route: /__bar-harness
          </p>
        </header>

        <Section title="Bar state">
          {STATE_GROUPS.map((g) => (
            <div key={g.title} className="mb-2 last:mb-0">
              <div className="mb-1 text-[10px] font-medium uppercase tracking-wide text-black/35">
                {g.title}
              </div>
              <div className="flex flex-wrap gap-1.5">
                {g.states.map((s) => (
                  <Btn key={s.value} active={barState === s.value} onClick={() => setBarState(s.value)}>
                    {s.label}
                  </Btn>
                ))}
              </div>
            </div>
          ))}
        </Section>

        <Section title="State detail">
          <div className="space-y-2">
            <label className="flex items-center justify-between gap-2 text-[12px] text-black/70">
              <span>Audio level</span>
              <input
                type="range"
                min={0}
                max={1}
                step={0.05}
                value={audioLevel}
                onChange={(e) => setAudioLevel(Number(e.target.value))}
                className="w-40"
                style={{ accentColor: ACCENT }}
              />
            </label>
            <label className="flex flex-col gap-1 text-[12px] text-black/70">
              <span>Agent label (loading / responding)</span>
              <input
                value={agentLabel}
                onChange={(e) => setAgentLabel(e.target.value)}
                className="rounded-md border border-black/15 px-2 py-1 text-[12px] outline-none focus:border-[#0A84FF]"
              />
            </label>
            <label className="flex flex-col gap-1 text-[12px] text-black/70">
              <span>Error text</span>
              <input
                value={errorText}
                onChange={(e) => setErrorText(e.target.value)}
                className="rounded-md border border-black/15 px-2 py-1 text-[12px] outline-none focus:border-[#0A84FF]"
              />
            </label>
            <label className="flex flex-col gap-1 text-[12px] text-black/70">
              <span>Transcript / spoken text</span>
              <input
                value={transcriptText}
                onChange={(e) => {
                  setTranscriptText(e.target.value);
                  setSpokenText(e.target.value || "Here is what I found.");
                }}
                className="rounded-md border border-black/15 px-2 py-1 text-[12px] outline-none focus:border-[#0A84FF]"
              />
            </label>
          </div>
        </Section>

        <Section title="Signals">
          <div className="flex flex-wrap gap-1.5">
            <Btn active={hovered} onClick={() => setHovered((v) => !v)}>
              hover
            </Btn>
            <Btn onClick={() => void emit(EVENTS.BAR_TOGGLE_PANE, null)}>toggle chat pane</Btn>
            <Btn active={driving} onClick={() => setDriving((v) => !v)}>
              driving
            </Btn>
          </div>
          <div className="mt-2 space-y-1">
            <div className="text-[10px] font-medium uppercase tracking-wide text-black/35">
              Wake phrase
            </div>
            <div className="flex flex-wrap gap-1.5">
              <Btn active={wake === "none"} onClick={() => setWake("none")}>
                none
              </Btn>
              <Btn active={wake === "paused"} onClick={() => setWake("paused")}>
                configured, paused
              </Btn>
              <Btn active={wake === "armed"} onClick={() => setWake("armed")}>
                armed
              </Btn>
            </div>
          </div>
          <div className="mt-2">
            <NumberField label="Roster sessions (2+ shows strip)" value={rosterCount} onChange={(n) => setRosterCount(Math.max(0, Math.min(6, n)))} />
          </div>
        </Section>

        <Section title="Simulated window">
          <div className="space-y-1.5">
            <NumberField label="x (physical)" value={frame.x} onChange={(n) => harness.setFrame({ x: n })} />
            <NumberField label="y (physical)" value={frame.y} onChange={(n) => harness.setFrame({ y: n })} />
            <NumberField label="width" value={frame.width} onChange={(n) => harness.setFrame({ width: Math.max(1, n) })} />
            <NumberField label="height" value={frame.height} onChange={(n) => harness.setFrame({ height: Math.max(1, n) })} />
            <label className="mt-1 flex items-center gap-2 text-[12px] text-black/70">
              <input
                type="checkbox"
                checked={freezeAutoResize}
                onChange={(e) => harness.setFreezeAutoResize(e.target.checked)}
                style={{ accentColor: ACCENT }}
              />
              <span>Freeze auto-resize (hold size against the bar)</span>
            </label>
            {requested && freezeAutoResize && (
              <p className="text-[11px] text-[#0A84FF]">
                Bar wants {requested.width}×{requested.height} at {requested.x},{requested.y}
              </p>
            )}
          </div>
        </Section>

        <Section title="Display">
          <div className="space-y-1.5">
            <NumberField label="screen width (physical)" value={monitor.width} onChange={(n) => harness.setMonitor({ width: Math.max(320, n) })} />
            <NumberField label="screen height (physical)" value={monitor.height} onChange={(n) => harness.setMonitor({ height: Math.max(240, n) })} />
            <div className="flex flex-wrap gap-1.5 pt-1">
              <Btn onClick={() => harness.setMonitor({ width: 1440, height: 900 })}>1440×900</Btn>
              <Btn onClick={() => harness.setMonitor({ width: 2560, height: 1080 })}>2560×1080 (5-stop)</Btn>
            </div>
            <label className="flex items-center justify-between gap-2 pt-1 text-[12px] text-black/70">
              <span>Zoom</span>
              <input
                type="range"
                min={0.25}
                max={1}
                step={0.05}
                value={zoom}
                onChange={(e) => setZoom(Number(e.target.value))}
                className="w-40"
                style={{ accentColor: ACCENT }}
              />
            </label>
          </div>
        </Section>

        <Section title="Snap wells">
          <div className="flex flex-wrap gap-1.5">
            {wells.map((w) => (
              <Btn key={`${w.fx}-${w.fy}`} onClick={() => placeAtWell(w)}>
                {slotName(w.fx, w.fy)}
              </Btn>
            ))}
          </div>
        </Section>
      </aside>

      {/* ---- Simulated screen ---- */}
      <main className="relative flex-1 overflow-auto p-6">
        <div className="mb-3 text-[11px] text-black/45">
          Simulated {monitor.width}×{monitor.height} display. The bar sits in a
          container standing in for the OS window; drag the size fields or drop
          it into a well to watch its resize and snap logic.
        </div>
        <div
          className="relative"
          style={{ width: screenW * zoom, height: screenH * zoom }}
        >
          <div
            className="absolute left-0 top-0 origin-top-left overflow-hidden rounded-lg border border-black/15 bg-[#3a3a3c] shadow-inner"
            style={{ width: screenW, height: screenH, transform: `scale(${zoom})` }}
          >
            {/* menu bar strip, so top-inset wells read against something real */}
            <div className="absolute inset-x-0 top-0 h-[24px] bg-black/25" />

            {/* well markers at each landing spot's centre */}
            {wells.map((w) => (
              <div
                key={`m-${w.fx}-${w.fy}`}
                className="absolute size-2 -translate-x-1/2 -translate-y-1/2 rounded-full ring-1 ring-white/50"
                style={{
                  left: (w.x + w.width / 2) / scale,
                  top: (w.y + w.height / 2) / scale,
                  background: "rgba(255,255,255,0.35)",
                }}
                title={slotName(w.fx, w.fy)}
              />
            ))}

            {/* the simulated window: the real bar fills it via the scoped style */}
            <div
              className="jbh-window absolute rounded-md ring-1 ring-[#0A84FF]/40"
              style={{ left: winLeft, top: winTop, width: winW, height: winH }}
            >
              <FloatingBar />
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}
