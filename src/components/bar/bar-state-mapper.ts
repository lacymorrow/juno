import { UI } from "@/lib/constants.generated";
import type { AgentState as OrbAgentState } from "@/components/ui/elevenlabs-orb";
import type { PersonaState } from "@/components/ai-elements/persona";

/**
 * Returns a human-readable status label for display.
 */
export function getStatusLabel(barState: string): string {
  switch (barState) {
    case UI.BAR_STATES_DEFAULT:
      return "Ready";
    case UI.BAR_STATES_LISTENING:
      return "Listening...";
    case UI.BAR_STATES_DICTATING:
      return "Dictating...";
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return "Always Listening";
    case UI.BAR_STATES_TRANSCRIBING:
      return "Transcribing...";
    case UI.BAR_STATES_SUBMITTING:
      return "Submitting...";
    case UI.BAR_STATES_LOADING:
      return "Processing...";
    case UI.BAR_STATES_SPEAKING:
      return "Speaking...";
    case UI.BAR_STATES_AGENT_RESPONDING:
      return "Agent Working...";
    case UI.BAR_STATES_SUCCESS:
      return "Done";
    case UI.BAR_STATES_ERROR:
      return "Error";
    case UI.BAR_STATES_STOPPING:
      return "Stopping...";
    case UI.BAR_STATES_FINISHING:
      return "Finishing...";
    case UI.BAR_STATES_INPUT:
      return "Type a message";
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_SHRINKING:
      return "";
    case UI.BAR_STATES_DICTATION_READY:
      return "Dictation Ready";
    default:
      return "Ready";
  }
}

/**
 * Maps Juno's 16 bar states to ElevenLabs Orb AgentState.
 * Orb supports: null | "thinking" | "listening" | "talking"
 */
export function mapToOrbState(barState: string): OrbAgentState {
  switch (barState) {
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_DICTATING:
    case UI.BAR_STATES_DICTATION_READY:
    case UI.BAR_STATES_ALWAYS_LISTENING:
    case UI.BAR_STATES_INPUT:
      return "listening";

    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_TRANSCRIBING:
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_SHRINKING:
    case UI.BAR_STATES_FINISHING:
      return "thinking";

    case UI.BAR_STATES_SPEAKING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return "talking";

    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_SUCCESS:
    case UI.BAR_STATES_ERROR:
    case UI.BAR_STATES_STOPPING:
    default:
      return null;
  }
}

/**
 * Per-state color pair for the ElevenLabs orb (elevenlabs-orb.tsx), which takes
 * a two-tone `colors` gradient as [primary, secondary]. Motion still comes from
 * mapToOrbState() + audio volume; this is the color axis on top of it.
 *
 * The palette is a cohesive family grown from the orb's documented periwinkle
 * default: idle periwinkle, listening blues, thinking warm gold/amber, talking
 * teal-green, success green, error red, stopping dim slate. Each entry is a
 * lighter primary paired with a deeper secondary so the gradient reads on any
 * background. Kept tight, not a rainbow.
 */
export function mapToElevenLabsOrbColors(barState: string): [string, string] {
  switch (barState) {
    // Listening family — blues/cyan
    case UI.BAR_STATES_LISTENING:
      return ["#7EB6FF", "#3B82F6"];
    case UI.BAR_STATES_DICTATING:
      return ["#8FD0FF", "#38BDF8"];
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return ["#93C5FD", "#2563EB"];
    case UI.BAR_STATES_DICTATION_READY:
      return ["#BFDBFE", "#60A5FA"];
    case UI.BAR_STATES_INPUT:
      return ["#C7DBFF", "#7DA0E8"];

    // Thinking family — warm gold/amber
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_FINISHING:
      return ["#FCD34D", "#F59E0B"];
    case UI.BAR_STATES_SUBMITTING:
      return ["#FDE68A", "#F59E0B"];
    case UI.BAR_STATES_TRANSCRIBING:
      return ["#FCD34D", "#FBBF24"];
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_SHRINKING:
      return ["#FDE68A", "#FBBF24"];

    // Talking family — teal-green
    case UI.BAR_STATES_SPEAKING:
      return ["#5EEAD4", "#14B8A6"];
    case UI.BAR_STATES_AGENT_RESPONDING:
      return ["#6EE7B7", "#10B981"];

    // Success — green
    case UI.BAR_STATES_SUCCESS:
      return ["#86EFAC", "#22C55E"];

    // Error — red
    case UI.BAR_STATES_ERROR:
      return ["#FCA5A5", "#EF4444"];

    // Stopping — dim slate
    case UI.BAR_STATES_STOPPING:
      return ["#94A3B8", "#475569"];

    // Idle / default — periwinkle (the orb's documented default)
    case UI.BAR_STATES_DEFAULT:
    default:
      return ["#CADCFC", "#A0B9D1"];
  }
}

/**
 * Visual configuration for the React Bits orb (react-orb.tsx). The React orb
 * has no agentState — it is recolored by `hue` (degrees, base ~violet at 0) and
 * animated by `hoverIntensity` + `forceHoverState`. `reactive` says which audio
 * channel should further modulate `hoverIntensity` for mic/speech feedback.
 */
export interface ReactOrbConfig {
  hue: number;
  hoverIntensity: number;
  forceHoverState: boolean;
  reactive: "input" | "output" | null;
}

/**
 * Maps Juno's bar states to a React orb appearance. Mirrors the four states the
 * ElevenLabs orb documents (idle, listening, thinking, talking) and extends the
 * scheme across Juno's remaining states so the whole set reads cohesively.
 * Hues are offsets from the base violet: listening leans blue/cyan, thinking
 * warms, talking goes teal/green, error goes red, stopping dims.
 */
export function mapToReactOrbConfig(barState: string): ReactOrbConfig {
  switch (barState) {
    // Listening family — blue/cyan, mic-reactive
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_DICTATING:
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return { hue: 140, hoverIntensity: 0.35, forceHoverState: true, reactive: "input" };

    // Ready-to-listen / typing — blue/cyan, calmer, no live audio yet
    case UI.BAR_STATES_DICTATION_READY:
    case UI.BAR_STATES_INPUT:
      return { hue: 130, hoverIntensity: 0.28, forceHoverState: true, reactive: null };

    // Thinking family — warmer, active
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_TRANSCRIBING:
    case UI.BAR_STATES_FINISHING:
      return { hue: 40, hoverIntensity: 0.42, forceHoverState: true, reactive: null };

    // Transitions — warm but gentler
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_SHRINKING:
      return { hue: 40, hoverIntensity: 0.32, forceHoverState: true, reactive: null };

    // Talking family — teal/green, speech-reactive
    case UI.BAR_STATES_SPEAKING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return { hue: 90, hoverIntensity: 0.45, forceHoverState: true, reactive: "output" };

    // Brief success — green flash
    case UI.BAR_STATES_SUCCESS:
      return { hue: 95, hoverIntensity: 0.5, forceHoverState: true, reactive: null };

    // Error — red, stronger
    case UI.BAR_STATES_ERROR:
      return { hue: 180, hoverIntensity: 0.65, forceHoverState: true, reactive: null };

    // Stopping — dim, low intensity, no forced hover
    case UI.BAR_STATES_STOPPING:
      return { hue: 20, hoverIntensity: 0.1, forceHoverState: false, reactive: null };

    // Idle / default — base violet, resting
    case UI.BAR_STATES_DEFAULT:
    default:
      return { hue: 0, hoverIntensity: 0.15, forceHoverState: false, reactive: null };
  }
}

/**
 * Maps Juno's 16 bar states to AI Elements Persona state.
 * Persona supports: "idle" | "listening" | "thinking" | "speaking" | "asleep"
 */
export function mapToPersonaState(barState: string): PersonaState {
  switch (barState) {
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_DICTATING:
    case UI.BAR_STATES_DICTATION_READY:
    case UI.BAR_STATES_ALWAYS_LISTENING:
    case UI.BAR_STATES_INPUT:
      return "listening";

    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_TRANSCRIBING:
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_SHRINKING:
    case UI.BAR_STATES_FINISHING:
      return "thinking";

    case UI.BAR_STATES_SPEAKING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return "speaking";

    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_SUCCESS:
    case UI.BAR_STATES_ERROR:
    case UI.BAR_STATES_STOPPING:
    default:
      return "idle";
  }
}
