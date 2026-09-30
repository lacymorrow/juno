import { UI } from "@/lib/constants.generated";
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
