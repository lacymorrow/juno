import { UI } from "@/lib/constants.generated";
import type { AgentState } from "@/components/ui/bar-visualizer";
import type { VoiceButtonState } from "@/components/ui/voice-button";

/**
 * Maps Juno's 16 bar states to ElevenLabs AgentState for BarVisualizer.
 * ElevenLabs supports: "connecting" | "initializing" | "listening" | "speaking" | "thinking"
 */
export function mapToAgentState(barState: string): AgentState {
  switch (barState) {
    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_INPUT:
    case UI.BAR_STATES_DICTATING:
    case UI.BAR_STATES_DICTATION_READY:
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return "listening";

    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_TRANSCRIBING:
      return "thinking";

    case UI.BAR_STATES_SPEAKING:
    case UI.BAR_STATES_AGENT_RESPONDING:
      return "speaking";

    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_SHRINKING:
      return "initializing";

    case UI.BAR_STATES_FINISHING:
    case UI.BAR_STATES_SUCCESS:
    case UI.BAR_STATES_ERROR:
      return "listening";

    default:
      return "listening";
  }
}

/**
 * Maps Juno's bar states to VoiceButton states.
 * VoiceButton supports: "idle" | "recording" | "processing" | "success" | "error"
 */
export function mapToVoiceButtonState(barState: string): VoiceButtonState {
  switch (barState) {
    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_INPUT:
    case UI.BAR_STATES_DICTATION_READY:
      return "idle";

    case UI.BAR_STATES_LISTENING:
    case UI.BAR_STATES_DICTATING:
    case UI.BAR_STATES_ALWAYS_LISTENING:
      return "recording";

    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_TRANSCRIBING:
    case UI.BAR_STATES_EXPANDING:
    case UI.BAR_STATES_SHRINKING:
    case UI.BAR_STATES_SPEAKING:
    case UI.BAR_STATES_AGENT_RESPONDING:
    case UI.BAR_STATES_FINISHING:
      return "processing";

    case UI.BAR_STATES_SUCCESS:
      return "success";

    case UI.BAR_STATES_ERROR:
      return "error";

    default:
      return "idle";
  }
}

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
