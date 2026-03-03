import { UI } from "@/lib/constants.generated";
import type { AgentState as OrbAgentState } from "@/components/ui/orb";
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
    default:
      return null;
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
    default:
      return "idle";
  }
}
