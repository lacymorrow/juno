// Canonical ChatMessage type — single source of truth for the chat conversation model.
// All consumers import from here.

/** What copy, share and save start from: one assistant reply plus the question it answered. */
export type ResponseExportInput = {
  question?: string;
  content: string;
  spoken: string[];
  timestamp?: number;
};

export type ChatMessage = {
  role:
    | "user"
    | "assistant"
    | "system"
    | "tool_call_request"
    | "tool_call_result"
    | "thinking";
  content: string;
  isJsx?: boolean;
  /**
   * Guidance the app wrote about itself (onboarding, status), not an answer to
   * anything. It reads like a reply, so without this flag the UI offers to copy
   * and share "Setup complete. Welcome to Juno!".
   */
  notice?: boolean;
  screenshot_base64?: string;
  tool_name?: string;
  tool_args?: any;
  tool_output?: any;
  success?: boolean;
  /**
   * The result line for a `tool_call_request` that has completed.
   *
   * A tool call and its result are one row, not two: `success` switches the row
   * from running to done, and this carries the outcome text. `content` stays as
   * the request description so the row keeps its title after it resolves.
   */
  result_content?: string;
  timestamp?: number;
  isStreaming?: boolean;
  messageId?: string;
  agent_state?: string;
  tool_id?: string;
  approval_state?: "pending" | "approved" | "denied";
  ax_grounded?: boolean;
  ax_role?: string | null;
  ax_label?: string | null;
  ax_screen_coordinate?: [number, number];
  ax_action?: string;
  risk_level?: "low" | "medium" | "high" | "critical";
  target_app?: string;
  approval_timeout_seconds?: number;
  continuation_request_id?: string;
  continuation_state?: "pending" | "stopped" | "continued";
  tts_metadata?: {
    has_spoken_content: boolean;
    tts_parts: string[];
    total_spoken_text: string;
  };
};
