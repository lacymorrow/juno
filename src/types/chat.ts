// Canonical ChatMessage type — single source of truth for the chat conversation model.
// All consumers import from here.

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
  screenshot_base64?: string;
  tool_name?: string;
  tool_args?: any;
  tool_output?: any;
  success?: boolean;
  timestamp?: number;
  isStreaming?: boolean;
  messageId?: string;
  agent_state?: string;
  tool_id?: string;
  approval_state?: "pending" | "approved" | "denied";
  continuation_request_id?: string;
  continuation_state?: "pending" | "stopped" | "continued";
  tts_metadata?: {
    has_spoken_content: boolean;
    tts_parts: string[];
    total_spoken_text: string;
  };
};
